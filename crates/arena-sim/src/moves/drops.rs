//! The drops of blood and of fire, which fall and leave a stain where they land.

use std::f64::consts::PI;

use libm::{atan2, cos, sin};

use super::{Chance, Id, Seeded};
use crate::clip::{Clip, Effect};
use crate::map::Map;

const LIVE: usize = 600;
const STAMPS: usize = 800;
const MARGIN: f64 = 120.0;

/// The drops of a spray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spray {
  /// Drops of blood.
  Splash,
  /// Drops of fire.
  Embers,
}

/// Drops that a caller throws: `count` drops of `spray` from `at`, about the direction `rotation`,
/// in degrees from up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burst {
  /// The drops.
  pub spray: Spray,
  /// The number of drops.
  pub count: u32,
  /// The point of the map that throws them.
  pub at: [f64; 2],
  /// Their direction, in degrees from up.
  pub rotation: f64,
  /// The fighter that throws them, when a fighter throws them.
  pub by: Option<Id>,
}

/// A drop in flight, or its stain.
#[derive(Clone, Debug, PartialEq)]
pub struct Drop {
  /// The effect of the drop.
  pub effect: Effect,
  /// Its clip, placed on the map.
  pub clip: Clip,
  /// Says whether it ended: its stain played, or it left the map.
  pub over: bool,
  /// The fighter that threw it, when a fighter threw it.
  pub by: Option<Id>,
  rot: f64,
  speed: f64,
  fall: f64,
  last: u16,
}

/// The drops in flight, at most 600, and the stains, at most 800, the oldest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drops {
  pub(super) live: Vec<Drop>,
  stamps: Vec<Drop>,
}

impl Spray {
  /// The effect of the drops, the last frame of their stain, and the least scale of a drop.
  fn look(self) -> (Effect, u16, f64) {
    match self {
      Self::Splash => (Effect::Splash, 15, 0.5),
      Self::Embers => (Effect::Embers, 9, 0.4),
    }
  }
}

impl Drops {
  /// The drops in flight.
  pub fn live(&self) -> &[Drop] {
    &self.live
  }

  /// The stains, which stay stopped where their drop ended.
  pub fn stamps(&self) -> &[Drop] {
    &self.stamps
  }

  /// Throws the drops of `burst`, which a [`Seeded`] of `seed` makes.
  pub(super) fn burst(&mut self, burst: &Burst, seed: u32) {
    let mut chance = Seeded::new(seed);
    for _ in 0..burst.count {
      self.make(burst, &mut chance);
    }
  }

  fn make(&mut self, burst: &Burst, chance: &mut Seeded) {
    let (effect, last, scale) = burst.spray.look();
    let mut clip = effect.clip();
    let [x, y] = burst.at;
    (clip.x, clip.y) = (x.trunc(), y.trunc());
    let rot = (burst.rotation.trunc() + (-50.0 + chance.chance() * 100.0) - 90.0) * PI / 180.0;
    clip.scale_x = scale + chance.chance();
    clip.scale_y = scale + chance.chance();
    let by = burst.by;
    self.live.push(Drop { effect, clip, over: false, by, rot, speed: 10.0, fall: 0.0, last });
    if self.live.len() > LIVE {
      self.live.drain(..self.live.len() - LIVE);
    }
  }

  pub(super) fn tick(&mut self, map: &Map) {
    for drop in &mut self.live {
      drop.clip.advance();
    }
    let mut at = 0;
    while at < self.live.len() {
      let drop = &mut self.live[at];
      if !drop.over {
        let touched = map.hit(drop.clip.x, drop.clip.y);
        drop.update(touched);
        at += 1;
        continue;
      }
      let drop = self.live.remove(at);
      self.stamp(drop, map);
    }
  }

  fn stamp(&mut self, mut drop: Drop, map: &Map) {
    let (x, y) = (drop.clip.x, drop.clip.y);
    if x < -MARGIN
      || y < -MARGIN
      || x > f64::from(map.width) + MARGIN
      || y > f64::from(map.height) + MARGIN
    {
      return;
    }
    drop.clip.set_playing(false);
    self.stamps.push(drop);
    if self.stamps.len() > STAMPS {
      self.stamps.drain(..self.stamps.len() - STAMPS);
    }
  }
}

impl Drop {
  fn update(&mut self, touched: bool) {
    let clip = &mut self.clip;
    if touched {
      clip.rotation = 0.0;
      if clip.frame() == 1 {
        clip.set_playing(true);
      }
      self.over |= clip.frame() == self.last;
    } else {
      let dx = cos(self.rot) * self.speed;
      self.fall += 1.0;
      let dy = sin(self.rot) * self.speed + self.fall;
      clip.rotation = atan2(-dx, dy) * 180.0 / PI;
      clip.x += dx;
      clip.y += dy;
    }
    self.over |= clip.y > 2500.0 || clip.y < -1000.0;
  }
}
