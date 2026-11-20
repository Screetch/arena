//! The drops of blood and of fire, which fall and leave a stain where they land.

use std::f64::consts::PI;

use libm::{atan2, cos, sin};

use super::Chance;
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

/// A drop in flight, or its stain.
#[derive(Clone, Debug)]
pub struct Drop {
  /// The effect of the drop.
  pub effect: Effect,
  /// Its clip, placed on the map.
  pub clip: Clip,
  /// Says whether it ended: its stain played, or it left the map.
  pub over: bool,
  rot: f64,
  speed: f64,
  fall: f64,
  last: u16,
}

/// The drops in flight, at most 600, and the stains, at most 800, the oldest first.
#[derive(Clone, Debug, Default)]
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

  pub(super) fn make(
    &mut self,
    spray: Spray,
    x: f64,
    y: f64,
    rotation: f64,
    chance: &mut impl Chance,
  ) {
    let (effect, last, scale) = spray.look();
    let mut clip = effect.clip();
    (clip.x, clip.y) = (x.trunc(), y.trunc());
    let rot = (rotation.trunc() + (-50.0 + chance.chance() * 100.0) - 90.0) * PI / 180.0;
    clip.scale_x = scale + chance.chance();
    clip.scale_y = scale + chance.chance();
    self.live.push(Drop { effect, clip, over: false, rot, speed: 10.0, fall: 0.0, last });
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
