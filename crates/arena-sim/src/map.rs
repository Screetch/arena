//! The map of a world: its size and its solid parts, in units of the world, with x to the right
//! and y down.

/// A world and its solid parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Map {
  /// The width of the world, from x 0.
  pub width: i32,
  /// The height of the world, from y 0.
  pub height: i32,
  /// The solid rectangles.
  pub blocks: &'static [Block],
  /// The solid slopes.
  pub ramps: &'static [Ramp],
}

/// A solid rectangle. It holds its left and top edges, and not its right and bottom edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
  /// The x of its left edge.
  pub x: i32,
  /// The y of its top edge.
  pub y: i32,
  /// Its width.
  pub width: i32,
  /// Its height.
  pub height: i32,
}

/// A point of a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
  /// Its x.
  pub x: i32,
  /// Its y.
  pub y: i32,
}

/// A solid slope from the point `from` to the point `to`, on its right, down to the lower of the
/// two.
///
/// The slope is a stair of columns one unit wide, from the x of `from` to the x of `to`, which it
/// does not hold. The top of each column is the y of the line at the left edge of the column,
/// rounded to the nearest unit, and a half rounds down the screen, to the larger y.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ramp {
  /// The left end of its line.
  pub from: Point,
  /// The right end of its line.
  pub to: Point,
}

/// The first map of arena: a floor, a wall at each side, platforms, two pillars, and a slope up to
/// a step on the right.
pub const FIRST: Map = Map {
  width: 4800,
  height: 2000,
  blocks: &[
    Block { x: 0, y: 1800, width: 4800, height: 200 },
    Block { x: 0, y: 0, width: 40, height: 2000 },
    Block { x: 4760, y: 0, width: 40, height: 2000 },
    Block { x: 600, y: 1640, width: 240, height: 30 },
    Block { x: 950, y: 1500, width: 240, height: 30 },
    Block { x: 1300, y: 1360, width: 240, height: 30 },
    Block { x: 1650, y: 1220, width: 300, height: 30 },
    Block { x: 300, y: 1150, width: 220, height: 30 },
    Block { x: 900, y: 950, width: 480, height: 40 },
    Block { x: 2100, y: 1000, width: 40, height: 800 },
    Block { x: 2140, y: 1000, width: 140, height: 30 },
    Block { x: 2420, y: 880, width: 40, height: 720 },
    Block { x: 2600, y: 1100, width: 400, height: 30 },
    Block { x: 3100, y: 950, width: 300, height: 30 },
    Block { x: 3500, y: 1200, width: 300, height: 30 },
    Block { x: 3150, y: 1560, width: 360, height: 30 },
    Block { x: 4200, y: 1620, width: 560, height: 180 },
  ],
  ramps: &[Ramp { from: Point { x: 3800, y: 1800 }, to: Point { x: 4200, y: 1620 } }],
};

impl Block {
  /// Says whether the block holds the point (x, y).
  pub fn contains(&self, x: f64, y: f64) -> bool {
    (f64::from(self.x)..f64::from(self.x + self.width)).contains(&x)
      && (f64::from(self.y)..f64::from(self.y + self.height)).contains(&y)
  }
}

impl Ramp {
  /// Says whether the slope holds the point (x, y).
  pub fn contains(&self, x: f64, y: f64) -> bool {
    if !(f64::from(self.from.x)..f64::from(self.to.x)).contains(&x) {
      return false;
    }
    #[expect(
      clippy::cast_possible_truncation,
      reason = "x lies between the two ends of the line, so its floor is an i32"
    )]
    let column = x.floor() as i32;
    (f64::from(self.top(column))..f64::from(self.from.y.max(self.to.y))).contains(&y)
  }

  fn top(&self, column: i32) -> i32 {
    let run = self.to.x - self.from.x;
    let line = self.from.y * run + (self.to.y - self.from.y) * (column - self.from.x);
    (2 * line + run).div_euclid(2 * run)
  }
}

#[cfg(test)]
mod tests {
  use super::{Block, FIRST};

  #[test]
  fn a_block_holds_its_left_and_top_edges_and_not_its_right_and_bottom_edges() {
    let block = Block { x: 600, y: 1640, width: 240, height: 30 };
    assert!(block.contains(600.0, 1640.0));
    assert!(block.contains(839.9, 1669.9));
    assert!(!block.contains(840.0, 1650.0));
    assert!(!block.contains(700.0, 1670.0));
    assert!(!block.contains(599.9, 1650.0));
    assert!(!block.contains(700.0, 1639.9));
  }

  #[test]
  fn the_slope_has_one_top_for_each_column_and_rounds_a_half_down_the_screen() {
    let ramp = FIRST.ramps[0];
    let edge = |x: f64, y: f64| ramp.contains(x, y) && !ramp.contains(x, y - 0.1);
    assert!(!ramp.contains(3800.5, 1799.9));
    assert!(!ramp.contains(3801.5, 1799.9));
    assert!(edge(3802.5, 1799.0));
    assert!(edge(3809.0, 1796.0) && edge(3809.9, 1796.0));
    assert!(edge(3810.5, 1796.0));
    assert!(edge(3811.5, 1795.0));
    assert!(edge(4000.0, 1710.0));
    assert!(edge(4199.5, 1620.0));
    assert!(ramp.contains(4000.0, 1799.9));
    assert!(!ramp.contains(4000.0, 1800.0));
    assert!(!ramp.contains(3799.9, 1799.0));
    assert!(!ramp.contains(4200.0, 1700.0));
  }

  #[test]
  fn each_solid_of_the_first_map_lies_in_its_world() {
    let world = Block { x: 0, y: 0, width: FIRST.width, height: FIRST.height };
    for block in FIRST.blocks {
      let (right, bottom) = (block.x + block.width, block.y + block.height);
      assert!(world.contains(block.x.into(), block.y.into()), "{block:?}");
      assert!(world.contains((right - 1).into(), (bottom - 1).into()), "{block:?}");
    }
    for ramp in FIRST.ramps {
      assert!(world.contains(ramp.from.x.into(), ramp.from.y.into()), "{ramp:?}");
      assert!(world.contains((ramp.to.x - 1).into(), ramp.to.y.into()), "{ramp:?}");
    }
  }
}
