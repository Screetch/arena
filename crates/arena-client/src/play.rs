//! The play of a fighter on the first map: the keys, the pointer and the wheel of the window give
//! its intents, each frame plays the steps that the time owes, the camera follows the fighter, and
//! each frame measures the characters of the world.

use arena_sim::clip::FPS;
use arena_sim::map::FIRST;
use arena_sim::moves::{Id, Intents, Key as Move, Seeded, World};
use winit::keyboard::{Key, NamedKey};

use crate::sample::mul;
use crate::sprites::{Drawn, effect, figure};
use crate::view::View;

/// The seed of the chance of the shots and of the drops.
const SEED: u32 = 1;

/// The fighter of the player.
const ME: Id = Id(1);

/// The most steps that one frame plays. A frame that owes more forgets the rest.
const STEPS: u32 = 5;

/// The place of the figure about the feet of the fighter, in units of the map.
const FEET: [f64; 2] = [-0.35, -3.95];

/// The units about the camera in which a stuck shot, a stain or a drop shows.
const MARGIN: f64 = 120.0;

/// A fighter on the first map, its intents, and the view that follows its camera.
pub struct Play {
  world: World<Seeded>,
  intents: Intents,
  view: View,
  pointer: [f64; 2],
  camera: [f64; 2],
  owed: f64,
  last: Option<f64>,
}

impl Play {
  /// A fighter of the role 1 at its start, on a window of `size` pixels.
  pub fn new(size: [u32; 2]) -> Self {
    let view = View::new(&FIRST, size);
    let mut world = World::new(FIRST, Seeded::new(SEED));
    world.add(ME, 1);
    let intents = world.fighters()[&ME].intents();
    Self { world, intents, view, pointer: [0.0; 2], camera: [0.0, 700.0], owed: 0.0, last: None }
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
  }

  /// Holds or lets go `key`: the arrows or W A S D move, Space or Shift plays the special of the
  /// role, 1, 2 and 3 take a weapon, C takes the next role, and + and - zoom. The fighter heeds
  /// each at its next step. Says whether the key is one of these.
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
    self.intents.keys[moved as usize] = pressed;
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
          self.intents.slot = text.parse().unwrap_or(1);
        }
        return true;
      }
      "c" => {
        if pressed {
          self.intents.role = self.intents.role % 3 + 1;
          self.intents.slot = 1;
        }
        return true;
      }
      _ => return pressed && self.view.key(key),
    };
    self.intents.keys[moved as usize] = pressed;
    true
  }

  /// Moves the pointer to the pixel `at` of the window: at each step, the fighter aims at the point
  /// of the map under it.
  pub fn point(&mut self, at: [f64; 2]) {
    self.pointer = at;
  }

  /// Holds or lets go the button of the shots.
  pub fn button(&mut self, pressed: bool) {
    self.intents.button = pressed;
  }

  /// Takes the next weapon of the role for a turn of the wheel down, and the last one for a turn
  /// up.
  pub fn wheel(&mut self, down: bool) {
    let slot = self.intents.slot;
    self.intents.slot = if down { slot % 3 + 1 } else { (slot + 1) % 3 + 1 };
  }

  /// Plays the steps that the time owes at `now` seconds since the last frame, at most 5, and
  /// gives the characters of the fighters, of their shots, of their effects and of the drops, in
  /// the order that they cover each other.
  pub fn frame(&mut self, now: f64) -> Vec<Drawn> {
    if let Some(last) = self.last {
      self.owed += (now - last) * f64::from(FPS);
    }
    let mut steps = 0;
    while self.owed >= 1.0 && steps < STEPS {
      self.owed -= 1.0;
      self.step();
      steps += 1;
    }
    if self.owed > f64::from(STEPS) {
      self.owed = 0.0;
    }
    self.last = Some(now);
    self.view.follow(self.camera);
    self.sprites(now % 1000.0)
  }

  /// Plays one step, in which the fighter aims at the point of the map under the pointer, and then
  /// moves the camera a quarter of the way to the fighter.
  fn step(&mut self) {
    let scale = self.view.scale();
    self.intents.aim = [0, 1].map(|axis| self.camera[axis] + self.pointer[axis] / scale);
    self.world.intend(ME, self.intents);
    self.world.frame();
    let (fighter, [width, height]) = (&self.world.fighters()[&ME], self.view.shown());
    let target = framed(fighter.x - width / 2.0, width, f64::from(FIRST.width));
    self.camera[0] += (target - self.camera[0]) * 0.25;
    let target = framed(fighter.y - height * 0.6, height, f64::from(FIRST.height));
    self.camera[1] += (target - self.camera[1]) * 0.25;
  }

  fn sprites(&self, time: f64) -> Vec<Drawn> {
    let (world, matrix) = (&self.world, self.view.matrix());
    let screen = self.view.size().map(f64::from);
    let [left, top] = self.camera;
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
    for fighter in world.fighters().values() {
      let feet = mul(matrix, [1.0, 0.0, 0.0, 1.0, fighter.x + FEET[0], fighter.y + FEET[1]]);
      drawn.extend(figure(fighter.figure(), feet, screen, time));
    }
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

/// The start of a side of the camera of `size` units that shows `value` on a side of the map of
/// `level` units: in the map, or centred on it when the map is shorter.
fn framed(value: f64, size: f64, level: f64) -> f64 {
  if size >= level { (level - size) / 2.0 } else { value.min(level - size).max(0.0) }
}

#[cfg(test)]
mod tests {
  use arena_sim::clip::Clip;
  use arena_sim::moves::{Burst, Fighter, Spray};
  use winit::keyboard::{Key, NamedKey};

  use super::{ME, Play};
  use crate::sprites::FIGURE;

  fn named(key: NamedKey) -> Key {
    Key::Named(key)
  }

  fn letter(text: &str) -> Key {
    Key::Character(text.into())
  }

  fn fighter(play: &Play) -> &Fighter {
    &play.world().fighters()[&ME]
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
    let fighter = fighter(&play);
    assert_eq!((fighter.x, fighter.y, fighter.role, fighter.weapon), (200.0, 1750.0, 1, 1));
    assert_eq!(play.view().size(), [1600, 1000]);
  }

  #[test]
  fn the_time_of_the_window_plays_40_steps_a_second_and_at_most_5_in_a_frame() {
    let mut play = Play::new([1600, 1000]);
    let reload = |play: &Play| fighter(play).reload;
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
      let x = fighter(&play).x;
      assert!(play.key(&key, true));
      play_steps(&mut play, 10);
      assert!(play.key(&key, false));
      play_steps(&mut play, 10);
      assert!((fighter(&play).x - x) * way > 0.0, "{key:?}");
    }
    for key in [named(NamedKey::ArrowUp), letter("w")] {
      play.key(&key, true);
      play_steps(&mut play, 8);
      play.key(&key, false);
      assert!(fighter(&play).y < 1700.0, "{key:?}");
      play_steps(&mut play, 60);
    }
    for key in [named(NamedKey::ArrowDown), letter("s")] {
      play.key(&key, true);
      play_steps(&mut play, 4);
      let body = fighter(&play).figure().named("spr").and_then(Clip::label);
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
      play_steps(&mut play, 1);
      assert!(play.key(&key, false));
      assert!((fighter(&play).energy - 40.0).abs() < 1e-9, "{key:?}");
      play_steps(&mut play, 59);
    }
    let weapon = |play: &mut Play| {
      play_steps(play, 1);
      fighter(play).weapon
    };
    assert!(play.key(&letter("2"), true) && play.key(&letter("2"), false));
    assert_eq!(weapon(&mut play), 2);
    play.wheel(true);
    assert_eq!(weapon(&mut play), 3);
    play.wheel(false);
    assert_eq!(weapon(&mut play), 2);
    assert!(play.key(&letter("C"), true) && play.key(&letter("c"), false));
    assert_eq!(weapon(&mut play), 4);
    assert_eq!((fighter(&play).role, fighter(&play).slot), (2, 1));
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
    let [left, top] = play.camera;
    play_steps(&mut play, 1);
    let fighter_now = fighter(&play);
    assert!(
      (fighter_now.aim_x - left - 800.0).abs() < 1e-9
        && (fighter_now.aim_y - top - 200.0).abs() < 1e-9
    );
    play.button(true);
    play_steps(&mut play, 1);
    assert!(fighter(&play).pressed);
    play.button(false);
    play_steps(&mut play, 1);
    assert!(!fighter(&play).pressed);
  }

  /// A view wider and higher than the map centres the camera on it, and the camera moves a quarter
  /// of the way at each step.
  #[test]
  #[expect(clippy::float_cmp, reason = "the camera moves by exact quarters")]
  fn a_view_larger_than_the_map_centres_the_camera() {
    let mut play = Play::new([4800, 1000]);
    for _ in 0..4 {
      assert!(play.key(&letter("-"), true));
    }
    assert_eq!(play.view().shown(), [9600.0, 2000.0]);
    play_steps(&mut play, 1);
    assert_eq!(play.camera, [-600.0, 525.0]);
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
    let burst = |spray, count, at| Burst { spray, count, at, rotation: 0.0, by: None };
    play.world.burst(burst(Spray::Splash, 3, [300.0, 1700.0]));
    play.world.burst(burst(Spray::Embers, 3, [300.0, 300.0]));
    play.world.burst(burst(Spray::Splash, 1, [4000.0, 1000.0]));
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
