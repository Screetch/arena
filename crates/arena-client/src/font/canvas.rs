//! The glyphs of a browser, which draws them with the 2D text of a canvas from the fonts of the
//! system.

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use super::{Glyphs, NAMED, em};

/// A canvas out of the page, which draws each glyph in the first font of the list that the
/// browser finds on the system, else in its default monospace font.
pub struct CanvasFont {
  canvas: HtmlCanvasElement,
  context: CanvasRenderingContext2d,
}

/// Why the browser cannot draw the glyphs.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// The page holds no document, or the browser gives no 2D context to a canvas.
  #[error("the browser gives no canvas to draw text on")]
  NoCanvas,
}

impl CanvasFont {
  /// Makes the canvas of the glyphs.
  pub fn new() -> Result<Self, Error> {
    let canvas = web_sys::window()
      .and_then(|window| window.document())
      .and_then(|document| document.create_element("canvas").ok())
      .and_then(|element| element.dyn_into::<HtmlCanvasElement>().ok())
      .ok_or(Error::NoCanvas)?;
    let context = canvas
      .get_context("2d")
      .ok()
      .flatten()
      .and_then(|context| context.dyn_into::<CanvasRenderingContext2d>().ok())
      .ok_or(Error::NoCanvas)?;
    Ok(Self { canvas, context })
  }
}

impl Glyphs for CanvasFont {
  #[expect(
    clippy::expect_used,
    reason = "a canvas of at least 4 pixels on a side that holds only the text of the client \
              measures text, draws it and gives its pixels"
  )]
  fn draw(&self, character: char, size: [u32; 2]) -> Vec<u8> {
    self.canvas.set_width(size[0]);
    self.canvas.set_height(size[1]);
    self.context.set_font(&format!("bold {}px {}, monospace", em(size), NAMED.join(", ")));
    self.context.set_fill_style_str("white");
    let text = character.to_string();
    let metrics = self.context.measure_text(&text).expect("the canvas measures text");
    let middle = f64::from(size[1]) / 2.0;
    let baseline =
      middle + (metrics.font_bounding_box_ascent() - metrics.font_bounding_box_descent()) / 2.0;
    self
      .context
      .fill_text(&text, f64::from(size[0]) / 2.0 - metrics.width() / 2.0, baseline)
      .expect("the canvas draws text");
    let [width, height] = size.map(f64::from);
    let pixels =
      self.context.get_image_data(0.0, 0.0, width, height).expect("the canvas gives its pixels");
    pixels.data().0.into_iter().skip(3).step_by(4).collect()
  }
}
