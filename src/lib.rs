//! Continental's core simulation logic, kept separate from rendering so the
//! decision loop can be unit tested without a window. `src/main.rs` is a
//! thin macroquad UI on top of this crate's public API — it adds no
//! decision logic of its own.

mod agent;
mod house;

pub use agent::{Agent, ENERGY_MAX, HUNGER_MAX};
pub use house::House;
