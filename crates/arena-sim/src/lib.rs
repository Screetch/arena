//! The shared simulation of arena.
//!
//! It is deterministic and depends on no window, no graphics adapter and no network, so the server
//! and each client run the same code.

pub mod clip;
pub mod map;
pub mod moves;
