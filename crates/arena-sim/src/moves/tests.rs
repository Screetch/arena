//! The scenes of the moves: each file of `scenes/` gives a role, a start, a seed and steps on its
//! first line, then the state of the world after each step.
//!
//! A step is a number of steps to play, a key pressed or released (`+up`, `-special`), a weapon
//! (`w2`), the button of the shots (`+mouse`, `-mouse`), a look at a point of the map
//! (`look:600,1600`), or drops thrown from a point (`splash:900,1700,90,5`). A line of state holds
//! the fighter, then the shots in flight, the stuck shots, the effects, the drops and the stains,
//! parted by `|`, each item parted by `,`. A part `=` is the same as on the line before.

use super::shot::{Kind, Launch};
use super::{Drop, Drops, Key, Seeded, Shot, Spawn, Spray, World};
use crate::clip::Clip;
use crate::map::{Block, FIRST, Map};

#[derive(Clone, Debug, PartialEq)]
enum Value {
  Number(f64),
  Word(String),
}

impl Value {
  #[expect(clippy::float_cmp, reason = "a scene records each number exactly")]
  fn matches(&self, expected: &str) -> bool {
    match self {
      Self::Number(number) => expected.parse::<f64>().is_ok_and(|wanted| wanted == *number),
      Self::Word(word) => word == expected,
    }
  }
}

fn number(value: f64) -> Value {
  Value::Number(value)
}

fn flag(value: bool) -> Value {
  Value::Number(f64::from(u8::from(value)))
}

fn clip_row(kind: impl std::fmt::Debug, clip: &Clip) -> Vec<Value> {
  vec![Value::Word(format!("{kind:?}")), number(clip.x), number(clip.y), number(clip.rotation)]
}

fn named<'a>(clip: &'a Clip, name: &str) -> &'a Clip {
  clip.named(name).expect("the figure holds its parts")
}

/// The state of the world as the files give it, part by part.
fn state(world: &World<Seeded>) -> Vec<Vec<Value>> {
  let (fighter, figure) = (&world.fighter, world.figure());
  let (body, arm) = (named(figure, "spr"), named(figure, "weapon"));
  let shown = arm.named(&format!("w{}", fighter.weapon)).map_or(0, Clip::frame);
  let mut parts = vec![vec![
    number(fighter.x),
    number(fighter.y),
    number(fighter.speed_x),
    number(fighter.fall),
    number(fighter.drift_x),
    number(fighter.drift_y),
    number(fighter.slope_y),
    flag(fighter.grounded),
    number(f64::from(fighter.wall)),
    flag(fighter.on_ledge),
    number(fighter.slope),
    number(fighter.energy),
    number(f64::from(fighter.stunned)),
    number(f64::from(fighter.brake)),
    number(f64::from(fighter.jump_wait)),
    number(f64::from(fighter.charge)),
    number(f64::from(fighter.reload)),
    Value::Word(body.label().unwrap_or("null").to_owned()),
    number(f64::from(body.frame())),
    number(body.scale_x),
    number(figure.rotation),
    number(arm.rotation),
    number(arm.y),
    number(f64::from(fighter.weapon)),
    number(f64::from(shown)),
    number(world.camera()[0]),
    number(world.camera()[1]),
  ]];
  let shot = |shot: &Shot| {
    let clip = &shot.clip;
    let tail =
      [number(clip.alpha), number(f64::from(clip.frame())), flag(clip.dead), flag(shot.landed)];
    [clip_row(shot.effect, clip), tail.to_vec()].concat()
  };
  let effect = |spawn: &Spawn| {
    let clip = &spawn.clip;
    let tail = [number(clip.scale_y), number(f64::from(clip.frame())), flag(clip.dead)];
    [clip_row(spawn.effect, clip), tail.to_vec()].concat()
  };
  let drop = |drop: &Drop| {
    let clip = &drop.clip;
    let tail = [
      number(f64::from(clip.frame())),
      number(clip.scale_x),
      number(clip.scale_y),
      flag(drop.over),
    ];
    [clip_row(drop.effect, clip), tail.to_vec()].concat()
  };
  parts.push(joined(world.shots().iter().map(shot)));
  parts.push(joined(world.stuck().iter().map(shot)));
  parts.push(joined(world.effects().iter().map(effect)));
  parts.push(joined(world.drops().live().iter().map(drop)));
  parts.push(joined(world.drops().stamps().iter().map(drop)));
  parts
}

fn joined(rows: impl Iterator<Item = Vec<Value>>) -> Vec<Value> {
  let rows: Vec<Vec<Value>> = rows.collect();
  rows.join(&Value::Word(",".to_owned()))
}

fn act(world: &mut World<Seeded>, step: &str) {
  if let Some(slot) = step.strip_prefix('w') {
    world.change_weapon(slot.parse().expect("a weapon is 1, 2 or 3"));
  } else if let Some(point) = step.strip_prefix("look:") {
    let [x, y] = numbers(point);
    let camera = world.camera();
    world.look([x - camera[0], y - camera[1]]);
  } else if let Some((spray, at)) = step.split_once(':') {
    let [x, y, rotation, count] = numbers(at);
    let spray = if spray == "splash" { Spray::Splash } else { Spray::Embers };
    world.burst(spray, whole(count), x, y, rotation);
  } else if step == "+mouse" {
    world.press();
  } else if step == "-mouse" {
    world.release();
  } else {
    let key = match &step[1..] {
      "up" => Key::Up,
      "down" => Key::Down,
      "left" => Key::Left,
      "right" => Key::Right,
      _ => Key::Special,
    };
    world.key(key, step.starts_with('+'));
  }
}

fn numbers<const N: usize>(text: &str) -> [f64; N] {
  let numbers: Vec<f64> = text.split(',').map(|number| number.parse().expect("a number")).collect();
  numbers.try_into().expect("the step gives each of its numbers")
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a count of drops")]
fn whole(count: f64) -> u32 {
  count as u32
}

fn play(name: &str, text: &str) {
  let mut lines = text.lines();
  let head: Vec<&str> = lines.next().expect("a scene starts with its steps").split(' ').collect();
  let [role, x, y, seed] = numbers::<4>(&head[..4].join(","));
  let mut world = World::new(
    FIRST,
    [1750.0, 1000.0],
    Seeded::new(whole(seed)),
    u8::try_from(whole(role)).expect("a role is 1, 2 or 3"),
  );
  (world.fighter.x, world.fighter.y) = (x, y);
  let mut before: Vec<Vec<&str>> = vec![Vec::new(); 6];
  let mut frame = 0;
  for step in &head[4..] {
    let Ok(count) = step.parse::<u32>() else {
      act(&mut world, step);
      continue;
    };
    for _ in 0..count {
      world.frame();
      frame += 1;
      let line = lines.next().expect("the scene gives the state of each step");
      let expected: Vec<Vec<&str>> = line
        .split('|')
        .zip(&before)
        .map(
          |(part, last)| {
            if part.trim() == "=" { last.clone() } else { part.split_whitespace().collect() }
          },
        )
        .collect();
      for (at, (got, wanted)) in state(&world).iter().zip(&expected).enumerate() {
        let same = got.len() == wanted.len()
          && got.iter().zip(wanted).all(|(got, wanted)| got.matches(wanted));
        assert!(same, "{name}, step {frame}, part {at}:\n got {got:?}\nwant {wanted:?}");
      }
      before = expected;
    }
  }
  assert_eq!(lines.next(), None, "{name} gives more steps than it plays");
}

#[test]
fn each_scene_plays_to_the_state_that_it_records_at_each_step() {
  let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/src/moves/scenes");
  let mut scenes: Vec<_> = std::fs::read_dir(folder)
    .expect("the scenes stand")
    .map(|entry| entry.expect("a scene").path())
    .collect();
  scenes.sort();
  assert_eq!(scenes.len(), 42);
  for path in scenes {
    let text = std::fs::read_to_string(&path).expect("a scene reads");
    play(&path.file_stem().expect("a name").to_string_lossy(), &text);
  }
}

/// The stuck shots keep the last 150, and a shot that lands joins them at the end.
#[test]
fn the_stuck_shots_keep_the_last_150() {
  let mut world = World::new(FIRST, [1750.0, 1000.0], Seeded::new(1), 1);
  let launch =
    |serial| Launch { x: 400.0, y: 1700.0, rotation: 90.0, attack: 1, hand: [0.0; 2], serial };
  let arrow = |serial| Shot::new(Kind::Arrow, &launch(serial), &mut Seeded::new(1)).0;
  world.stuck = (1..=150).map(arrow).collect();
  let mut landed = arrow(151);
  landed.landed = true;
  world.shots.push(landed);
  world.frame();
  let serials: Vec<u64> = world.stuck().iter().map(|shot| shot.serial).collect();
  assert_eq!(serials, (2..=151).collect::<Vec<_>>());
  assert!(world.stuck()[149].clip.dead);
  assert_eq!(world.stuck()[149].clip.label(), Some("dead"));
}

/// The drops in flight keep the last 600 and the stains the last 800, and a drop that ends far out
/// of the map leaves no stain.
#[test]
fn the_drops_keep_the_last_600_and_the_stains_the_last_800() {
  let mut chance = Seeded::new(7);
  let mut drops = Drops::default();
  for _ in 0..600 {
    drops.make(Spray::Embers, 900.0, 1700.0, 0.0, &mut chance);
  }
  let second = drops.live()[1].clip.scale_x;
  drops.make(Spray::Embers, 900.0, 1700.0, 0.0, &mut chance);
  assert_eq!((drops.live().len(), drops.live()[0].clip.scale_x), (600, second));
  let end = |drops: &mut Drops| {
    for drop in &mut drops.live {
      drop.over = true;
    }
    drops.tick(&FIRST);
  };
  end(&mut drops);
  let second = drops.stamps()[1].clip.scale_x;
  for _ in 0..201 {
    drops.make(Spray::Splash, 900.0, 1700.0, 0.0, &mut chance);
  }
  end(&mut drops);
  assert_eq!((drops.stamps().len(), drops.stamps()[0].clip.scale_x), (800, second));
  assert!(drops.live().is_empty() && drops.stamps().iter().all(|stamp| !stamp.clip.playing()));
  drops.make(Spray::Splash, 4921.0, 1700.0, 0.0, &mut chance);
  end(&mut drops);
  assert_eq!((drops.live().len(), drops.stamps().len()), (0, 800));
}

/// A ceiling that holds the head pushes the fighter down a unit at a time, at most 20, and the
/// ceiling on both sides of the head pushes it to neither side.
#[test]
fn a_ceiling_pushes_the_fighter_down_and_to_neither_side() {
  const SQUEEZE: Map = Map {
    width: 400,
    height: 400,
    blocks: &[
      Block { x: 0, y: 300, width: 400, height: 100 },
      Block { x: 0, y: 0, width: 400, height: 240 },
    ],
    ramps: &[],
  };
  let mut world = World::new(SQUEEZE, [1750.0, 1000.0], Seeded::new(1), 1);
  (world.fighter.x, world.fighter.y) = (200.0, 290.0);
  world.frame();
  let fighter = &world.fighter;
  assert_eq!((fighter.x, fighter.y, fighter.touching, fighter.wall), (200.0, 311.0, true, 0));
}

/// A camera wider and higher than the map centres it, and moves a quarter of the way at each step.
#[test]
#[expect(clippy::float_cmp, reason = "the camera moves by exact quarters")]
fn a_camera_larger_than_the_map_centres_it() {
  let mut world = World::new(FIRST, [6000.0, 3000.0], Seeded::new(1), 1);
  world.frame();
  assert_eq!(world.camera(), [-150.0, 400.0]);
}
