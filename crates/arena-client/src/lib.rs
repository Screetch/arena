//! The client of arena.
//!
//! It draws the world only as characters: each cell of the screen measures the part of the map,
//! of the fighter and of its effects under it, and shows one character of [`cells::RAMP`], with
//! its colour and its opacity.

pub mod cells;
pub mod font;
#[cfg(test)]
mod gpu;
pub mod paint;
pub mod play;
mod sample;
pub mod screen;
mod shapes;
pub mod sprites;
pub mod view;
