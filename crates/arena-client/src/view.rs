//! The part of the world that the window shows, and the keys that zoom it.

use std::ops::Range;

use arena_sim::map::Map;
use winit::keyboard::Key;

use crate::cells::{Grid, TILE};
use crate::sample::Matrix;

/// The units of the world that a view shows at the zoom 1, as much as the window holds with no
/// distortion.
const SHOWN: [f64; 2] = [1600.0, 1000.0];

/// The steps of the zoom, each a quarter of a power of two: from 0.5 to 4.
const ZOOMS: Range<i32> = -4..9;

/// A rectangle of the world on a window, whose top left corner follows a camera.
///
/// Its scale, in pixels of the window for each unit of the world, fits the window to about 1600 by
/// 1000 units at the zoom 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
  world: [f64; 2],
  size: [u32; 2],
  zoom: i32,
  left: f64,
  top: f64,
}

/// A tile of the grid that a view shows, with the pixel of the window at its top left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
  /// The column of the tile in the grid of tiles.
  pub column: u16,
  /// The row of the tile in the grid of tiles.
  pub row: u16,
  /// The pixel of the top left corner of the tile, which can lie out of the window.
  pub origin: [i32; 2],
}

impl View {
  /// The view of `map` on a window of `size` pixels, from the top left corner of the world. Each
  /// side of the window holds at least one pixel.
  pub fn new(map: &Map, size: [u32; 2]) -> Self {
    Self {
      world: [f64::from(map.width), f64::from(map.height)],
      size,
      zoom: 0,
      left: 0.0,
      top: 0.0,
    }
  }

  /// The size of the window, in pixels.
  pub fn size(&self) -> [u32; 2] {
    self.size
  }

  /// The pixels of the window for each unit of the world.
  pub fn scale(&self) -> f64 {
    let fit = (f64::from(self.size[0]) / SHOWN[0]).min(f64::from(self.size[1]) / SHOWN[1]);
    fit * 2f64.powf(f64::from(self.zoom) / 4.0)
  }

  /// The units of the world that the view shows, in x and in y.
  pub fn shown(&self) -> [f64; 2] {
    let scale = self.scale();
    self.size.map(|pixels| f64::from(pixels) / scale)
  }

  /// Fits the view to a window of `size` pixels, with the same top left corner in the world.
  pub fn resize(&mut self, size: [u32; 2]) {
    self.size = size;
  }

  /// Puts the top left corner of the view on `camera`, in units of the world, each rounded to the
  /// nearest whole unit, a half up.
  pub fn follow(&mut self, camera: [f64; 2]) {
    [self.left, self.top] = camera.map(|at| (at + 0.5).floor());
  }

  /// The matrix from the units of the world to the pixels of the window.
  pub fn matrix(&self) -> Matrix {
    let scale = self.scale();
    [scale, 0.0, 0.0, scale, -self.left * scale, -self.top * scale]
  }

  /// Zooms in for + or = and out for -. Says whether the key is one of these.
  pub fn key(&mut self, key: &Key) -> bool {
    let step = match key {
      Key::Character(text) if text == "+" || text == "=" => 1,
      Key::Character(text) if text == "-" => -1,
      _ => return false,
    };
    self.zoom = (self.zoom + step).clamp(ZOOMS.start, ZOOMS.end - 1);
    true
  }

  /// The tiles of `grid` that the view shows, row by row.
  pub fn tiles(&self, grid: &Grid) -> Vec<Spot> {
    let (span, scale, shown) = (grid.span(), self.scale(), self.shown());
    let side = f64::from(TILE) * f64::from(grid.cell());
    let corner = [(-self.left * scale).round(), (-self.top * scale).round()];
    let columns = indices(self.left, shown[0], self.world[0], span);
    let mut spots = Vec::new();
    for row in indices(self.top, shown[1], self.world[1], span) {
      for column in columns.clone() {
        let origin = [corner[0] + f64::from(column) * side, corner[1] + f64::from(row) * side];
        spots.push(Spot { column, row, origin: origin.map(pixel) });
      }
    }
    spots
  }
}

/// The tiles of `span` units that a side of `shown` units from `start` shows in the world.
fn indices(start: f64, shown: f64, world: f64, span: f64) -> Range<u16> {
  index((start.max(0.0) / span).floor())..index(((start + shown).min(world) / span).ceil())
}

#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "an index of a tile is a whole number from 0, and a world holds far fewer than 65536 \
            tiles on a side"
)]
fn index(value: f64) -> u16 {
  value as u16
}

#[expect(
  clippy::cast_possible_truncation,
  reason = "a pixel of a tile is a whole number, and a world at the largest zoom is far under 2^31 \
            pixels"
)]
fn pixel(value: f64) -> i32 {
  value as i32
}

#[cfg(test)]
mod tests {
  use arena_sim::map::FIRST;
  use winit::keyboard::{Key, NamedKey};

  use super::{Spot, View};
  use crate::cells::Grid;

  fn first(view: &View) -> Spot {
    view.tiles(&Grid::new(view.scale()))[0]
  }

  fn press(view: &mut View, key: &Key, times: usize) {
    for _ in 0..times {
      assert!(view.key(key));
    }
  }

  #[test]
  #[expect(clippy::float_cmp, reason = "a view of whole pixels holds exact units")]
  fn a_view_shows_the_tiles_under_its_camera() {
    let mut view = View::new(&FIRST, [1600, 1000]);
    view.follow([0.0, 1000.0]);
    let spots = view.tiles(&Grid::new(view.scale()));
    assert!((view.scale() - 1.0).abs() < 1e-12);
    assert_eq!(view.shown(), [1600.0, 1000.0]);
    assert_eq!(spots.len(), 5 * 4);
    assert_eq!(spots[0], Spot { column: 0, row: 2, origin: [0, -232] });
    assert_eq!(spots[19], Spot { column: 4, row: 5, origin: [4 * 384, 5 * 384 - 1000] });
  }

  #[test]
  #[expect(clippy::float_cmp, reason = "a view of whole pixels holds exact units")]
  fn a_view_rounds_its_camera_to_whole_units_a_half_up() {
    let mut view = View::new(&FIRST, [1600, 1000]);
    view.follow([99.5, 874.5]);
    assert_eq!(first(&view), Spot { column: 0, row: 2, origin: [-100, -107] });
    assert_eq!(view.matrix(), [1.0, 0.0, 0.0, 1.0, -100.0, -875.0]);
    view.follow([-150.5, -0.4]);
    assert_eq!(view.matrix(), [1.0, 0.0, 0.0, 1.0, 150.0, 0.0]);
    assert_eq!(first(&view), Spot { column: 0, row: 0, origin: [150, 0] });
  }

  #[test]
  #[expect(clippy::float_cmp, reason = "a view of whole pixels holds exact units")]
  fn plus_and_minus_zoom_by_quarters_of_a_power_of_two_from_one_half_to_four() {
    let mut view = View::new(&FIRST, [1600, 1000]);
    press(&mut view, &Key::Character("+".into()), 1);
    assert!((view.scale() - 2f64.powf(0.25)).abs() < 1e-12);
    press(&mut view, &Key::Character("=".into()), 20);
    assert!((view.scale() - 4.0).abs() < 1e-12);
    press(&mut view, &Key::Character("-".into()), 40);
    assert!((view.scale() - 0.5).abs() < 1e-12);
    assert_eq!(view.shown(), [3200.0, 2000.0]);
    assert!(!view.key(&Key::Character("a".into())));
    assert!(!view.key(&Key::Named(NamedKey::ArrowLeft)));
  }

  #[test]
  #[expect(clippy::float_cmp, reason = "a view of whole pixels holds exact units")]
  fn a_window_wider_or_higher_than_the_world_shows_it_whole() {
    let mut view = View::new(&FIRST, [8000, 1000]);
    view.follow([-1600.0, 1000.0]);
    assert_eq!(first(&view), Spot { column: 0, row: 2, origin: [1600, -232] });
    view.resize([1600, 4000]);
    assert_eq!(view.size(), [1600, 4000]);
    assert_eq!(view.shown(), [1600.0, 4000.0]);
    view.follow([0.0, -1000.0]);
    assert_eq!(first(&view), Spot { column: 0, row: 0, origin: [0, 1000] });
  }
}
