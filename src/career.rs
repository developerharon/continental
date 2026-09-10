//! `Career`: an agent's job, which gates which production action (if any)
//! is available to it.
//!
//! Milestone 5 only models this gating — it doesn't wire a production
//! action into the decision loop, and there's no farming/building logic
//! yet (no food, no built houses, nothing in `Agent::tick`). That's
//! milestone 6's job. This step is the same shape as milestone 3's House:
//! introduce the state, without pretending to have solved a later step.

/// An agent's job. Determines which `ProductionAction`, if any, is
/// available via `production_action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Career {
    /// No job yet — no production action available. The default, since
    /// nothing currently grants an agent a career at construction.
    #[default]
    Unemployed,
    Farmer,
    Builder,
}

/// A production action a `Career` unlocks. Not yet wired into
/// `Agent::tick` or given any effect on state — milestone 6 is what
/// actually makes farmers produce food and builders produce houses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionAction {
    Farm,
    Build,
}

impl Career {
    /// Which production action this career gates access to, if any.
    #[must_use]
    pub const fn production_action(self) -> Option<ProductionAction> {
        match self {
            Self::Unemployed => None,
            Self::Farmer => Some(ProductionAction::Farm),
            Self::Builder => Some(ProductionAction::Build),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unemployed_has_no_production_action() {
        assert_eq!(Career::Unemployed.production_action(), None);
    }

    #[test]
    fn farmer_can_farm() {
        assert_eq!(
            Career::Farmer.production_action(),
            Some(ProductionAction::Farm)
        );
    }

    #[test]
    fn builder_can_build() {
        assert_eq!(
            Career::Builder.production_action(),
            Some(ProductionAction::Build)
        );
    }

    #[test]
    fn unemployed_is_the_default() {
        assert_eq!(Career::default(), Career::Unemployed);
    }
}
