//! The cells of the screen, and the character that each one shows: its level, its colour and its
//! opacity, from the cover and the colour that the cell measures.

use std::f64::consts::PI;

use arena_sim::map::Map;
use libm::{pow, sin};

use crate::sample::Matrix;

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

/// The seconds of a breath.
const PERIOD: f64 = 2.8;

/// How a measure of cells becomes characters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
  /// The height of a cell in units, which the scale of the screen gives in pixels.
  pub cell: f64,
  /// The swing of the level with the time and the row.
  pub breath: f64,
  /// The swing of the level with a noise that is fixed for each cell.
  pub grain: f64,
  /// The multiplier of the colour.
  pub gain: f64,
  /// The multiplier of the light before it gives the level.
  pub density: f64,
  /// The multiplier of the opacity.
  pub opacity: f64,
  /// The opacity of the fill under each character, for each unit of its opacity.
  pub fill: f64,
  /// The multiplier of the colour of the fill.
  pub fill_shade: f64,
  /// The strength of the halo that a cover casts on the empty cells about it.
  pub halo: f64,
  /// The colour of the halo, from 0 to 255.
  pub halo_colour: [f64; 3],
  /// The part of the colour of the halo in the colour of each character.
  pub tint: f64,
  /// The colour, from 0 to 255, and the opacity of the glow about each character.
  pub shine: Option<[f64; 4]>,
  /// The blur of the glow, in heights of a cell.
  pub shine_size: f64,
}

impl Look {
  /// The look with no fill, no breath, no grain, no halo, no tint and no shine.
  pub const PLAIN: Self = Self {
    cell: 8.0,
    breath: 0.0,
    grain: 0.0,
    gain: 1.0,
    density: 1.0,
    opacity: 1.0,
    fill: 0.0,
    fill_shade: 0.5,
    halo: 0.0,
    halo_colour: [168.0, 85.0, 247.0],
    tint: 0.0,
    shine: None,
    shine_size: 1.2,
  };

  /// The look of the map.
  pub const MAP: Self =
    Self { grain: 1.0, gain: 1.7, density: 2.2, fill: 1.0, fill_shade: 0.6, ..Self::PLAIN };

  /// The characters of `cells`, the means of the cells of a patch of `columns`, row by row, each
  /// with its premultiplied red, green and blue and its cover. The patch starts at the cell
  /// `origin` of the grid of the screen, and `time` is the time in seconds.
  pub(crate) fn glyphs(
    &self,
    cells: &[[f32; 4]],
    columns: u16,
    origin: [f64; 2],
    time: f64,
  ) -> Vec<Glyph> {
    let rows = cells.len() / usize::from(columns);
    let mut glyphs = Vec::new();
    for (at, cell) in cells.iter().enumerate() {
      let (column, row) = (at % usize::from(columns), at / usize::from(columns));
      let spot = [origin[0] + whole(column), origin[1] + whole(row)];
      let glyph = self.glyph(cell, spot, time).or_else(|| {
        let spread = (self.halo > 0.0).then(|| spread(cells, columns, rows, [column, row]))?;
        self.halo(spread, spot, time)
      });
      let place = |value: usize| u16::try_from(value).unwrap_or(u16::MAX);
      glyphs.extend(glyph.map(|glyph| Glyph { column: place(column), row: place(row), ..glyph }));
    }
    glyphs
  }

  /// The character of a cell that the shapes cover, at the cell `spot` of the grid of the screen.
  fn glyph(&self, cell: &[f32; 4], spot: [f64; 2], time: f64) -> Option<Glyph> {
    let cover = f64::from(cell[3]);
    if cover <= 0.0 {
      return None;
    }
    let rgb = [0, 1, 2].map(|at| f64::from(single(f64::from(cell[at]) * 255.0)));
    let light = (0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]) / 255.0;
    let mut level = pow((light * self.density).min(1.0), 0.8) * 9.0;
    if cover > 0.04 {
      if self.breath > 0.0 {
        let tick = (time * 5.0).floor();
        let wave = sin(time * 2.0 * PI / PERIOD - spot[1] * (0.075 * self.cell)) * 1.3;
        level += self.breath * (wave + (hash(spot[0] + tick, spot[1] + tick) - 0.5) * 1.4);
      }
      if self.grain > 0.0 {
        level += self.grain * (hash(spot[0] * 0.37 + 11.0, spot[1] * 0.37 + 11.0) - 0.5) * 2.0;
      }
    }
    let mut level = (level + 0.5).floor();
    if cover > 0.04 {
      level = level.max(1.0);
    }
    let level = level.min(9.0);
    if level < 1.0 {
      return None;
    }
    let colour = [0, 1, 2].map(|at| {
      (rgb[at] / cover * self.gain).min(255.0) * (1.0 - self.tint)
        + self.halo_colour[at] * self.tint
    });
    let opacity = (cover * 1.5 * self.opacity).min(1.0);
    Some(Glyph {
      column: 0,
      row: 0,
      level: byte(level),
      ink: colour.map(|channel| byte((channel + 0.5).floor())),
      fill: colour.map(|channel| byte((channel * self.fill_shade + 0.5).floor())),
      opacity,
      fill_opacity: self.fill * opacity,
    })
  }

  /// The character of an empty cell in the halo of a cover that spreads `spread` over it.
  fn halo(&self, spread: f64, spot: [f64; 2], time: f64) -> Option<Glyph> {
    let tick = (time * 6.0).floor();
    let noise = hash(spot[0] * 1.7 + tick, spot[1] * 1.7 + tick);
    let level = (spread * self.halo * 5.0 * (0.6 + 0.8 * noise)).floor().min(4.0);
    (level >= 1.0).then(|| Glyph {
      column: 0,
      row: 0,
      level: byte(level),
      ink: self.halo_colour.map(|channel| byte((channel + 0.5).floor())),
      fill: [0; 3],
      opacity: (spread * self.halo * 2.2).min(0.9),
      fill_opacity: 0.0,
    })
  }
}

/// The mean cover of the 7 by 7 cells about the cell `at`, each weighted by 1 / (1 + i² + j²),
/// where a cell out of the patch covers nothing.
fn spread(cells: &[[f32; 4]], columns: u16, rows: usize, at: [usize; 2]) -> f64 {
  let (mut spread, mut weight) = (0.0, 0.0);
  for j in -3..=3 {
    for i in -3..=3 {
      let part = 1.0 / f64::from(1 + i * i + j * j);
      let (x, y) = (at[0].wrapping_add_signed(i as isize), at[1].wrapping_add_signed(j as isize));
      if x < usize::from(columns) && y < rows {
        spread += f64::from(cells[y * usize::from(columns) + x][3]) * part;
      }
      weight += part;
    }
  }
  spread / weight
}

/// A noise from 0 to 1 at the point (x, y).
fn hash(x: f64, y: f64) -> f64 {
  let wave = sin(x * 12.9898 + y * 78.233) * 43758.5453;
  wave - wave.floor()
}

/// A patch of the cells of the screen: `columns` by `rows` cells of `width` by `height` pixels,
/// from the pixel (`x0`, `y0`), a corner of the grid of the cells of that size on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patch {
  /// The x of the left edge, in pixels.
  pub x0: f64,
  /// The y of the top edge, in pixels.
  pub y0: f64,
  /// The cells in a row.
  pub columns: u16,
  /// The rows of cells.
  pub rows: u16,
  /// The width of a cell, in pixels.
  pub width: u32,
  /// The height of a cell, in pixels.
  pub height: u32,
}

impl Look {
  /// The patch of the cells of a screen of `screen` pixels that covers `bounds`, `[left, top,
  /// right, bottom]` in the units that `matrix` takes to pixels, and 4 cells more on each side
  /// when the look casts a halo. A cell is `cell` units high and as wide, in whole pixels, at most
  /// 64, and at least 4 high and 3 wide. A patch out of the screen, or of more than 40000 cells,
  /// is none.
  pub fn patch(&self, matrix: Matrix, bounds: [f64; 4], screen: [f64; 2]) -> Option<Patch> {
    let area = (matrix[0] * matrix[3] - matrix[1] * matrix[2]).abs().sqrt();
    let scale = if area > 0.0 { area } else { 1.0 };
    let side = (self.cell * scale + 0.5).floor().min(64.0);
    let (width, height) = (side.max(3.0), side.max(4.0));
    let pad = if self.halo > 0.0 { 4.0 } else { 0.0 };
    let corners = [
      [bounds[0], bounds[1]],
      [bounds[2], bounds[1]],
      [bounds[0], bounds[3]],
      [bounds[2], bounds[3]],
    ];
    let xs = corners.map(|[x, y]| matrix[0] * x + matrix[2] * y + matrix[4]);
    let ys = corners.map(|[x, y]| matrix[1] * x + matrix[3] * y + matrix[5]);
    let least = |values: [f64; 4]| values.into_iter().fold(f64::INFINITY, f64::min);
    let most = |values: [f64; 4]| values.into_iter().fold(f64::NEG_INFINITY, f64::max);
    let x0 = ((least(xs) / width).floor() - pad) * width;
    let y0 = ((least(ys) / height).floor() - pad) * height;
    let columns = (most(xs) / width).ceil() + pad - x0 / width;
    let rows = (most(ys) / height).ceil() + pad - y0 / height;
    let hidden =
      x0 + columns * width < 0.0 || y0 + rows * height < 0.0 || x0 > screen[0] || y0 > screen[1];
    let fits = columns > 0.0 && rows > 0.0 && columns * rows <= 40000.0;
    (fits && !hidden).then(|| Patch {
      x0,
      y0,
      columns: count(columns),
      rows: count(rows),
      width: pixels(width),
      height: pixels(height),
    })
  }
}

#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "a patch holds at most 40000 cells"
)]
fn count(value: f64) -> u16 {
  value as u16
}

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

/// The character of one cell, with its colours in sRGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
  /// The column of the cell in its tile or its patch.
  pub column: u16,
  /// The row of the cell in its tile or its patch.
  pub row: u16,
  /// The index of the character in [`RAMP`], from 1 to 9.
  pub level: u8,
  /// The colour of the character.
  pub ink: [u8; 3],
  /// The colour that fills the whole cell under the character.
  pub fill: [u8; 3],
  /// The opacity of the character.
  pub opacity: f64,
  /// The opacity of the fill.
  pub fill_opacity: f64,
}

impl Grid {
  /// The grid at `scale` pixels for each unit of the world.
  pub fn new(scale: f64) -> Self {
    Self { cell: pixels((8.0 * scale).round().max(4.0)), scale }
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
    let mut cells = Vec::with_capacity(usize::from(TILE) * usize::from(TILE));
    for y in 0..TILE {
      for x in 0..TILE {
        let mut sum = [0.0_f32; 4];
        for point in 0..16 {
          let (i, j) = (f64::from(x * 4 + point % 4), f64::from(y * 4 + point / 4));
          let Some(colour) = paint(map, left + (i + 0.5) * step, top + (j + 0.5) * step) else {
            continue;
          };
          for (sum, channel) in sum.iter_mut().zip(colour) {
            *sum = single(f64::from(*sum) + f64::from(channel) / 255.0);
          }
          sum[3] += 1.0;
        }
        cells.push(sum.map(|sum| sum / 16.0));
      }
    }
    let origin = [f64::from(column * TILE), f64::from(row * TILE)];
    Look::MAP.glyphs(&cells, TILE, origin, 0.0)
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

#[expect(
  clippy::cast_possible_truncation,
  reason = "a cell keeps its sums in f32, and a colour that falls on a half rounds as that precision \\
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
fn pixels(value: f64) -> u32 {
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

#[expect(clippy::cast_precision_loss, reason = "a column or a row of a patch is far under 2^52")]
fn whole(value: usize) -> f64 {
  value as f64
}

#[cfg(test)]
mod tests {
  use super::{Glyph, Grid, Look, Patch, RAMP};
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
  fn a_patch_covers_its_box_on_the_grid_of_the_screen_or_is_none_out_of_it_or_too_large() {
    let point =
      Look::PLAIN.patch([0.0, 0.0, 0.0, 0.0, 100.0, 100.0], [-8.0, -8.0, 8.0, 8.0], [800.0, 600.0]);
    let at = Patch { x0: 96.0, y0: 96.0, columns: 1, rows: 1, width: 8, height: 8 };
    assert_eq!(point, Some(at));
    let away = Look::PLAIN.patch(
      [1.0, 0.0, 0.0, 1.0, -100.0, -100.0],
      [0.0, 0.0, 10.0, 10.0],
      [800.0, 600.0],
    );
    assert_eq!(away, None);
    let large =
      Look::PLAIN.patch([1.0, 0.0, 0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 2000.0, 2000.0], [800.0, 600.0]);
    assert_eq!(large, None);
  }

  #[test]
  fn a_cell_is_a_whole_number_of_pixels_and_at_least_four() {
    assert_eq!((Grid::new(2.0).cell(), Grid::new(2.0).span()), (16, 384.0));
    assert_eq!((Grid::new(1.37).cell(), Grid::new(0.4).cell()), (11, 4));
    assert!((Grid::new(0.4).span() - 480.0).abs() < 1e-9);
  }
}
