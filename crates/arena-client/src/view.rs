//! The part of the world that the window shows, and the keys that move it.

use std::ops::Range;

use arena_sim::map::Map;
use winit::keyboard::{Key, NamedKey};

use crate::cells::{Grid, TILE};

/// The units of the world that a view shows at the zoom 1, as much as the window holds with no
/// distortion.
const SHOWN: [f64; 2] = [1600.0, 1000.0];

/// The steps of the zoom, each a quarter of a power of two: from 0.5 to 4.
const ZOOMS: Range<i32> = -4..9;

/// A rectangle of the world on a window, which stays in the world.
///
/// Its scale, in pixels of the window for each unit of the world, fits the window to about 1600 by
/// 1000 units at the zoom 1. When the world is narrower or lower than the view, the view centres
/// it.
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
  /// The view of `map` on a window of `size` pixels, on the bottom left corner of the world. Each
  /// side of the window holds at least one pixel.
  pub fn new(map: &Map, size: [u32; 2]) -> Self {
    let world = [f64::from(map.width), f64::from(map.height)];
    let mut view = Self { world, size, zoom: 0, left: 0.0, top: world[1] };
    view.clamp();
    view
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

  /// Fits the view to a window of `size` pixels, with the same top left corner in the world.
  pub fn resize(&mut self, size: [u32; 2]) {
    self.size = size;
    self.clamp();
  }

  /// Moves the view a sixteenth of its size for an arrow key, and zooms in for + or = and out for
  /// -, about the centre of the view. Says whether the key is one of these.
  pub fn key(&mut self, key: &Key) -> bool {
    match key {
      Key::Named(NamedKey::ArrowLeft) => self.pan([-1.0, 0.0]),
      Key::Named(NamedKey::ArrowRight) => self.pan([1.0, 0.0]),
      Key::Named(NamedKey::ArrowUp) => self.pan([0.0, -1.0]),
      Key::Named(NamedKey::ArrowDown) => self.pan([0.0, 1.0]),
      Key::Character(text) if text == "+" || text == "=" => self.zoom(1),
      Key::Character(text) if text == "-" => self.zoom(-1),
      _ => return false,
    }
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

  fn shown(&self) -> [f64; 2] {
    let scale = self.scale();
    self.size.map(|pixels| f64::from(pixels) / scale)
  }

  fn pan(&mut self, way: [f64; 2]) {
    let shown = self.shown();
    self.left += way[0] * shown[0] / 16.0;
    self.top += way[1] * shown[1] / 16.0;
    self.clamp();
  }

  fn zoom(&mut self, step: i32) {
    let before = self.shown();
    self.zoom = (self.zoom + step).clamp(ZOOMS.start, ZOOMS.end - 1);
    let after = self.shown();
    self.left += (before[0] - after[0]) / 2.0;
    self.top += (before[1] - after[1]) / 2.0;
    self.clamp();
  }

  fn clamp(&mut self) {
    let shown = self.shown();
    self.left = within(self.left, shown[0], self.world[0]);
    self.top = within(self.top, shown[1], self.world[1]);
  }
}

/// The start of a side of `shown` units in a side of the world: in the world, or centred on it
/// when the world is shorter.
fn within(start: f64, shown: f64, world: f64) -> f64 {
  if shown >= world { (world - shown) / 2.0 } else { start.clamp(0.0, world - shown) }
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
  fn a_view_starts_on_the_bottom_left_corner_and_shows_its_tiles() {
    let view = View::new(&FIRST, [1600, 1000]);
    let spots = view.tiles(&Grid::new(view.scale()));
    assert!((view.scale() - 1.0).abs() < 1e-12);
    assert_eq!(spots.len(), 5 * 4);
    assert_eq!(spots[0], Spot { column: 0, row: 2, origin: [0, -232] });
    assert_eq!(spots[19], Spot { column: 4, row: 5, origin: [4 * 384, 5 * 384 - 1000] });
  }

  #[test]
  fn an_arrow_moves_a_sixteenth_of_the_view_and_the_view_stays_in_the_world() {
    let mut view = View::new(&FIRST, [1600, 1000]);
    press(&mut view, &Key::Named(NamedKey::ArrowLeft), 1);
    press(&mut view, &Key::Named(NamedKey::ArrowDown), 1);
    assert_eq!(first(&view).origin, [0, -232]);
    press(&mut view, &Key::Named(NamedKey::ArrowRight), 1);
    press(&mut view, &Key::Named(NamedKey::ArrowUp), 2);
    assert_eq!(first(&view), Spot { column: 0, row: 2, origin: [-100, -107] });
    press(&mut view, &Key::Named(NamedKey::ArrowRight), 100);
    assert_eq!(first(&view), Spot { column: 8, row: 2, origin: [8 * 384 - 3200, -107] });
  }

  #[test]
  fn plus_and_minus_zoom_by_quarters_of_a_power_of_two_from_one_half_to_four() {
    let mut view = View::new(&FIRST, [1600, 1000]);
    press(&mut view, &Key::Character("+".into()), 1);
    assert!((view.scale() - 2f64.powf(0.25)).abs() < 1e-12);
    press(&mut view, &Key::Character("=".into()), 20);
    assert!((view.scale() - 4.0).abs() < 1e-12);
    press(&mut view, &Key::Character("-".into()), 40);
    assert!((view.scale() - 0.5).abs() < 1e-12);
    assert_eq!(first(&view), Spot { column: 0, row: 0, origin: [0, 0] });
    assert!(!view.key(&Key::Character("a".into())));
    assert!(!view.key(&Key::Named(NamedKey::Space)));
  }

  #[test]
  fn a_zoom_keeps_the_centre_of_the_view() {
    let mut view = View::new(&FIRST, [1600, 1000]);
    press(&mut view, &Key::Named(NamedKey::ArrowRight), 16);
    press(&mut view, &Key::Named(NamedKey::ArrowUp), 4);
    press(&mut view, &Key::Character("+".into()), 4);
    assert!((view.scale() - 2.0).abs() < 1e-12);
    assert_eq!(first(&view), Spot { column: 5, row: 2, origin: [5 * 768 - 4000, 2 * 768 - 2000] });
  }

  #[test]
  fn a_window_wider_or_higher_than_the_world_centres_it() {
    let mut view = View::new(&FIRST, [8000, 1000]);
    assert_eq!(first(&view), Spot { column: 0, row: 2, origin: [1600, -232] });
    view.resize([1600, 4000]);
    assert_eq!(view.size(), [1600, 4000]);
    assert_eq!(first(&view), Spot { column: 0, row: 0, origin: [0, 1000] });
  }
}
