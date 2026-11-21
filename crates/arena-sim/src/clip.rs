//! The clips of the figure and of its effects, which play [`FPS`] frames a second.
//!
//! A clip is a tree. Its sprite, in a [`Book`], is a timeline of frames: each frame places the
//! children of the clip, each with a matrix, an opacity and filters, and a frame can hold a script
//! that stops the clip, removes it or sends it to another frame. A child is a clip too, or a shape
//! of the book, which the client draws.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::words::Words;

/// The frames that a clip plays each second.
pub const FPS: u32 = 40;

/// A book of sprites and shapes. Each book numbers its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Book {
  /// The figure, its bodies and its weapons.
  Figure,
  /// The shots, the specials of the roles, the impacts and the drops.
  Effects,
}

/// A frame of a timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At<'a> {
  /// The first frame of a label.
  Label(&'a str),
  /// A frame, from 1.
  Frame(u16),
}

/// The effects of the book of effects: the shots of the 9 weapons, the specials of the 3 roles,
/// the impact, the parry, and the drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
  /// The shot of the weapon 8.
  Lightning,
  /// The shot of the weapon 7.
  Fireball,
  /// The shot of the weapon 9.
  Shockwave,
  /// The shot of the weapon 1.
  Arrow,
  /// The shot of the weapon 2.
  Slash,
  /// The shot of the weapon 4.
  Sweep,
  /// The shot of the weapon 5.
  Spear,
  /// The shot of the weapon 3, which bursts into smoke where it lands.
  SmokeBomb,
  /// The shot of the weapon 6.
  ShieldBash,
  /// The jump in the air of the role 2.
  AirJump,
  /// The trail of the warp of the role 1.
  WarpTrail,
  /// The thrust of the role 3, and each end of the warp of the role 1.
  Thrust,
  /// An impact, with its variants as labels.
  Impact,
  /// A parry.
  Parry,
  /// A drop of blood, and its stain on the ground.
  Splash,
  /// A drop of fire, and its stain on the ground.
  Embers,
}

impl Effect {
  /// Each effect, in the order of its declaration.
  pub const ALL: [Self; 16] = [
    Self::Lightning,
    Self::Fireball,
    Self::Shockwave,
    Self::Arrow,
    Self::Slash,
    Self::Sweep,
    Self::Spear,
    Self::SmokeBomb,
    Self::ShieldBash,
    Self::AirJump,
    Self::WarpTrail,
    Self::Thrust,
    Self::Impact,
    Self::Parry,
    Self::Splash,
    Self::Embers,
  ];

  /// The sprite of the effect in the book of effects.
  pub fn id(self) -> u16 {
    match self {
      Self::Lightning => 179,
      Self::Fireball => 192,
      Self::Shockwave => 246,
      Self::Arrow => 198,
      Self::Slash => 182,
      Self::Sweep => 172,
      Self::Spear => 185,
      Self::SmokeBomb => 251,
      Self::ShieldBash => 248,
      Self::AirJump => 242,
      Self::WarpTrail => 296,
      Self::Thrust => 243,
      Self::Impact => 169,
      Self::Parry => 131,
      Self::Splash => 234,
      Self::Embers => 227,
    }
  }

  /// The clip of the effect, on its first frame.
  pub fn clip(self) -> Clip {
    Clip::new(Book::Effects, self.id())
  }
}

/// The clip of the figure in the book of the figure.
pub const FIGURE: u16 = 114;

impl Clip {
  /// The figure, dressed: its flag hidden, its weapon placed by its own fields at its hand, and
  /// its body playing from the label `idle`.
  #[expect(clippy::expect_used, reason = "the clip of the figure holds its flag, body and weapon")]
  pub fn figure() -> Self {
    let mut figure = Self::new(Book::Figure, FIGURE);
    figure.named_mut("flag").expect("the figure holds its flag").visible = false;
    let weapon = figure.named_mut("weapon").expect("the figure holds its weapon");
    (weapon.free, weapon.x, weapon.y) = (true, 0.65, -57.0);
    figure.named_mut("spr").expect("the figure holds its body").play_from(At::Label("idle"));
    figure
  }

  /// Stops the body of the figure at the frame `body`, from 1 to 7, and shows the weapon
  /// `weapon` alone, from 1 to 9, on its first frame, or no weapon for 0.
  #[expect(clippy::expect_used, reason = "the clip of the figure holds its body and its weapon")]
  pub fn wear(&mut self, body: u16, weapon: u8) {
    let spr = self.named_mut("spr").expect("the figure holds its body");
    spr.named_mut("body").expect("each frame of the body holds its role").stop_at(At::Frame(body));
    let weapons = self.named_mut("weapon").expect("the figure holds its weapon");
    for one in 1..=9 {
      let shown = weapons.named_mut(&format!("w{one}")).expect("the figure holds each weapon");
      shown.visible = one == weapon;
      if one == weapon {
        shown.stop_at(At::Frame(1));
      }
    }
  }
}

/// Where a frame puts a child of its clip.
#[derive(Debug, PartialEq)]
pub struct Place {
  /// The depth of the child. A frame that places a child with the same depth, instance and id
  /// keeps it, and makes the others anew.
  pub depth: u16,
  /// The instance of the child at its depth.
  pub instance: u16,
  /// The sprite, the shape or the morph shape of the child, in the book of its parent.
  pub id: u16,
  /// The name by which its parent finds the child.
  pub name: Option<String>,
  /// The matrix `[a, b, c, d, e, f]` from the units of the child to those of its parent.
  pub matrix: [f64; 6],
  /// The multiplier of the opacity of the child and of its own children.
  pub alpha: f64,
  /// The ratio of a morph shape, from 0 to 65535.
  pub ratio: f64,
  /// The shadow under the child.
  pub shadow: Option<Shadow>,
  /// The blur of the child, in x and in y.
  pub blur: Option<[f64; 2]>,
}

/// A shadow under a child, which its filter casts with no offset.
#[derive(Debug, PartialEq)]
pub struct Shadow {
  /// Its red, green and blue, from 0 to 255, and its opacity, from 0 to 1.
  pub colour: [f64; 4],
  /// The multiplier of its opacity.
  pub strength: f64,
  /// Its blur, in x and in y.
  pub blur: [f64; 2],
}

/// A clip of a book, and the clips that its frame places.
#[derive(Clone, Debug, PartialEq)]
#[expect(
  clippy::struct_excessive_bools,
  reason = "the timeline, its scripts, its owner and its parent each set one flag of a clip apart"
)]
pub struct Clip {
  book: Book,
  id: u16,
  sprite: Option<&'static Sprite>,
  frame: u16,
  playing: bool,
  parts: Vec<Part>,
  /// Says whether a script removed the clip, or its owner ended it.
  pub dead: bool,
  /// Says whether the clip and its children are drawn.
  pub visible: bool,
  /// Says whether the clip stands by its own fields alone, and not by the place that its parent
  /// gives it.
  pub free: bool,
  /// The x of the clip in its parent, over its place.
  pub x: f64,
  /// The y of the clip in its parent, over its place.
  pub y: f64,
  /// The rotation of the clip, in degrees, clockwise on the screen.
  pub rotation: f64,
  /// The scale of the clip in x: -1 flips it.
  pub scale_x: f64,
  /// The scale of the clip in y.
  pub scale_y: f64,
  /// The opacity of the clip.
  pub alpha: f64,
}

/// A child of a clip, with the place that the frame of the clip gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
  /// The place of the child.
  pub place: &'static Place,
  /// The child.
  pub clip: Clip,
}

#[derive(Debug, Default)]
struct Sprite {
  frames: Vec<Vec<Place>>,
  labels: Vec<(String, u16)>,
  scripts: HashMap<u16, Vec<Op>>,
}

#[derive(Clone, Copy, Debug)]
enum Op {
  Stop,
  Remove,
  Go { frame: u16, play: bool },
}

static FIGURE_SPRITES: LazyLock<HashMap<u16, Sprite>> =
  LazyLock::new(|| sprites(include_str!("../data/figure.txt")));
static EFFECT_SPRITES: LazyLock<HashMap<u16, Sprite>> =
  LazyLock::new(|| sprites(include_str!("../data/effects.txt")));

impl Book {
  fn sprite(self, id: u16) -> Option<&'static Sprite> {
    let sprites: &'static HashMap<u16, Sprite> = match self {
      Self::Figure => &FIGURE_SPRITES,
      Self::Effects => &EFFECT_SPRITES,
    };
    sprites.get(&id)
  }
}

impl Clip {
  /// The clip of `id` in `book`, on its first frame. It plays when its sprite has more than one
  /// frame, and the script of that frame runs.
  pub fn new(book: Book, id: u16) -> Self {
    let sprite = book.sprite(id);
    let mut clip = Self {
      book,
      id,
      sprite,
      frame: 1,
      playing: sprite.is_some_and(|sprite| sprite.frames.len() > 1),
      parts: Vec::new(),
      dead: false,
      visible: true,
      free: false,
      x: 0.0,
      y: 0.0,
      rotation: 0.0,
      scale_x: 1.0,
      scale_y: 1.0,
      alpha: 1.0,
    };
    clip.sync();
    clip.run_script();
    clip
  }

  /// The book of the clip.
  pub fn book(&self) -> Book {
    self.book
  }

  /// The sprite or the shape of the clip in its book.
  pub fn id(&self) -> u16 {
    self.id
  }

  /// Says whether the clip is a sprite of its book, with a timeline, and not a shape.
  pub fn is_sprite(&self) -> bool {
    self.sprite.is_some()
  }

  /// The frame of the clip, from 1.
  pub fn frame(&self) -> u16 {
    self.frame
  }

  /// The frames of the timeline of the clip: 1 for a shape.
  pub fn frames(&self) -> u16 {
    self.sprite.map_or(1, Sprite::frames)
  }

  /// Says whether the clip moves to its next frame at each advance.
  pub fn playing(&self) -> bool {
    self.playing
  }

  /// Plays the clip from its frame, or stops it there.
  pub fn set_playing(&mut self, playing: bool) {
    self.playing = playing;
  }

  /// The label of the frame: the last label that starts at the frame or before it.
  pub fn label(&self) -> Option<&'static str> {
    let mut best = None;
    let mut start = 0;
    for (name, frame) in self.sprite.map_or(&[][..], |sprite| &sprite.labels) {
      if *frame <= self.frame && *frame >= start {
        start = *frame;
        best = Some(name.as_str());
      }
    }
    best
  }

  /// Says whether the timeline of the clip holds `label`.
  pub fn has_label(&self, label: &str) -> bool {
    self.sprite.is_some_and(|sprite| sprite.label(label).is_some())
  }

  /// Says whether the timeline of the clip holds a label.
  pub fn has_labels(&self) -> bool {
    self.sprite.is_some_and(|sprite| !sprite.labels.is_empty())
  }

  /// Goes to `at` and plays from there. A frame out of the timeline goes to its nearest end, and
  /// a label that the timeline does not hold changes nothing.
  pub fn play_from(&mut self, at: At<'_>) {
    self.go(at, true);
  }

  /// Goes to `at` and stops there, as [`Clip::play_from`] goes.
  pub fn stop_at(&mut self, at: At<'_>) {
    self.go(at, false);
  }

  /// Plays the next frame of the clip when it plays, from the first after the last, and then
  /// advances each of its children, also a child that this frame made.
  pub fn advance(&mut self) {
    if let Some(sprite) = self.sprite
      && self.playing
      && sprite.frames.len() > 1
    {
      self.frame = if self.frame >= sprite.frames() { 1 } else { self.frame + 1 };
      self.sync();
      self.run_script();
    }
    for part in &mut self.parts {
      part.clip.advance();
    }
  }

  /// The children of the clip, in the order that its frame places them, the lowest first.
  pub fn parts(&self) -> &[Part] {
    &self.parts
  }

  /// The child of the clip that its frame names `name`.
  pub fn named(&self, name: &str) -> Option<&Self> {
    self.parts.iter().rev().find(|part| part.place.name.as_deref() == Some(name)).map(|p| &p.clip)
  }

  /// The child of the clip that its frame names `name`, to change.
  pub fn named_mut(&mut self, name: &str) -> Option<&mut Self> {
    let part = self.parts.iter_mut().rev().find(|part| part.place.name.as_deref() == Some(name));
    part.map(|part| &mut part.clip)
  }

  fn go(&mut self, at: At<'_>, play: bool) {
    let frame = match at {
      At::Label(name) => self.sprite.and_then(|sprite| sprite.label(name)),
      At::Frame(frame) => Some(self.sprite.map_or(frame, |sprite| frame.clamp(1, sprite.frames()))),
    };
    let Some(frame) = frame.filter(|&frame| frame > 0) else { return };
    self.frame = frame;
    self.playing = play;
    self.sync();
    self.run_script();
  }

  fn sync(&mut self) {
    let Some(sprite) = self.sprite else { return };
    let mut old = std::mem::take(&mut self.parts);
    let places = &sprite.frames[usize::from(self.frame - 1)];
    self.parts = places
      .iter()
      .map(|place| {
        let kept = old.iter().position(|part| {
          (part.place.depth, part.place.instance, part.clip.id)
            == (place.depth, place.instance, place.id)
        });
        let clip =
          kept.map_or_else(|| Self::new(self.book, place.id), |at| old.swap_remove(at).clip);
        Part { place, clip }
      })
      .collect();
  }

  fn run_script(&mut self) {
    let Some(ops) = self.sprite.and_then(|sprite| sprite.scripts.get(&self.frame)) else { return };
    for op in ops {
      match *op {
        Op::Stop => self.playing = false,
        Op::Remove => self.dead = true,
        Op::Go { frame, play } => {
          self.frame = frame;
          self.sync();
          self.playing = play;
          self.run_script();
          return;
        }
      }
    }
  }
}

/// A sprite is one of its book, so two sprites are equal when they are the same one.
impl PartialEq for Sprite {
  fn eq(&self, other: &Self) -> bool {
    std::ptr::eq(self, other)
  }
}

impl Sprite {
  #[expect(
    clippy::cast_possible_truncation,
    reason = "a timeline holds far fewer than 65536 frames"
  )]
  fn frames(&self) -> u16 {
    self.frames.len() as u16
  }

  fn label(&self, name: &str) -> Option<u16> {
    self.labels.iter().find(|(label, _)| label == name).map(|&(_, frame)| frame)
  }
}

/// The sprites of a book, from its lines of words: a line `sprite` starts each sprite, a line
/// `frame` each of its frames and a line `place` each child of a frame, as [`place`] reads it. A
/// line `label` names a frame, and a line `script` gives the operations of a frame in order:
/// `stop`, `remove`, and `play` or `halt` at a `label` or at a `frame`.
#[expect(
  clippy::expect_used,
  reason = "the books are data of the crate, which its tests read whole"
)]
fn sprites(text: &str) -> HashMap<u16, Sprite> {
  let mut read: Vec<(u16, Sprite)> = Vec::new();
  for line in text.lines() {
    let mut words = Words::new(line);
    let kind = words.next();
    if kind == Some("sprite") {
      read.push((words.number(), Sprite::default()));
      continue;
    }
    let (_, sprite) = read.last_mut().expect("a line sprite starts the data of a book");
    match kind {
      Some("label") => {
        let name = words.word().to_owned();
        sprite.labels.push((name, words.number()));
      }
      Some("script") => {
        let frame = words.number();
        let ops = script(&mut words, sprite);
        sprite.scripts.insert(frame, ops);
      }
      Some("frame") => sprite.frames.push(Vec::new()),
      _ => {
        sprite.frames.last_mut().expect("a line frame starts its places").push(place(&mut words));
      }
    }
  }
  read.into_iter().collect()
}

#[expect(
  clippy::expect_used,
  reason = "the books are data of the crate, which its tests read whole"
)]
fn script(words: &mut Words<'_>, sprite: &Sprite) -> Vec<Op> {
  let mut ops = Vec::new();
  while let Some(op) = words.next() {
    ops.push(match op {
      "stop" => Op::Stop,
      "remove" => Op::Remove,
      _ => {
        let frame = match words.next() {
          Some("label") => {
            sprite.label(words.word()).expect("a script goes to a label of its sprite")
          }
          _ => words.number(),
        };
        Op::Go { frame, play: op == "play" }
      }
    });
  }
  ops
}

/// A child of a frame: its depth, its instance, its id, its name or `-`, the six numbers of its
/// matrix, its opacity and its ratio, then its filters: `shadow` with its colour, its strength and
/// its blur, and `blur` with its blur.
fn place(words: &mut Words<'_>) -> Place {
  let [depth, instance, id] = words.numbers();
  let name = Some(words.word()).filter(|&name| name != "-").map(str::to_owned);
  let matrix = words.numbers();
  let [alpha, ratio] = words.numbers();
  let (mut shadow, mut blur) = (None, None);
  while let Some(filter) = words.next() {
    if filter == "shadow" {
      let colour = words.numbers();
      let strength = words.number();
      shadow = Some(Shadow { colour, strength, blur: words.numbers() });
    } else {
      blur = Some(words.numbers());
    }
  }
  Place { depth, instance, id, name, matrix, alpha, ratio, shadow, blur }
}

#[cfg(test)]
mod tests {
  use super::{At, Book, Clip, Effect};

  const FIGURE: u16 = 114;

  fn played(clip: &mut Clip, steps: usize, frame: impl Fn(&Clip) -> u16) -> Vec<u16> {
    (0..steps)
      .map(|_| {
        clip.advance();
        frame(clip)
      })
      .collect()
  }

  fn body(figure: &Clip) -> &Clip {
    figure.named("spr").expect("the figure holds its body")
  }

  fn body_mut(figure: &mut Clip) -> &mut Clip {
    figure.named_mut("spr").expect("the figure holds its body")
  }

  #[test]
  fn the_figure_places_its_flag_its_body_and_its_weapon() {
    let figure = Clip::new(Book::Figure, FIGURE);
    let parts: Vec<_> =
      figure.parts().iter().map(|part| (part.place.name.as_deref(), part.clip.id())).collect();
    assert_eq!(parts, [(Some("flag"), 53), (Some("spr"), 85), (Some("weapon"), 113)]);
    let spr = body(&figure);
    assert_eq!(
      (spr.frame(), spr.playing(), spr.label(), spr.frames()),
      (1, true, Some("idle"), 108)
    );
    let place = &figure.parts()[1].place;
    let shadow = place.shadow.as_ref().expect("the body casts a shadow");
    assert_eq!(
      (place.matrix, shadow.colour, shadow.strength),
      ([1.0, 0.0, 0.0, 1.0, -0.9, -2.65], [0.0, 0.0, 0.0, 1.0], 10.0)
    );
    assert_eq!(figure.book(), Book::Figure);
    assert!(figure.is_sprite() && !figure.has_labels() && spr.has_labels());
  }

  #[test]
  fn a_script_sends_a_loop_back_to_its_label_and_stops_a_jump() {
    let mut figure = Clip::new(Book::Figure, FIGURE);
    body_mut(&mut figure).play_from(At::Label("run"));
    let run: Vec<u16> = (19..=35).chain(18..=35).chain(18..=22).collect();
    assert_eq!(played(&mut figure, 40, |figure| body(figure).frame()), run);
    assert_eq!(body(&figure).label(), Some("run"));
    body_mut(&mut figure).play_from(At::Label("jump"));
    assert_eq!(played(&mut figure, 5, |figure| body(figure).frame()), [53; 5]);
    assert!(!body(&figure).playing());
  }

  #[test]
  fn the_label_of_a_frame_is_the_last_that_starts_at_it_or_before() {
    let mut figure = Clip::new(Book::Figure, FIGURE);
    let spr = body_mut(&mut figure);
    let cases = [
      (1, "idle", 1),
      (14, "idle", 1),
      (15, "run1", 15),
      (18, "run", 18),
      (52, "runB", 40),
      (54, "jumpD", 54),
      (105, "sit", 105),
      (107, "shock", 107),
      (500, "shock", 106),
      (0, "idle", 1),
    ];
    for (at, label, frame) in cases {
      spr.stop_at(At::Frame(at));
      assert_eq!((spr.label(), spr.frame()), (Some(label), frame), "{at}");
    }
    spr.play_from(At::Label("nothing"));
    assert_eq!((spr.frame(), spr.playing()), (1, false));
    assert!(spr.has_label("wallrun") && !spr.has_label("nothing"));
  }

  #[test]
  fn a_frame_keeps_the_children_that_it_places_again() {
    let mut figure = Clip::new(Book::Figure, FIGURE);
    let spr = body_mut(&mut figure);
    let role = spr.named_mut("body").expect("the body holds its role");
    role.stop_at(At::Frame(3));
    role.visible = false;
    spr.play_from(At::Label("run"));
    figure.advance();
    let role = body(&figure).named("body").expect("the body holds its role");
    assert_eq!((role.frame(), role.playing(), role.visible), (3, false, false));
    let mut role = Clip::new(Book::Figure, 65);
    assert_eq!((role.frame(), role.playing()), (1, false));
    role.play_from(At::Frame(2));
    role.advance();
    let made: Vec<_> =
      role.parts().iter().map(|part| (part.clip.id(), part.clip.frame())).collect();
    assert_eq!((role.frame(), made), (3, vec![(59, 1), (61, 1)]));
  }

  #[test]
  fn a_weapon_plays_its_second_strike_and_goes_back_to_its_first_frame() {
    let mut figure = Clip::new(Book::Figure, FIGURE);
    let weapon = figure.named_mut("weapon").expect("the figure holds its weapon");
    let strike = weapon.named_mut("w2").expect("the weapon 2 stands");
    assert_eq!((strike.frame(), strike.playing(), strike.label()), (1, false, None));
    strike.play_from(At::Label("fire2"));
    let frame =
      |figure: &Clip| figure.named("weapon").and_then(|w| w.named("w2")).map_or(0, Clip::frame);
    let mut expected: Vec<u16> = (3..=10).collect();
    expected.extend([1; 12]);
    assert_eq!(played(&mut figure, 20, frame), expected);
  }

  #[test]
  fn each_effect_plays_its_own_timeline() {
    let frames = Effect::ALL.map(|effect| effect.clip().frames());
    assert_eq!(frames, [5, 20, 15, 29, 2, 2, 3, 16, 1, 16, 14, 12, 5, 11, 15, 9]);
  }

  #[test]
  fn an_effect_with_no_script_loops_and_a_script_stops_and_removes_one_at_its_end() {
    let mut lightning = Clip::new(Book::Effects, 179);
    assert_eq!(played(&mut lightning, 6, Clip::frame), [2, 3, 4, 5, 1, 2]);
    let mut jump = Clip::new(Book::Effects, 242);
    let frames = played(&mut jump, 14, Clip::frame);
    assert_eq!(frames, (2..=15).collect::<Vec<_>>());
    assert!(jump.playing() && !jump.dead);
    jump.advance();
    assert_eq!((jump.frame(), jump.playing(), jump.dead), (16, false, true));
    let mut fireball = Clip::new(Book::Effects, 192);
    fireball.set_playing(true);
    fireball.advance();
    let flame = fireball.named("spr").expect("the fireball holds its flame");
    assert_eq!((fireball.frame(), flame.id(), flame.frames()), (2, 189, 35));
    let shape = &fireball.named("spr").expect("the fireball holds its flame").parts()[0].clip;
    assert!(!shape.is_sprite());
  }
}
