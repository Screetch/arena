//! The cells of the screen, and the character that each one shows of the map.

use arena_sim::map::Map;

/// The characters of the cells, from the empty cell to the cell with the most light. A cell
/// shows the character at its level, from 1 to 9.
pub const RAMP: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

/// The side of a tile, in cells.
pub const TILE: u16 = 48;

/// The colour of the screen behind the map, in sRGB.
pub const BACK: [u8; 3] = [0x2b, 0x2f, 0x3a];

const BODY: [u8; 3] = [0x4a, 0x51, 0x62];
const TOP: [u8; 3] = [0x6a, 0x74, 0x90];
const BAND: f64 = 3.0;

/// The grid of square cells at one scale, in pixels for each unit of the world.
///
/// A cell is 8 units of the world at a scale of 1, and a whole number of pixels, at least 4. So
/// it holds 8 units only when 8 times the scale is a whole number. The grid starts at the origin
/// of the world, and its tiles of 48 by 48 cells too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
  cell: u32,
  scale: f64,
}

/// The character of one cell of a tile, with its colours in sRGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
  /// The column of the cell in its tile.
  pub column: u16,
  /// The row of the cell in its tile.
  pub row: u16,
  /// The index of the character in [`RAMP`], from 1 to 9.
  pub level: u8,
  /// The colour of the character.
  pub ink: [u8; 3],
  /// The colour that fills the whole cell under the character.
  pub fill: [u8; 3],
  /// The opacity of the character and of the fill.
  pub opacity: f32,
}

/// The sums of the 16 points of a cell: the premultiplied red, green and blue, from 0 to 1, and
/// the coverage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Cover {
  sum: [f32; 4],
}

impl Grid {
  /// The grid at `scale` pixels for each unit of the world.
  pub fn new(scale: f64) -> Self {
    Self { cell: whole((8.0 * scale).round().max(4.0)), scale }
  }

  /// The side of a cell, in pixels.
  pub fn cell(&self) -> u32 {
    self.cell
  }

  /// The side of a tile, in units of the world.
  pub fn span(&self) -> f64 {
    f64::from(TILE) * f64::from(self.cell) / self.scale
  }

  /// The glyph of each cell of the tile at (`column`, `row`) of the tiles that shows a character,
  /// row by row.
  pub fn tile(&self, map: &Map, column: u16, row: u16) -> Vec<Glyph> {
    let step = f64::from(self.cell) / self.scale / 4.0;
    let (left, top) = (f64::from(column) * self.span(), f64::from(row) * self.span());
    let mut glyphs = Vec::new();
    for y in 0..TILE {
      for x in 0..TILE {
        let mut cover = Cover::default();
        for point in 0..16 {
          let (i, j) = (f64::from(x * 4 + point % 4), f64::from(y * 4 + point / 4));
          cover.add(map, left + (i + 0.5) * step, top + (j + 0.5) * step);
        }
        let at = (column * TILE + x, row * TILE + y);
        glyphs.extend(cover.glyph(at).map(|glyph| Glyph { column: x, row: y, ..glyph }));
      }
    }
    glyphs
  }
}

impl Cover {
  fn add(&mut self, map: &Map, x: f64, y: f64) {
    let Some(colour) = paint(map, x, y) else { return };
    for (sum, channel) in self.sum.iter_mut().zip(colour) {
      *sum = single(f64::from(*sum) + f64::from(channel) / 255.0);
    }
    self.sum[3] += 1.0;
  }

  fn glyph(self, (column, row): (u16, u16)) -> Option<Glyph> {
    let mean = self.sum.map(|sum| sum / 16.0);
    let coverage = f64::from(mean[3]);
    if coverage <= 0.04 {
      return None;
    }
    let rgb = [0, 1, 2].map(|at| f64::from(single(f64::from(mean[at]) * 255.0)));
    let light = (0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]) / 255.0;
    let level = (light * 2.2).clamp(0.0, 1.0).powf(0.8) * 9.0 + (grain(column, row) - 0.5) * 2.0;
    let ink = rgb.map(|channel| (channel / coverage * 1.7).min(255.0));
    Some(Glyph {
      column,
      row,
      level: byte(level.round().clamp(1.0, 9.0)),
      ink: ink.map(|channel| byte(channel.round())),
      fill: ink.map(|channel| byte((channel * 0.6).round())),
      opacity: (mean[3] * 1.5).min(1.0),
    })
  }
}

/// The colour of the point (x, y) of the map: the band of a block covers the body of each solid.
fn paint(map: &Map, x: f64, y: f64) -> Option<[u8; 3]> {
  let mut body = false;
  for block in map.blocks.iter().filter(|block| block.contains(x, y)) {
    if y < f64::from(block.y) + BAND {
      return Some(TOP);
    }
    body = true;
  }
  (body || map.ramps.iter().any(|ramp| ramp.contains(x, y))).then_some(BODY)
}

/// A noise from 0 to 1 that is fixed for each cell of the grid of the world.
fn grain(column: u16, row: u16) -> f64 {
  let (x, y) = (f64::from(column) * 0.37 + 11.0, f64::from(row) * 0.37 + 11.0);
  let wave = (x * 12.9898 + y * 78.233).sin() * 43758.5453;
  wave - wave.floor()
}

#[expect(
  clippy::cast_possible_truncation,
  reason = "a cell keeps its sums in f32, and a colour that falls on a half rounds as that precision \
            gives it"
)]
fn single(value: f64) -> f32 {
  value as f32
}

#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "the side of a cell is a whole number of pixels from 4, far under the largest u32"
)]
fn whole(value: f64) -> u32 {
  value as u32
}

#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "each caller gives a whole number from 0 to 255"
)]
fn byte(value: f64) -> u8 {
  value as u8
}

#[cfg(test)]
mod tests {
  use super::{Glyph, Grid, RAMP};
  use arena_sim::map::FIRST;

  fn glyph(scale: f64, tile: (u16, u16), cell: (u16, u16)) -> Glyph {
    let glyphs = Grid::new(scale).tile(&FIRST, tile.0, tile.1);
    *glyphs.iter().find(|glyph| (glyph.column, glyph.row) == cell).expect("the cell shows a glyph")
  }

  #[test]
  fn each_cell_shows_the_character_colours_and_opacity_of_what_covers_it() {
    let cases = [
      (2.0, (0, 4), (2, 3), '#', [126, 138, 167], [75, 83, 100], 1.0),
      (2.0, (0, 4), (12, 33), '#', [139, 153, 186], [84, 92, 112], 1.0),
      (2.0, (0, 2), (37, 47), '.', [180, 197, 245], [108, 118, 147], 0.1875),
      (2.0, (0, 2), (42, 47), '=', [180, 197, 245], [108, 118, 147], 0.375),
      (2.0, (10, 4), (44, 10), '-', [126, 138, 167], [75, 83, 100], 0.5625),
      (2.0, (10, 4), (45, 10), '=', [153, 167, 206], [92, 100, 123], 0.75),
      (3.3, (10, 4), (2, 36), '*', [148, 161, 198], [89, 97, 119], 0.9375),
    ];
    for (scale, tile, cell, character, ink, fill, opacity) in cases {
      let got = glyph(scale, tile, cell);
      assert_eq!(
        (RAMP[usize::from(got.level)], got.ink, got.fill, got.opacity),
        (character, ink, fill, opacity),
        "the cell {cell:?} of the tile {tile:?} at the scale {scale}"
      );
    }
  }

  #[test]
  fn a_tile_shows_one_glyph_for_each_cell_that_a_solid_covers() {
    let grid = Grid::new(2.0);
    assert_eq!(grid.tile(&FIRST, 10, 4).len(), 1373);
    assert_eq!(grid.tile(&FIRST, 5, 0), []);
  }

  #[test]
  fn a_cell_is_a_whole_number_of_pixels_and_at_least_four() {
    assert_eq!((Grid::new(2.0).cell(), Grid::new(2.0).span()), (16, 384.0));
    assert_eq!((Grid::new(1.37).cell(), Grid::new(0.4).cell()), (11, 4));
    assert!((Grid::new(0.4).span() - 480.0).abs() < 1e-9);
  }
}
