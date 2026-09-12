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
//!
//! `tick` records what it did to a bounded internal log (`Agent::log`)
//! instead of printing it — this crate does no I/O of its own at all.
//! Deciding whether/how to show that log (e.g. only for a selected agent)
//! is a UI concern; `main.rs` reads it the same way it reads hunger/energy.
//!
//! Grid movement: `select` still only ever decides *which* need wins —
//! location never enters into that comparison. What location changes is
//! whether the winning action can actually execute this tick: `tick` checks
//! `required_location` for the selected action (a `Restaurant`'s position
//! for Eat, the agent's own `House`'s position for Rest, a sensed
//! `Workplace`'s position for Produce, no location for Idle) and, if the
//! agent isn't there yet, moves one cell toward it via `move_toward`
//! instead of running `act` — the action fires on whichever later tick the
//! agent actually arrives. Because `select` reruns fresh every tick, an
//! agent mid-walk to one need will happily redirect toward a different,
//! now more urgent one — the same "no stale plans" principle `replan`
//! already existed for, not a special case bolted on. This is also what
//! turns a fixed priority order (Eat/Rest over Produce) into a work ->
//! lunch -> work -> home rhythm without any explicit schedule: hunger
//! crossing its threshold mid-shift sends the agent to eat, then back to
//! work once relieved, and low energy eventually sends it home — the same
//! mechanism, not a special case for "work hours".

use crate::{Career, House, ProductionAction};
use std::collections::VecDeque;

/// How many recent tick log lines an agent keeps. Bounded so a long-running
/// simulation doesn't grow this without limit — old entries fall off as new
/// ones arrive. A UI showing this log is free to display fewer than this.
const LOG_CAPACITY: usize = 20;

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
    /// How much hunger accumulates per tick for *this* agent — defaults to
    /// `HUNGER_DECAY_PER_TICK` but is overridable via
    /// `set_hunger_decay_rate`, so two agents can genuinely eat at
    /// different frequencies without any RNG or scripted schedule: just a
    /// different, individually explainable number driving the same
    /// threshold comparison every other agent uses.
    hunger_decay_per_tick: f32,
    /// Current grid position. Moves at most one cell per tick, toward
    /// whatever `required_location` returns for the selected action — see
    /// module docs and `move_toward`.
    position: (i32, i32),
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
    /// The last `LOG_CAPACITY` ticks' worth of "what state led to what
    /// action" lines, oldest first. See `log` and `tick`.
    log: VecDeque<String>,
}

impl Agent {
    #[must_use]
    pub fn new(name: &'static str, position: (i32, i32)) -> Self {
        Self {
            name,
            hunger: 0.0,
            energy: ENERGY_MAX,
            hunger_decay_per_tick: HUNGER_DECAY_PER_TICK,
            position,
            home: None,
            career: Career::default(),
            food: 0.0,
            log: VecDeque::new(),
        }
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        self.name
    }

    #[must_use]
    pub const fn hunger(&self) -> f32 {
        self.hunger
    }

    #[must_use]
    pub const fn energy(&self) -> f32 {
        self.energy
    }

    #[must_use]
    pub const fn position(&self) -> (i32, i32) {
        self.position
    }

    /// Recent tick history, oldest first — see `LOG_CAPACITY`. Each line is
    /// the state that led to a decision and the decision itself, the same
    /// information `tick` used to print directly; a caller (e.g. the UI)
    /// decides whether/how much of it to show.
    #[must_use]
    pub fn log(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.log.iter().map(String::as_str)
    }

    #[must_use]
    pub const fn food(&self) -> f32 {
        self.food
    }

    #[must_use]
    pub const fn home(&self) -> Option<&House> {
        self.home.as_ref()
    }

    /// Grants this agent ownership of `house`. Direct, single-owner Rust
    /// ownership — `house` moves onto this `Agent` as a plain field, no
    /// `Rc`/`RefCell`. `World` (milestone 7) is what actually brokers
    /// houses between multiple agents; `Agent` itself never needs to know
    /// about any other agent to do that safely — it just accepts a house
    /// it's handed, one owner at a time.
    pub const fn claim_house(&mut self, house: House) {
        self.home = Some(house);
    }

    #[must_use]
    pub const fn career(&self) -> Career {
        self.career
    }

    pub const fn set_career(&mut self, career: Career) {
        self.career = career;
    }

    /// Overrides this agent's individual hunger decay rate — see the
    /// `hunger_decay_per_tick` field docs. A higher rate means this agent
    /// crosses `HUNGER_EAT_THRESHOLD` sooner and more often than one left
    /// at the default.
    pub const fn set_hunger_decay_rate(&mut self, rate: f32) {
        self.hunger_decay_per_tick = rate;
    }

    /// Which production action this agent's career currently gates access
    /// to, if any. `select` uses this to fall back to work when neither
    /// survival need is due.
    #[must_use]
    pub fn available_production_action(&self) -> Option<ProductionAction> {
        self.career.production_action()
    }

    /// sense: read current state — the raw hunger and energy values.
    const fn sense(&self) -> (f32, f32) {
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
    /// Note: `select` never looks at `position` — it decides *which* need
    /// wins on urgency (and ownership) alone. Whether the winning action
    /// can fire this tick or the agent has to walk toward it first is
    /// `tick`'s job, via `required_location` — see module docs.
    fn select(&self, urgency: Urgency) -> Action {
        type Rule = fn(&Agent, &Urgency) -> Option<Action>;

        const RULES: &[Rule] = &[
            // Both survival needs due at once: higher urgency wins, hunger
            // on a tie — a direct comparison, never a coin flip.
            |agent, u| {
                (agent.hunger_due(u) && agent.energy_due(u)).then_some(if u.hunger >= u.energy {
                    Action::Eat
                } else {
                    Action::Rest
                })
            },
            // Hunger alone is due.
            |agent, u| agent.hunger_due(u).then_some(Action::Eat),
            // Energy alone is due, and there's a house to rest in.
            |agent, u| agent.energy_due(u).then_some(Action::Rest),
            // Neither survival need is pressing: do the job, if there is one.
            |agent, _| agent.available_production_action().map(Action::Produce),
        ];

        RULES
            .iter()
            .find_map(|rule| rule(self, &urgency))
            .unwrap_or(Action::Idle)
    }

    /// Whether hunger has crossed its threshold. Shared by more than one
    /// rule in `select`, so the threshold check lives in exactly one place.
    const fn hunger_due(&self, urgency: &Urgency) -> bool {
        urgency.hunger >= HUNGER_EAT_THRESHOLD
    }

    /// Whether energy has crossed its threshold *and* the agent has a
    /// house to rest in — see `select`'s ownership constraint docs. Shared
    /// by more than one rule, so this (threshold + ownership together)
    /// lives in exactly one place.
    const fn energy_due(&self, urgency: &Urgency) -> bool {
        urgency.energy >= ENERGY_REST_THRESHOLD && self.home.is_some()
    }

    /// Where `action` requires this agent to physically be before it can
    /// execute, if anywhere: Eat needs the shared restaurant's position,
    /// Rest needs the agent's *own* house's position (never just any
    /// house — `select` already guarantees `self.home` is `Some` whenever
    /// it picks Rest), Produce needs whatever `Workplace` position the
    /// caller sensed for this agent's own production action (see `tick`
    /// and `World::tick`) — `None` if nothing in the world offers it, in
    /// which case Produce degrades to firing in place, same as before this
    /// existed. Idle never has a location requirement.
    fn required_location(
        &self,
        action: Action,
        restaurant_position: (i32, i32),
        workplace_position: Option<(i32, i32)>,
    ) -> Option<(i32, i32)> {
        match action {
            Action::Eat => Some(restaurant_position),
            Action::Rest => self.home.as_ref().map(House::position),
            Action::Produce(_) => workplace_position,
            Action::Idle => None,
        }
    }

    /// Moves this agent at most one grid cell toward `target`: each axis
    /// steps independently by `-1`, `0`, or `1` (`i32::signum` of the
    /// remaining distance on that axis), so a diagonal step is taken when
    /// both axes still differ — travel takes `max(|dx|, |dy|)` ticks, never
    /// more than one cell per tick, and does nothing once `position ==
    /// target`.
    fn move_toward(&mut self, target: (i32, i32)) {
        let (x, y) = self.position;
        let (tx, ty) = target;
        self.position = (x + (tx - x).signum(), y + (ty - y).signum());
    }

    /// act: apply the selected action's effect on this agent's own state,
    /// and hand back anything it produced that isn't this agent's alone to
    /// keep — currently just a built house. `act` never claims a produced
    /// house for itself; the caller (e.g. `World`) decides where it goes.
    /// `act` has no notion of location at all — by the time it's called,
    /// `tick` has already confirmed the agent is wherever the action
    /// requires (or that it requires nowhere in particular).
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
            // Built where the builder is currently standing — the one
            // place a house's position has to come from.
            Action::Produce(ProductionAction::Build) => Some(House::new(self.position)),
            Action::Idle => None,
        }
    }

    /// replan: let needs progress (decay) so the next tick senses fresh
    /// state instead of acting on a stale plan. Hunger decays at this
    /// agent's own `hunger_decay_per_tick` rather than a shared constant —
    /// see that field's docs.
    fn replan(&mut self) {
        self.hunger = (self.hunger + self.hunger_decay_per_tick).min(HUNGER_MAX);
        self.energy = (self.energy - ENERGY_DECAY_PER_TICK).max(0.0);
    }

    /// Runs one full sense -> evaluate -> select -> act -> replan cycle.
    /// `restaurant_position` and `workplace_position` are sensed world
    /// state, the same way hunger and energy are sensed agent state — see
    /// module docs. `workplace_position` is `None` when nothing in the
    /// world offers this agent's production action (including simply
    /// having no career). If the selected action requires being somewhere
    /// this agent isn't yet (`required_location`), this tick moves one
    /// cell closer instead of running the action; `act` only ever runs
    /// once the agent is already there (or the action needs nowhere in
    /// particular).
    ///
    /// Returns a house if this tick's action produced one — `Agent` has no
    /// way to know whether it needs one or another agent does, so it never
    /// keeps it; the caller is responsible for placing it (see `World`).
    /// `#[must_use]` here isn't about purity (this call is all side
    /// effects) — it's a guard against silently losing a produced house if
    /// a future caller forgets to check the return value.
    #[must_use]
    pub fn tick(
        &mut self,
        restaurant_position: (i32, i32),
        workplace_position: Option<(i32, i32)>,
    ) -> Option<House> {
        let (sensed_hunger, sensed_energy) = self.sense();
        let food_before = self.food;
        let urgency = self.evaluate(sensed_hunger, sensed_energy);
        let action = self.select(urgency);

        let required = self.required_location(action, restaurant_position, workplace_position);
        let (produced_house, walking) = match required {
            Some(target) if target != self.position => {
                self.move_toward(target);
                (None, true)
            }
            _ => (self.act(action), false),
        };

        // Plain ASCII "->" rather than an em dash: macroquad's default font
        // doesn't have that glyph and renders it as a tofu box (visible in
        // the UI's log panel) instead of failing loudly, so this is the
        // kind of thing that's easy to miss without actually looking at it
        // rendered.
        let suffix = if walking { " (walking)" } else { "" };
        self.push_log(format!(
            "{action:?}{suffix} -> H{sensed_hunger:.0} E{sensed_energy:.0} F{food_before:.0}"
        ));
        self.replan();
        produced_house
    }

    /// Appends one line to `log`, dropping the oldest line first once
    /// `LOG_CAPACITY` is reached — a ring buffer via `VecDeque`, so this
    /// stays O(1) regardless of how long the simulation has been running.
    fn push_log(&mut self, line: String) {
        if self.log.len() >= LOG_CAPACITY {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Arbitrary shared starting point for tests that don't care about
    /// position at all.
    const ORIGIN: (i32, i32) = (0, 0);

    #[test]
    fn idle_tick_only_applies_decay() {
        let mut agent = Agent::new("Test", ORIGIN);
        let _ = agent.tick(ORIGIN, None);
        assert_eq!(agent.hunger, HUNGER_DECAY_PER_TICK);
        assert_eq!(agent.energy, ENERGY_MAX - ENERGY_DECAY_PER_TICK);
    }

    #[test]
    fn eats_when_hunger_crosses_threshold() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.hunger = HUNGER_EAT_THRESHOLD;
        // Already at the restaurant, so Eat fires this tick instead of
        // walking toward it first.
        let _ = agent.tick(ORIGIN, None);
        // Eat relieves hunger, then replan's decay still applies this tick.
        let expected = (HUNGER_EAT_THRESHOLD - HUNGER_EAT_RELIEF).max(0.0) + HUNGER_DECAY_PER_TICK;
        assert_eq!(agent.hunger, expected);
    }

    #[test]
    fn eating_never_drops_hunger_below_zero() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.hunger = HUNGER_EAT_RELIEF - 1.0; // less than a single Eat's relief
        agent.act(Action::Eat);
        assert_eq!(agent.hunger, 0.0);
    }

    #[test]
    fn rests_when_energy_crosses_threshold() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.claim_house(House::new(ORIGIN)); // house is where the agent stands
        // Urgency threshold 70 => due once energy <= ENERGY_MAX - 70 = 30.
        agent.energy = ENERGY_MAX - ENERGY_REST_THRESHOLD;
        let _ = agent.tick(ORIGIN, None);
        let expected = (ENERGY_MAX - ENERGY_REST_THRESHOLD + ENERGY_REST_RELIEF).min(ENERGY_MAX)
            - ENERGY_DECAY_PER_TICK;
        assert_eq!(agent.energy, expected);
    }

    #[test]
    fn resting_never_exceeds_energy_max() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.energy = ENERGY_MAX; // already full
        agent.act(Action::Rest);
        assert_eq!(agent.energy, ENERGY_MAX);
    }

    #[test]
    fn idle_when_neither_need_is_due() {
        let agent = Agent::new("Test", ORIGIN);
        let urgency = agent.evaluate(HUNGER_EAT_THRESHOLD - 1.0, ENERGY_MAX);
        assert_eq!(agent.select(urgency), Action::Idle);
    }

    #[test]
    fn higher_urgency_wins_when_both_needs_are_due() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.claim_house(House::new(ORIGIN));

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
        let mut agent = Agent::new("Test", ORIGIN);
        agent.claim_house(House::new(ORIGIN)); // both needs due requires owning a house
        let tied = Urgency {
            hunger: 75.0,
            energy: 75.0,
        };
        assert_eq!(agent.select(tied), Action::Eat);
    }

    #[test]
    fn hunger_never_exceeds_max_even_without_eating() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.hunger = HUNGER_MAX - 1.0;
        agent.replan();
        assert_eq!(agent.hunger, HUNGER_MAX);
    }

    #[test]
    fn agents_start_without_a_house() {
        let agent = Agent::new("Test", ORIGIN);
        assert!(agent.home().is_none());
    }

    #[test]
    fn claiming_a_house_grants_ownership() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.claim_house(House::new(ORIGIN));
        assert!(agent.home().is_some());
    }

    #[test]
    fn resting_is_unavailable_without_a_house() {
        let agent = Agent::new("Test", ORIGIN); // no house claimed
        let urgency = Urgency {
            hunger: 0.0,
            energy: 90.0, // as urgent as it gets — would clearly pick Rest if it could
        };
        assert_eq!(agent.select(urgency), Action::Idle);
    }

    #[test]
    fn resting_is_available_once_a_house_is_claimed() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.claim_house(House::new(ORIGIN));
        let urgency = Urgency {
            hunger: 0.0,
            energy: 90.0,
        };
        assert_eq!(agent.select(urgency), Action::Rest);
    }

    #[test]
    fn energy_keeps_draining_when_the_agent_has_no_house_to_rest_in() {
        let mut agent = Agent::new("Test", ORIGIN); // no house claimed
        agent.energy = 0.0; // already exhausted, and stuck that way
        let _ = agent.tick(ORIGIN, None);
        assert_eq!(agent.energy, 0.0);
    }

    #[test]
    fn agents_start_unemployed() {
        let agent = Agent::new("Test", ORIGIN);
        assert_eq!(agent.career(), Career::Unemployed);
        assert_eq!(agent.available_production_action(), None);
    }

    #[test]
    fn set_career_changes_the_available_production_action() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.set_career(Career::Farmer);
        assert_eq!(agent.career(), Career::Farmer);
        assert_eq!(
            agent.available_production_action(),
            Some(ProductionAction::Farm)
        );
    }

    #[test]
    fn produces_when_no_survival_need_is_due_and_a_career_is_set() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.set_career(Career::Farmer);
        let urgency = agent.evaluate(0.0, ENERGY_MAX); // neither need due
        assert_eq!(
            agent.select(urgency),
            Action::Produce(ProductionAction::Farm)
        );
    }

    #[test]
    fn survival_needs_preempt_production() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.set_career(Career::Farmer);
        let urgency = agent.evaluate(HUNGER_EAT_THRESHOLD, ENERGY_MAX); // hunger due
        assert_eq!(agent.select(urgency), Action::Eat);
    }

    #[test]
    fn farming_increases_food_stock() {
        let mut agent = Agent::new("Test", ORIGIN);
        agent.act(Action::Produce(ProductionAction::Farm));
        assert_eq!(agent.food, FOOD_PRODUCED_PER_TICK);
    }

    #[test]
    fn building_produces_a_house_without_claiming_it() {
        let mut agent = Agent::new("Test", ORIGIN);
        let produced = agent.act(Action::Produce(ProductionAction::Build));
        assert!(produced.is_some());
        assert!(agent.home().is_none()); // act() never self-claims; World does
    }

    #[test]
    fn building_places_the_house_at_the_builders_current_position() {
        let mut agent = Agent::new("Test", (3, 4));
        let produced = agent.act(Action::Produce(ProductionAction::Build));
        assert_eq!(produced.unwrap().position(), (3, 4));
    }

    #[test]
    fn new_agent_has_an_empty_log() {
        let agent = Agent::new("Test", ORIGIN);
        assert_eq!(agent.log().count(), 0);
    }

    #[test]
    fn tick_appends_one_line_to_the_log() {
        let mut agent = Agent::new("Test", ORIGIN);
        let _ = agent.tick(ORIGIN, None);
        assert_eq!(agent.log().count(), 1);
        assert!(agent.log().next().unwrap().contains("Idle"));
    }

    #[test]
    fn log_never_grows_past_its_capacity() {
        let mut agent = Agent::new("Test", ORIGIN);
        for _ in 0..(LOG_CAPACITY + 5) {
            let _ = agent.tick(ORIGIN, None);
        }
        assert_eq!(agent.log().count(), LOG_CAPACITY);
    }

    #[test]
    fn move_toward_never_overshoots_the_target_in_one_tick() {
        let mut agent = Agent::new("Test", (0, 0));
        agent.move_toward((5, 1));
        // One cell per tick on each axis — never a teleport straight to it.
        assert_eq!(agent.position(), (1, 1));
    }

    #[test]
    fn move_toward_does_nothing_once_already_at_the_target() {
        let mut agent = Agent::new("Test", (2, 2));
        agent.move_toward((2, 2));
        assert_eq!(agent.position(), (2, 2));
    }

    #[test]
    fn agent_walks_toward_the_restaurant_instead_of_eating_when_not_there_yet() {
        let mut agent = Agent::new("Test", (0, 0));
        agent.hunger = HUNGER_EAT_THRESHOLD;
        let hunger_before = agent.hunger;
        let restaurant_position = (5, 0);
        let _ = agent.tick(restaurant_position, None);
        // Moved one cell toward the restaurant instead of eating.
        assert_eq!(agent.position(), (1, 0));
        // Hunger only rose (replan's decay) — Eat never actually ran.
        assert_eq!(agent.hunger, hunger_before + HUNGER_DECAY_PER_TICK);
    }

    #[test]
    fn eating_is_deferred_until_the_agent_reaches_the_restaurant() {
        let mut agent = Agent::new("Test", (0, 0));
        agent.hunger = HUNGER_EAT_THRESHOLD;
        let restaurant_position = (1, 0);
        let _ = agent.tick(restaurant_position, None); // ticks 1 cell closer, doesn't eat
        assert_eq!(agent.position(), restaurant_position);
        let hunger_at_restaurant = agent.hunger;
        let _ = agent.tick(restaurant_position, None); // now at the restaurant: eats
        assert!(agent.hunger < hunger_at_restaurant);
    }

    #[test]
    fn resting_requires_being_at_the_agents_own_house_position() {
        let mut agent = Agent::new("Test", (0, 0));
        agent.claim_house(House::new((3, 0)));
        agent.energy = ENERGY_MAX - ENERGY_REST_THRESHOLD; // due
        let energy_before = agent.energy;
        let _ = agent.tick(ORIGIN, None); // restaurant position irrelevant here
        // Walked toward the house instead of resting in place.
        assert_eq!(agent.position(), (1, 0));
        assert_eq!(agent.energy, energy_before - ENERGY_DECAY_PER_TICK);
    }

    #[test]
    fn agent_walks_toward_its_workplace_instead_of_producing_when_not_there_yet() {
        let mut agent = Agent::new("Test", (0, 0));
        agent.set_career(Career::Farmer);
        let food_before = agent.food;
        let _ = agent.tick(ORIGIN, Some((3, 0)));
        // Moved one cell toward the workplace instead of producing.
        assert_eq!(agent.position(), (1, 0));
        assert_eq!(agent.food, food_before);
    }

    #[test]
    fn production_is_deferred_until_the_agent_reaches_its_workplace() {
        let mut agent = Agent::new("Test", (0, 0));
        agent.set_career(Career::Farmer);
        let workplace_position = (1, 0);
        let _ = agent.tick(ORIGIN, Some(workplace_position)); // steps closer, doesn't produce
        assert_eq!(agent.position(), workplace_position);
        assert_eq!(agent.food, 0.0);
        let _ = agent.tick(ORIGIN, Some(workplace_position)); // now at the workplace: produces
        assert_eq!(agent.food, FOOD_PRODUCED_PER_TICK);
    }

    #[test]
    fn production_fires_in_place_when_no_matching_workplace_exists() {
        let mut agent = Agent::new("Test", (0, 0));
        agent.set_career(Career::Farmer);
        let _ = agent.tick(ORIGIN, None); // nothing in the world offers Farm
        // Degrades to firing in place, same as before Produce had a location.
        assert_eq!(agent.position(), (0, 0));
        assert_eq!(agent.food, FOOD_PRODUCED_PER_TICK);
    }

    #[test]
    fn a_faster_hunger_decay_rate_reaches_the_eat_threshold_sooner() {
        let mut fast = Agent::new("Fast", ORIGIN);
        fast.set_hunger_decay_rate(HUNGER_DECAY_PER_TICK * 2.0);
        let mut normal = Agent::new("Normal", ORIGIN);

        let ticks_to_threshold = |agent: &mut Agent| -> u32 {
            let mut ticks = 0;
            while agent.hunger < HUNGER_EAT_THRESHOLD {
                agent.replan();
                ticks += 1;
            }
            ticks
        };

        assert!(ticks_to_threshold(&mut fast) < ticks_to_threshold(&mut normal));
    }
}
