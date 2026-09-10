//! `Agent` and the standing decision-loop convention: every tick runs
//! sense -> evaluate -> select -> act -> replan. Needs are plain numbers
//! that decay over time; priority between them is decided by comparing or
//! scoring those numbers — never by randomness.

use crate::House;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Eat,
    Rest,
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
}

impl Agent {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            hunger: 0.0,
            energy: ENERGY_MAX,
            home: None,
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

    pub fn home(&self) -> Option<&House> {
        self.home.as_ref()
    }

    /// Grants this agent ownership of `house`. Direct, single-owner Rust
    /// ownership — `house` moves onto this `Agent` as a plain field, no
    /// `Rc`/`RefCell`. That's enough at today's one-agent, one-house scale;
    /// milestone 7 (multiple agents contending over shared resources) is
    /// where sharing a house safely actually has to be designed, not
    /// pre-solved here.
    pub fn claim_house(&mut self, house: House) {
        self.home = Some(house);
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

    /// select: pick the winning action from evaluated urgency. When only one
    /// need is past its threshold, that need wins outright. When both are,
    /// the higher urgency score wins — a direct comparison, never
    /// randomness. Equal urgency is a deliberate fixed tiebreak (hunger
    /// wins), not a coin flip, so the choice stays explainable from state
    /// alone.
    ///
    /// Ownership constraint: Rest is only ever a candidate if the agent
    /// owns a house (`self.home.is_some()`) — an agent that hasn't claimed
    /// one can be as exhausted as it likes and will never select Rest. This
    /// is the only place that check happens; `act` doesn't re-check it
    /// because `act` only ever receives what `select` already gated.
    fn select(&self, urgency: Urgency) -> Action {
        let hunger_due = urgency.hunger >= HUNGER_EAT_THRESHOLD;
        let energy_due = urgency.energy >= ENERGY_REST_THRESHOLD && self.home.is_some();

        match (hunger_due, energy_due) {
            (false, false) => Action::Idle,
            (true, false) => Action::Eat,
            (false, true) => Action::Rest,
            (true, true) => {
                if urgency.hunger >= urgency.energy {
                    Action::Eat
                } else {
                    Action::Rest
                }
            }
        }
    }

    /// act: apply the selected action's effect on state.
    fn act(&mut self, action: Action) {
        match action {
            Action::Eat => self.hunger = (self.hunger - HUNGER_EAT_RELIEF).max(0.0),
            Action::Rest => self.energy = (self.energy + ENERGY_REST_RELIEF).min(ENERGY_MAX),
            Action::Idle => {}
        }
    }

    /// replan: let needs progress (decay) so the next tick senses fresh
    /// state instead of acting on a stale plan.
    fn replan(&mut self) {
        self.hunger = (self.hunger + HUNGER_DECAY_PER_TICK).min(HUNGER_MAX);
        self.energy = (self.energy - ENERGY_DECAY_PER_TICK).max(0.0);
    }

    pub fn tick(&mut self) {
        let (sensed_hunger, sensed_energy) = self.sense();
        let urgency = self.evaluate(sensed_hunger, sensed_energy);
        let action = self.select(urgency);
        self.act(action);
        println!(
            "{}: hunger={:.1} energy={:.1} -> {:?} -> hunger={:.1} energy={:.1}",
            self.name, sensed_hunger, sensed_energy, action, self.hunger, self.energy
        );
        self.replan();
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
}
