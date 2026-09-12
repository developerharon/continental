//! `Restaurant`: a shared world object where any agent can Eat.
//!
//! Unlike `House`, a `Restaurant` is never owned or claimed by any agent —
//! there's no contention over it the way `World` brokers houses. It's just
//! a fixed grid position that Eat requires an agent to be at (see
//! `Agent::required_location`), reachable by every agent regardless of
//! whose turn it is. `World` holds exactly one for now — the user asked
//! for shared eating, not multiple restaurants; adding more is a small,
//! separate extension if it's ever wanted.

pub struct Restaurant {
    position: (i32, i32),
}

impl Restaurant {
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
    fn restaurant_reports_its_position() {
        let restaurant = Restaurant::new((4, 4));
        assert_eq!(restaurant.position(), (4, 4));
    }
}
