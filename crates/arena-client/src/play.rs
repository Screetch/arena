//! The play of the fighter on the first map: the keys, the pointer and the wheel of the window
//! move it, and each frame plays the steps that the time owes and measures its characters.

use arena_sim::clip::FPS;
use arena_sim::map::FIRST;
use arena_sim::moves::{Key as Move, Seeded, World};
use winit::keyboard::{Key, NamedKey};

use crate::sample::mul;
use crate::sprites::{Drawn, effect, figure};
use crate::view::View;

/// The seed of the chance of the shots and of the drops.
const SEED: u32 = 1;

/// The most steps that one frame plays. A frame that owes more forgets the rest.
const STEPS: u32 = 5;

/// The place of the figure about the feet of the fighter, in units of the map.
const FEET: [f64; 2] = [-0.35, -3.95];

/// The units about the camera in which a stuck shot, a stain or a drop shows.
const MARGIN: f64 = 120.0;

/// A fighter on the first map, and the view that follows its camera.
pub struct Play {
  world: World<Seeded>,
  view: View,
  pointer: [f64; 2],
  owed: f64,
  last: Option<f64>,
}

impl Play {
  /// A fighter of the role 1 at its start, on a window of `size` pixels.
  pub fn new(size: [u32; 2]) -> Self {
    let view = View::new(&FIRST, size);
    let world = World::new(FIRST, view.shown(), Seeded::new(SEED), 1);
    Self { world, view, pointer: [0.0; 2], owed: 0.0, last: None }
  }

  /// The world of the fighter.
  pub fn world(&self) -> &World<Seeded> {
    &self.world
  }

  /// The view of the map.
  pub fn view(&self) -> &View {
    &self.view
  }

  /// Fits the view to a window of `size` pixels.
  pub fn resize(&mut self, size: [u32; 2]) {
    self.view.resize(size);
    self.fit();
  }

  /// Presses or releases `key`: the arrows or W A S D move, Space or Shift plays the special of
  /// the role, 1, 2 and 3 take a weapon, C takes the next role from the start, and + and - zoom.
  /// Says whether the key is one of these.
  pub fn key(&mut self, key: &Key, pressed: bool) -> bool {
    let moved = match key {
      Key::Named(NamedKey::ArrowUp) => Move::Up,
      Key::Named(NamedKey::ArrowDown) => Move::Down,
      Key::Named(NamedKey::ArrowLeft) => Move::Left,
      Key::Named(NamedKey::ArrowRight) => Move::Right,
      Key::Named(NamedKey::Space | NamedKey::Shift) => Move::Special,
      Key::Character(text) => return self.character(&text.to_lowercase(), key, pressed),
      _ => return false,
    };
    self.world.key(moved, pressed);
    true
  }

  fn character(&mut self, text: &str, key: &Key, pressed: bool) -> bool {
    let moved = match text {
      "w" => Move::Up,
      "s" => Move::Down,
      "a" => Move::Left,
      "d" => Move::Right,
      "1" | "2" | "3" => {
        if pressed {
          self.world.change_weapon(text.parse().unwrap_or(1));
        }
        return true;
      }
      "c" => {
        if pressed {
          let role = self.world.fighter.role % 3 + 1;
          self.world = World::new(FIRST, self.view.shown(), Seeded::new(SEED), role);
          self.fit();
        }
        return true;
      }
      _ => {
        let zoomed = pressed && self.view.key(key);
        if zoomed {
          self.fit();
        }
        return zoomed;
      }
    };
    self.world.key(moved, pressed);
    true
  }

  /// Moves the pointer to the pixel `at` of the window: the fighter aims at the point of the map
  /// under it.
  pub fn point(&mut self, at: [f64; 2]) {
    self.pointer = at;
    self.fit();
  }

  /// Presses or releases the button of the shots.
  pub fn button(&mut self, pressed: bool) {
    if pressed {
      self.world.press();
    } else {
      self.world.release();
    }
  }

  /// Takes the next weapon of the role for a turn of the wheel down, and the last one for a turn
  /// up.
  pub fn wheel(&mut self, down: bool) {
    let slot = self.world.fighter.slot;
    self.world.change_weapon(if down { slot % 3 + 1 } else { (slot + 1) % 3 + 1 });
  }

  /// Plays the steps that the time owes at `now` seconds since the last frame, at most 5, and
  /// gives the characters of the fighter, of its shots, of its effects and of the drops, in the
  /// order that they cover each other.
  pub fn frame(&mut self, now: f64) -> Vec<Drawn> {
    if let Some(last) = self.last {
      self.owed += (now - last) * f64::from(FPS);
    }
    let mut steps = 0;
    while self.owed >= 1.0 && steps < STEPS {
      self.owed -= 1.0;
      self.world.frame();
      steps += 1;
    }
    if self.owed > f64::from(STEPS) {
      self.owed = 0.0;
    }
    self.last = Some(now);
    self.view.follow(self.world.camera());
    self.sprites(now % 1000.0)
  }

  /// Gives the camera of the world the units that the view shows, and aims at the pointer.
  fn fit(&mut self) {
    self.world.resize(self.view.shown());
    let scale = self.view.scale();
    self.world.look(self.pointer.map(|at| at / scale));
  }

  fn sprites(&self, time: f64) -> Vec<Drawn> {
    let (world, matrix) = (&self.world, self.view.matrix());
    let screen = self.view.size().map(f64::from);
    let [left, top] = world.camera();
    let [width, height] = self.view.shown();
    let near = |x: f64, y: f64| {
      x > left - MARGIN
        && x < left + width + MARGIN
        && y > top - MARGIN
        && y < top + height + MARGIN
    };
    let mut drawn = Vec::new();
    for shot in world.stuck().iter().filter(|shot| near(shot.clip.x, shot.clip.y)) {
      drawn.extend(effect(shot.effect, &shot.clip, matrix, screen, time));
    }
    let stamps = world.drops().stamps().iter();
    for stamp in stamps.filter(|stamp| near(stamp.clip.x, stamp.clip.y)) {
      drawn.extend(effect(stamp.effect, &stamp.clip, matrix, screen, time));
    }
    let fighter = &world.fighter;
    let feet = mul(matrix, [1.0, 0.0, 0.0, 1.0, fighter.x + FEET[0], fighter.y + FEET[1]]);
    drawn.extend(figure(world.figure(), feet, screen, time));
    for shot in world.shots() {
      drawn.extend(effect(shot.effect, &shot.clip, matrix, screen, time));
    }
    for spawn in world.effects() {
      drawn.extend(effect(spawn.effect, &spawn.clip, matrix, screen, time));
    }
    for drop in world.drops().live().iter().filter(|drop| near(drop.clip.x, drop.clip.y)) {
      drawn.extend(effect(drop.effect, &drop.clip, matrix, screen, time));
    }
    drawn
  }
}

#[cfg(test)]
mod tests {
  use arena_sim::clip::Clip;
  use arena_sim::moves::Spray;
  use winit::keyboard::{Key, NamedKey};

  use super::Play;
  use crate::sprites::FIGURE;

  fn named(key: NamedKey) -> Key {
    Key::Named(key)
  }

  fn letter(text: &str) -> Key {
    Key::Character(text.into())
  }

  /// Plays `steps` steps of the fighter, one frame for each step.
  fn play_steps(play: &mut Play, steps: u32) {
    let start = play.last.unwrap_or(0.0);
    play.frame(start);
    for step in 1..=steps {
      play.frame(start + f64::from(step) / 40.0);
    }
  }

  #[test]
  fn the_fighter_starts_at_its_start_in_the_role_1_with_its_first_weapon() {
    let play = Play::new([1600, 1000]);
    let fighter = &play.world().fighter;
    assert_eq!((fighter.x, fighter.y, fighter.role, fighter.weapon), (200.0, 1750.0, 1, 1));
    assert_eq!(play.view().size(), [1600, 1000]);
  }

  #[test]
  fn the_time_of_the_window_plays_40_steps_a_second_and_at_most_5_in_a_frame() {
    let mut play = Play::new([1600, 1000]);
    let reload = |play: &Play| play.world().fighter.reload;
    play.frame(1.0);
    assert_eq!(reload(&play), 0);
    play.frame(1.0625);
    assert_eq!(reload(&play), 2);
    play.frame(1.125);
    assert_eq!(reload(&play), 5);
    play.frame(11.125);
    assert_eq!(reload(&play), 10);
    play.frame(11.1375);
    assert_eq!(reload(&play), 10);
    play.frame(11.15);
    assert_eq!(reload(&play), 11);
  }

  #[test]
  fn the_arrows_and_w_a_s_d_move_the_fighter() {
    let mut play = Play::new([1600, 1000]);
    play_steps(&mut play, 30);
    for (key, way) in [
      (named(NamedKey::ArrowRight), 1.0),
      (letter("D"), 1.0),
      (named(NamedKey::ArrowLeft), -1.0),
      (letter("a"), -1.0),
    ] {
      let x = play.world().fighter.x;
      assert!(play.key(&key, true));
      play_steps(&mut play, 10);
      assert!(play.key(&key, false));
      play_steps(&mut play, 10);
      assert!((play.world().fighter.x - x) * way > 0.0, "{key:?}");
    }
    for key in [named(NamedKey::ArrowUp), letter("w")] {
      play.key(&key, true);
      play_steps(&mut play, 8);
      play.key(&key, false);
      assert!(play.world().fighter.y < 1700.0, "{key:?}");
      play_steps(&mut play, 60);
    }
    for key in [named(NamedKey::ArrowDown), letter("s")] {
      play.key(&key, true);
      play_steps(&mut play, 4);
      let body = play.world().figure().named("spr").and_then(Clip::label);
      assert_eq!(body, Some("sit"), "{key:?}");
      play.key(&key, false);
      play_steps(&mut play, 4);
    }
  }

  #[test]
  fn space_or_shift_plays_the_special_and_the_keys_and_the_wheel_take_a_weapon_or_a_role() {
    let mut play = Play::new([1600, 1000]);
    play_steps(&mut play, 30);
    for key in [named(NamedKey::Space), named(NamedKey::Shift)] {
      assert!(play.key(&key, true));
      assert!(play.key(&key, false));
      assert!((play.world().fighter.energy - 40.0).abs() < 1e-9, "{key:?}");
      play_steps(&mut play, 60);
    }
    let weapon = |play: &Play| play.world().fighter.weapon;
    assert!(play.key(&letter("2"), true) && play.key(&letter("2"), false));
    assert_eq!(weapon(&play), 2);
    play.wheel(true);
    assert_eq!(weapon(&play), 3);
    play.wheel(false);
    assert_eq!(weapon(&play), 2);
    assert!(play.key(&letter("C"), true) && play.key(&letter("c"), false));
    let fighter = &play.world().fighter;
    assert_eq!((fighter.role, fighter.weapon, fighter.x), (2, 4, 200.0));
    assert!(!play.key(&letter("x"), true) && !play.key(&named(NamedKey::Enter), true));
  }

  #[test]
  fn plus_and_minus_zoom_and_the_fighter_aims_at_the_pointer() {
    let mut play = Play::new([1600, 1000]);
    assert!(play.key(&letter("+"), true));
    assert!(!play.key(&letter("+"), false));
    assert!((play.view().scale() - 2f64.powf(0.25)).abs() < 1e-12);
    assert!(play.key(&letter("-"), true));
    play.resize([800, 500]);
    play.point([400.0, 100.0]);
    let [left, top] = play.world().camera();
    play_steps(&mut play, 1);
    let fighter = &play.world().fighter;
    assert!(
      (fighter.aim_x - left - 800.0).abs() < 1e-9 && (fighter.aim_y - top - 200.0).abs() < 1e-9
    );
    play.button(true);
    assert!(play.world().fighter.pressed);
    play.button(false);
    assert!(!play.world().fighter.pressed);
  }

  #[test]
  fn a_frame_measures_the_stuck_shots_the_stains_the_figure_the_shots_the_effects_and_the_drops() {
    let mut play = Play::new([1600, 1000]);
    let drawn = play.frame(0.0);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].look, FIGURE);
    play_steps(&mut play, 30);
    play.point([500.0, 1000.0]);
    play.button(true);
    play_steps(&mut play, 2);
    play.button(false);
    play_steps(&mut play, 60);
    assert!(!play.world().stuck().is_empty());
    play.world.burst(Spray::Splash, 3, 300.0, 1700.0, 0.0);
    play.world.burst(Spray::Embers, 3, 300.0, 300.0, 0.0);
    play.world.burst(Spray::Splash, 1, 4000.0, 1000.0, 0.0);
    play_steps(&mut play, 40);
    assert!(!play.world().drops().stamps().is_empty() && !play.world().drops().live().is_empty());
    play.key(&named(NamedKey::Space), true);
    let drawn = play.frame(play.last.unwrap_or(0.0) + 0.025);
    let figure = drawn.iter().position(|drawn| drawn.look == FIGURE).expect("the figure shows");
    let world = play.world();
    let counts = [world.stuck().len(), world.drops().stamps().len(), world.effects().len()];
    assert_eq!((counts, world.drops().live().len()), ([1, 1, 3], 6));
    assert_eq!(figure, 2, "the stuck arrow and the stain lie under the figure");
    assert_eq!(drawn.len(), figure + 1 + 3 + 2, "4 drops lie out of the view and show nothing");
  }
}
