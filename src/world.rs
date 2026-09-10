//! `World`: orchestrates multiple agents and brokers the one shared,
//! contested resource that exists so far — houses.
//!
//! This is milestone 7's answer to the tension CLAUDE.md flagged back at
//! milestone 4 and deliberately deferred: sharing a resource across
//! multiple agents without `Rc<RefCell<>>`. The answer here is to not
//! actually share anything concurrently. A `House` always has exactly one
//! owner at any moment — either sitting in `World::available_houses`, or
//! moved into some `Agent::home` — and `World` is the single neutral party
//! that brokers the transfer between those two places via plain moves
//! (`Vec::pop`/`push`). There's never a point where two things hold the
//! same `House` at once, so there's nothing to guard with runtime
//! borrow-checking or reference counting.

use crate::{Agent, House};

pub struct World {
    agents: Vec<Agent>,
    /// Houses built but not yet claimed by any agent. The real contention
    /// point: when there are more houseless agents than houses here in a
    /// given tick, some agent goes without — resolved deterministically by
    /// agent order (see `tick`), never randomness.
    available_houses: Vec<House>,
}

impl World {
    pub fn new(agents: Vec<Agent>) -> Self {
        Self {
            agents,
            available_houses: Vec::new(),
        }
    }

    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    /// Advances every agent by one tick, in a fixed order (index 0 first,
    /// same order every tick). That order is the entire contention rule:
    /// before acting, a houseless agent claims a house from the shared
    /// pool if one is waiting; when the pool is scarcer than the demand
    /// for it, whichever agent comes first in `agents` gets it and later
    /// ones don't — deterministic and explainable from agent order, never
    /// randomness. A tick's own Build output isn't claimable until the
    /// next tick, since it's only added to the pool after every agent this
    /// tick has already had its turn to claim.
    pub fn tick(&mut self) {
        for agent in &mut self.agents {
            if agent.home().is_none()
                && let Some(house) = self.available_houses.pop()
            {
                agent.claim_house(house);
            }
            if let Some(house) = agent.tick() {
                self.available_houses.push(house);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Career;

    fn houseless_builder(name: &'static str) -> Agent {
        let mut agent = Agent::new(name);
        agent.set_career(Career::Builder);
        agent
    }

    #[test]
    fn building_deposits_a_house_into_the_shared_pool_not_the_builder() {
        let mut world = World::new(vec![houseless_builder("Builder")]);
        world.tick(); // neither need is due yet, so this tick: Produce(Build)
        assert_eq!(world.available_houses.len(), 1);
        assert!(world.agents()[0].home().is_none());
    }

    #[test]
    fn a_built_house_is_claimed_by_a_houseless_agent_on_a_later_tick() {
        let mut world = World::new(vec![houseless_builder("Builder")]);
        world.tick(); // builds a house, deposited into the pool
        world.tick(); // this tick: claims from the pool before acting
        assert!(world.agents()[0].home().is_some());
    }

    #[test]
    fn a_scarce_house_goes_to_the_first_houseless_agent_in_order() {
        let mut world = World::new(vec![Agent::new("A"), Agent::new("B")]);
        world.available_houses.push(House::new());
        world.tick();
        assert!(world.agents()[0].home().is_some());
        assert!(world.agents()[1].home().is_none());
    }

    #[test]
    fn an_agent_that_already_owns_a_house_does_not_take_from_the_pool() {
        let mut agent = Agent::new("A");
        agent.claim_house(House::new());
        let mut world = World::new(vec![agent]);
        world.available_houses.push(House::new());
        world.tick();
        assert_eq!(world.available_houses.len(), 1); // untouched
    }
}
