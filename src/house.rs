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
//!
//! `position` was added once movement made "where" meaningful: Rest now
//! requires the agent to actually be at its own house's position (see
//! `Agent::required_location`), not just to own a house. A built house is
//! positioned wherever its builder stood at the time (see `Agent::act`).

/// A world object an agent can own, and a place on the grid Rest requires
/// the owning agent to actually be at.
pub struct House {
    position: (i32, i32),
}

impl House {
    #[must_use]
    pub const fn new(position: (i32, i32)) -> Self {
        Self { position }
    }

    #[must_use]
    pub const fn position(&self) -> (i32, i32) {
        self.position
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn house_reports_the_position_it_was_built_at() {
        let house = House::new((3, 5));
        assert_eq!(house.position(), (3, 5));
    }
}
