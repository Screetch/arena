//! The glyphs of the characters of the cells, drawn from the fonts of the system: from the file of
//! a font on a system, and through the 2D text of a canvas in a browser.

#[cfg(target_arch = "wasm32")]
mod canvas;
#[cfg(not(target_arch = "wasm32"))]
mod system;

#[cfg(target_arch = "wasm32")]
pub use canvas::{CanvasFont, Error};
#[cfg(not(target_arch = "wasm32"))]
pub use system::{Error, SystemFont};

/// The families of the font, in the order of choice. When the system holds none of them, its
/// default monospace font draws the glyphs.
const NAMED: [&str; 2] = ["Menlo", "Consolas"];

/// A source of the glyphs of the characters of [`crate::cells::RAMP`].
///
/// A browser gives its fonts in another way than a system, so each platform has a source of its
/// own, and the drawing reads only this interface.
pub trait Glyphs {
  /// Draws `character` in bold, centred in a cell of `size` pixels, its width and its height, at
  /// the least of 1.15 times its height and 1.9 times its width, as one byte of coverage for each
  /// pixel, row by row. The cell clips the glyph.
  fn draw(&self, character: char, size: [u32; 2]) -> Vec<u8>;
}

/// The size of the em of the font in a cell of `size` pixels, its width and its height: the least
/// of 1.15 times its height and 1.9 times its width, in whole pixels, at least 4.
#[expect(
  clippy::cast_precision_loss,
  reason = "a side of a cell is far under 2^24, which an f32 holds whole"
)]
fn em(size: [u32; 2]) -> f32 {
  (1.15 * size[1] as f32).min(1.9 * size[0] as f32).round().max(4.0)
}
