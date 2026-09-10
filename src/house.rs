//! `House`: the first world object an agent can own.
//!
//! Milestone 3 only introduces the type and gives an agent one to own — it
//! doesn't gate any behavior on that ownership yet (e.g. Rest still doesn't
//! care whether the agent has a house). That's milestone 4's job, and
//! deliberately so: this step keeps ownership in its plainest Rust shape (a
//! `House` lives directly on the `Agent` that owns it, via a normal owned
//! field), so the real ownership/borrowing tension milestone 4 is meant to
//! surface — sharing a house safely once there's more than one agent and
//! more than one house — shows up honestly there instead of being
//! pre-solved here with structure it doesn't need yet.

/// A world object an agent can own. No fields yet — nothing reads any
/// property of a house yet, so there's nothing to add until a later
/// milestone actually needs it (e.g. capacity, or which agent owns it once
/// houses can be shared/contested).
pub struct House;

impl House {
    pub fn new() -> Self {
        Self
    }
}

impl Default for House {
    fn default() -> Self {
        Self::new()
    }
}
