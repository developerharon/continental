//! `Workplace`: where Produce actually happens.
//!
//! Like `Restaurant`, a `Workplace` is never owned or claimed — any agent
//! whose career unlocks the matching `ProductionAction` can use it. Unlike
//! `Restaurant` there isn't just one: each `ProductionAction` gets its own
//! (a Farm tile, a construction site, ...), so `World` holds a `Vec` of
//! these rather than a single instance — see `World::tick`, which looks up
//! the one matching each agent's own career before ticking it.

use crate::ProductionAction;

pub struct Workplace {
    position: (i32, i32),
    action: ProductionAction,
}

impl Workplace {
    #[must_use]
    pub const fn new(position: (i32, i32), action: ProductionAction) -> Self {
        Self { position, action }
    }

    #[must_use]
    pub const fn position(&self) -> (i32, i32) {
        self.position
    }

    #[must_use]
    pub const fn action(&self) -> ProductionAction {
        self.action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workplace_reports_its_position_and_action() {
        let workplace = Workplace::new((6, 2), ProductionAction::Farm);
        assert_eq!(workplace.position(), (6, 2));
        assert_eq!(workplace.action(), ProductionAction::Farm);
    }
}
