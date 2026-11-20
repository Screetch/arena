//! The map on a target: the tiles that a view shows, each measured once for each scale and kept.

use std::collections::HashMap;

use arena_sim::map::Map;

use crate::cells::{Glyph, Grid, TILE};
use crate::font::Glyphs;
use crate::paint::{Painter, Plan, Tile};
use crate::sprites::Drawn;
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

  /// Draws on `target` the part of the map that `view` shows, and over it `sprites` in their
  /// order.
  pub fn draw(&mut self, view: &View, sprites: &[Drawn], target: &wgpu::TextureView) {
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
        let plan = Plan { cells: [u32::from(TILE); 2], cell: [grid.cell(); 2], margin: 0 };
        Kept { tile: (!glyphs.is_empty()).then(|| self.painter.bake(plan, glyphs)), shown: 0 }
      });
      kept.shown = self.frame;
    }
    let baked: Vec<(Tile, [i32; 2])> =
      sprites.iter().flat_map(|drawn| self.sprite(drawn)).collect();
    let mut tiles: Vec<(&Tile, [i32; 2])> = spots
      .iter()
      .filter_map(|spot| {
        let kept = self.kept.get(&(spot.column, spot.row));
        kept.and_then(|kept| kept.tile.as_ref()).map(|tile| (tile, spot.origin))
      })
      .collect();
    tiles.extend(baked.iter().map(|(tile, origin)| (tile, *origin)));
    self.painter.paint(target, view.size(), &tiles);
    while self.kept.len() > 2 * spots.len() {
      let oldest = self.kept.iter().min_by_key(|(_, kept)| kept.shown).map(|(&at, _)| at);
      oldest.and_then(|at| self.kept.remove(&at));
    }
  }
}

impl<G: Glyphs> Screen<G> {
  /// The textures of the characters of `drawn`, each with the pixel of its top left corner: its
  /// glow under its characters when its look shines.
  fn sprite(&mut self, drawn: &Drawn) -> Vec<(Tile, [i32; 2])> {
    let patch = drawn.patch;
    let (cells, cell) = ([patch.columns, patch.rows].map(u32::from), [patch.width, patch.height]);
    let origin = [pixel(patch.x0), pixel(patch.y0)];
    let Some(colour) = drawn.look.shine else {
      return vec![(self.painter.bake(Plan { cells, cell, margin: 0 }, &drawn.glyphs), origin)];
    };
    let sigma = f64::from(patch.height) * drawn.look.shine_size / 2.0;
    let margin = (3.0 * sigma).ceil();
    let glyphs = self.painter.bake(Plan { cells, cell, margin: whole(margin) }, &drawn.glyphs);
    let colour = [colour[0] / 255.0, colour[1] / 255.0, colour[2] / 255.0, colour[3]];
    let glow = self.painter.glow(&glyphs, sigma, colour);
    let origin = origin.map(|at| at - pixel(margin));
    vec![(glow, origin), (glyphs, origin)]
  }
}

#[expect(
  clippy::cast_possible_truncation,
  reason = "a pixel of a patch on the screen is a whole number far under 2^31"
)]
fn pixel(value: f64) -> i32 {
  value as i32
}

#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "a margin is a whole number of pixels from 0, far under 2^32"
)]
fn whole(value: f64) -> u32 {
  value as u32
}

#[cfg(test)]
mod tests {
  use arena_sim::map::FIRST;
  use winit::keyboard::Key;

  use super::{Map, Screen};
  use crate::cells::{Glyph, Look, Patch};
  use crate::gpu::Gpu;
  use crate::sprites::Drawn;
  use crate::view::View;

  #[test]
  fn the_screen_shows_the_bottom_left_corner_of_the_world_over_the_back() {
    let gpu = Gpu::held();
    let mut screen = Screen::new(gpu.painter(), FIRST);
    let mut view = View::new(&FIRST, [320, 200]);
    view.follow([0.0, 1000.0]);
    let pixels = gpu.painted([320, 200], |target| screen.draw(&view, &[], target));
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
    let pan = |view: &mut View, step: u32| view.follow([f64::from(step * 100).min(3200.0), 1000.0]);
    pan(&mut view, 0);
    let tiles = view.tiles(&super::Grid::new(view.scale()));
    let first = (tiles[0].column, tiles[0].row);
    let before = gpu.painted([320, 200], |target| screen.draw(&view, &[], target));
    assert!(before.contains(&[126, 138, 167, 255]));
    for step in 1..=40 {
      pan(&mut view, step);
      gpu.painted([320, 200], |target| screen.draw(&view, &[], target));
    }
    assert!(!screen.kept.contains_key(&first));
    // A tile that is measured again would now measure an empty map and show only the back.
    screen.map = Map { blocks: &[], ramps: &[], ..FIRST };
    let mut after = Vec::new();
    for step in (0..40).rev() {
      pan(&mut view, step);
      after = gpu.painted([320, 200], |target| screen.draw(&view, &[], target));
    }
    assert_eq!(after, before);
    view.key(&Key::Character("+".into()));
    let zoomed = gpu.painted([320, 200], |target| screen.draw(&view, &[], target));
    assert!(zoomed.iter().all(|pixel| *pixel == BACK));
  }

  #[test]
  fn the_screen_keeps_at_most_twice_the_tiles_that_it_shows_and_forgets_them_at_a_new_scale() {
    let gpu = Gpu::held();
    let mut screen = Screen::new(gpu.painter(), FIRST);
    let mut view = View::new(&FIRST, [320, 200]);
    let shown = |view: &View| view.tiles(&super::Grid::new(view.scale())).len();
    for step in 0..40 {
      view.follow([f64::from(step * 100).min(3200.0), 1000.0]);
      gpu.painted([320, 200], |target| screen.draw(&view, &[], target));
      assert!(screen.kept.len() <= 2 * shown(&view));
    }
    assert_eq!(screen.kept.len(), 2 * shown(&view));
    view.key(&Key::Character("+".into()));
    gpu.painted([320, 200], |target| screen.draw(&view, &[], target));
    assert_eq!(screen.kept.len(), shown(&view));
  }
  #[test]
  fn a_sprite_shows_its_characters_at_its_patch_and_a_shining_look_its_glow_about_them() {
    const BACK: [u8; 4] = [43, 47, 58, 255];
    let gpu = Gpu::held();
    let mut screen = Screen::new(gpu.painter(), Map { blocks: &[], ramps: &[], ..FIRST });
    let view = View::new(&FIRST, [80, 50]);
    let glyph = Glyph {
      column: 1,
      row: 0,
      level: 9,
      ink: [255, 0, 0],
      fill: [0; 3],
      opacity: 1.0,
      fill_opacity: 0.0,
    };
    let patch = Patch { x0: 16.0, y0: 8.0, columns: 2, rows: 1, width: 8, height: 8 };
    let plain = Drawn { patch, glyphs: vec![glyph], look: Look::PLAIN };
    let pixels =
      gpu.painted([80, 50], |target| screen.draw(&view, std::slice::from_ref(&plain), target));
    let at = |pixels: &[[u8; 4]], x: usize, y: usize| pixels[y * 80 + x];
    assert_eq!(
      (at(&pixels, 25, 12), at(&pixels, 29, 12), at(&pixels, 20, 12)),
      ([255, 0, 0, 255], BACK, BACK)
    );
    let shine = Look { shine: Some([168.0, 85.0, 247.0, 0.75]), ..Look::PLAIN };
    let glowing = Drawn { look: shine, ..plain };
    let pixels = gpu.painted([80, 50], |target| screen.draw(&view, &[glowing], target));
    let near = at(&pixels, 22, 12);
    assert_eq!(at(&pixels, 25, 12), [255, 0, 0, 255]);
    assert!(near != BACK && near[2] > BACK[2] && near[0] > BACK[0], "{near:?}");
    assert_eq!(at(&pixels, 79, 49), BACK);
  }
}
