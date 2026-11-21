//! The scenes of the moves: each file of `scenes/` gives a role, a start, a seed and steps on its
//! first line, then a line for each step that it plays: the point that the fighter aims at in the
//! step, then the state of the world after it.
//!
//! A step is a number of steps to play, a key held or let go (`+up`, `-special`), a weapon (`w2`),
//! the button of the shots (`+mouse`, `-mouse`), or drops thrown from a point
//! (`splash:900,1700,90,5`). A line holds the aim, the fighter, the shots in flight, the stuck
//! shots, the effects, the drops and the stains, parted by `|`, each item parted by `,`. A part `=`
//! is the same as on the line before.

use super::shot::{Kind, Launch};
use super::{
  Burst, Drop, Drops, Fighter, Id, Intents, Key, START, Seeded, Shot, Spawn, Spray, Start, World,
};
use crate::clip::Clip;
use crate::map::{Block, FIRST, Map};

/// The fighter of a scene.
const ME: Id = Id(1);

/// The steps between two copies of the state of a scene, each of which plays the rest of the scene
/// beside it.
const COPIES: u32 = 50;

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

fn me(world: &World<Seeded>) -> &Fighter {
  &world.fighters()[&ME]
}

/// The state of the world as the files give it, part by part.
fn state(world: &World<Seeded>) -> Vec<Vec<Value>> {
  let fighter = me(world);
  let figure = fighter.figure();
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

/// Changes `intents` as the step `step` says, or gives the burst that it throws.
fn act(intents: &mut Intents, step: &str) -> Option<Burst> {
  if let Some(slot) = step.strip_prefix('w') {
    intents.slot = slot.parse().expect("a weapon is 1, 2 or 3");
  } else if let Some((spray, at)) = step.split_once(':') {
    let [x, y, rotation, count] = numbers(at);
    let spray = if spray == "splash" { Spray::Splash } else { Spray::Embers };
    return Some(Burst { spray, count: whole(count), at: [x, y], rotation, by: None });
  } else if step.ends_with("mouse") {
    intents.button = step.starts_with('+');
  } else {
    let key = match &step[1..] {
      "up" => Key::Up,
      "down" => Key::Down,
      "left" => Key::Left,
      "right" => Key::Right,
      _ => Key::Special,
    };
    intents.keys[key as usize] = step.starts_with('+');
  }
  None
}

fn numbers<const N: usize>(text: &str) -> [f64; N] {
  let numbers: Vec<f64> = text.split(',').map(|number| number.parse().expect("a number")).collect();
  numbers.try_into().expect("the step gives each of its numbers")
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a count of drops")]
fn whole(count: f64) -> u32 {
  count as u32
}

/// The world of a scene from the first line of its file, and the intents of its fighter.
fn scene(head: &[&str]) -> (World<Seeded>, Intents) {
  let [role, x, y, seed] = numbers::<4>(&head[..4].join(","));
  let mut world = World::new(FIRST, Seeded::new(whole(seed)));
  assert!(world.add(ME, u8::try_from(whole(role)).expect("a role is 1, 2 or 3")));
  let fighter = world.state.fighters.get_mut(&ME).expect("the fighter of the scene came");
  (fighter.x, fighter.y) = (x, y);
  let intents = fighter.intents;
  (world, intents)
}

/// The parts of a line of a scene, where a part `=` is the part of the line before.
fn parts<'a>(line: &'a str, before: &[Vec<&'a str>]) -> Vec<Vec<&'a str>> {
  let part = |(part, last): (&'a str, &Vec<&'a str>)| {
    if part.trim() == "=" { last.clone() } else { part.split_whitespace().collect() }
  };
  line.split('|').zip(before).map(part).collect()
}

fn play(name: &str, text: &str) {
  let mut lines = text.lines();
  let head: Vec<&str> = lines.next().expect("a scene starts with its steps").split(' ').collect();
  let (mut world, mut intents) = scene(&head);
  let mut copies: Vec<World<Seeded>> = Vec::new();
  let mut before: Vec<Vec<&str>> = vec![Vec::new(); 7];
  let mut frame = 0;
  for step in &head[4..] {
    let Ok(count) = step.parse::<u32>() else {
      if let Some(burst) = act(&mut intents, step) {
        for one in std::iter::once(&mut world).chain(&mut copies) {
          one.burst(burst);
        }
      }
      continue;
    };
    for _ in 0..count {
      if frame % COPIES == 0 {
        copies.push(World::from_state(FIRST, world.state().clone()));
      }
      let line = lines.next().expect("the scene gives the state of each step");
      let expected = parts(line, &before);
      intents.aim = numbers(&expected[0].join(","));
      for one in std::iter::once(&mut world).chain(&mut copies) {
        assert!(one.intend(ME, intents));
        one.frame();
      }
      frame += 1;
      for (at, (got, wanted)) in state(&world).iter().zip(&expected[1..]).enumerate() {
        let same = got.len() == wanted.len()
          && got.iter().zip(wanted).all(|(got, wanted)| got.matches(wanted));
        assert!(same, "{name}, step {frame}, part {at}:\n got {got:?}\nwant {wanted:?}");
      }
      let drifted = copies.iter().position(|copy| copy.state() != world.state());
      assert_eq!(drifted, None, "{name}, step {frame}: a world from a copy of the state drifts");
      before = expected;
    }
  }
  assert_eq!(lines.next(), None, "{name} gives more steps than it plays");
}

/// Each scene plays with one fighter, and each world that a copy of its state rebuilds every 50
/// steps gives the same state at each later step.
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

/// The intents of the fighter `id` at `step` of a brawl: it runs, jumps, plays its special,
/// shoots, takes each weapon and, from the step 160, the next role, and asks a weapon and a role
/// out of their range on the way.
fn brawl(id: u32, step: u32) -> Intents {
  let mut keys = [false; 5];
  keys[Key::Right as usize] = step % 60 < 25;
  keys[Key::Left as usize] = step % 60 >= 40;
  keys[Key::Up as usize] = step % 40 == 10 + id;
  keys[Key::Special as usize] = (12 + id..16 + id).contains(&(step % 50));
  let slot = if step % 100 == 99 { 0 } else { (step / 70 + id) % 3 + 1 };
  let role = match step {
    0..150 => id % 3 + 1,
    150..160 => 0,
    _ => (id + 1) % 3 + 1,
  };
  Intents {
    keys,
    aim: [600.0 + f64::from(id) * 100.0, 1500.0],
    button: step % 20 < 3 + id,
    slot: u8::try_from(slot).expect("a slot is under 4"),
    role: u8::try_from(role).expect("a role is under 4"),
  }
}

/// Plays the step `step` of a brawl: the fighters 3, 1 and 2 come at the step 0, the fighter 3
/// leaves at the step 100, the fighter 4 comes at the step 110, and the fighter 2 throws embers
/// each 45 steps.
fn brawl_step(world: &mut World<Seeded>, step: u32) -> Vec<Start> {
  match step {
    0 => {
      for (id, role) in [(3, 3), (1, 1), (2, 2)] {
        assert!(world.add(Id(id), role));
      }
    }
    100 => assert!(world.remove(Id(3))),
    110 => assert!(world.add(Id(4), 2)),
    _ => {}
  }
  if step % 45 == 30 {
    let at = [700.0, 1600.0];
    world.burst(Burst { spray: Spray::Embers, count: 4, at, rotation: 0.0, by: Some(Id(2)) });
  }
  let ids: Vec<Id> = world.fighters().keys().copied().collect();
  for id in ids {
    assert!(world.intend(id, brawl(id.0, step)));
  }
  world.frame()
}

#[test]
fn two_worlds_that_get_the_same_intents_stay_equal() {
  let (mut one, mut two) = (World::new(FIRST, Seeded::new(5)), World::new(FIRST, Seeded::new(5)));
  for step in 0..300 {
    assert_eq!(brawl_step(&mut one, step), brawl_step(&mut two, step), "step {step}");
    assert_eq!(one.state(), two.state(), "step {step}");
  }
  let ids: Vec<Id> = one.fighters().keys().copied().collect();
  assert_eq!(ids, [Id(1), Id(2), Id(4)]);
  let roles: Vec<u8> = one.fighters().values().map(|fighter| fighter.role).collect();
  assert_eq!(roles, [3, 1, 3]);
}

/// Two fighters that add in either order and shoot in the same steps throw their shots in the
/// order of their ids, and the two worlds stay equal.
#[test]
fn the_order_of_the_ids_rules_each_step_and_not_the_order_of_adding() {
  let duel = |ids: [Id; 2]| {
    let mut world = World::new(FIRST, Seeded::new(3));
    let intents =
      Intents { keys: [false; 5], aim: [900.0, 1700.0], button: true, slot: 2, role: 3 };
    for id in ids {
      assert!(world.add(id, 3) && world.intend(id, intents));
    }
    for _ in 0..8 {
      world.frame();
    }
    world
  };
  let (first, second) = (duel([Id(1), Id(2)]), duel([Id(2), Id(1)]));
  assert_eq!(first.state(), second.state());
  let by: Vec<Id> = first.shots().iter().map(|shot| shot.by).collect();
  assert_eq!(by, [Id(1), Id(2), Id(1), Id(2)]);
}

/// A world with no fighter that starts what each step of another world lists, after the same step,
/// holds the same effects and the same drops.
#[test]
fn a_world_that_starts_what_a_step_lists_holds_the_same_effects_and_drops() {
  let (mut source, mut shown) =
    (World::new(FIRST, Seeded::new(5)), World::new(FIRST, Seeded::new(9)));
  let mut kinds = [0; 3];
  for step in 0..300 {
    let started = brawl_step(&mut source, step);
    shown.frame();
    for start in &started {
      shown.start(start);
      kinds[match start {
        Start::Effect(cue) if cue.early => 0,
        Start::Effect(_) => 1,
        Start::Burst { .. } => 2,
      }] += 1;
    }
    assert_eq!(shown.effects(), source.effects(), "step {step}");
    assert_eq!(shown.drops(), source.drops(), "step {step}");
  }
  assert!(kinds.iter().all(|&count| count > 0), "{kinds:?}");
}

#[test]
fn a_world_adds_a_fighter_once_in_a_role_and_removes_it_with_its_shots_in_flight() {
  let mut world = World::new(FIRST, Seeded::new(1));
  assert!(!world.add(ME, 0) && !world.add(ME, 4));
  assert!(world.add(ME, 2) && !world.add(ME, 1));
  let fighter = me(&world);
  let ([x, y], role, weapon, slot) =
    ([fighter.x, fighter.y], fighter.role, fighter.weapon, fighter.slot);
  assert_eq!(([x, y], role, weapon, slot), (START, 2, 4, 1));
  let intents = Intents { button: true, ..fighter.intents };
  assert!(world.intend(ME, intents));
  for _ in 0..8 {
    world.frame();
  }
  let by: Vec<Id> = world.shots().iter().map(|shot| shot.by).collect();
  assert_eq!(by, [ME]);
  assert!(world.remove(ME) && !world.remove(ME) && !world.intend(ME, intents));
  assert!(world.shots().is_empty() && world.fighters().is_empty());
}

/// A stunned fighter keeps its weapon, and takes the weapon that it asks once its stun ends.
#[test]
fn a_stunned_fighter_takes_the_weapon_that_it_asks_once_its_stun_ends() {
  let mut world = World::new(FIRST, Seeded::new(1));
  assert!(world.add(ME, 2));
  world.state.fighters.get_mut(&ME).expect("the fighter came").stunned = 3;
  let intents = Intents { slot: 2, ..me(&world).intents };
  assert!(world.intend(ME, intents));
  let mut weapons = Vec::new();
  for _ in 0..5 {
    world.frame();
    weapons.push(me(&world).weapon);
  }
  assert_eq!(weapons, [4, 4, 4, 5, 5]);
}

/// The stuck shots keep the last 150, and a shot that lands joins them at the end.
#[test]
fn the_stuck_shots_keep_the_last_150() {
  let mut world = World::new(FIRST, Seeded::new(1));
  assert!(world.add(ME, 1));
  let launch = |serial| Launch {
    x: 400.0,
    y: 1700.0,
    rotation: 90.0,
    attack: 1,
    hand: [0.0; 2],
    serial,
    by: ME,
  };
  let arrow = |serial| Shot::new(Kind::Arrow, &launch(serial), &mut Seeded::new(1)).0;
  world.state.stuck = (1..=150).map(arrow).collect();
  let mut landed = arrow(151);
  landed.landed = true;
  world.state.shots.push(landed);
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
  let splash =
    |count, x| Burst { spray: Spray::Splash, count, at: [x, 1700.0], rotation: 0.0, by: None };
  let embers = Burst { spray: Spray::Embers, ..splash(600, 900.0) };
  let mut drops = Drops::default();
  drops.burst(&embers, 7);
  let second = drops.live()[1].clip.scale_x;
  drops.burst(&Burst { count: 1, ..embers }, 8);
  assert_eq!((drops.live().len(), drops.live()[0].clip.scale_x), (600, second));
  let end = |drops: &mut Drops| {
    for drop in &mut drops.live {
      drop.over = true;
    }
    drops.tick(&FIRST);
  };
  end(&mut drops);
  let second = drops.stamps()[1].clip.scale_x;
  drops.burst(&splash(201, 900.0), 9);
  end(&mut drops);
  assert_eq!((drops.stamps().len(), drops.stamps()[0].clip.scale_x), (800, second));
  assert!(drops.live().is_empty() && drops.stamps().iter().all(|stamp| !stamp.clip.playing()));
  drops.burst(&splash(1, 4921.0), 10);
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
  let mut world = World::new(SQUEEZE, Seeded::new(1));
  assert!(world.add(ME, 1));
  let fighter = world.state.fighters.get_mut(&ME).expect("the fighter came");
  (fighter.x, fighter.y) = (200.0, 290.0);
  world.frame();
  let fighter = me(&world);
  assert_eq!((fighter.x, fighter.y, fighter.touching, fighter.wall), (200.0, 311.0, true, 0));
}
