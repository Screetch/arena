//! The moves of the fighters on a map, with their shots, their effects and the drops, at [`FPS`]
//! steps a second.
//!
//! The rules probe points of the map, as [`Map::hit`] says, and truncate each number where they
//! truncate it. Each sine, cosine and arc tangent comes from `libm`, and not from the system, so
//! that the same steps give the same state on each machine. A step moves the fighters in the order
//! of their ids, so the order in which a caller adds them changes nothing.
//!
//! [`FPS`]: crate::clip::FPS

mod drops;
mod shot;
mod step;

use std::collections::BTreeMap;
use std::f64::consts::PI;

use libm::{atan2, cos, sin};

pub use drops::{Burst, Drop, Drops, Spray};
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

/// The id of a fighter in a world, which the caller gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub u32);

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

/// What a fighter asks for each step, until it asks again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intents {
  /// The keys that it holds, each at the place of its [`Key`]. The first step that sees
  /// [`Key::Special`] held plays the special of the role.
  pub keys: [bool; 5],
  /// The point of the map that it aims at.
  pub aim: [f64; 2],
  /// Says whether the button of the shots is held. A weapon that charges charges while it is held
  /// and throws its shot when it is let go. Another strikes, twice when the press comes soon after
  /// the last release.
  pub button: bool,
  /// The key of its weapon, from 1 to 3. A stunned fighter takes it when its stun ends, and a
  /// value out of 1 to 3 changes nothing.
  pub slot: u8,
  /// Its role, from 1 to 3. A new role takes the first weapon of the role, then the weapon of
  /// `slot`. A value out of 1 to 3 changes nothing.
  pub role: u8,
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

/// A fighter: the state of its moves, in units of the map and in steps, its figure, and the last
/// intents that it got.
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
  figure: Clip,
  intents: Intents,
  held: [bool; 5],
  thrust: bool,
}

/// An effect that plays where a fighter started it, until its script removes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Spawn {
  /// The effect.
  pub effect: Effect,
  /// Its clip, placed on the map.
  pub clip: Clip,
  /// The fighter that started it.
  pub by: Id,
}

/// An effect that a fighter starts at `at`, turned by `rotation` and scaled in y by `scale_y`. A
/// scale of 0, as a warp of no length gives, stands for 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
  /// The effect.
  pub effect: Effect,
  /// The point of the map where it plays.
  pub at: [f64; 2],
  /// Its rotation, in degrees.
  pub rotation: f64,
  /// Its scale in y.
  pub scale_y: f64,
  /// The fighter that starts it.
  pub by: Id,
  /// Says whether it started before the clips of its step played, so that its clip played one
  /// frame in that step.
  pub early: bool,
}

/// What a world started, which [`World::start`] starts the same on another world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Start {
  /// An effect.
  Effect(Cue),
  /// A burst of drops, which a [`Seeded`] of `seed` makes.
  Burst {
    /// The burst.
    burst: Burst,
    /// The seed that the burst drew from the chance of its world.
    seed: u32,
  },
}

/// The whole state of a world: its fighters, its shots, its stuck shots, its effects, its drops and
/// the bursts that its next step throws, its step and its chance. A world on the same map that a
/// caller makes from a copy gives the same steps as the world that the copy came from, when it gets
/// the same intents.
#[derive(Clone, Debug, PartialEq)]
pub struct State<C> {
  fighters: BTreeMap<Id, Fighter>,
  chance: C,
  effects: Vec<Spawn>,
  shots: Vec<Shot>,
  stuck: Vec<Shot>,
  drops: Drops,
  bursts: Vec<Burst>,
  tick: u64,
  serial: u64,
}

/// The fighters on a map, with their shots, their effects and the drops, which the caller steps.
#[derive(Debug)]
pub struct World<C> {
  map: Map,
  state: State<C>,
  started: Vec<Start>,
}

/// One fighter in a step, with the parts of the world that its moves change.
struct Turn<'a, C> {
  id: Id,
  early: bool,
  fighter: &'a mut Fighter,
  map: Map,
  chance: &'a mut C,
  effects: &'a mut Vec<Spawn>,
  shots: &'a mut Vec<Shot>,
  serial: &'a mut u64,
  started: &'a mut Vec<Start>,
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
  /// A fighter of `role` at [`START`], with the first weapon of its role.
  fn new(role: u8) -> Self {
    let [x, y] = START;
    let intents =
      Intents { keys: [false; 5], aim: [x + 100.0, y - 60.0], button: false, slot: 1, role };
    let mut fighter = Self {
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
      aim_x: intents.aim[0],
      aim_y: intents.aim[1],
      figure: Clip::figure(),
      intents,
      held: [false; 5],
      thrust: false,
    };
    fighter.choose(role);
    fighter
  }

  /// The figure of the fighter, with its body, its weapon and its tilt.
  pub fn figure(&self) -> &Clip {
    &self.figure
  }

  /// The last intents that the fighter got, which it keeps until it gets new ones.
  pub fn intents(&self) -> Intents {
    self.intents
  }

  fn hand(&self) -> [f64; 2] {
    let t = (self.figure.rotation + 90.0) * RAD;
    let reach = if sitting(&self.figure) { 40.0 } else { 60.0 };
    [self.x - reach * cos(t), self.y - reach * sin(t)]
  }

  /// Takes `role` and the first weapon of that role.
  fn choose(&mut self, role: u8) {
    self.role = role;
    self.thrust = false;
    self.slot = 1;
    self.show_weapon(1 + (role - 1) * 3);
    self.change_weapon(1);
  }

  /// Takes the weapon of `slot` of the role, unless the fighter is stunned.
  fn change_weapon(&mut self, slot: u8) {
    if self.stunned != 0 {
      return;
    }
    self.slot = slot;
    self.show_weapon(slot + (self.role - 1) * 3);
  }

  fn show_weapon(&mut self, weapon: u8) {
    self.weapon = weapon;
    self.figure.wear(u16::from(self.role) + 1, weapon);
  }

  fn press(&mut self) {
    self.pressed = true;
    self.charge = 1;
    if !weapon_of(self.weapon).pressure {
      self.shot = if self.double_wait > 0 { 2 } else { 1 };
    }
  }

  fn release(&mut self) {
    if !self.pressed {
      return;
    }
    self.pressed = false;
    let pressure = weapon_of(self.weapon).pressure;
    if pressure && self.charge > 0 {
      self.shot = self.charge;
    }
    if !pressure && self.double_wait == 0 {
      self.double_wait = 10;
    }
    self.charge = 0;
  }

  fn charge(&mut self) {
    let pressure = weapon_of(self.weapon).pressure;
    self.double_wait = self.double_wait.saturating_sub(1);
    if self.pressed && pressure && self.charge > 0 {
      if self.charge < 35 {
        self.charge += 1;
      }
      let shown = shown(&mut self.figure, self.weapon);
      if shown.frame() < 36 {
        shown.stop_at(At::Frame(whole(self.charge)));
      }
    }
    if self.pressed && !pressure && self.shot == 0 && self.double_wait == 0 {
      self.shot = 1;
    }
  }

  fn gain(&mut self) {
    self.air_jump_wait = self.air_jump_wait.saturating_sub(1);
    self.energy = (self.energy + 4.0).min(100.0);
    self.thrusted = false;
  }
}

impl Cue {
  fn spawn(&self) -> Spawn {
    let mut clip = self.effect.clip();
    let scale_y = if self.scale_y == 0.0 { 1.0 } else { self.scale_y };
    (clip.x, clip.y, clip.rotation, clip.scale_y) =
      (self.at[0], self.at[1], self.rotation + 0.0, scale_y);
    clip.play_from(At::Frame(1));
    Spawn { effect: self.effect, clip, by: self.by }
  }
}

impl<C: Chance> World<C> {
  /// A world on `map` with no fighter. `chance` gives the chance of the shots and the seeds of the
  /// bursts of drops.
  pub fn new(map: Map, chance: C) -> Self {
    let state = State {
      fighters: BTreeMap::new(),
      chance,
      effects: Vec::new(),
      shots: Vec::new(),
      stuck: Vec::new(),
      drops: Drops::default(),
      bursts: Vec::new(),
      tick: 0,
      serial: 0,
    };
    Self::from_state(map, state)
  }

  /// The world on `map` of `state`, which a world on the same map gave.
  pub fn from_state(map: Map, state: State<C>) -> Self {
    Self { map, state, started: Vec::new() }
  }

  /// The whole state of the world, which a caller can copy and give to [`World::from_state`].
  pub fn state(&self) -> &State<C> {
    &self.state
  }

  /// The fighters, in the order of their ids.
  pub fn fighters(&self) -> &BTreeMap<Id, Fighter> {
    &self.state.fighters
  }

  /// The effects that play, the oldest first.
  pub fn effects(&self) -> &[Spawn] {
    &self.state.effects
  }

  /// The shots in flight, the oldest first. A shot that ended stays one step more.
  pub fn shots(&self) -> &[Shot] {
    &self.state.shots
  }

  /// The shots stuck in a solid, the oldest first: at most 150.
  pub fn stuck(&self) -> &[Shot] {
    &self.state.stuck
  }

  /// The drops in flight and their stains.
  pub fn drops(&self) -> &Drops {
    &self.state.drops
  }

  /// Adds a fighter of `role`, from 1 to 3, with the first weapon of its role, at [`START`]. Says
  /// whether it came: a role out of 1 to 3, or an id that a fighter holds, adds nothing.
  pub fn add(&mut self, id: Id, role: u8) -> bool {
    if !(1..=3).contains(&role) || self.state.fighters.contains_key(&id) {
      return false;
    }
    self.state.fighters.insert(id, Fighter::new(role));
    true
  }

  /// Removes the fighter `id` and its shots in flight, which follow its hand. Says whether it
  /// stood.
  pub fn remove(&mut self, id: Id) -> bool {
    self.state.shots.retain(|shot| shot.by != id);
    self.state.fighters.remove(&id).is_some()
  }

  /// Gives the fighter `id` the intents of the next steps. Says whether it stands.
  pub fn intend(&mut self, id: Id, intents: Intents) -> bool {
    let Some(fighter) = self.state.fighters.get_mut(&id) else { return false };
    fighter.intents = intents;
    true
  }

  /// Asks the next step to throw the drops of `burst`, after the drops that fly, from a seed that
  /// it draws from the chance of the world.
  pub fn burst(&mut self, burst: Burst) {
    self.state.bursts.push(burst);
  }

  /// Starts what a step of another world started, as it stands at the end of that step, and lists
  /// nothing. A caller gives each start that the step listed, in its order, after the same step of
  /// this world.
  pub fn start(&mut self, start: &Start) {
    match start {
      Start::Effect(cue) => {
        let mut spawn = cue.spawn();
        if cue.early {
          spawn.clip.advance();
        }
        self.state.effects.push(spawn);
      }
      Start::Burst { burst, seed } => self.state.drops.burst(burst, *seed),
    }
  }

  /// Plays one step: the intents of each fighter, the clips, the shots, the moves of each fighter
  /// in the order of the ids, a gain of energy and a shot each fourth step, the drops, and the
  /// bursts that a caller asked. Gives what the step started: the effects and the bursts of drops.
  #[expect(
    clippy::redundant_closure_for_method_calls,
    reason = "each turn borrows the world for a lifetime of its own, which a path to the method \
              cannot name"
  )]
  pub fn frame(&mut self) -> Vec<Start> {
    self.state.tick += 1;
    let tick = self.state.tick;
    self.each(true, |turn| turn.heed());
    let state = &mut self.state;
    for fighter in state.fighters.values_mut() {
      fighter.figure.advance();
    }
    for spawn in &mut state.effects {
      spawn.clip.advance();
    }
    state.effects.retain(|spawn| !spawn.clip.dead);
    for shot in &mut state.shots {
      shot.clip.advance();
      if let Some(twin) = state.stuck.iter_mut().find(|twin| twin.serial == shot.serial) {
        twin.clip.advance();
      }
    }
    for fighter in state.fighters.values_mut() {
      fighter.reload += 1;
    }
    self.fly();
    self.each(false, |turn| turn.act(tick));
    self.state.drops.tick(&self.map);
    for burst in std::mem::take(&mut self.state.bursts) {
      let seed = seed_of(self.state.chance.chance());
      self.state.drops.burst(&burst, seed);
      self.started.push(Start::Burst { burst, seed });
    }
    std::mem::take(&mut self.started)
  }

  fn each(&mut self, early: bool, mut act: impl FnMut(&mut Turn<'_, C>)) {
    let State { fighters, chance, effects, shots, serial, .. } = &mut self.state;
    for (&id, fighter) in fighters.iter_mut() {
      act(&mut Turn {
        id,
        early,
        fighter,
        map: self.map,
        chance: &mut *chance,
        effects: &mut *effects,
        shots: &mut *shots,
        serial: &mut *serial,
        started: &mut self.started,
      });
    }
  }

  #[expect(clippy::expect_used, reason = "a fighter that leaves takes its shots in flight")]
  fn fly(&mut self) {
    let State { fighters, shots, stuck, .. } = &mut self.state;
    let mut at = shots.len();
    while at > 0 {
      at -= 1;
      if shots[at].clip.dead {
        shots.remove(at);
        continue;
      }
      let shot = &mut shots[at];
      if !shot.landed {
        let fighter =
          fighters.get(&shot.by).expect("a fighter that leaves takes its shots in flight");
        shot.update(fighter.hand(), &self.map);
      } else if shot.sticks() {
        shot.clip.dead = true;
        shot.clip.stop_at(At::Label("dead"));
        stuck.push(shot.clone());
        if stuck.len() > STUCK {
          stuck.remove(0);
        }
      } else {
        shot.clip.set_playing(true);
        shot.clip.dead |= !shot.clip.has_labels() || shot.clip.label() == Some("dead");
      }
    }
  }
}

impl<C: Chance> Turn<'_, C> {
  /// Takes the intents of the fighter: its role, its weapon, its aim, its keys and its button.
  fn heed(&mut self) {
    let intents = self.fighter.intents;
    if (1..=3).contains(&intents.role) && intents.role != self.fighter.role {
      self.fighter.choose(intents.role);
    }
    if (1..=3).contains(&intents.slot) && intents.slot != self.fighter.slot {
      self.fighter.change_weapon(intents.slot);
    }
    [self.fighter.aim_x, self.fighter.aim_y] = intents.aim;
    let special = Key::Special as usize;
    let held = std::mem::replace(&mut self.fighter.held, intents.keys)[special];
    if intents.keys[special] && !held {
      self.special();
    }
    if !intents.keys[special] {
      self.fighter.thrust = false;
    }
    if intents.button && !self.fighter.pressed {
      self.fighter.press();
    }
    if !intents.button {
      self.fighter.release();
    }
  }

  /// The charge, the moves, and a gain of energy and a shot each fourth step.
  fn act(&mut self, tick: u64) {
    self.fighter.charge();
    self.step();
    if tick.is_multiple_of(4) {
      self.fighter.gain();
      self.fire();
    }
  }

  fn spawn(&mut self, effect: Effect, at: [f64; 2], rotation: f64, scale_y: f64) {
    let cue = Cue { effect, at, rotation, scale_y, by: self.id, early: self.early };
    self.effects.push(cue.spawn());
    self.started.push(Start::Effect(cue));
  }

  fn special(&mut self) {
    let fighter = &mut *self.fighter;
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
      3 => fighter.thrust |= fighter.energy > 6.0,
      _ if fighter.energy > 60.0 => self.warp(),
      _ => {}
    }
  }

  /// Moves the fighter along its aim to the last free point before a solid, at most 390 units
  /// away, with a trail and a thrust at each end.
  fn warp(&mut self) {
    let figure = &self.fighter.figure;
    let t = (figure.rotation + arm(figure).rotation + 90.0) * RAD;
    let [from_x, from_y] = self.fighter.hand();
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

  fn fire(&mut self) {
    if self.fighter.shot != 0 {
      self.shoot(self.fighter.shot);
      self.fighter.shot = 0;
    }
  }

  fn shoot(&mut self, attack: u32) {
    let fighter = &mut *self.fighter;
    let weapon = weapon_of(fighter.weapon);
    if fighter.stunned != 0 {
      return;
    }
    let extra = if weapon.pressure { f64::from(attack) } else { 0.0 };
    if weapon.delay >= fighter.reload || fighter.energy < weapon.energy + extra / 2.0 {
      if weapon.pressure {
        shown(&mut fighter.figure, fighter.weapon).stop_at(At::Frame(1));
      }
      return;
    }
    fighter.reload = 0;
    fighter.energy -= (weapon.energy + f64::from(attack) / 2.0).trunc();
    let shown = shown(&mut fighter.figure, fighter.weapon);
    let second = !weapon.pressure && attack == 2 && shown.has_label("fire2");
    shown.play_from(At::Label(if second { "fire2" } else { "fire" }));
    let t = (fighter.figure.rotation + 90.0) * RAD;
    let reach = if sitting(&fighter.figure) { -40.0 } else { -60.0 };
    let (x, y) = (fighter.x + (reach * cos(t)).trunc(), fighter.y + (reach * sin(t)).trunc());
    *self.serial += 1;
    let rotation = fighter.figure.rotation + arm(&fighter.figure).rotation;
    let (hand, serial, by) = (fighter.hand(), *self.serial, self.id);
    let launch = Launch { x, y, rotation, attack, hand, serial, by };
    let (shot, recoil) = Shot::new(weapon.shot, &launch, &mut *self.chance);
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

/// Rounds to the nearest whole number, and a half up.
fn round(value: f64) -> f64 {
  let floor = value.floor();
  if value - floor >= 0.5 { floor + 1.0 } else { floor }
}

#[expect(clippy::cast_possible_truncation, reason = "a charge is at most 35")]
fn whole(charge: u32) -> u16 {
  charge as u16
}

#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "a chance from 0 to 1, which it never gives, times 2^32 is a whole u32 once truncated"
)]
fn seed_of(chance: f64) -> u32 {
  (chance * 4_294_967_296.0) as u32
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
