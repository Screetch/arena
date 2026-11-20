//! The figure and the effects on the screen: the box and the look of each, and the measure of its
//! clip into characters.

use std::f64::consts::PI;

use arena_sim::clip::{Book, Clip, Effect};
use libm::{cos, sin};

use crate::cells::{Glyph, Look, Patch};
use crate::sample::{Matrix, mul, sample};
use crate::shapes::shapes;

/// The look of the figure.
pub const FIGURE: Look = Look { cell: 4.5, breath: 1.0, fill: 0.6, fill_shade: 0.5, ..Look::PLAIN };

const SPELL: Look = Look { cell: 6.0, density: 1.6, ..Look::PLAIN };
const BOMB: Look =
  Look { cell: 3.0, density: 3.0, opacity: 2.5, fill: 0.6, fill_shade: 0.5, ..Look::PLAIN };
const WEAPON: Look = Look { cell: 4.5, ..BOMB };
const SPECIAL: Look = Look { cell: 6.0, ..Look::PLAIN };
const IMPACT: Look =
  Look { cell: 4.5, density: 1.6, opacity: 1.5, fill: 0.6, fill_shade: 0.5, ..Look::PLAIN };
const SMOKE: Look = Look {
  cell: 8.0,
  halo: 1.0,
  halo_colour: [168.0, 85.0, 247.0],
  tint: 0.25,
  shine: Some([168.0, 85.0, 247.0, 0.75]),
  ..Look::PLAIN
};

/// The box of the figure, about its feet: `[left, top, right, bottom]`.
const FIGURE_BOX: [f64; 4] = [-120.0, -170.0, 120.0, 120.0];

/// The shape whose box gives the box of the smoke of a smoke bomb.
const SMOKE_SHAPE: u16 = 249;

/// The characters of a clip on a patch of the screen, in the look that draws them.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
  /// The patch of the cells.
  pub patch: Patch,
  /// The characters of the cells, in the patch.
  pub glyphs: Vec<Glyph>,
  /// The look of the characters.
  pub look: Look,
}

/// The characters of `figure`, whose units `matrix` takes to the pixels of a screen of `screen`
/// pixels, at `time` seconds.
pub fn figure(figure: &Clip, matrix: Matrix, screen: [f64; 2], time: f64) -> Option<Drawn> {
  figure.visible.then_some(())?;
  drawn(figure, matrix, false, FIGURE_BOX, FIGURE, screen, time)
}

/// The characters of `effect`, whose clip `clip` stands at its place on the map, whose units
/// `matrix` takes to the pixels of a screen of `screen` pixels, at `time` seconds. A smoke bomb
/// that burst shows its smoke, in a box about the clip on the map.
#[expect(clippy::float_cmp, reason = "a clip with no scale holds exactly 1")]
pub fn effect(
  effect: Effect,
  clip: &Clip,
  matrix: Matrix,
  screen: [f64; 2],
  time: f64,
) -> Option<Drawn> {
  if !clip.visible || clip.alpha <= 0.0 {
    return None;
  }
  if effect == Effect::SmokeBomb && clip.frame() != 1 {
    let [x, y, w, h] = shapes(Book::Effects)
      .shapes
      .get(&SMOKE_SHAPE)
      .and_then(|shape| shape.bounds)
      .unwrap_or([0.0; 4]);
    let (side, up, down) = ((-x).max(x + w) * 1.1, -y * 1.1, (y + h) * 1.1);
    let bounds = [clip.x - side, clip.y - up, clip.x + side, clip.y + down];
    return drawn(clip, matrix, false, bounds, SMOKE, screen, time);
  }
  let mut inside = mul(matrix, [1.0, 0.0, 0.0, 1.0, clip.x, clip.y]);
  if clip.rotation != 0.0 {
    let turn = clip.rotation * PI / 180.0;
    let (c, s) = (cos(turn), sin(turn));
    inside = mul(inside, [c, s, -s, c, 0.0, 0.0]);
  }
  if clip.scale_x != 1.0 || clip.scale_y != 1.0 {
    inside = mul(inside, [clip.scale_x, 0.0, 0.0, clip.scale_y, 0.0, 0.0]);
  }
  let (bounds, look) = framing(effect);
  drawn(clip, inside, true, bounds, look, screen, time)
}

/// The box of an effect about its origin, `[left, top, right, bottom]`, and its look.
fn framing(effect: Effect) -> ([f64; 4], Look) {
  match effect {
    Effect::Lightning => ([-42.0, -165.0, 46.0, 50.0], SPELL),
    Effect::Fireball => ([-55.0, -50.0, 55.0, 95.0], SPELL),
    Effect::Shockwave => ([-130.0, -130.0, 130.0, 130.0], SPELL),
    Effect::Arrow => ([-19.0, -40.0, 19.0, 32.0], WEAPON),
    Effect::Slash => ([-17.0, -65.0, 17.0, 66.0], WEAPON),
    Effect::Sweep => ([-83.0, -77.0, 84.0, 54.0], WEAPON),
    Effect::Spear => ([-13.0, -15.0, 13.0, 87.0], WEAPON),
    Effect::SmokeBomb => ([-9.0, -9.0, 9.0, 9.0], BOMB),
    Effect::ShieldBash => ([-64.0, -18.0, 66.0, 46.0], WEAPON),
    Effect::AirJump => ([-102.0, -79.0, 103.0, 59.0], SPECIAL),
    Effect::WarpTrail => ([-16.0, -18.0, 21.0, 113.0], SPECIAL),
    Effect::Thrust => ([-52.0, -95.0, 52.0, 90.0], SPECIAL),
    Effect::Impact => ([-75.0, -169.0, 78.0, 56.0], IMPACT),
    Effect::Parry => ([-85.0, -20.0, 83.0, 15.0], IMPACT),
    Effect::Splash | Effect::Embers => ([-40.0, -40.0, 40.0, 40.0], IMPACT),
  }
}

fn drawn(
  clip: &Clip,
  matrix: Matrix,
  own: bool,
  bounds: [f64; 4],
  look: Look,
  screen: [f64; 2],
  time: f64,
) -> Option<Drawn> {
  let patch = look.patch(matrix, bounds, screen)?;
  let cells = sample(clip, matrix, own, &patch);
  let origin = [patch.x0 / f64::from(patch.width), patch.y0 / f64::from(patch.height)];
  let glyphs = look.glyphs(&cells, patch.columns, origin, time);
  Some(Drawn { patch, glyphs, look })
}

#[cfg(test)]
mod tests {
  use arena_sim::clip::{At, Clip, Effect};

  use std::fmt::Debug;
  use std::str::FromStr;

  use super::{Drawn, effect, figure};

  const SCREEN: [f64; 2] = [2000.0, 2000.0];

  /// `value` to thousandths, as a browser writes it: a half rounds up.
  #[expect(clippy::float_cmp, reason = "a half lies exactly between two thousandths")]
  fn thousandths(value: f64) -> f64 {
    let doubled = value * 2000.0;
    if doubled.fract() == 0.0 && doubled.rem_euclid(2.0) == 1.0 {
      return (doubled + 1.0) / 2000.0;
    }
    format!("{value:.3}").parse().expect("a number")
  }

  fn number(word: &str) -> f64 {
    word.parse().expect("a number")
  }

  fn field<T: FromStr<Err: Debug>>(words: &[&str], at: usize) -> T {
    words[at].parse().expect("a field of the case")
  }

  /// Checks `drawn` against the lines of a case: its grid, then one line for each character.
  fn check(case: &str, drawn: Option<&Drawn>, lines: &[&str]) {
    let Some(drawn) = drawn else {
      assert_eq!(lines, ["grid none"], "{case}");
      return;
    };
    let patch = drawn.patch;
    let grid = [
      patch.x0,
      patch.y0,
      f64::from(patch.columns),
      f64::from(patch.rows),
      f64::from(patch.width),
      f64::from(patch.height),
    ];
    let want: Vec<f64> = lines[0].split(' ').skip(1).map(number).collect();
    assert_eq!(grid.to_vec(), want, "{case}");
    assert_eq!(drawn.glyphs.len(), lines.len() - 1, "{case}");
    for (glyph, line) in drawn.glyphs.iter().zip(&lines[1..]) {
      let ink = glyph.ink.map(f64::from);
      let mut got = vec![
        f64::from(glyph.column),
        f64::from(glyph.row),
        f64::from(glyph.level),
        ink[0],
        ink[1],
        ink[2],
        thousandths(glyph.opacity),
      ];
      if glyph.fill_opacity > 0.0 {
        let fill = glyph.fill.map(f64::from);
        got.extend([fill[0], fill[1], fill[2], thousandths(glyph.fill_opacity)]);
      }
      let want: Vec<f64> = line.split(' ').map(number).collect();
      assert_eq!(got, want, "{case}");
    }
  }

  /// The cases of a file: each starts with a line that `head` names, then its grid and its cells.
  fn cases<'a>(text: &'a str, head: &str) -> Vec<(&'a str, Vec<&'a str>)> {
    let mut cases: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in text.lines() {
      if line.starts_with(head) {
        cases.push((line, Vec::new()));
      } else {
        cases.last_mut().expect("a case starts its file").1.push(line);
      }
    }
    cases
  }

  fn at(word: &str) -> At<'_> {
    word.parse().map_or(At::Label(word), At::Frame)
  }

  #[test]
  fn the_figure_shows_the_characters_of_each_pose() {
    let text = include_str!("sprites/poses.txt");
    for (case, lines) in cases(text, "pose ") {
      let words: Vec<&str> = case.split(' ').collect();
      let (weapon, steps, shown): (u8, usize, u16) =
        (field(&words, 2), field(&words, 4), field(&words, 8));
      let [tilt, arm, k, time] = [6, 7, 9, 10].map(|at| number(words[at]));
      let mut clip = Clip::figure();
      clip.wear(field(&words, 1), weapon);
      let hand = clip.named_mut("weapon").expect("the figure holds its weapon");
      if let Some(chosen) = hand.named_mut(&format!("w{weapon}"))
        && shown > 0
      {
        chosen.stop_at(At::Frame(shown));
      }
      let spr = clip.named_mut("spr").expect("the figure holds its body");
      match at(words[3]) {
        At::Label(label) => spr.play_from(At::Label(label)),
        At::Frame(frame) => spr.stop_at(At::Frame(frame)),
      }
      for _ in 0..steps {
        clip.advance();
      }
      let spr = clip.named_mut("spr").expect("the figure holds its body");
      spr.scale_x = if words[5] == "1" { -1.0 } else { 1.0 };
      let (flip, sitting) = (spr.scale_x, spr.label() == Some("sit"));
      clip.rotation = tilt;
      let hand = clip.named_mut("weapon").expect("the figure holds its weapon");
      (hand.rotation, hand.scale_x, hand.y) = (arm, flip, if sitting { -30.0 } else { -57.0 });
      let drawn = figure(&clip, [k, 0.0, 0.0, k, 1000.0, 1000.0], SCREEN, time);
      check(case, drawn.as_ref(), &lines);
    }
  }

  #[test]
  fn a_hidden_figure_shows_nothing() {
    let mut clip = Clip::figure();
    clip.visible = false;
    assert_eq!(figure(&clip, [1.0, 0.0, 0.0, 1.0, 1000.0, 1000.0], SCREEN, 0.0), None);
  }

  #[test]
  fn each_effect_shows_the_characters_of_each_frame() {
    let text = include_str!("sprites/effects.txt");
    for (case, lines) in cases(text, "effect ") {
      let words: Vec<&str> = case.split(' ').collect();
      let kind =
        Effect::ALL.into_iter().find(|kind| format!("{kind:?}") == words[1]).expect("an effect");
      let (steps, charge): (usize, u16) = (field(&words, 3), field(&words, 4));
      let [rotation, alpha, sx, sy, k, time, x, y] =
        [5, 7, 8, 9, 10, 11, 12, 13].map(|at| number(words[at]));
      let mut clip = kind.clip();
      clip.stop_at(at(words[2]));
      if let Some(flame) = clip.named_mut("spr")
        && charge > 0
      {
        flame.stop_at(At::Frame(charge));
      }
      for _ in 0..steps {
        clip.advance();
      }
      (clip.x, clip.y, clip.rotation) = (x, y, rotation);
      (clip.scale_x, clip.scale_y, clip.alpha) =
        (if words[6] == "1" { -sx } else { sx }, sy, alpha);
      let drawn = effect(kind, &clip, [k, 0.0, 0.0, k, 1000.0, 1000.0], SCREEN, time);
      check(case, drawn.as_ref(), &lines);
    }
  }
}
