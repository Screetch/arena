use std::collections::BTreeSet;

use arena_sim::map::FIRST;
use arena_sim::moves::{Burst, Chance, Id, Intents, Key, Seeded, Spray, Start, World};

use super::{
  Answer, Client, Error, Hello, Refusal, STATE_EVERY, Server, Shared, State, Steps, VERSION,
  Welcome,
};

/// The fighters of a brawl.
const FIGHTERS: u32 = 10;

/// The most bytes of the message of a state of a brawl, which [`FIGHTERS`] fighters that each fire
/// gave.
const MOST: usize = 15_902;

/// The role of the fighter `id`.
fn role(id: u32) -> u8 {
  u8::try_from(id % 3 + 1).expect("a role is under 4")
}

/// The intents of the fighter `id` at `step` of a brawl: it runs from side to side, jumps, plays
/// its special, takes each weapon of its role in turn, and fires.
fn intents(id: u32, step: u32) -> Intents {
  let mut keys = [false; 5];
  let run = (step + id * 7) % 80;
  keys[Key::Right as usize] = run < 30;
  keys[Key::Left as usize] = run >= 50;
  keys[Key::Up as usize] = (step + id).is_multiple_of(45);
  keys[Key::Special as usize] = (step + id * 3) % 70 < 3;
  Intents {
    keys,
    aim: [400.0 + f64::from(id) * 380.0, 1400.0 + f64::from(id % 3) * 100.0],
    button: (step + id) % 16 < 8,
    slot: u8::try_from((step / 60 + id) % 3 + 1).expect("a key of a weapon is under 4"),
    role: role(id),
  }
}

/// Plays the step `step` of a brawl, in which the fighter 2 throws embers each 50 steps, and gives
/// what it started.
fn play(world: &mut World<Seeded>, step: u32) -> Vec<Start> {
  if step % 50 == 25 {
    let at = [900.0, 1600.0];
    world.burst(Burst { spray: Spray::Embers, count: 5, at, rotation: 0.0, by: Some(Id(2)) });
  }
  for id in 1..=FIGHTERS {
    assert!(world.intend(Id(id), intents(id, step)));
  }
  world.frame()
}

/// A world of [`FIGHTERS`] fighters after `steps` steps of a brawl, and what each step started.
fn brawl(steps: u32) -> (World<Seeded>, Vec<Vec<Start>>) {
  let mut world = World::new(FIRST, Seeded::new(11));
  for id in 1..=FIGHTERS {
    assert!(world.add(Id(id), role(id)));
  }
  let starts = (0..steps).map(|step| play(&mut world, step)).collect();
  (world, starts)
}

/// The shared part of the state of `world`, with what its last [`STATE_EVERY`] steps started.
fn shared(world: &World<Seeded>, starts: &[Vec<Start>]) -> Shared {
  let last = starts.len() - STATE_EVERY as usize;
  Shared { world: world.state().clone(), starts: starts[last..].to_vec() }
}

/// A message of each kind, and of each answer to a hello.
fn messages() -> (Vec<Client>, Vec<Server>) {
  let (world, starts) = brawl(200);
  assert!(!world.shots().is_empty() && !world.stuck().is_empty());
  let started = starts.concat();
  assert!(started.iter().any(|start| matches!(start, Start::Effect(_))));
  assert!(started.iter().any(|start| matches!(start, Start::Burst { .. })));
  let shared = Shared { world: world.state().clone(), starts };
  let clients = vec![
    Client::Hello(Hello { version: VERSION, role: 2 }),
    Client::Intents(Steps { sequence: 7, intents: [5, 6, 7].map(|step| intents(4, step)) }),
  ];
  let welcome = |answer| Server::Welcome(Welcome { version: VERSION, answer });
  let servers = vec![
    welcome(Answer::Joined { fighter: Id(3), step: 200 }),
    welcome(Answer::Refused(Refusal::Version)),
    welcome(Answer::Refused(Refusal::Role)),
    Server::State(State { applied: 9, shared }),
  ];
  (clients, servers)
}

#[test]
fn each_message_encodes_and_decodes_to_an_equal_value() {
  let (clients, servers) = messages();
  for message in clients {
    assert_eq!(Client::decode(&message.encode()), Ok(message));
  }
  for message in servers {
    assert_eq!(Server::decode(&message.encode()), Ok(message));
  }
}

#[test]
fn the_shared_part_that_the_server_encodes_once_gives_the_state_of_each_client() {
  let (world, starts) = brawl(60);
  let shared = shared(&world, &starts);
  let encoded = shared.encode();
  for applied in [0, 1, 58, u32::MAX] {
    let message = Server::State(State { applied, shared: shared.clone() });
    assert_eq!(encoded.state(applied), message.encode(), "{applied}");
  }
}

#[test]
fn a_hello_or_a_welcome_of_another_version_gives_that_version_and_no_more() {
  let other = VERSION + 1;
  let mut hello = Client::Hello(Hello { version: other, role: 1 }).encode();
  let answer = Answer::Refused(Refusal::Version);
  let mut welcome = Server::Welcome(Welcome { version: other, answer }).encode();
  for bytes in [&mut hello, &mut welcome] {
    bytes.extend([0xff; 4]);
  }
  assert_eq!(Client::decode(&hello), Err(Error::Version(other)));
  assert_eq!(Server::decode(&welcome), Err(Error::Version(other)));
}

#[test]
fn a_world_rebuilt_from_a_decoded_state_steps_as_the_world_of_the_server() {
  let (mut server, starts) = brawl(150);
  let message = shared(&server, &starts).encode().state(150);
  let shared = match Server::decode(&message) {
    Ok(Server::State(State { applied: 150, shared })) => shared,
    other => panic!("{other:?}"),
  };
  assert_eq!(shared.world.step(), 150);
  let mut client = World::from_state(FIRST, shared.world);
  assert_eq!(client.state(), server.state());
  for step in 150..300 {
    assert_eq!(play(&mut client, step), play(&mut server, step), "step {step}");
    assert_eq!(client.state(), server.state(), "step {step}");
  }
  assert_eq!(client.fighters().len(), 10);
}

/// The message of the largest state of a brawl, in which each fighter fires until the shots stuck
/// in the map reach their most, stays under [`MOST`] bytes.
#[test]
fn the_state_of_10_fighters_that_each_fire_stays_under_its_bound() {
  let (mut world, mut starts) = brawl(0);
  let (mut most, mut fired) = (0, BTreeSet::new());
  for step in 0..1600 {
    starts.push(play(&mut world, step));
    fired.extend(world.shots().iter().map(|shot| shot.by));
    if starts.len() >= STATE_EVERY as usize {
      most = most.max(shared(&world, &starts).encode().state(step).len());
    }
  }
  assert_eq!(fired, (1..=FIGHTERS).map(Id).collect());
  assert_eq!(world.stuck().len(), 150);
  assert!(most <= MOST, "the largest state holds {most} bytes, over {MOST}");
}

/// A whole number from 0 to `end`, which it never gives.
#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_precision_loss,
  clippy::cast_sign_loss,
  reason = "a chance from 0 to 1, which it never gives, times a small length is a whole index"
)]
fn below(chance: &mut Seeded, end: usize) -> usize {
  (chance.chance() * end as f64) as usize
}

fn byte(chance: &mut Seeded) -> u8 {
  u8::try_from(below(chance, 256)).expect("a byte is under 256")
}

/// A decoder of one side, which gives no message.
type Decode = fn(&[u8]) -> Result<(), Error>;

fn client(bytes: &[u8]) -> Result<(), Error> {
  Client::decode(bytes).map(drop)
}

fn server(bytes: &[u8]) -> Result<(), Error> {
  Server::decode(bytes).map(drop)
}

/// Decodes `bytes` as each side does. A test that calls it with bytes that it does not know fails
/// only when a decoder panics.
fn either(bytes: &[u8]) -> [bool; 2] {
  [client(bytes).is_ok(), server(bytes).is_ok()]
}

#[test]
fn a_decoder_gives_an_error_and_never_panics_on_bytes_that_are_not_a_message() {
  let (clients, servers) = messages();
  let mut chance = Seeded::new(29);
  let samples = clients
    .iter()
    .map(|message| (message.encode(), client as Decode))
    .chain(servers.iter().map(|message| (message.encode(), server as Decode)));
  for (bytes, decode) in samples {
    assert_eq!(decode(&bytes), Ok(()));
    let mut longer = bytes.clone();
    longer.push(0);
    assert_eq!(decode(&longer), Err(Error::Trailing(1)));
    for _ in 0..200 {
      let cut = below(&mut chance, bytes.len());
      let end = Err(Error::Bytes(postcard::Error::DeserializeUnexpectedEnd));
      assert_eq!(decode(&bytes[..cut]), end, "cut at {cut} of {}", bytes.len());
      let mut changed = bytes.clone();
      changed[below(&mut chance, bytes.len())] ^= byte(&mut chance) | 1;
      either(&changed);
    }
  }
  for _ in 0..2000 {
    let length = below(&mut chance, 300);
    let bytes: Vec<u8> = (0..length).map(|_| byte(&mut chance)).collect();
    either(&bytes);
  }
}
