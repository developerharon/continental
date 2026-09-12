//! Continental's core simulation logic, kept separate from rendering so the
//! decision loop can be unit tested without a window. `src/main.rs` is a
//! thin macroquad UI on top of this crate's public API — it adds no
//! decision logic of its own.

mod agent;
mod career;
mod house;
mod restaurant;
mod workplace;
mod world;

pub use agent::{Agent, ENERGY_MAX, HUNGER_MAX};
pub use career::{Career, ProductionAction};
pub use house::House;
pub use restaurant::Restaurant;
pub use workplace::Workplace;
pub use world::World;
