//! The map on a target: the tiles that a view shows, each measured once for each scale and kept.

use std::collections::HashMap;

use arena_sim::map::Map;

use crate::cells::{Glyph, Grid};
use crate::font::Glyphs;
use crate::paint::{Painter, Tile};
use crate::view::View;

/// The tiles of a map that the painter drew at the scale of the last view.
///
/// The screen measures the glyphs of a tile once for each scale and keeps them until the scale
/// changes, so a tile that comes back into view is not measured again. A tile with no glyph holds
/// no texture. The textures are the costly part to keep: the screen holds at most twice the
/// tiles that the last view showed, and forgets first the texture that it showed least lately.
pub struct Screen<G> {
  painter: Painter<G>,
  map: Map,
  grid: Option<Grid>,
  glyphs: HashMap<(u16, u16), Vec<Glyph>>,
  kept: HashMap<(u16, u16), Kept>,
  frame: u64,
}

struct Kept {
  tile: Option<Tile>,
  shown: u64,
}

impl<G: Glyphs> Screen<G> {
  /// The screen of `map`, which `painter` draws.
  pub fn new(painter: Painter<G>, map: Map) -> Self {
    Self { painter, map, grid: None, glyphs: HashMap::new(), kept: HashMap::new(), frame: 0 }
  }

  /// Draws on `target` the part of the map that `view` shows.
  pub fn draw(&mut self, view: &View, target: &wgpu::TextureView) {
    let grid = Grid::new(view.scale());
    if self.grid != Some(grid) {
      self.glyphs.clear();
      self.kept.clear();
      self.grid = Some(grid);
    }
    self.frame += 1;
    let spots = view.tiles(&grid);
    for spot in &spots {
      let at = (spot.column, spot.row);
      let kept = self.kept.entry(at).or_insert_with(|| {
        let glyphs =
          self.glyphs.entry(at).or_insert_with(|| grid.tile(&self.map, spot.column, spot.row));
        Kept { tile: (!glyphs.is_empty()).then(|| self.painter.bake(grid, glyphs)), shown: 0 }
      });
      kept.shown = self.frame;
    }
    let tiles: Vec<(&Tile, [i32; 2])> = spots
      .iter()
      .filter_map(|spot| {
        let kept = self.kept.get(&(spot.column, spot.row));
        kept.and_then(|kept| kept.tile.as_ref()).map(|tile| (tile, spot.origin))
      })
      .collect();
    self.painter.paint(target, view.size(), &tiles);
    while self.kept.len() > 2 * spots.len() {
      let oldest = self.kept.iter().min_by_key(|(_, kept)| kept.shown).map(|(&at, _)| at);
      oldest.and_then(|at| self.kept.remove(&at));
    }
  }
}

#[cfg(test)]
mod tests {
  use arena_sim::map::FIRST;
  use winit::keyboard::{Key, NamedKey};

  use super::{Map, Screen};
  use crate::gpu::Gpu;
  use crate::view::View;

  #[test]
  fn the_screen_shows_the_bottom_left_corner_of_the_world_over_the_back() {
    let gpu = Gpu::held();
    let mut screen = Screen::new(gpu.painter(), FIRST);
    let view = View::new(&FIRST, [320, 200]);
    let pixels = gpu.painted([320, 200], |target| screen.draw(&view, target));
    let at = |x: usize, y: usize| pixels[y * 320 + x];
    assert_eq!(at(1, 195), [126, 138, 167, 255]);
    assert_eq!(at(2, 195), [75, 83, 100, 255]);
    assert_eq!(at(200, 50), [43, 47, 58, 255]);
  }

  #[test]
  fn a_tile_is_measured_once_for_each_scale_and_not_again_when_it_comes_back_into_view() {
    const BACK: [u8; 4] = [43, 47, 58, 255];
    let gpu = Gpu::held();
    let mut screen = Screen::new(gpu.painter(), FIRST);
    let mut view = View::new(&FIRST, [320, 200]);
    let tiles = view.tiles(&super::Grid::new(view.scale()));
    let first = (tiles[0].column, tiles[0].row);
    let before = gpu.painted([320, 200], |target| screen.draw(&view, target));
    assert!(before.contains(&[126, 138, 167, 255]));
    for _ in 0..40 {
      view.key(&Key::Named(NamedKey::ArrowRight));
      gpu.painted([320, 200], |target| screen.draw(&view, target));
    }
    assert!(!screen.kept.contains_key(&first));
    // A tile that is measured again would now measure an empty map and show only the back.
    screen.map = Map { blocks: &[], ramps: &[], ..FIRST };
    let mut after = Vec::new();
    for _ in 0..40 {
      view.key(&Key::Named(NamedKey::ArrowLeft));
      after = gpu.painted([320, 200], |target| screen.draw(&view, target));
    }
    assert_eq!(after, before);
    view.key(&Key::Character("+".into()));
    let zoomed = gpu.painted([320, 200], |target| screen.draw(&view, target));
    assert!(zoomed.iter().all(|pixel| *pixel == BACK));
  }

  #[test]
  fn the_screen_keeps_at_most_twice_the_tiles_that_it_shows_and_forgets_them_at_a_new_scale() {
    let gpu = Gpu::held();
    let mut screen = Screen::new(gpu.painter(), FIRST);
    let mut view = View::new(&FIRST, [320, 200]);
    let shown = |view: &View| view.tiles(&super::Grid::new(view.scale())).len();
    for _ in 0..40 {
      gpu.painted([320, 200], |target| screen.draw(&view, target));
      assert!(screen.kept.len() <= 2 * shown(&view));
      view.key(&Key::Named(NamedKey::ArrowRight));
    }
    assert_eq!(screen.kept.len(), 2 * shown(&view));
    view.key(&Key::Character("+".into()));
    gpu.painted([320, 200], |target| screen.draw(&view, target));
    assert_eq!(screen.kept.len(), shown(&view));
  }
}
