//! Milestone 1: a single agent with a single need (hunger), running the
//! standing decision loop every tick: sense -> evaluate -> select -> act -> replan.
//!
//! Hunger is a plain number that decays over time. There's only one need and
//! one action here, so "evaluate" and "select" don't have anything to compare
//! against yet — but the loop stays split into these stages on purpose, so
//! milestone 2 (a second need + priority comparison) drops in without
//! restructuring anything.

/// How much hunger accumulates per tick if the agent doesn't eat.
const HUNGER_DECAY_PER_TICK: f32 = 5.0;
/// Hunger level at or above which eating becomes the priority.
const HUNGER_EAT_THRESHOLD: f32 = 60.0;
/// How much a single Eat action reduces hunger by.
const HUNGER_EAT_RELIEF: f32 = 40.0;
/// Hunger is clamped to this range; 0 = fully fed, 100 = starving.
const HUNGER_MAX: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Eat,
    Idle,
}

struct Agent {
    name: &'static str,
    hunger: f32,
}

impl Agent {
    fn new(name: &'static str) -> Self {
        Self { name, hunger: 0.0 }
    }

    /// sense: read current state. With one need this is just the raw value,
    /// but it stays a named step so more state can be read here later.
    fn sense(&self) -> f32 {
        self.hunger
    }

    /// evaluate: turn sensed state into an urgency score. Trivial for a
    /// single need (urgency == hunger), but kept separate from `select` so
    /// milestone 2 can compare urgency across multiple needs here.
    fn evaluate(&self, hunger: f32) -> f32 {
        hunger
    }

    /// select: pick the winning action from evaluated urgency. Deterministic
    /// threshold comparison only — never randomness, so the choice is always
    /// explainable from `hunger` alone.
    fn select(&self, urgency: f32) -> Action {
        if urgency >= HUNGER_EAT_THRESHOLD {
            Action::Eat
        } else {
            Action::Idle
        }
    }

    /// act: apply the selected action's effect on state.
    fn act(&mut self, action: Action) {
        match action {
            Action::Eat => self.hunger = (self.hunger - HUNGER_EAT_RELIEF).max(0.0),
            Action::Idle => {}
        }
    }

    /// replan: let needs progress (decay) so the next tick senses fresh
    /// state instead of acting on a stale plan.
    fn replan(&mut self) {
        self.hunger = (self.hunger + HUNGER_DECAY_PER_TICK).min(HUNGER_MAX);
    }

    fn tick(&mut self) {
        let sensed_hunger = self.sense();
        let urgency = self.evaluate(sensed_hunger);
        let action = self.select(urgency);
        self.act(action);
        println!(
            "{}: hunger={:.1} -> {:?} -> hunger={:.1}",
            self.name, sensed_hunger, action, self.hunger
        );
        self.replan();
    }
}

fn main() {
    let mut agent = Agent::new("Agent-0");
    for _ in 0..20 {
        agent.tick();
    }
}
