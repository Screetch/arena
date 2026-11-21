//! The shots of the weapons, which fly from the hand of a fighter.

use std::f64::consts::PI;

use libm::{atan2, cos, sin};
use serde::{Deserialize, Serialize};

use super::{Chance, Id, RAD, round};
use crate::clip::{At, Clip, Effect};
use crate::map::Map;

/// A shot of a weapon in flight, or stuck in a solid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shot {
  /// The effect of the shot.
  pub effect: Effect,
  /// Its clip, placed on the map.
  pub clip: Clip,
  /// Says whether it met a solid.
  pub landed: bool,
  /// The fighter that threw it.
  pub by: Id,
  pub(super) serial: u64,
  flight: Flight,
}

/// The shots of the weapons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
  Arrow,
  Slash,
  SmokeBomb,
  Sweep,
  Spear,
  ShieldBash,
  Fireball,
  Lightning,
  Shockwave,
}

/// Where a weapon throws a shot: from (`x`, `y`) to `rotation`, in degrees from up, with the
/// charge or the strike `attack`, while the hand of the fighter `by` stands at `hand`.
pub(super) struct Launch {
  pub(super) x: f64,
  pub(super) y: f64,
  pub(super) rotation: f64,
  pub(super) attack: u32,
  pub(super) hand: [f64; 2],
  pub(super) serial: u64,
  pub(super) by: Id,
}

/// What a shot does to the fighter that throws it.
pub(super) enum Recoil {
  /// It brakes the fall of the fighter for 10 steps.
  Brake,
  /// It stuns the fighter for 30 steps.
  Stun,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Flight {
  /// It fades in 5 steps where it struck.
  Flash { left: f64 },
  /// It waves along its way.
  Wave { speed: f64, turns: f64, rot: f64, wave: f64 },
  /// It stays at the hand and fades in 15 steps.
  Ring { left: f64 },
  /// It falls, unless its full charge holds it straight, and slows once it opens.
  Arrow { speed: f64, fall: f64, age: u32, full: bool, rot: f64 },
  /// It moves from the hand, in whole units, and fades in `life` steps.
  Swing { speed: f64, left: f64, life: f64, xx: f64, yy: f64, rot: f64, turns: bool },
  /// It falls along its way.
  Throw { speed: f64, fall: f64, rot: f64 },
  /// It falls along its way until it meets a solid, and then bursts into smoke.
  Bomb { speed: f64, fall: f64, popped: bool, rot: f64 },
}

impl Kind {
  fn effect(self) -> Effect {
    match self {
      Self::Arrow => Effect::Arrow,
      Self::Slash => Effect::Slash,
      Self::SmokeBomb => Effect::SmokeBomb,
      Self::Sweep => Effect::Sweep,
      Self::Spear => Effect::Spear,
      Self::ShieldBash => Effect::ShieldBash,
      Self::Fireball => Effect::Fireball,
      Self::Lightning => Effect::Lightning,
      Self::Shockwave => Effect::Shockwave,
    }
  }
}

impl Shot {
  pub(super) fn new(
    kind: Kind,
    launch: &Launch,
    chance: &mut impl Chance,
  ) -> (Self, Option<Recoil>) {
    let effect = kind.effect();
    let mut clip = effect.clip();
    clip.rotation = launch.rotation;
    let rot = (launch.rotation - 90.0) * RAD;
    let ahead = |reach: f64| (launch.x + cos(rot) * reach, launch.y + sin(rot) * reach);
    let attack = f64::from(launch.attack);
    let mut recoil = None;
    let flight = match kind {
      Kind::Lightning => {
        (clip.x, clip.y) = ahead(30.0);
        let frame = round(1.0 + chance.chance() * 5.0);
        clip.stop_at(At::Frame(frame_of(frame.min(f64::from(clip.frames())))));
        Flight::Flash { left: 5.0 }
      }
      Kind::Fireball => {
        (clip.x, clip.y) = ahead(40.0);
        fireball(&mut clip, attack, rot)
      }
      Kind::Shockwave => {
        [clip.x, clip.y] = launch.hand;
        clip.rotation = 0.0;
        Flight::Ring { left: 15.0 }
      }
      Kind::Arrow => {
        (clip.x, clip.y) = ahead(20.0);
        arrow(&mut clip, launch.attack, rot, &mut recoil)
      }
      Kind::Slash => {
        (clip.x, clip.y) = ahead(10.0);
        if launch.attack != 2 {
          clip.stop_at(At::Frame(2));
        }
        Flight::Swing { speed: 4.0, left: 13.0, life: 13.0, xx: 0.0, yy: 0.0, rot, turns: true }
      }
      Kind::Sweep => {
        (clip.x, clip.y) = (launch.x, launch.y);
        sweep(&mut clip, launch, rot, &mut recoil)
      }
      Kind::Spear => {
        (clip.x, clip.y) = ahead(20.0);
        if launch.attack == 35 {
          clip.stop_at(At::Frame(2));
        }
        Flight::Throw { speed: 10.0 + attack * 0.6, fall: 0.0, rot }
      }
      Kind::SmokeBomb => {
        clip.x = launch.x + (cos(rot) * 30.0).trunc();
        clip.y = launch.y + (sin(rot) * 30.0).trunc();
        Flight::Bomb { speed: 10.0, fall: 0.0, popped: false, rot }
      }
      Kind::ShieldBash => {
        (clip.x, clip.y) = (launch.x, launch.y);
        let (xx, yy) = ((cos(rot) * 10.0).trunc(), (sin(rot) * 10.0).trunc());
        Flight::Swing { speed: 3.0, left: 14.0, life: 14.0, xx, yy, rot, turns: true }
      }
    };
    (Self { effect, clip, landed: false, by: launch.by, serial: launch.serial, flight }, recoil)
  }

  /// Says whether the shot sticks in the solid that it meets.
  pub(super) fn sticks(&self) -> bool {
    matches!(self.flight, Flight::Arrow { .. } | Flight::Throw { .. })
  }

  /// Moves the shot one step, with the hand of the fighter at `hand`.
  pub(super) fn update(&mut self, hand: [f64; 2], map: &Map) {
    let Self { clip, flight, landed, .. } = self;
    match flight {
      Flight::Flash { left } => fade(clip, left, 5.0),
      Flight::Ring { left } => {
        [clip.x, clip.y] = hand;
        fade(clip, left, 15.0);
      }
      Flight::Wave { speed, turns, rot, wave } => {
        *turns += 1.0;
        let turn = *rot + -cos(*turns / 3.0) * *wave;
        go(clip, cos(turn) * *speed, sin(turn) * *speed);
        clip.dead |= clip.x > 5000.0 || clip.x < -10.0 || clip.y > 5000.0;
      }
      Flight::Arrow { speed, fall, age, full, rot } => {
        *age += 1;
        let dx = cos(*rot) * *speed;
        if !*full {
          *fall += 1.0;
        }
        go(clip, dx, sin(*rot) * *speed + *fall);
        if *age > 6 && *full && clip.frame() == 2 {
          clip.play_from(At::Frame(3));
        }
        if clip.frame() > 2 {
          *speed *= 0.8;
        }
        clip.dead |= outside(clip) || clip.frame() == 27;
      }
      Flight::Swing { speed, left, life, xx, yy, rot, turns } => {
        *xx = (*xx + cos(*rot) * *speed).trunc();
        *yy = (*yy + sin(*rot) * *speed).trunc();
        if *turns {
          clip.rotation = 180.0 + atan2(-cos(*rot), sin(*rot)) * 180.0 / PI;
        }
        (clip.x, clip.y) = (hand[0] + *xx, hand[1] + *yy);
        fade(clip, left, *life);
      }
      Flight::Throw { speed, fall, rot } => {
        *fall += 1.0;
        go(clip, cos(*rot) * *speed, sin(*rot) * *speed + *fall);
        clip.dead |= clip.x > 5000.0 || clip.x < -10.0 || clip.y > 5000.0;
      }
      Flight::Bomb { speed, fall, popped, rot } => {
        if !*popped {
          *fall += 1.0;
          go(clip, cos(*rot) * *speed, sin(*rot) * *speed + *fall);
        }
        clip.dead |= outside(clip) || clip.frame() == 16;
        if !*popped && map.hit(clip.x, clip.y) {
          *popped = true;
          clip.rotation = 0.0;
          clip.set_playing(true);
        }
        return;
      }
    }
    *landed |= matches!(flight, Flight::Wave { .. } | Flight::Arrow { .. } | Flight::Throw { .. })
      && map.hit(clip.x, clip.y);
  }
}

/// The flight of a fireball, whose charge `attack` gives its flame, its speed and its wave.
#[expect(clippy::expect_used, reason = "the clip of a fireball holds its flame")]
fn fireball(clip: &mut Clip, attack: f64, rot: f64) -> Flight {
  let flame = clip.named_mut("spr").expect("the clip of a fireball holds its flame");
  flame.stop_at(At::Frame(frame_of(attack.max(1.0))));
  let speed = if attack > 15.0 { 15.0 + (attack - 15.0) } else { 15.0 };
  Flight::Wave { speed, turns: 0.0, rot, wave: 1.0 - attack / 35.0 }
}

/// The flight of an arrow: a full charge holds it straight and brakes the fall of the fighter.
fn arrow(clip: &mut Clip, attack: u32, rot: f64, recoil: &mut Option<Recoil>) -> Flight {
  let full = attack == 35;
  if full {
    *recoil = Some(Recoil::Brake);
    clip.stop_at(At::Frame(2));
  }
  Flight::Arrow { speed: 15.0 + f64::from(attack), fall: 0.0, age: 0, full, rot }
}

/// The flight of a sweep: a double strike is slow and stuns the fighter, and a strike up or down
/// brakes its fall.
fn sweep(clip: &mut Clip, launch: &Launch, rot: f64, recoil: &mut Option<Recoil>) -> Flight {
  let mut speed = 3.0;
  let r = launch.rotation;
  if launch.attack == 2 {
    clip.stop_at(At::Frame(2));
    speed = 1.0;
    *recoil = Some(Recoil::Stun);
  } else if within(r, 60.0, 115.0) || within(r, -115.0, -60.0) {
    *recoil = Some(Recoil::Brake);
  }
  Flight::Swing { speed, left: 15.0, life: 15.0, xx: 0.0, yy: 0.0, rot, turns: false }
}

/// Says whether `value` lies between `low` and `high`, which it does not hold.
fn within(value: f64, low: f64, high: f64) -> bool {
  value > low && value < high
}

/// Moves `clip` by (`dx`, `dy`) and turns it along its way.
fn go(clip: &mut Clip, dx: f64, dy: f64) {
  clip.rotation = 180.0 + atan2(-dx, dy) * 180.0 / PI;
  clip.x += dx;
  clip.y += dy;
}

/// Fades `clip` over `life` steps, and ends it after the last.
fn fade(clip: &mut Clip, left: &mut f64, life: f64) {
  clip.alpha = *left / life;
  *left -= 1.0;
  clip.dead |= *left < 0.0;
}

fn outside(clip: &Clip) -> bool {
  clip.x > 5000.0 || clip.x < -10.0 || clip.y > 5000.0 || clip.y < -1000.0
}

#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "each caller gives a whole number from 1 to the frames of a timeline"
)]
fn frame_of(value: f64) -> u16 {
  value as u16
}
