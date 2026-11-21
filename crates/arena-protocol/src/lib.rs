//! The messages between a client and the server of arena, and their codec.
//!
//! The server is the one authority of each game. A client sends the intents of its player, never a
//! fact of the world, and the server sends the state of the world. The messages travel as the
//! binary messages of a WebSocket, the same for the native client and for the web client, and this
//! crate holds no socket.
//!
//! The world steps at [`FPS`] steps a second on the server, which sends a state after each
//! [`STATE_EVERY`] steps. Each state is whole and does not depend on an earlier one, so a state
//! that comes late gives way to the next.
//!
//! The codec is binary and compact, and keeps each `f64` bit for bit, so a client rebuilds exactly
//! the state of the server. A decoder gives an [`Error`] on bytes that are not a message, and never
//! panics.
//!
//! [`FPS`]: arena_sim::clip::FPS

use arena_sim::moves::{self, Id, Intents, Seeded, Start};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The version of the protocol. It comes first in a hello and in a welcome, and a side that
/// decodes one of another version gives [`Error::Version`].
pub const VERSION: u16 = 1;

/// The port on which the server listens.
pub const PORT: u16 = 7800;

/// The steps of the world after which the server sends a state.
pub const STATE_EVERY: u32 = 2;

/// The steps of intents that each message of intents holds, so that one late message loses none.
pub const REPEATS: usize = 3;

/// The variant of [`Client::Hello`] and of [`Server::Welcome`], which each version keeps first.
const GREETING: u32 = 0;

/// The variant of [`Server::State`].
const STATE: u32 = 1;

/// A message from a client to the server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Client {
  /// The first message of a client.
  Hello(Hello),
  /// The intents of its player.
  Intents(Steps),
}

/// The first message of a client: the version of its protocol, and the role that its player
/// chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
  /// The version of the protocol of the client.
  pub version: u16,
  /// The role of the fighter of the player, from 1 to 3.
  pub role: u8,
}

/// The intents of the last [`REPEATS`] steps of a player, the oldest first. The last is the
/// intents of the step `sequence`, and each one before it of the step before. A client numbers its
/// steps from 1, and gives the intents of its first step for each step before it. The server
/// applies each step after the last that it applied, so a late message loses nothing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Steps {
  /// The sequence of the last step.
  pub sequence: u32,
  /// The intents of each step.
  pub intents: [Intents; REPEATS],
}

/// A message from the server to a client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Server {
  /// The answer to a hello.
  Welcome(Welcome),
  /// The state of the world.
  State(State),
}

/// The answer of the server to a hello: the version of its protocol, then its answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
  /// The version of the protocol of the server.
  pub version: u16,
  /// The fighter of the player, or why the server refuses it.
  pub answer: Answer,
}

/// What the server answers to a hello.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Answer {
  /// The player plays the fighter `fighter`, and the world stands at the step `step`.
  Joined {
    /// The fighter of the player.
    fighter: Id,
    /// The step of the world.
    step: u64,
  },
  /// The server refuses the player.
  Refused(Refusal),
}

/// Why the server refuses a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
  /// The client speaks another version of the protocol.
  Version,
  /// The role lies out of 1 to 3.
  Role,
}

/// The state of the world for one client: the part that differs for each client, then the part
/// that is the same for each, which [`Shared::encode`] encodes once for all.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
  /// The sequence of the last step of intents of this client that the server applied.
  pub applied: u32,
  /// The part of the state that is the same for each client.
  pub shared: Shared,
}

/// What each client needs to show the same world as the server. A fighter that is no longer in a
/// state left the game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shared {
  /// The state of the world after its last step, with its fighters and its shots.
  pub world: moves::State<Seeded>,
  /// What each step since the last state started, the oldest first, and last what the step of
  /// `world` started. A client plays them as [`World::start`] says, since the effects and the
  /// drops travel only as these starts.
  ///
  /// [`World::start`]: arena_sim::moves::World::start
  pub starts: Vec<Vec<Start>>,
}

/// The bytes of a [`Shared`], which give the message of each client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoded(Vec<u8>);

/// What a decoder found in bytes that are not a message.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
  /// The bytes hold no message.
  #[error("the bytes hold no message: {0}")]
  Bytes(#[from] postcard::Error),
  /// Bytes follow the message.
  #[error("{0} bytes follow the message")]
  Trailing(usize),
  /// A hello or a welcome holds another version of the protocol, and the side that decoded it
  /// reads nothing more.
  #[error("the other side speaks the version {0} of the protocol")]
  Version(u16),
}

impl Client {
  /// The bytes of the message.
  pub fn encode(&self) -> Vec<u8> {
    bytes(self)
  }

  /// The message of `bytes`.
  pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
    decode(bytes)
  }
}

impl Server {
  /// The bytes of the message.
  pub fn encode(&self) -> Vec<u8> {
    bytes(self)
  }

  /// The message of `bytes`.
  pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
    decode(bytes)
  }
}

impl Shared {
  /// The bytes of the shared part, which the server encodes once for each state that it sends.
  pub fn encode(&self) -> Encoded {
    Encoded(bytes(self))
  }
}

impl Encoded {
  /// The bytes of the message [`Server::State`] of this shared part, for a client of which the
  /// server applied the steps of intents up to `applied`.
  pub fn state(&self, applied: u32) -> Vec<u8> {
    let mut message = bytes(&(STATE, applied));
    message.extend_from_slice(&self.0);
    message
  }
}

#[expect(
  clippy::expect_used,
  reason = "a message of this crate gives the length of each of its sequences, so it encodes"
)]
fn bytes(value: &impl Serialize) -> Vec<u8> {
  postcard::to_allocvec(value).expect("a message gives the length of each of its sequences")
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
  if let Ok((GREETING, rest)) = postcard::take_from_bytes::<u32>(bytes)
    && let Ok((version, _)) = postcard::take_from_bytes::<u16>(rest)
    && version != VERSION
  {
    return Err(Error::Version(version));
  }
  match postcard::take_from_bytes(bytes)? {
    (message, []) => Ok(message),
    (_, rest) => Err(Error::Trailing(rest.len())),
  }
}

#[cfg(test)]
mod tests;
