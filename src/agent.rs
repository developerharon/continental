//! `Agent` and the standing decision-loop convention: every tick runs
//! sense -> evaluate -> select -> act -> replan. Needs are plain numbers
//! that decay over time; priority between them is decided by comparing or
//! scoring those numbers — never by randomness.
//!
//! `select` (milestone 8) picks the winning action via an ordered list of
//! condition-action rules rather than a flat match — see its docs. `act`
//! (milestone 6) can also produce a house, which it hands back to the
//! caller instead of claiming for itself — see `World` (milestone 7) for
//! why: sharing a house across multiple agents is brokered there, not here.

use crate::{Career, House, ProductionAction};

/// How much hunger accumulates per tick if the agent doesn't eat.
const HUNGER_DECAY_PER_TICK: f32 = 5.0;
/// Hunger level at or above which eating becomes the priority.
const HUNGER_EAT_THRESHOLD: f32 = 60.0;
/// How much a single Eat action reduces hunger by.
const HUNGER_EAT_RELIEF: f32 = 40.0;
/// Hunger is clamped to this range; 0 = fully fed, 100 = starving. Public so
/// callers (e.g. the UI) can scale a 0-100 display against the same value
/// the sim uses, instead of duplicating the number.
pub const HUNGER_MAX: f32 = 100.0;

/// How much energy drains per tick if the agent doesn't rest.
const ENERGY_DECAY_PER_TICK: f32 = 4.0;
/// Energy *urgency* (see `Urgency`) at or above which resting becomes the
/// priority — this is in urgency space, so it corresponds to energy having
/// dropped to `ENERGY_MAX - ENERGY_REST_THRESHOLD` (30.0) or below.
const ENERGY_REST_THRESHOLD: f32 = 70.0;
/// How much a single Rest action restores energy by.
const ENERGY_REST_RELIEF: f32 = 60.0;
/// Energy is clamped to this range; 0 = exhausted, 100 = fully rested. Public
/// for the same reason as `HUNGER_MAX`: so callers can scale a display
/// against the sim's own value instead of duplicating the number.
pub const ENERGY_MAX: f32 = 100.0;

/// How much a single Farm action adds to the agent's food stock. Arbitrary
/// for now — nothing consumes food yet, so there's no balance to tune
/// against. Food isn't clamped to a max: it's a stockpile, not a 0-100 need.
const FOOD_PRODUCED_PER_TICK: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Eat,
    Rest,
    /// Do the job this agent's career allows, if any (see
    /// `Agent::available_production_action`).
    Produce(ProductionAction),
    Idle,
}

/// Per-need urgency, both put on the same 0-100 "higher = more urgent"
/// scale so they can be compared directly in `select`. Hunger's urgency is
/// the raw hunger value; energy's is inverted (`ENERGY_MAX - energy`) since
/// low energy, not high, is what's bad.
struct Urgency {
    hunger: f32,
    energy: f32,
}

pub struct Agent {
    name: &'static str,
    hunger: f32,
    energy: f32,
    /// The house this agent owns, if any. Agents start without one —
    /// ownership has to be granted via `claim_house`, so there's a real
    /// "doesn't own a house" state for the ownership constraint in
    /// `select` to mean something, rather than every agent trivially
    /// owning one from birth as milestone 3 had it.
    home: Option<House>,
    /// The agent's job. Gates which production action is available via
    /// `available_production_action`, which `select` (milestone 6) uses
    /// to fall back to work instead of Idle when neither need is due.
    career: Career,
    /// Food stockpiled by Farm actions. Purely a stockpile so far —
    /// nothing reads it back to satisfy hunger; that's not part of any
    /// milestone yet.
    food: f32,
}

impl Agent {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            hunger: 0.0,
            energy: ENERGY_MAX,
            home: None,
            career: Career::default(),
            food: 0.0,
        }
    }

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn hunger(&self) -> f32 {
        self.hunger
    }

    pub fn energy(&self) -> f32 {
        self.energy
    }

    pub fn food(&self) -> f32 {
        self.food
    }

    pub fn home(&self) -> Option<&House> {
        self.home.as_ref()
    }

    /// Grants this agent ownership of `house`. Direct, single-owner Rust
    /// ownership — `house` moves onto this `Agent` as a plain field, no
    /// `Rc`/`RefCell`. `World` (milestone 7) is what actually brokers
    /// houses between multiple agents; `Agent` itself never needs to know
    /// about any other agent to do that safely — it just accepts a house
    /// it's handed, one owner at a time.
    pub fn claim_house(&mut self, house: House) {
        self.home = Some(house);
    }

    pub fn career(&self) -> Career {
        self.career
    }

    pub fn set_career(&mut self, career: Career) {
        self.career = career;
    }

    /// Which production action this agent's career currently gates access
    /// to, if any. `select` uses this to fall back to work when neither
    /// survival need is due.
    pub fn available_production_action(&self) -> Option<ProductionAction> {
        self.career.production_action()
    }

    /// sense: read current state — the raw hunger and energy values.
    fn sense(&self) -> (f32, f32) {
        (self.hunger, self.energy)
    }

    /// evaluate: turn sensed state into per-need urgency scores that are
    /// directly comparable to each other (see `Urgency`).
    fn evaluate(&self, hunger: f32, energy: f32) -> Urgency {
        Urgency {
            hunger,
            energy: ENERGY_MAX - energy,
        }
    }

    /// select: pick the winning action via an ordered list of
    /// condition-action rules — the first rule whose condition holds
    /// fires, and later rules are never evaluated. This (milestone 8)
    /// replaces the flat match earlier milestones used with something
    /// closer to SOAR-style production rules: an explicit, inspectable,
    /// in-order list instead of a bespoke priority formula. Still fully
    /// deterministic — rule order is the only tiebreak, never randomness —
    /// and behaviorally identical to what it replaced.
    ///
    /// This does *not* attempt the rest of what "SOAR-inspired" could
    /// mean — a real working-memory fact store, impasses/subgoaling,
    /// chunking. Those are a much bigger, speculative undertaking that
    /// this milestone's one-line stretch-goal description doesn't justify
    /// on its own; if actually wanted, that's worth discussing first
    /// rather than backing into silently here.
    ///
    /// Ownership constraint: Rest is only ever a candidate if the agent
    /// owns a house (`self.home.is_some()`) — an agent that hasn't claimed
    /// one can be as exhausted as it likes and will never select Rest.
    fn select(&self, urgency: Urgency) -> Action {
        type Rule = fn(&Agent, &Urgency) -> Option<Action>;

        const RULES: &[Rule] = &[
            // Both survival needs due at once: higher urgency wins, hunger
            // on a tie — a direct comparison, never a coin flip.
            |agent, u| {
                let hunger_due = u.hunger >= HUNGER_EAT_THRESHOLD;
                let energy_due = u.energy >= ENERGY_REST_THRESHOLD && agent.home.is_some();
                (hunger_due && energy_due).then_some(if u.hunger >= u.energy {
                    Action::Eat
                } else {
                    Action::Rest
                })
            },
            // Hunger alone is due.
            |_, u| (u.hunger >= HUNGER_EAT_THRESHOLD).then_some(Action::Eat),
            // Energy alone is due, and there's a house to rest in.
            |agent, u| {
                (u.energy >= ENERGY_REST_THRESHOLD && agent.home.is_some()).then_some(Action::Rest)
            },
            // Neither survival need is pressing: do the job, if there is one.
            |agent, _| agent.available_production_action().map(Action::Produce),
        ];

        RULES
            .iter()
            .find_map(|rule| rule(self, &urgency))
            .unwrap_or(Action::Idle)
    }

    /// act: apply the selected action's effect on this agent's own state,
    /// and hand back anything it produced that isn't this agent's alone to
    /// keep — currently just a built house. `act` never claims a produced
    /// house for itself; the caller (e.g. `World`) decides where it goes.
    fn act(&mut self, action: Action) -> Option<House> {
        match action {
            Action::Eat => {
                self.hunger = (self.hunger - HUNGER_EAT_RELIEF).max(0.0);
                None
            }
            Action::Rest => {
                self.energy = (self.energy + ENERGY_REST_RELIEF).min(ENERGY_MAX);
                None
            }
            Action::Produce(ProductionAction::Farm) => {
                self.food += FOOD_PRODUCED_PER_TICK;
                None
            }
            Action::Produce(ProductionAction::Build) => Some(House::new()),
            Action::Idle => None,
        }
    }

    /// replan: let needs progress (decay) so the next tick senses fresh
    /// state instead of acting on a stale plan.
    fn replan(&mut self) {
        self.hunger = (self.hunger + HUNGER_DECAY_PER_TICK).min(HUNGER_MAX);
        self.energy = (self.energy - ENERGY_DECAY_PER_TICK).max(0.0);
    }

    /// Runs one full sense -> evaluate -> select -> act -> replan cycle.
    /// Returns a house if this tick's action produced one — `Agent` has no
    /// way to know whether it needs one or another agent does, so it never
    /// keeps it; the caller is responsible for placing it (see `World`).
    pub fn tick(&mut self) -> Option<House> {
        let (sensed_hunger, sensed_energy) = self.sense();
        let food_before = self.food;
        let urgency = self.evaluate(sensed_hunger, sensed_energy);
        let action = self.select(urgency);
        let produced_house = self.act(action);
        println!(
            "{}: hunger={:.1} energy={:.1} food={:.1} -> {:?} -> hunger={:.1} energy={:.1} food={:.1}",
            self.name,
            sensed_hunger,
            sensed_energy,
            food_before,
            action,
            self.hunger,
            self.energy,
            self.food
        );
        self.replan();
        produced_house
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_tick_only_applies_decay() {
        let mut agent = Agent::new("Test");
        agent.tick();
        assert_eq!(agent.hunger, HUNGER_DECAY_PER_TICK);
        assert_eq!(agent.energy, ENERGY_MAX - ENERGY_DECAY_PER_TICK);
    }

    #[test]
    fn eats_when_hunger_crosses_threshold() {
        let mut agent = Agent::new("Test");
        agent.hunger = HUNGER_EAT_THRESHOLD;
        agent.tick();
        // Eat relieves hunger, then replan's decay still applies this tick.
        let expected = (HUNGER_EAT_THRESHOLD - HUNGER_EAT_RELIEF).max(0.0) + HUNGER_DECAY_PER_TICK;
        assert_eq!(agent.hunger, expected);
    }

    #[test]
    fn eating_never_drops_hunger_below_zero() {
        let mut agent = Agent::new("Test");
        agent.hunger = HUNGER_EAT_RELIEF - 1.0; // less than a single Eat's relief
        agent.act(Action::Eat);
        assert_eq!(agent.hunger, 0.0);
    }

    #[test]
    fn rests_when_energy_crosses_threshold() {
        let mut agent = Agent::new("Test");
        agent.claim_house(House::new());
        // Urgency threshold 70 => due once energy <= ENERGY_MAX - 70 = 30.
        agent.energy = ENERGY_MAX - ENERGY_REST_THRESHOLD;
        agent.tick();
        let expected = (ENERGY_MAX - ENERGY_REST_THRESHOLD + ENERGY_REST_RELIEF).min(ENERGY_MAX)
            - ENERGY_DECAY_PER_TICK;
        assert_eq!(agent.energy, expected);
    }

    #[test]
    fn resting_never_exceeds_energy_max() {
        let mut agent = Agent::new("Test");
        agent.energy = ENERGY_MAX; // already full
        agent.act(Action::Rest);
        assert_eq!(agent.energy, ENERGY_MAX);
    }

    #[test]
    fn idle_when_neither_need_is_due() {
        let agent = Agent::new("Test");
        let urgency = agent.evaluate(HUNGER_EAT_THRESHOLD - 1.0, ENERGY_MAX);
        assert_eq!(agent.select(urgency), Action::Idle);
    }

    #[test]
    fn higher_urgency_wins_when_both_needs_are_due() {
        let mut agent = Agent::new("Test");
        agent.claim_house(House::new());

        let hunger_more_urgent = Urgency {
            hunger: 90.0,
            energy: 80.0,
        };
        assert_eq!(agent.select(hunger_more_urgent), Action::Eat);

        let energy_more_urgent = Urgency {
            hunger: 80.0,
            energy: 90.0,
        };
        assert_eq!(agent.select(energy_more_urgent), Action::Rest);
    }

    #[test]
    fn equal_urgency_breaks_the_tie_toward_hunger_deterministically() {
        let mut agent = Agent::new("Test");
        agent.claim_house(House::new()); // both needs due requires owning a house
        let tied = Urgency {
            hunger: 75.0,
            energy: 75.0,
        };
        assert_eq!(agent.select(tied), Action::Eat);
    }

    #[test]
    fn hunger_never_exceeds_max_even_without_eating() {
        let mut agent = Agent::new("Test");
        agent.hunger = HUNGER_MAX - 1.0;
        agent.replan();
        assert_eq!(agent.hunger, HUNGER_MAX);
    }

    #[test]
    fn agents_start_without_a_house() {
        let agent = Agent::new("Test");
        assert!(agent.home().is_none());
    }

    #[test]
    fn claiming_a_house_grants_ownership() {
        let mut agent = Agent::new("Test");
        agent.claim_house(House::new());
        assert!(agent.home().is_some());
    }

    #[test]
    fn resting_is_unavailable_without_a_house() {
        let agent = Agent::new("Test"); // no house claimed
        let urgency = Urgency {
            hunger: 0.0,
            energy: 90.0, // as urgent as it gets — would clearly pick Rest if it could
        };
        assert_eq!(agent.select(urgency), Action::Idle);
    }

    #[test]
    fn resting_is_available_once_a_house_is_claimed() {
        let mut agent = Agent::new("Test");
        agent.claim_house(House::new());
        let urgency = Urgency {
            hunger: 0.0,
            energy: 90.0,
        };
        assert_eq!(agent.select(urgency), Action::Rest);
    }

    #[test]
    fn energy_keeps_draining_when_the_agent_has_no_house_to_rest_in() {
        let mut agent = Agent::new("Test"); // no house claimed
        agent.energy = 0.0; // already exhausted, and stuck that way
        agent.tick();
        assert_eq!(agent.energy, 0.0);
    }

    #[test]
    fn agents_start_unemployed() {
        let agent = Agent::new("Test");
        assert_eq!(agent.career(), Career::Unemployed);
        assert_eq!(agent.available_production_action(), None);
    }

    #[test]
    fn set_career_changes_the_available_production_action() {
        let mut agent = Agent::new("Test");
        agent.set_career(Career::Farmer);
        assert_eq!(agent.career(), Career::Farmer);
        assert_eq!(
            agent.available_production_action(),
            Some(ProductionAction::Farm)
        );
    }

    #[test]
    fn produces_when_no_survival_need_is_due_and_a_career_is_set() {
        let mut agent = Agent::new("Test");
        agent.set_career(Career::Farmer);
        let urgency = agent.evaluate(0.0, ENERGY_MAX); // neither need due
        assert_eq!(
            agent.select(urgency),
            Action::Produce(ProductionAction::Farm)
        );
    }

    #[test]
    fn survival_needs_preempt_production() {
        let mut agent = Agent::new("Test");
        agent.set_career(Career::Farmer);
        let urgency = agent.evaluate(HUNGER_EAT_THRESHOLD, ENERGY_MAX); // hunger due
        assert_eq!(agent.select(urgency), Action::Eat);
    }

    #[test]
    fn farming_increases_food_stock() {
        let mut agent = Agent::new("Test");
        agent.act(Action::Produce(ProductionAction::Farm));
        assert_eq!(agent.food, FOOD_PRODUCED_PER_TICK);
    }

    #[test]
    fn building_produces_a_house_without_claiming_it() {
        let mut agent = Agent::new("Test");
        let produced = agent.act(Action::Produce(ProductionAction::Build));
        assert!(produced.is_some());
        assert!(agent.home().is_none()); // act() never self-claims; World does
    }
}
