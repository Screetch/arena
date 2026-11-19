//! The glyphs of a browser, drawn in a headless browser, which `sh script/web.sh test` runs. The
//! other tests of the client build only for a system, so these stand apart from their module.
#![cfg(target_arch = "wasm32")]
#![expect(clippy::expect_used, reason = "a test fails at once when the page gives no canvas")]

use arena_client::cells::RAMP;
use arena_client::font::{CanvasFont, Glyphs};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn the_canvas_of_the_browser_draws_each_character_of_the_ramp_but_the_space() {
  let font = CanvasFont::new().expect("the page gives a canvas with a 2D context");
  let ink = |character: char, side: u32| {
    let mask = font.draw(character, side);
    assert_eq!(mask.len(), (side * side) as usize, "{character}");
    mask.iter().map(|&coverage| u32::from(coverage)).sum::<u32>()
  };
  for (level, &character) in RAMP.iter().enumerate() {
    assert_eq!(ink(character, 16) == 0, level == 0, "{character}");
  }
  assert!(ink('.', 16) < ink('@', 16));
  assert!(ink('@', 8) > 0);
}

#[wasm_bindgen_test]
fn a_glyph_leaves_no_ink_on_the_next_one() {
  let font = CanvasFont::new().expect("the page gives a canvas with a 2D context");
  assert_ne!(font.draw('@', 16), vec![0; 256]);
  assert_eq!(font.draw(' ', 16), vec![0; 256]);
}
