//! The measure of a clip on the cells of a patch of the screen: its shapes, painted over each
//! other in the order of its frames, with the shadows and the blurs of their filters, at 4 by 4
//! points in each cell. Each point keeps its colour in f32, as the cells keep it.

use std::f64::consts::PI;
use std::ops::Range;

use arena_sim::clip::Clip;
use libm::{cos, exp, sin};

use crate::cells::Patch;
use crate::shapes::{Edge, Fill, Gradient, Morph, MorphFill, Path, Record, Shape, Sub, shapes};

/// A matrix `[a, b, c, d, e, f]`, which takes (x, y) to (a x + c y + e, b x + d y + f).
pub(crate) type Matrix = [f64; 6];

/// The product of `m` and `n`: `n` first, then `m`.
pub(crate) fn mul(m: Matrix, n: Matrix) -> Matrix {
  [
    m[0] * n[0] + m[2] * n[1],
    m[1] * n[0] + m[3] * n[1],
    m[0] * n[2] + m[2] * n[3],
    m[1] * n[2] + m[3] * n[3],
    m[0] * n[4] + m[2] * n[5] + m[4],
    m[1] * n[4] + m[3] * n[5] + m[5],
  ]
}

fn invert(m: Matrix) -> Option<Matrix> {
  let det = m[0] * m[3] - m[1] * m[2];
  (det != 0.0).then(|| {
    [
      m[3] / det,
      -m[1] / det,
      -m[2] / det,
      m[0] / det,
      (m[2] * m[5] - m[3] * m[4]) / det,
      (m[1] * m[4] - m[0] * m[5]) / det,
    ]
  })
}

/// The matrix of a clip at (`x`, `y`), turned by `rotation` degrees and scaled by `sx` and `sy`.
fn place(x: f64, y: f64, rotation: f64, sx: f64, sy: f64) -> Matrix {
  let turn = rotation * PI / 180.0;
  let (cosine, sine) = (cos(turn), sin(turn));
  [cosine * sx, sine * sx, -sine * sy, cosine * sy, x, y]
}

/// A step of the paint of a clip.
enum Item {
  Shape {
    shape: &'static Shape,
    matrix: Matrix,
    alpha: f64,
    blur: f64,
  },
  Morph {
    morph: &'static Morph,
    ratio: f64,
    matrix: Matrix,
    alpha: f64,
    blur: f64,
  },
  /// Starts a layer, whose shapes cast a shadow of this colour, from 0 to 1, and this blur.
  Open {
    colour: [f64; 4],
    blur: f64,
  },
  /// Ends the last layer, and casts its shadow under it.
  Close,
}

/// The state of a clip that its children take in a walk of its tree.
#[derive(Clone, Copy)]
struct Walk {
  matrix: Matrix,
  alpha: f64,
  tint: f64,
  blur: f64,
  ratio: f64,
}

/// The steps of the paint of `clip`, through `matrix`. A clip that stands `own` is already placed
/// by `matrix`, else its own fields place it.
#[expect(clippy::float_cmp, reason = "a clip with no scale holds exactly 1")]
fn walk(clip: &Clip, own: bool, at: Walk, items: &mut Vec<Item>) {
  let mut matrix = at.matrix;
  if !own
    && (clip.x != 0.0
      || clip.y != 0.0
      || clip.rotation != 0.0
      || clip.scale_x != 1.0
      || clip.scale_y != 1.0)
  {
    matrix = mul(matrix, place(clip.x, clip.y, clip.rotation, clip.scale_x, clip.scale_y));
  }
  let alpha = at.alpha * clip.alpha;
  let book = shapes(clip.book());
  if let Some(shape) = book.shapes.get(&clip.id()) {
    items.push(Item::Shape { shape, matrix, alpha, blur: at.blur });
    return;
  }
  if let Some(morph) = book.morphs.get(&clip.id()) {
    items.push(Item::Morph { morph, ratio: at.ratio / 65535.0, matrix, alpha, blur: at.blur });
    return;
  }
  for part in clip.parts().iter().filter(|part| part.clip.visible) {
    let place = part.place;
    let tint = at.tint * place.alpha;
    let shadow = place.shadow.as_ref().map(|shadow| {
      let [red, green, blue, opacity] = shadow.colour;
      let colour = [red / 255.0, green / 255.0, blue / 255.0, (opacity * shadow.strength).min(1.0)];
      Item::Open { colour, blur: f64::midpoint(shadow.blur[0], shadow.blur[1]) }
    });
    let has_shadow = shadow.is_some();
    items.extend(shadow);
    let blur = place.blur.map_or(at.blur, |[x, y]| tenths((x + y) / 4.0));
    let matrix = if part.clip.free { matrix } else { mul(matrix, place.matrix) };
    let child = Walk { matrix, alpha: alpha * tint, tint, blur, ratio: place.ratio };
    walk(&part.clip, false, child, items);
    if has_shadow {
      items.push(Item::Close);
    }
  }
}

/// `value` rounded to tenths, a half up.
fn tenths(value: f64) -> f64 {
  (value * 10.0 + 0.5).floor() / 10.0
}

/// The means of the cells of `patch` that `clip` covers through `matrix`, row by row: the
/// premultiplied red, green and blue, and the cover. A clip that stands `own` is already placed by
/// `matrix`, else its own fields place it.
pub(crate) fn sample(clip: &Clip, matrix: Matrix, own: bool, patch: &Patch) -> Vec<[f32; 4]> {
  let mut items = Vec::new();
  walk(clip, own, Walk { matrix, alpha: 1.0, tint: 1.0, blur: 0.0, ratio: 0.0 }, &mut items);
  let mut paint = Paint::new(patch);
  let mut layers: Vec<(Vec<f32>, [f64; 4], f64)> = Vec::new();
  for item in &items {
    match *item {
      Item::Open { colour, blur } => {
        let blank = paint.blank();
        let under = std::mem::replace(&mut paint.buffer, blank);
        layers.push((under, colour, blur));
      }
      Item::Close => paint.close(&mut layers),
      Item::Shape { shape, matrix, alpha, blur } => {
        paint.item(&shape.paths, shape.bounds, matrix, alpha, blur);
      }
      Item::Morph { morph, ratio, matrix, alpha, blur } => {
        paint.item(&morphed(morph, ratio), None, matrix, alpha, blur);
      }
    }
  }
  paint.cells()
}

/// The points of a patch: 4 by 4 in each cell, each with its premultiplied colour and its cover.
struct Paint {
  x0: f64,
  y0: f64,
  columns: usize,
  rows: usize,
  sx: f64,
  sy: f64,
  buffer: Vec<f32>,
  cover: Vec<f32>,
}

/// The box and the inverse matrix of a shape that shows nothing out of its box.
type Bounds = Option<([f64; 4], Matrix)>;

impl Paint {
  fn new(patch: &Patch) -> Self {
    let (columns, rows) = (usize::from(patch.columns) * 4, usize::from(patch.rows) * 4);
    Self {
      x0: patch.x0,
      y0: patch.y0,
      columns,
      rows,
      sx: f64::from(patch.width) / 4.0,
      sy: f64::from(patch.height) / 4.0,
      buffer: vec![0.0; columns * rows * 4],
      cover: vec![0.0; columns * rows],
    }
  }

  fn blank(&self) -> Vec<f32> {
    vec![0.0; self.columns * self.rows * 4]
  }

  #[expect(clippy::expect_used, reason = "a walk closes each layer that it opens")]
  fn close(&mut self, layers: &mut Vec<(Vec<f32>, [f64; 4], f64)>) {
    let (under, colour, blur) = layers.pop().expect("a walk closes each layer that it opens");
    let layer = std::mem::replace(&mut self.buffer, under);
    self.shadowed(&layer, colour, blur);
  }

  fn item(
    &mut self,
    paths: &[Path],
    bounds: Option<[f64; 4]>,
    matrix: Matrix,
    alpha: f64,
    blur: f64,
  ) {
    if alpha.is_nan() || alpha <= 0.0 {
      return;
    }
    let bounds = match bounds {
      Some(bounds) => match invert(matrix) {
        Some(inverse) => Some((bounds, inverse)),
        None => return,
      },
      None => None,
    };
    if blur > 0.0 {
      let blank = self.blank();
      let under = std::mem::replace(&mut self.buffer, blank);
      self.paths(paths, bounds, matrix, alpha);
      let layer = std::mem::replace(&mut self.buffer, under);
      self.blurred(&layer, blur);
    } else {
      self.paths(paths, bounds, matrix, alpha);
    }
  }

  fn paths(&mut self, paths: &[Path], bounds: Bounds, matrix: Matrix, alpha: f64) {
    for path in paths {
      if let Some(fill) = &path.fill {
        self.fill(path, fill, bounds, matrix, alpha);
      }
      if let Some(stroke) = path.stroke
        && path.width > 0.0
      {
        self.stroke(path, stroke, bounds, matrix, alpha);
      }
    }
  }

  /// The centre of the point (i, j), in pixels of the screen.
  fn centre(&self, i: usize, j: usize) -> (f64, f64) {
    (self.x0 + (whole(i) + 0.5) * self.sx, self.y0 + (whole(j) + 0.5) * self.sy)
  }

  /// Paints `colour`, from 0 to 1, at the opacity `alpha` over the point `at`, unless the point
  /// lies out of `bounds`.
  fn put(&mut self, at: usize, colour: [f64; 3], alpha: f64, bounds: Bounds) {
    if alpha <= 0.0 {
      return;
    }
    if let Some((bounds, inverse)) = bounds {
      let (px, py) = self.centre(at % self.columns, at / self.columns);
      let lx = inverse[0] * px + inverse[2] * py + inverse[4];
      let ly = inverse[1] * px + inverse[3] * py + inverse[5];
      if lx < bounds[0]
        || ly < bounds[1]
        || lx > bounds[0] + bounds[2]
        || ly > bounds[1] + bounds[3]
      {
        return;
      }
    }
    let (at, keep) = (at * 4, 1.0 - alpha);
    for (k, channel) in colour.into_iter().enumerate() {
      self.buffer[at + k] = single(channel * alpha + f64::from(self.buffer[at + k]) * keep);
    }
    self.buffer[at + 3] = single(alpha + f64::from(self.buffer[at + 3]) * keep);
  }
}

impl Paint {
  /// Fills `path` with `fill`, at the centre of each point inside it.
  fn fill(&mut self, path: &Path, fill: &Fill, bounds: Bounds, matrix: Matrix, alpha: f64) {
    let to_point =
      [1.0 / self.sx, 0.0, 0.0, 1.0 / self.sy, -self.x0 / self.sx - 0.5, -self.y0 / self.sy - 0.5];
    let onto = mul(to_point, matrix);
    let subs: Vec<Vec<f64>> = path
      .subs
      .iter()
      .map(|sub| {
        let mut points = Vec::with_capacity(sub.points.len());
        for pair in sub.points.chunks(2) {
          let (x, y) = (pair[0], pair[1]);
          points.extend([onto[0] * x + onto[2] * y + onto[4], onto[1] * x + onto[3] * y + onto[5]]);
        }
        points
      })
      .collect();
    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
    for y in subs.iter().flat_map(|sub| sub.iter().skip(1).step_by(2)) {
      low = low.min(*y);
      high = high.max(*y);
    }
    let source = match fill {
      Fill::Solid([red, green, blue, opacity]) => Source::Solid([*red, *green, *blue], *opacity),
      Fill::Gradient(gradient) => match invert(matrix) {
        Some(inverse) => Source::Gradient(gradient, inverse),
        None => return,
      },
    };
    let last = (high.ceil() - 1.0).min(whole(self.rows) - 1.0);
    let mut crossed = Vec::new();
    let mut row = low.ceil().max(0.0);
    while row <= last {
      crossings(&subs, row, &mut crossed);
      for &[start, end] in crossed.as_chunks::<2>().0 {
        let from = start.ceil().max(0.0);
        let to = (end.ceil() - 1.0).min(whole(self.columns) - 1.0);
        self.span(row, (from, to), source, alpha, bounds);
      }
      row += 1.0;
    }
  }

  /// Paints `source` over the points of `row` from the column `columns.0` to `columns.1`.
  fn span(
    &mut self,
    row: f64,
    columns: (f64, f64),
    source: Source<'_>,
    alpha: f64,
    bounds: Bounds,
  ) {
    let mut column = columns.0;
    while column <= columns.1 {
      let at = index(row) * self.columns + index(column);
      let (colour, opacity) = match source {
        Source::Solid(colour, opacity) => (colour, opacity),
        Source::Gradient(gradient, inverse) => self.colour_at(gradient, inverse, (column, row)),
      };
      self.put(at, colour, opacity * alpha, bounds);
      column += 1.0;
    }
  }
}

/// The places where the edges of `subs` cross `row`, sorted along it.
fn crossings(subs: &[Vec<f64>], row: f64, list: &mut Vec<f64>) {
  list.clear();
  for sub in subs {
    let len = sub.len();
    for k in (0..len).step_by(2) {
      let (x0, y0, x1, y1) = (sub[k], sub[k + 1], sub[(k + 2) % len], sub[(k + 3) % len]);
      if (y0 <= row && row < y1) || (y1 <= row && row < y0) {
        let x = x0 + (row - y0) * (x1 - x0) / (y1 - y0);
        let at = list.iter().rposition(|&other| other <= x).map_or(0, |at| at + 1);
        list.insert(at, x);
      }
    }
  }
}

/// The colour of a fill at a point.
#[derive(Clone, Copy)]
enum Source<'a> {
  Solid([f64; 3], f64),
  /// A gradient, and the inverse of the matrix of its shape.
  Gradient(&'a Gradient, Matrix),
}

impl Paint {
  /// The colour and the opacity of `gradient` at the point `point`, its column and its row.
  fn colour_at(&self, gradient: &Gradient, inverse: Matrix, point: (f64, f64)) -> ([f64; 3], f64) {
    let (px, py) = (self.x0 + (point.0 + 0.5) * self.sx, self.y0 + (point.1 + 0.5) * self.sy);
    let lx = inverse[0] * px + inverse[2] * py + inverse[4];
    let ly = inverse[1] * px + inverse[3] * py + inverse[5];
    let space = gradient.inverse;
    let gx = space[0] * lx + space[2] * ly + space[4];
    let gy = space[1] * lx + space[3] * ly + space[5];
    let along = if gradient.radial {
      (gx * gx + gy * gy).sqrt() / 819.2
    } else {
      (gx + 819.2) * 1638.4 / (1638.4 * 1638.4)
    };
    let along = along.clamp(0.0, 1.0);
    let stops = &gradient.stops;
    let mut stop = 0;
    while stop + 1 < stops.len() && stops[stop + 1][0] < along {
      stop += 1;
    }
    let (low, high) = (stops[stop], stops[(stop + 1).min(stops.len() - 1)]);
    let part = if stop + 1 >= stops.len() || high[0] <= low[0] {
      f64::from(u8::from(along > low[0]))
    } else {
      ((along - low[0]) / (high[0] - low[0])).clamp(0.0, 1.0)
    };
    let mix = |channel: usize| low[channel] + (high[channel] - low[channel]) * part;
    ([mix(1), mix(2), mix(3)], mix(4))
  }

  /// Strokes `path` with `stroke`: each point within half the width of its line takes the part of
  /// it that the line covers, the most that any segment gives.
  fn stroke(&mut self, path: &Path, stroke: [f64; 4], bounds: Bounds, m: Matrix, alpha: f64) {
    let width = path.width * (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
    let half = width / 2.0;
    let reach = (half + 0.5).max(1.0);
    let mut touched = Vec::new();
    for sub in &path.subs {
      let points = &sub.points;
      let len = points.len();
      let last = if sub.closed { len } else { len.saturating_sub(2) };
      for k in (0..last).step_by(2) {
        let n = (k + 2) % len;
        let a = (
          m[0] * points[k] + m[2] * points[k + 1] + m[4],
          m[1] * points[k] + m[3] * points[k + 1] + m[5],
        );
        let b = (
          m[0] * points[n] + m[2] * points[n + 1] + m[4],
          m[1] * points[n] + m[3] * points[n + 1] + m[5],
        );
        self.segment(a, b, width, reach, &mut touched);
      }
    }
    let [red, green, blue, opacity] = stroke;
    for at in touched {
      let cover = f64::from(self.cover[at]);
      self.put(at, [red, green, blue], opacity * alpha * cover, bounds);
      self.cover[at] = 0.0;
    }
  }

  /// Covers the points near the segment from `start` to `end`, in pixels, of a line of `width`.
  fn segment(
    &mut self,
    start: (f64, f64),
    end: (f64, f64),
    width: f64,
    reach: f64,
    touched: &mut Vec<usize>,
  ) {
    let ((ax, ay), (bx, by)) = (start, end);
    let half = width / 2.0;
    let first =
      |low: f64, origin: f64, step: f64| ((low - reach - origin) / step - 0.5).ceil().max(0.0);
    let last = |high: f64, origin: f64, step: f64, count: usize| {
      ((high + reach - origin) / step - 0.5).floor().min(whole(count) - 1.0)
    };
    let columns =
      (first(ax.min(bx), self.x0, self.sx), last(ax.max(bx), self.x0, self.sx, self.columns));
    let rows = (first(ay.min(by), self.y0, self.sy), last(ay.max(by), self.y0, self.sy, self.rows));
    let (dx, dy) = (bx - ax, by - ay);
    let dd = dx * dx + dy * dy;
    let mut row = rows.0;
    while row <= rows.1 {
      let py = self.y0 + (row + 0.5) * self.sy;
      let mut column = columns.0;
      while column <= columns.1 {
        let px = self.x0 + (column + 0.5) * self.sx;
        let along =
          if dd == 0.0 { 0.0 } else { (((px - ax) * dx + (py - ay) * dy) / dd).clamp(0.0, 1.0) };
        let (ex, ey) = (px - ax - along * dx, py - ay - along * dy);
        let squared = ex * ex + ey * ey;
        if squared < reach * reach {
          let distance = squared.sqrt();
          let cover = if width >= 1.0 {
            (half + 0.5 - distance).clamp(0.0, 1.0)
          } else {
            width * (1.0 - distance).clamp(0.0, 1.0)
          };
          self.touch(index(row) * self.columns + index(column), cover, touched);
        }
        column += 1.0;
      }
      row += 1.0;
    }
  }

  /// Keeps the most cover of a line at the point `at`, and the points that a line touched. A
  /// point within the reach of a line has a cover over 0.
  fn touch(&mut self, at: usize, cover: f64, touched: &mut Vec<usize>) {
    if self.cover[at] == 0.0 {
      touched.push(at);
    }
    if cover > f64::from(self.cover[at]) {
      self.cover[at] = single(cover);
    }
  }

  /// The points that `layer` covers, grown by `rx` and `ry` points and kept in the patch: a range
  /// of columns and a range of rows, both empty when the layer covers nothing.
  fn extent(&self, layer: &[f32], rx: usize, ry: usize) -> (Range<usize>, Range<usize>) {
    let mut found: Option<[usize; 4]> = None;
    for (at, point) in layer.chunks(4).enumerate() {
      if point[3] > 0.0 {
        let (i, j) = (at % self.columns, at / self.columns);
        let [i0, i1, j0, _] = found.unwrap_or([i, i, j, j]);
        found = Some([i0.min(i), i1.max(i), j0, j]);
      }
    }
    found.map_or((0..0, 0..0), |[i0, i1, j0, j1]| {
      let columns = i0.saturating_sub(rx)..(i1 + rx + 1).min(self.columns);
      (columns, j0.saturating_sub(ry)..(j1 + ry + 1).min(self.rows))
    })
  }

  /// Paints `layer` over the points, with the shadow that its cover casts under it: of `colour`,
  /// blurred by `blur` pixels.
  fn shadowed(&mut self, layer: &[f32], colour: [f64; 4], blur: f64) {
    let sigma = (blur / 2.0).max(0.01);
    let (kx, ky) = (kernel(sigma, self.sx), kernel(sigma, self.sy));
    let (columns, rows) = self.extent(layer, kx.len() / 2, ky.len() / 2);
    let width = columns.len();
    let mut across = vec![0.0_f32; width * rows.len()];
    for j in rows.clone() {
      for i in columns.clone() {
        let mut sum = 0.0;
        for (d, weight) in kx.iter().enumerate() {
          let x = (i + d).wrapping_sub(kx.len() / 2);
          if columns.contains(&x) {
            sum += f64::from(layer[(j * self.columns + x) * 4 + 3]) * weight;
          }
        }
        across[(j - rows.start) * width + i - columns.start] = single(sum);
      }
    }
    for j in rows.clone() {
      for i in columns.clone() {
        let mut sum = 0.0;
        for (d, weight) in ky.iter().enumerate() {
          let y = (j + d).wrapping_sub(ky.len() / 2);
          if rows.contains(&y) {
            sum += f64::from(across[(y - rows.start) * width + i - columns.start]) * weight;
          }
        }
        let at = (j * self.columns + i) * 4;
        let (shade, cover) = (sum.min(1.0) * colour[3], f64::from(layer[at + 3]));
        if shade <= 0.0 && cover <= 0.0 {
          continue;
        }
        let under = shade * (1.0 - cover);
        let keep = 1.0 - (cover + under);
        for (c, tone) in colour.iter().take(3).enumerate() {
          let below = f64::from(self.buffer[at + c]);
          self.buffer[at + c] = single(f64::from(layer[at + c]) + tone * under + below * keep);
        }
        self.buffer[at + 3] = single(cover + under + f64::from(self.buffer[at + 3]) * keep);
      }
    }
  }

  /// Paints `layer` over the points, blurred by `sigma` pixels.
  fn blurred(&mut self, layer: &[f32], sigma: f64) {
    let (kx, ky) = (kernel(sigma, self.sx), kernel(sigma, self.sy));
    let (columns, rows) = self.extent(layer, kx.len() / 2, ky.len() / 2);
    let width = columns.len();
    let mut across = vec![0.0_f32; width * rows.len() * 4];
    for j in rows.clone() {
      for i in columns.clone() {
        let o = ((j - rows.start) * width + i - columns.start) * 4;
        for (d, weight) in kx.iter().enumerate() {
          let x = (i + d).wrapping_sub(kx.len() / 2);
          if columns.contains(&x) {
            let at = (j * self.columns + x) * 4;
            for c in 0..4 {
              across[o + c] = single(f64::from(across[o + c]) + f64::from(layer[at + c]) * weight);
            }
          }
        }
      }
    }
    for j in rows.clone() {
      for i in columns.clone() {
        let mut sum = [0.0; 4];
        for (d, weight) in ky.iter().enumerate() {
          let y = (j + d).wrapping_sub(ky.len() / 2);
          if rows.contains(&y) {
            let o = ((y - rows.start) * width + i - columns.start) * 4;
            for (c, total) in sum.iter_mut().enumerate() {
              *total += f64::from(across[o + c]) * weight;
            }
          }
        }
        if sum[3] <= 0.0 {
          continue;
        }
        let at = (j * self.columns + i) * 4;
        let alpha = sum[3].min(1.0);
        let keep = 1.0 - alpha;
        for (c, total) in sum.iter().take(3).enumerate() {
          self.buffer[at + c] = single(total + f64::from(self.buffer[at + c]) * keep);
        }
        self.buffer[at + 3] = single(alpha + f64::from(self.buffer[at + 3]) * keep);
      }
    }
  }

  /// The mean of the 16 points of each cell, row by row.
  fn cells(&self) -> Vec<[f32; 4]> {
    let (columns, rows) = (self.columns / 4, self.rows / 4);
    let mut cells = vec![[0.0_f32; 4]; columns * rows];
    for (at, cell) in cells.iter_mut().enumerate() {
      let (column, row) = (at % columns, at / columns);
      for j in 0..4 {
        for i in 0..4 {
          let point = ((row * 4 + j) * self.columns + column * 4 + i) * 4;
          for (c, value) in cell.iter_mut().enumerate() {
            *value = single(f64::from(*value) + f64::from(self.buffer[point + c]));
          }
        }
      }
      for value in cell.iter_mut() {
        *value /= 16.0;
      }
    }
    cells
  }
}

/// The weights of a Gaussian of `sigma` pixels at points `step` pixels apart, over 3 sigmas each
/// side. A kernel whose weights add to more than 1 is scaled to 1.
fn kernel(sigma: f64, step: f64) -> Vec<f64> {
  let reach = (3.0 * sigma / step).ceil().max(1.0);
  let mut weights = Vec::new();
  let mut d = -reach;
  while d <= reach {
    weights.push(
      step / ((2.0 * PI).sqrt() * sigma) * exp(-(d * step) * (d * step) / (2.0 * sigma * sigma)),
    );
    d += 1.0;
  }
  let sum: f64 = weights.iter().sum();
  if sum > 1.0 {
    for weight in &mut weights {
      *weight /= sum;
    }
  }
  weights
}

/// The paths of `morph` at `ratio`, from 0 at its start to 1 at its end: its fills, each in the
/// order of its first edge. Each curve becomes up to 32 straight edges.
fn morphed(morph: &Morph, ratio: f64) -> Vec<Path> {
  let mut paths = Vec::new();
  for (style, outline) in outlines(morph, ratio) {
    let fill = Some(morph_fill(&morph.fills[style - 1], ratio));
    paths.push(Path { subs: outline.subs, fill, stroke: None, width: 0.0 });
  }
  paths
}

/// The outlines of the fills of `morph` at `ratio`, each with its style, in the order of its first
/// edge.
fn outlines(morph: &Morph, ratio: f64) -> Styled {
  let mut fills = Vec::new();
  let (mut start_at, mut end_at) = ((0.0, 0.0), (0.0, 0.0));
  let mut styles = [0.0; 2];
  for record in &morph.records {
    match *record {
      Record::Style { start, end, fills: [fill, other] } => {
        (start_at, end_at) = (start.unwrap_or(start_at), end.unwrap_or(end_at));
        for (style, value) in styles.iter_mut().zip([fill, other]) {
          if value >= 0.0 {
            *style = value;
          }
        }
      }
      Record::Edge { start, end } => {
        let (from, to) = (quad(start, start_at), quad(end, end_at));
        let mixed = [
          lerp(start_at.0, end_at.0, ratio),
          lerp(start_at.1, end_at.1, ratio),
          lerp(from[0], to[0], ratio),
          lerp(from[1], to[1], ratio),
          lerp(from[2], to[2], ratio),
          lerp(from[3], to[3], ratio),
        ];
        add(&mut fills, styles[0], mixed);
        add(&mut fills, styles[1], mixed);
        (start_at, end_at) = ((from[2], from[3]), (to[2], to[3]));
      }
    }
  }
  fills
}

/// The outlines of a morph shape, each with its style.
type Styled = Vec<(usize, Outline)>;

/// The sub-paths of one style of a morph shape, and the end of its last edge.
struct Outline {
  subs: Vec<Sub>,
  last: (f64, f64),
}

/// Adds the curve `edge`, from (x0, y0) by the control (cx, cy) to (x1, y1), to the outline of
/// `style`: a new sub-path starts unless the curve starts at the end of the last one.
#[expect(clippy::expect_used, reason = "the first edge of an outline starts its first sub-path")]
fn add(outlines: &mut Vec<(usize, Outline)>, style: f64, edge: [f64; 6]) {
  if style == 0.0 {
    return;
  }
  let style = index(style);
  let [x0, y0, cx, cy, x1, y1] = edge;
  let at = outlines.iter().position(|(other, _)| *other == style).unwrap_or_else(|| {
    outlines.push((style, Outline { subs: Vec::new(), last: (f64::NAN, f64::NAN) }));
    outlines.len() - 1
  });
  let outline = &mut outlines[at].1;
  if !((outline.last.0 - x0).abs() < 0.01 && (outline.last.1 - y0).abs() < 0.01) {
    outline.subs.push(Sub { points: vec![x0, y0], closed: false });
  }
  let count = (hypot(cx - x0, cy - y0) + hypot(x1 - cx, y1 - cy)).ceil().clamp(1.0, 32.0);
  let points =
    &mut outline.subs.last_mut().expect("an outline holds a sub-path at its first edge").points;
  let mut k = 1.0;
  while k <= count {
    let t = k / count;
    let u = 1.0 - t;
    points.push(u * u * x0 + 2.0 * u * t * cx + t * t * x1);
    points.push(u * u * y0 + 2.0 * u * t * cy + t * t * y1);
    k += 1.0;
  }
  outline.last = (x1, y1);
}

/// The length of (a, b), with the scale and the compensated sum of the rule of a browser.
fn hypot(a: f64, b: f64) -> f64 {
  let most = a.abs().max(b.abs());
  if most == 0.0 {
    return 0.0;
  }
  let (mut sum, mut compensation) = (0.0, 0.0);
  for value in [a, b] {
    let n = value.abs() / most;
    let summand = n * n - compensation;
    let preliminary = sum + summand;
    compensation = (preliminary - sum) - summand;
    sum = preliminary;
  }
  sum.sqrt() * most
}

/// The control point and the end of `edge` from `from`.
fn quad(edge: Edge, from: (f64, f64)) -> [f64; 4] {
  let (px, py) = from;
  match edge {
    Edge::Line { dx, dy } => [px + dx / 2.0, py + dy / 2.0, px + dx, py + dy],
    Edge::Curve { cx, cy, ax, ay } => [px + cx, py + cy, px + cx + ax, py + cy + ay],
  }
}

fn lerp(start: f64, end: f64, ratio: f64) -> f64 {
  start + (end - start) * ratio
}

/// A colour of a morph shape at `ratio`: its red, green and blue rounded to whole steps of 255,
/// and its opacity to thousandths.
fn colour_of(start: [f64; 4], end: [f64; 4], ratio: f64) -> [f64; 4] {
  let channel = |c: usize| (lerp(start[c], end[c], ratio) + 0.5).floor() / 255.0;
  [
    channel(0),
    channel(1),
    channel(2),
    (lerp(start[3], end[3], ratio) * 1000.0 + 0.5).floor() / 1000.0,
  ]
}

/// The gradient of a morph shape at `ratio`. The gradients of the books have an area at each
/// ratio that their timelines give.
#[expect(clippy::expect_used, reason = "the gradients of the books have an area")]
fn morph_fill(fill: &MorphFill, ratio: f64) -> Fill {
  let matrix: Matrix = [0, 1, 2, 3, 4, 5].map(|k| lerp(fill.start[k], fill.end[k], ratio));
  let inverse = invert(matrix).expect("the gradients of the books have an area");
  let mut last = 0.0;
  let stops = fill
    .stops
    .iter()
    .map(|stop| {
      let at = (lerp(stop[0], stop[5], ratio) / 255.0).clamp(0.0, 1.0).max(last);
      last = at;
      let colour = colour_of(
        [stop[1], stop[2], stop[3], stop[4]],
        [stop[6], stop[7], stop[8], stop[9]],
        ratio,
      );
      [at, colour[0], colour[1], colour[2], colour[3]]
    })
    .collect();
  Fill::Gradient(Gradient { radial: fill.radial, inverse, stops })
}

#[expect(
  clippy::cast_possible_truncation,
  reason = "a point keeps its colour in f32, as the cells keep it"
)]
fn single(value: f64) -> f32 {
  value as f32
}

#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "each caller gives a whole number from 0, far under the points of a patch"
)]
fn index(value: f64) -> usize {
  value as usize
}

#[expect(clippy::cast_precision_loss, reason = "an index of a point is far under 2^52")]
fn whole(value: usize) -> f64 {
  value as f64
}
