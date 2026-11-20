//! The shapes of the books of clips, which the client draws: the paths of each shape, the morph
//! shapes, and the boxes that clip the shapes of the effects.

use std::collections::HashMap;
use std::sync::LazyLock;

use arena_sim::clip::Book;
use arena_sim::words::Words;

/// A shape: its paths, painted in order, and the box out of which it shows nothing.
#[derive(Debug, Default)]
pub(crate) struct Shape {
  pub(crate) paths: Vec<Path>,
  /// The box `[x, y, width, height]`, in the units of the shape.
  pub(crate) bounds: Option<[f64; 4]>,
}

/// A path: its sub-paths, its fill by the even-odd rule, and its stroke.
#[derive(Debug, Default)]
pub(crate) struct Path {
  pub(crate) subs: Vec<Sub>,
  pub(crate) fill: Option<Fill>,
  /// The red, green, blue and opacity of the stroke, from 0 to 1.
  pub(crate) stroke: Option<[f64; 4]>,
  pub(crate) width: f64,
}

/// The points `x, y, x, y...` of a sub-path. A fill closes each sub-path, and a stroke only a
/// closed one.
#[derive(Debug, Default)]
pub(crate) struct Sub {
  pub(crate) points: Vec<f64>,
  pub(crate) closed: bool,
}

/// The fill of a path.
#[derive(Debug)]
pub(crate) enum Fill {
  /// A colour: its red, green, blue and opacity, from 0 to 1.
  Solid([f64; 4]),
  /// A gradient.
  Gradient(Gradient),
}

/// A gradient: along x from -819.2 to 819.2, or round from 0 to a radius of 819.2, in its own
/// space, which the inverse of its matrix gives from the space of its shape.
#[derive(Debug)]
pub(crate) struct Gradient {
  pub(crate) radial: bool,
  pub(crate) inverse: [f64; 6],
  /// Its stops: the place, from 0 to 1, then the red, green, blue and opacity.
  pub(crate) stops: Vec<[f64; 5]>,
}

/// A morph shape: its outline, which pairs each record of its start shape with its end shape, so
/// that a ratio mixes them, and its fills. The morph shapes of the books have no line.
#[derive(Debug, Default)]
pub(crate) struct Morph {
  pub(crate) fills: Vec<MorphFill>,
  pub(crate) records: Vec<Record>,
}

/// A gradient fill of a morph shape, with its matrix and its stops at the start and at the end:
/// each stop gives its place from 0 to 255, then its red, green and blue from 0 to 255 and its
/// opacity from 0 to 1, at the start then at the end.
#[derive(Debug)]
pub(crate) struct MorphFill {
  pub(crate) radial: bool,
  pub(crate) start: [f64; 6],
  pub(crate) end: [f64; 6],
  pub(crate) stops: Vec<[f64; 10]>,
}

/// A record of the outline of a morph shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Record {
  /// A move of the pen of the start and of the end, where none keeps its place, and the fills
  /// that follow, where -1 keeps the last one and 0 is none.
  Style { start: Option<(f64, f64)>, end: Option<(f64, f64)>, fills: [f64; 2] },
  /// An edge of the start and its edge at the end.
  Edge { start: Edge, end: Edge },
}

/// An edge of the outline of a morph shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Edge {
  /// A straight edge by (`dx`, `dy`).
  Line { dx: f64, dy: f64 },
  /// A curved edge by the control (`cx`, `cy`) and the anchor (`ax`, `ay`), each from the last.
  Curve { cx: f64, cy: f64, ax: f64, ay: f64 },
}

/// The shapes and the morph shapes of a book.
#[derive(Debug, Default)]
pub(crate) struct Shapes {
  pub(crate) shapes: HashMap<u16, Shape>,
  pub(crate) morphs: HashMap<u16, Morph>,
}

static FIGURE: LazyLock<Shapes> = LazyLock::new(|| read(include_str!("../data/figure.txt")));
static EFFECTS: LazyLock<Shapes> = LazyLock::new(|| read(include_str!("../data/effects.txt")));

/// The shapes of `book`.
pub(crate) fn shapes(book: Book) -> &'static Shapes {
  match book {
    Book::Figure => &FIGURE,
    Book::Effects => &EFFECTS,
  }
}

/// The shapes of a book, from its lines of words. A line `shape` starts a shape, with a line
/// `box` for its box, a line `path` for each path and a line `sub` for each sub-path. A line
/// `morph` starts a morph shape, with a line `fill` for each fill, and a line `style` or `edge` for
/// each record of its outline.
#[expect(
  clippy::expect_used,
  reason = "the books are data of the crate, which its tests read whole"
)]
fn read(text: &str) -> Shapes {
  let mut book = Shapes::default();
  let (mut shape, mut morph) = (None, None);
  for line in text.lines() {
    let mut words = Words::new(line);
    let kind = words.word();
    let held = |id: Option<u16>| id.expect("a line shape or morph comes first");
    match kind {
      "shape" => (shape, morph) = (Some(words.number()), None),
      "morph" => (shape, morph) = (None, Some(words.number())),
      "box" => book.shapes.entry(held(shape)).or_default().bounds = Some(words.numbers()),
      "path" => book.shapes.entry(held(shape)).or_default().paths.push(path(&mut words)),
      "sub" => {
        let paths = &mut book.shapes.entry(held(shape)).or_default().paths;
        let path = paths.last_mut().expect("a line path starts its sub-paths");
        path.subs.push(Sub { closed: words.word() == "1", points: words.map(number).collect() });
      }
      "fill" => book.morphs.entry(held(morph)).or_default().fills.push(morph_fill(&mut words)),
      "style" => {
        let (start, end) = (point(&mut words), point(&mut words));
        let style = Record::Style { start, end, fills: words.numbers() };
        book.morphs.entry(held(morph)).or_default().records.push(style);
      }
      _ => {
        let (start, end) = (edge(&mut words), edge(&mut words));
        book.morphs.entry(held(morph)).or_default().records.push(Record::Edge { start, end });
      }
    }
  }
  book
}

#[expect(
  clippy::expect_used,
  reason = "the books are data of the crate, which its tests read whole"
)]
fn number(word: &str) -> f64 {
  word.parse().expect("the data holds a number here")
}

/// A path: its width, its stroke or `-`, and its fill or `-`.
fn path(words: &mut Words<'_>) -> Path {
  let width = words.number();
  let stroke = match words.word() {
    "-" => None,
    red => Some([number(red), words.number(), words.number(), words.number()]),
  };
  let fill = match words.word() {
    "-" => None,
    "solid" => Some(Fill::Solid(words.numbers())),
    kind => {
      let inverse = words.numbers();
      let count: usize = words.number();
      let stops = (0..count).map(|_| words.numbers()).collect();
      Some(Fill::Gradient(Gradient { radial: kind == "radial", inverse, stops }))
    }
  };
  Path { subs: Vec::new(), fill, stroke, width }
}

fn morph_fill(words: &mut Words<'_>) -> MorphFill {
  let radial = words.word() == "radial";
  let (start, end) = (words.numbers(), words.numbers());
  let count: usize = words.number();
  MorphFill { radial, start, end, stops: (0..count).map(|_| words.numbers()).collect() }
}

/// A point, or none for `-`.
fn point(words: &mut Words<'_>) -> Option<(f64, f64)> {
  match words.word() {
    "-" => None,
    x => Some((number(x), words.number())),
  }
}

/// An edge: `l` with its move, or `c` with its control and its anchor.
fn edge(words: &mut Words<'_>) -> Edge {
  if words.word() == "l" {
    let [dx, dy] = words.numbers();
    Edge::Line { dx, dy }
  } else {
    let [cx, cy, ax, ay] = words.numbers();
    Edge::Curve { cx, cy, ax, ay }
  }
}
