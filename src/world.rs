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
//!
//! `World` also holds the one `Restaurant` — unlike a `House`, it's never
//! claimed or moved, just a shared position every agent senses each tick
//! (see `Agent::tick`) so Eat knows where to walk to — and every
//! `Workplace`, one per `ProductionAction`, looked up per agent by its own
//! career so Produce knows where to walk to as well.

use crate::{Agent, House, Restaurant, Workplace};

pub struct World {
    agents: Vec<Agent>,
    /// Houses built but not yet claimed by any agent. The real contention
    /// point: when there are more houseless agents than houses here in a
    /// given tick, some agent goes without — resolved deterministically by
    /// agent order (see `tick`), never randomness.
    available_houses: Vec<House>,
    /// The shared place every agent eats at. One for now — the user asked
    /// for shared eating, not multiple restaurants; a `Vec<Restaurant>` is
    /// a small, separate extension if that's ever wanted.
    restaurant: Restaurant,
    /// Where each `ProductionAction` happens — a Farm tile, a construction
    /// site, and so on. A `Vec` rather than one named field per career so
    /// `World` doesn't need new fields or new `tick` logic if a third
    /// production action ever exists; `tick` just looks up whichever one
    /// matches each agent's own career.
    workplaces: Vec<Workplace>,
}

impl World {
    #[must_use]
    pub const fn new(
        agents: Vec<Agent>,
        restaurant: Restaurant,
        workplaces: Vec<Workplace>,
    ) -> Self {
        Self {
            agents,
            available_houses: Vec::new(),
            restaurant,
            workplaces,
        }
    }

    #[must_use]
    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    #[must_use]
    pub const fn restaurant(&self) -> &Restaurant {
        &self.restaurant
    }

    #[must_use]
    pub fn workplaces(&self) -> &[Workplace] {
        &self.workplaces
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
            // Each agent gets the position of whichever Workplace matches
            // its own production action, if any — `None` if it has no
            // career, or if nothing in `workplaces` offers what its career
            // unlocks (see `Agent::required_location`'s fallback).
            let workplace_position = self
                .workplaces
                .iter()
                .find(|workplace| agent.available_production_action() == Some(workplace.action()))
                .map(Workplace::position);
            if let Some(house) = agent.tick(self.restaurant.position(), workplace_position) {
                self.available_houses.push(house);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Career, ProductionAction};

    /// Arbitrary shared restaurant position for tests that don't care
    /// where it is.
    const RESTAURANT_POS: (i32, i32) = (0, 0);

    fn houseless_builder(name: &'static str) -> Agent {
        let mut agent = Agent::new(name, RESTAURANT_POS);
        agent.set_career(Career::Builder);
        agent
    }

    #[test]
    fn building_deposits_a_house_into_the_shared_pool_not_the_builder() {
        let mut world = World::new(
            vec![houseless_builder("Builder")],
            Restaurant::new(RESTAURANT_POS),
            vec![Workplace::new(RESTAURANT_POS, ProductionAction::Build)],
        );
        world.tick(); // neither need is due yet, so this tick: Produce(Build)
        assert_eq!(world.available_houses.len(), 1);
        assert!(world.agents()[0].home().is_none());
    }

    #[test]
    fn a_built_house_is_claimed_by_a_houseless_agent_on_a_later_tick() {
        let mut world = World::new(
            vec![houseless_builder("Builder")],
            Restaurant::new(RESTAURANT_POS),
            vec![Workplace::new(RESTAURANT_POS, ProductionAction::Build)],
        );
        world.tick(); // builds a house, deposited into the pool
        world.tick(); // this tick: claims from the pool before acting
        assert!(world.agents()[0].home().is_some());
    }

    #[test]
    fn a_scarce_house_goes_to_the_first_houseless_agent_in_order() {
        let mut world = World::new(
            vec![
                Agent::new("A", RESTAURANT_POS),
                Agent::new("B", RESTAURANT_POS),
            ],
            Restaurant::new(RESTAURANT_POS),
            vec![],
        );
        world.available_houses.push(House::new(RESTAURANT_POS));
        world.tick();
        assert!(world.agents()[0].home().is_some());
        assert!(world.agents()[1].home().is_none());
    }

    #[test]
    fn an_agent_that_already_owns_a_house_does_not_take_from_the_pool() {
        let mut agent = Agent::new("A", RESTAURANT_POS);
        agent.claim_house(House::new(RESTAURANT_POS));
        let mut world = World::new(vec![agent], Restaurant::new(RESTAURANT_POS), vec![]);
        world.available_houses.push(House::new(RESTAURANT_POS));
        world.tick();
        assert_eq!(world.available_houses.len(), 1); // untouched
    }

    #[test]
    fn world_gives_each_agent_the_workplace_matching_its_own_career() {
        let mut farmer = Agent::new("Farmer", (0, 0));
        farmer.set_career(Career::Farmer);
        let mut builder = Agent::new("Builder", (0, 0));
        builder.set_career(Career::Builder);

        // Placed on different axes so each agent's very first step reveals
        // which workplace it's actually heading toward.
        let farm_position = (0, 5);
        let construction_position = (5, 0);
        let mut world = World::new(
            vec![farmer, builder],
            Restaurant::new(RESTAURANT_POS),
            vec![
                Workplace::new(farm_position, ProductionAction::Farm),
                Workplace::new(construction_position, ProductionAction::Build),
            ],
        );

        world.tick(); // neither survival need is due yet: both walk to work
        assert_eq!(world.agents()[0].position(), (0, 1)); // farmer, toward the farm
        assert_eq!(world.agents()[1].position(), (1, 0)); // builder, toward the construction site
    }
}
