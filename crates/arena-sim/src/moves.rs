//! The moves of a fighter on a map, with its shots, its effects and the drops, at [`FPS`] steps a
//! second.
//!
//! The rules probe points of the map, as [`Map::hit`] says, and truncate each number where they
//! truncate it. Each sine, cosine and arc tangent comes from `libm`, and not from the system, so
//! that the same steps give the same state on each machine.
//!
//! [`FPS`]: crate::clip::FPS

mod drops;
mod shot;
mod step;

use std::f64::consts::PI;

use libm::{atan2, cos, sin};

pub use drops::{Drop, Drops, Spray};
pub use shot::Shot;
use shot::{Kind, Launch, Recoil};

use crate::clip::{At, Clip, Effect};
use crate::map::Map;

/// The point of the map where a fighter starts, at its feet.
pub const START: [f64; 2] = [200.0, 1750.0];

const RAD: f64 = PI / 180.0;
const SIDE: f64 = 20.0;
const REACH: f64 = SIDE + 4.0;
const STUCK: usize = 150;

/// A source of chance, which the caller gives to a world.
pub trait Chance {
  /// A number from 0, which it can give, to 1, which it never gives.
  fn chance(&mut self) -> f64;
}

/// A source of chance from a seed, which gives the same numbers on each machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seeded(u32);

impl Seeded {
  /// The source of the seed `seed`.
  pub fn new(seed: u32) -> Self {
    Self(seed)
  }
}

impl Chance for Seeded {
  fn chance(&mut self) -> f64 {
    self.0 = self.0.wrapping_add(0x6d2b_79f5);
    let mut t = self.0;
    t = (t ^ (t >> 15)).wrapping_mul(t | 1);
    t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 0x3d));
    f64::from(t ^ (t >> 14)) / 4_294_967_296.0
  }
}

/// A key that moves a fighter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
  /// Jumps, runs up a wall and climbs a ledge.
  Up,
  /// Crouches and lets go of a ledge.
  Down,
  /// Runs to the left.
  Left,
  /// Runs to the right.
  Right,
  /// The special of the role.
  Special,
}

/// The animation that the moves ask of the body of a fighter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
  /// Stands.
  Idle,
  /// Runs to the side that it faces.
  Run,
  /// Runs back, to the side that it does not face.
  RunBack,
  /// Crouches.
  Sit,
  /// Goes up in the air.
  Jump,
  /// Falls.
  Fall,
  /// Holds a wall.
  Hang,
  /// Holds a ledge.
  Ledge,
  /// Runs up a wall.
  WallRun,
}

impl Pose {
  /// The label of the pose in the timeline of the body.
  pub fn label(self) -> &'static str {
    match self {
      Self::Idle => "idle",
      Self::Run => "run",
      Self::RunBack => "runB",
      Self::Sit => "sit",
      Self::Jump => "jump",
      Self::Fall => "jumpD",
      Self::Hang => "hang",
      Self::Ledge => "hang2",
      Self::WallRun => "wallrun",
    }
  }
}

/// The state of a fighter, in units of the map and in steps.
#[derive(Clone, Debug, PartialEq)]
#[expect(
  clippy::struct_excessive_bools,
  reason = "the rules set each flag of a fighter at its own point of a step"
)]
pub struct Fighter {
  /// The x of its feet.
  pub x: f64,
  /// The y of its feet.
  pub y: f64,
  /// The speed in x that a kick gives, which fades.
  pub drift_x: f64,
  /// The speed in y that a kick gives, which fades.
  pub drift_y: f64,
  /// Its speed of run.
  pub speed_x: f64,
  /// The speed in y that a slope gives to a run.
  pub slope_y: f64,
  /// Its speed of fall, up when it is under 0.
  pub fall: f64,
  /// The kick in x of the next step.
  pub kick_x: f64,
  /// The kick in y of the next step.
  pub kick_y: f64,
  /// The move in x of the last step.
  pub step_x: f64,
  /// The move in y of the last step.
  pub step_y: f64,
  /// Says whether its feet stand on a solid.
  pub grounded: bool,
  /// Says whether it holds a ledge.
  pub on_ledge: bool,
  /// The side of the wall that it holds: -1, 1, or 0 for none.
  pub wall: i32,
  /// Says whether a side of its body touches a solid.
  pub touching: bool,
  /// The angle of the ground under it, in degrees, from -30 to 30.
  pub slope: f64,
  /// The steps until it can jump again.
  pub jump_wait: u32,
  /// The steps since its feet left the ground.
  pub air_time: u32,
  /// The steps of a brake on its fall.
  pub brake: u32,
  /// The steps until it can act again.
  pub stunned: u32,
  /// The steps until it can jump in the air again.
  pub air_jump_wait: u32,
  /// Its energy, from 0 to 100.
  pub energy: f64,
  /// Says whether a thrust of the role 3 pushed it since the last gain of energy.
  pub thrusted: bool,
  /// The pose that its moves ask.
  pub pose: Pose,
  /// The pose that a wall, a ledge or a halt forces over its moves.
  pub forced: Option<Pose>,
  /// Its role, from 1 to 3.
  pub role: u8,
  /// Its weapon, from 1 to 9.
  pub weapon: u8,
  /// The key of its weapon, from 1 to 3.
  pub slot: u8,
  /// The charge of a held shot, up to 35.
  pub charge: u32,
  /// Says whether the button of the shots is held.
  pub pressed: bool,
  /// The steps in which a second press strikes twice.
  pub double_wait: u32,
  /// The shot that the next fire throws: 0 for none, 2 for a double strike, else its charge.
  pub shot: u32,
  /// The steps since the last shot.
  pub reload: u32,
  /// The tilt of its body, in degrees.
  pub tilt: f64,
  /// The tilt that its body moves to.
  pub tilt_target: f64,
  /// The x of the point that it aims at.
  pub aim_x: f64,
  /// The y of the point that it aims at.
  pub aim_y: f64,
}

/// An effect that plays where the moves start it, until its script removes it.
#[derive(Clone, Debug)]
pub struct Spawn {
  /// The effect.
  pub effect: Effect,
  /// Its clip, placed on the map.
  pub clip: Clip,
}

/// A fighter on a map, with its shots, its effects and the drops, which the caller steps.
#[derive(Debug)]
pub struct World<C> {
  /// The state of the fighter.
  pub fighter: Fighter,
  map: Map,
  chance: C,
  figure: Clip,
  held: [bool; 5],
  thrust: bool,
  effects: Vec<Spawn>,
  shots: Vec<Shot>,
  stuck: Vec<Shot>,
  drops: Drops,
  tick: u64,
  serial: u64,
  view: [f64; 2],
  pointer: [f64; 2],
  camera: [f64; 2],
}

struct Weapon {
  shot: Kind,
  delay: u32,
  energy: f64,
  pressure: bool,
}

/// The weapons, from 1 to 9: their shot, the steps between two shots, the energy of a shot, and
/// whether a held button charges the shot.
const WEAPONS: [Weapon; 9] = [
  Weapon { shot: Kind::Arrow, delay: 5, energy: 15.0, pressure: true },
  Weapon { shot: Kind::Slash, delay: 15, energy: 20.0, pressure: false },
  Weapon { shot: Kind::SmokeBomb, delay: 60, energy: 40.0, pressure: true },
  Weapon { shot: Kind::Sweep, delay: 5, energy: 17.0, pressure: false },
  Weapon { shot: Kind::Spear, delay: 15, energy: 15.0, pressure: true },
  Weapon { shot: Kind::ShieldBash, delay: 27, energy: 20.0, pressure: false },
  Weapon { shot: Kind::Fireball, delay: 5, energy: 15.0, pressure: true },
  Weapon { shot: Kind::Lightning, delay: 1, energy: 10.0, pressure: false },
  Weapon { shot: Kind::Shockwave, delay: 8, energy: 15.0, pressure: false },
];

impl Fighter {
  fn at(x: f64, y: f64) -> Self {
    Self {
      x,
      y,
      drift_x: 0.0,
      drift_y: 0.0,
      speed_x: 0.0,
      slope_y: 0.0,
      fall: 0.0,
      kick_x: 0.0,
      kick_y: 0.0,
      step_x: 0.0,
      step_y: 0.0,
      grounded: false,
      on_ledge: false,
      wall: 0,
      touching: false,
      slope: 0.0,
      jump_wait: 0,
      air_time: 0,
      brake: 0,
      stunned: 0,
      air_jump_wait: 0,
      energy: 100.0,
      thrusted: false,
      pose: Pose::Idle,
      forced: None,
      role: 0,
      weapon: 0,
      slot: 1,
      charge: 0,
      pressed: false,
      double_wait: 0,
      shot: 0,
      reload: 0,
      tilt: 0.0,
      tilt_target: 0.0,
      aim_x: x + 100.0,
      aim_y: y - 60.0,
    }
  }
}

impl<C: Chance> World<C> {
  /// A fighter of `role`, from 1 to 3, with the first weapon of its role, at [`START`] on `map`,
  /// with a camera that shows `view` units of the map. `chance` gives the chance of the shots and
  /// of the drops.
  pub fn new(map: Map, view: [f64; 2], chance: C, role: u8) -> Self {
    let mut world = Self {
      fighter: Fighter::at(START[0], START[1]),
      map,
      chance,
      figure: Clip::figure(),
      held: [false; 5],
      thrust: false,
      effects: Vec::new(),
      shots: Vec::new(),
      stuck: Vec::new(),
      drops: Drops::default(),
      tick: 0,
      serial: 0,
      view,
      pointer: [500.0, 250.0],
      camera: [0.0, 700.0],
    };
    world.choose(role);
    world
  }

  /// The figure of the fighter, with its body, its weapon and its tilt.
  pub fn figure(&self) -> &Clip {
    &self.figure
  }

  /// The effects that play, the oldest first.
  pub fn effects(&self) -> &[Spawn] {
    &self.effects
  }

  /// The shots in flight, the oldest first. A shot that ended stays one step more.
  pub fn shots(&self) -> &[Shot] {
    &self.shots
  }

  /// The shots stuck in a solid, the oldest first: at most 150.
  pub fn stuck(&self) -> &[Shot] {
    &self.stuck
  }

  /// The drops in flight and their stains.
  pub fn drops(&self) -> &Drops {
    &self.drops
  }

  /// The top left corner of the camera on the map.
  pub fn camera(&self) -> [f64; 2] {
    self.camera
  }

  /// Gives the camera a view of `view` units of the map.
  pub fn resize(&mut self, view: [f64; 2]) {
    self.view = view;
  }

  /// Moves the pointer to `pointer`, in units of the map from the corner of the camera. At each
  /// step, the fighter aims at the point of the map under it.
  pub fn look(&mut self, pointer: [f64; 2]) {
    self.pointer = pointer;
  }

  /// Presses or releases `key`.
  pub fn key(&mut self, key: Key, down: bool) {
    let held = self.held[key as usize];
    self.held[key as usize] = down;
    if key == Key::Special {
      if down && !held {
        self.special();
      }
      if !down {
        self.thrust = false;
      }
    }
  }

  /// Presses the button of the shots: a weapon that charges charges while it is held, and another
  /// strikes, twice when the press comes soon after the last release.
  pub fn press(&mut self) {
    let fighter = &mut self.fighter;
    fighter.pressed = true;
    fighter.charge = 1;
    if !weapon_of(fighter.weapon).pressure {
      fighter.shot = if fighter.double_wait > 0 { 2 } else { 1 };
    }
  }

  /// Releases the button of the shots: a weapon that charges throws its shot.
  pub fn release(&mut self) {
    let fighter = &mut self.fighter;
    if !fighter.pressed {
      return;
    }
    fighter.pressed = false;
    let pressure = weapon_of(fighter.weapon).pressure;
    if pressure && fighter.charge > 0 {
      fighter.shot = fighter.charge;
    }
    if !pressure && fighter.double_wait == 0 {
      fighter.double_wait = 10;
    }
    fighter.charge = 0;
  }

  /// Takes the weapon of `slot`, from 1 to 3, of the role. A stunned fighter takes none, and the
  /// answer says so.
  pub fn change_weapon(&mut self, slot: u8) -> bool {
    if self.fighter.stunned != 0 {
      return false;
    }
    self.fighter.slot = slot;
    self.show_weapon(slot + (self.fighter.role - 1) * 3);
    true
  }

  /// Gives the fighter `role`, from 1 to 3, and the first weapon of that role.
  pub fn choose(&mut self, role: u8) {
    self.fighter.role = role;
    self.thrust = false;
    self.fighter.slot = 1;
    self.show_weapon(1 + (role - 1) * 3);
    self.change_weapon(1);
  }

  /// Throws `count` drops of `spray` from the point (`x`, `y`), about the direction `rotation`, in
  /// degrees from up.
  pub fn burst(&mut self, spray: Spray, count: u32, x: f64, y: f64, rotation: f64) {
    for _ in 0..count {
      self.drops.make(spray, x, y, rotation, &mut self.chance);
    }
  }

  /// Plays one step: the clips, the shots, the aim, the moves of the fighter, a gain of energy and
  /// a shot each fourth step, the camera, and the drops.
  pub fn frame(&mut self) {
    self.tick += 1;
    self.figure.advance();
    for spawn in &mut self.effects {
      spawn.clip.advance();
    }
    self.effects.retain(|spawn| !spawn.clip.dead);
    for shot in &mut self.shots {
      shot.clip.advance();
      if let Some(twin) = self.stuck.iter_mut().find(|twin| twin.serial == shot.serial) {
        twin.clip.advance();
      }
    }
    self.fighter.reload += 1;
    self.fly();
    self.fighter.aim_x = self.camera[0] + self.pointer[0];
    self.fighter.aim_y = self.camera[1] + self.pointer[1];
    self.charge();
    self.step();
    if self.tick.is_multiple_of(4) {
      self.gain();
      self.fire();
    }
    let (fighter, [width, height]) = (&self.fighter, self.view);
    let target = framed(fighter.x - width / 2.0, width, f64::from(self.map.width));
    self.camera[0] = approach(self.camera[0], target, 0.25);
    let target = framed(fighter.y - height * 0.6, height, f64::from(self.map.height));
    self.camera[1] = approach(self.camera[1], target, 0.25);
    self.drops.tick(&self.map);
  }

  fn show_weapon(&mut self, weapon: u8) {
    self.fighter.weapon = weapon;
    self.figure.wear(u16::from(self.fighter.role) + 1, weapon);
  }

  fn hand(&self) -> [f64; 2] {
    let t = (self.figure.rotation + 90.0) * RAD;
    let reach = if sitting(&self.figure) { 40.0 } else { 60.0 };
    [self.fighter.x - reach * cos(t), self.fighter.y - reach * sin(t)]
  }

  /// Starts `effect` at `at`, turned by `rotation` and scaled in y by `scale_y`. A scale of 0, as
  /// a warp of no length gives, stands for 1.
  fn spawn(&mut self, effect: Effect, at: [f64; 2], rotation: f64, scale_y: f64) {
    let mut clip = effect.clip();
    let scale_y = if scale_y == 0.0 { 1.0 } else { scale_y };
    (clip.x, clip.y, clip.rotation, clip.scale_y) = (at[0], at[1], rotation + 0.0, scale_y);
    clip.play_from(At::Frame(1));
    self.effects.push(Spawn { effect, clip });
  }

  fn special(&mut self) {
    let fighter = &mut self.fighter;
    if fighter.stunned != 0 {
      return;
    }
    match fighter.role {
      2 => {
        if fighter.energy > 45.0 && !fighter.grounded && fighter.air_jump_wait == 0 {
          fighter.energy -= 45.0;
          fighter.fall = -23.0;
          fighter.jump_wait = 5;
          fighter.air_jump_wait = 10;
          let at = [fighter.x, fighter.y];
          self.spawn(Effect::AirJump, at, 0.0, 1.0);
        }
      }
      3 => self.thrust |= fighter.energy > 6.0,
      _ if fighter.energy > 60.0 => self.warp(),
      _ => {}
    }
  }

  /// Moves the fighter along its aim to the last free point before a solid, at most 390 units
  /// away, with a trail and a thrust at each end.
  fn warp(&mut self) {
    let t = (self.figure.rotation + arm(&self.figure).rotation + 90.0) * RAD;
    let [from_x, from_y] = self.hand();
    let map = self.map;
    let mut far = 1;
    while far < 40 {
      let (x, y) =
        (from_x + -10.0 * f64::from(far) * cos(t), from_y + -10.0 * f64::from(far) * sin(t));
      if map.hit(x, y) || map.hit(x, y - 40.0) || map.hit(x, y + 40.0) {
        break;
      }
      far += 1;
    }
    self.fighter.energy -= 60.0;
    self.fighter.brake = 20;
    let (x, y) = if far > 6 {
      (from_x + -10.0 * f64::from(far - 3) * cos(t), from_y + -10.0 * f64::from(far - 3) * sin(t))
    } else {
      (from_x, from_y)
    };
    let (last_x, last_y) = (self.fighter.x, self.fighter.y);
    let rotation = (atan2(-(last_x - x), last_y - 60.0 - y) * 180.0 / PI).trunc();
    let (dx, dy) = (last_x - x, last_y - 60.0 - y);
    let size = (dx * dx + dy * dy).sqrt();
    self.spawn(Effect::WarpTrail, [x, y], rotation, size / 100.0);
    self.spawn(Effect::Thrust, [last_x, last_y - 60.0], rotation + 180.0, 1.0);
    self.spawn(Effect::Thrust, [x, y], rotation, 1.0);
    self.fighter.x = x;
    self.fighter.y = y + 60.0;
  }

  fn fly(&mut self) {
    let hand = self.hand();
    let mut at = self.shots.len();
    while at > 0 {
      at -= 1;
      if self.shots[at].clip.dead {
        self.shots.remove(at);
        continue;
      }
      let shot = &mut self.shots[at];
      if !shot.landed {
        shot.update(hand, &self.map);
      } else if shot.sticks() {
        shot.clip.dead = true;
        shot.clip.stop_at(At::Label("dead"));
        self.stuck.push(shot.clone());
        if self.stuck.len() > STUCK {
          self.stuck.remove(0);
        }
      } else {
        shot.clip.set_playing(true);
        shot.clip.dead |= !shot.clip.has_labels() || shot.clip.label() == Some("dead");
      }
    }
  }

  fn charge(&mut self) {
    let fighter = &mut self.fighter;
    let pressure = weapon_of(fighter.weapon).pressure;
    fighter.double_wait = fighter.double_wait.saturating_sub(1);
    if fighter.pressed && pressure && fighter.charge > 0 {
      if fighter.charge < 35 {
        fighter.charge += 1;
      }
      let shown = shown(&mut self.figure, fighter.weapon);
      if shown.frame() < 36 {
        shown.stop_at(At::Frame(whole(fighter.charge)));
      }
    }
    if fighter.pressed && !pressure && fighter.shot == 0 && fighter.double_wait == 0 {
      fighter.shot = 1;
    }
  }

  fn gain(&mut self) {
    let fighter = &mut self.fighter;
    fighter.air_jump_wait = fighter.air_jump_wait.saturating_sub(1);
    fighter.energy = (fighter.energy + 4.0).min(100.0);
    fighter.thrusted = false;
  }

  fn fire(&mut self) {
    if self.fighter.shot != 0 {
      self.shoot(self.fighter.shot);
      self.fighter.shot = 0;
    }
  }

  fn shoot(&mut self, attack: u32) {
    let fighter = &mut self.fighter;
    let weapon = weapon_of(fighter.weapon);
    if fighter.stunned != 0 {
      return;
    }
    let extra = if weapon.pressure { f64::from(attack) } else { 0.0 };
    if weapon.delay >= fighter.reload || fighter.energy < weapon.energy + extra / 2.0 {
      if weapon.pressure {
        shown(&mut self.figure, fighter.weapon).stop_at(At::Frame(1));
      }
      return;
    }
    fighter.reload = 0;
    fighter.energy -= (weapon.energy + f64::from(attack) / 2.0).trunc();
    let shown = shown(&mut self.figure, fighter.weapon);
    let second = !weapon.pressure && attack == 2 && shown.has_label("fire2");
    shown.play_from(At::Label(if second { "fire2" } else { "fire" }));
    let t = (self.figure.rotation + 90.0) * RAD;
    let reach = if sitting(&self.figure) { -40.0 } else { -60.0 };
    let (x, y) =
      (self.fighter.x + (reach * cos(t)).trunc(), self.fighter.y + (reach * sin(t)).trunc());
    self.serial += 1;
    let rotation = self.figure.rotation + arm(&self.figure).rotation;
    let launch = Launch { x, y, rotation, attack, hand: self.hand(), serial: self.serial };
    let (shot, recoil) = Shot::new(weapon.shot, &launch, &mut self.chance);
    match recoil {
      Some(Recoil::Brake) => self.fighter.brake = 10,
      Some(Recoil::Stun) => self.fighter.stunned = 30,
      None => {}
    }
    self.shots.push(shot);
  }
}

fn weapon_of(weapon: u8) -> &'static Weapon {
  &WEAPONS[usize::from(weapon - 1)]
}

fn approach(now: f64, target: f64, part: f64) -> f64 {
  now + (target - now) * part
}

/// The start of a side of the camera of `size` units that shows `value` on a side of the map of
/// `level` units: in the map, or centred on it when the map is shorter.
fn framed(value: f64, size: f64, level: f64) -> f64 {
  if size >= level { (level - size) / 2.0 } else { value.min(level - size).max(0.0) }
}

/// Rounds to the nearest whole number, and a half up.
fn round(value: f64) -> f64 {
  let floor = value.floor();
  if value - floor >= 0.5 { floor + 1.0 } else { floor }
}

#[expect(clippy::cast_possible_truncation, reason = "a charge is at most 35")]
fn whole(charge: u32) -> u16 {
  charge as u16
}

#[expect(clippy::expect_used, reason = "the figure holds its body")]
fn body(figure: &Clip) -> &Clip {
  figure.named("spr").expect("the figure holds its body")
}

#[expect(clippy::expect_used, reason = "the figure holds its body")]
fn body_mut(figure: &mut Clip) -> &mut Clip {
  figure.named_mut("spr").expect("the figure holds its body")
}

#[expect(clippy::expect_used, reason = "the figure holds its weapon")]
fn arm(figure: &Clip) -> &Clip {
  figure.named("weapon").expect("the figure holds its weapon")
}

#[expect(clippy::expect_used, reason = "the figure holds its weapon")]
fn arm_mut(figure: &mut Clip) -> &mut Clip {
  figure.named_mut("weapon").expect("the figure holds its weapon")
}

/// The clip of the weapon `weapon`, from 1 to 9, in the hand of the figure.
#[expect(clippy::expect_used, reason = "the figure holds each weapon")]
fn shown(figure: &mut Clip, weapon: u8) -> &mut Clip {
  arm_mut(figure).named_mut(&format!("w{weapon}")).expect("the figure holds each weapon")
}

fn sitting(figure: &Clip) -> bool {
  body(figure).label() == Some("sit")
}

#[cfg(test)]
mod tests;
