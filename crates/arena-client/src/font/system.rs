//! The glyphs of a system, drawn from the file of a font of the system.

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use fontdb::{Database, Family, Query, Weight};

use super::{Glyphs, NAMED, em};

/// A bold monospace font of the system: Menlo, else Consolas, else the default monospace font of
/// the system. When the system names a default that it does not hold, the font is the monospace
/// face of the system whose weight is nearest to bold.
pub struct SystemFont {
  name: String,
  font: FontVec,
}

/// Why no font of the system can draw the glyphs.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// The system holds no monospace font.
  #[error("the system holds no monospace font")]
  NoFont,
  /// The file of the font cannot be read.
  #[error("the file of the font {0} cannot be read")]
  Unread(String),
  /// The file of the font holds no font that can be read.
  #[error("the font {name} is not valid")]
  Invalid {
    /// The name of the font.
    name: String,
    /// The fault of the font.
    source: ab_glyph::InvalidFont,
  },
}

impl SystemFont {
  /// Finds the font in the fonts of the system, and reads it.
  pub fn find() -> Result<Self, Error> {
    let mut fonts = Database::new();
    fonts.load_system_fonts();
    Self::among(&fonts)
  }

  /// The PostScript name of the font, as `Menlo-Bold`.
  pub fn name(&self) -> &str {
    &self.name
  }

  fn among(fonts: &Database) -> Result<Self, Error> {
    let families = [Family::Name(NAMED[0]), Family::Name(NAMED[1]), Family::Monospace];
    let query = Query { families: &families, weight: Weight::BOLD, ..Query::default() };
    let face = fonts
      .query(&query)
      .and_then(|id| fonts.face(id))
      .or_else(|| {
        let mono = fonts.faces().filter(|face| face.monospaced);
        mono.min_by_key(|face| face.weight.0.abs_diff(Weight::BOLD.0))
      })
      .ok_or(Error::NoFont)?;
    let name = face.post_script_name.clone();
    match fonts
      .with_face_data(face.id, |data, index| FontVec::try_from_vec_and_index(data.to_vec(), index))
    {
      Some(Ok(font)) => Ok(Self { name, font }),
      Some(Err(source)) => Err(Error::Invalid { name, source }),
      None => Err(Error::Unread(name)),
    }
  }
}

impl Glyphs for SystemFont {
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "a side of a cell and each pixel in it are far under 2^24, and a coverage lies from 0 \
              to 1"
  )]
  fn draw(&self, character: char, size: [u32; 2]) -> Vec<u8> {
    let [width, height] = size.map(|side| side as f32);
    let units = self.font.units_per_em().unwrap_or(1.0);
    let scale = PxScale::from(em(size) * self.font.height_unscaled() / units);
    let font = self.font.as_scaled(scale);
    let mut glyph = font.scaled_glyph(character);
    glyph.position = point(
      width / 2.0 - font.h_advance(glyph.id) / 2.0,
      height / 2.0 + f32::midpoint(font.ascent(), font.descent()),
    );
    let mut mask = vec![0; (size[0] * size[1]) as usize];
    if let Some(outline) = font.outline_glyph(glyph) {
      let bounds = outline.px_bounds();
      outline.draw(|x, y, coverage| {
        let (x, y) = (bounds.min.x + x as f32, bounds.min.y + y as f32);
        if (0.0..width).contains(&x) && (0.0..height).contains(&y) {
          mask[(y as u32 * size[0] + x as u32) as usize] = (coverage * 255.0).round() as u8;
        }
      });
    }
    mask
  }
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;
  use std::sync::Arc;

  use fontdb::{Database, FaceInfo, ID, Language, Source, Stretch, Style, Weight};

  use super::{Error, SystemFont};
  use crate::cells::RAMP;
  use crate::font::Glyphs;

  fn holding(family: &str, monospaced: bool, source: Source) -> Database {
    let mut fonts = Database::new();
    fonts.push_face_info(FaceInfo {
      id: ID::dummy(),
      source,
      index: 0,
      families: vec![(family.to_owned(), Language::English_UnitedStates)],
      post_script_name: format!("{}-Bold", family.replace(' ', "")),
      style: Style::Normal,
      weight: Weight::BOLD,
      stretch: Stretch::Normal,
      monospaced,
    });
    fonts
  }

  fn text() -> Source {
    Source::Binary(Arc::new(b"not a font".to_vec()))
  }

  #[test]
  fn the_font_of_the_system_draws_each_character_of_the_ramp_but_the_space() {
    let font = SystemFont::find().expect("the system holds a monospace font");
    assert!(!font.name().is_empty());
    let ink = |character: char| {
      let mask = font.draw(character, [16, 16]);
      assert_eq!(mask.len(), 256, "{character}");
      mask.iter().map(|&coverage| u32::from(coverage)).sum::<u32>()
    };
    for (level, &character) in RAMP.iter().enumerate() {
      assert_eq!(ink(character) == 0, level == 0, "{character}");
    }
    assert!(ink('.') < ink('@'));
    let narrow = font.draw('@', [3, 4]);
    assert_eq!(narrow.len(), 12);
    assert!(narrow.iter().any(|&coverage| coverage > 0));
  }

  fn fault(fonts: &Database) -> Option<String> {
    SystemFont::among(fonts).err().as_ref().map(Error::to_string)
  }

  #[test]
  fn a_system_with_no_monospace_font_gives_no_font() {
    let none = Some("the system holds no monospace font");
    assert_eq!(fault(&Database::new()).as_deref(), none);
    assert_eq!(fault(&holding("Plain Sans", false, text())).as_deref(), none);
  }

  #[test]
  fn a_monospace_face_stands_for_a_default_that_the_system_does_not_hold() {
    let other = holding("Plain Mono", true, text());
    assert_eq!(fault(&other).as_deref(), Some("the font PlainMono-Bold is not valid"));
  }

  #[test]
  fn a_font_whose_file_is_gone_or_not_a_font_gives_its_name_in_the_error() {
    let gone = holding("Menlo", true, Source::File(PathBuf::from("/no/such/folder/menlo.ttc")));
    assert_eq!(fault(&gone).as_deref(), Some("the file of the font Menlo-Bold cannot be read"));
    let text = holding("Menlo", true, text());
    assert_eq!(fault(&text).as_deref(), Some("the font Menlo-Bold is not valid"));
  }
}
