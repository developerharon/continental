//! Milestone 1: a single agent with a single need (hunger), running the
//! standing decision loop every tick: sense -> evaluate -> select -> act -> replan.
//!
//! Hunger is a plain number that decays over time. There's only one need and
//! one action here, so "evaluate" and "select" don't have anything to compare
//! against yet — but the loop stays split into these stages on purpose, so
//! milestone 2 (a second need + priority comparison) drops in without
//! restructuring anything.
//!
//! The bottom of this file is a thin macroquad rendering pass over the tick
//! loop above: it reads `Agent` state each frame and draws it. It doesn't add
//! any decision logic of its own — `Agent`/`tick()` are untouched.

use macroquad::prelude::*;

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

/// Seconds of real time between ticks, so hunger visibly changes over time
/// instead of the whole run flashing by in one frame.
const TICK_INTERVAL_SECS: f32 = 1.0;
/// How long the "ate" flash stays on screen after a tick drops hunger.
const EAT_FLASH_SECS: f32 = 0.4;

fn window_conf() -> Conf {
    Conf {
        window_title: "Continental — milestone 1".to_owned(),
        window_width: 480,
        window_height: 240,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut agent = Agent::new("Agent-0");
    let mut tick_timer = 0.0;
    let mut flash_timer = 0.0;

    loop {
        let dt = get_frame_time();
        tick_timer += dt;
        flash_timer = (flash_timer - dt).max(0.0);

        if tick_timer >= TICK_INTERVAL_SECS {
            tick_timer -= TICK_INTERVAL_SECS;
            let hunger_before = agent.hunger;
            agent.tick();
            // tick() doesn't report which Action it picked, so infer the eat
            // moment from the one observable effect Eat has: hunger drops.
            // Idle + replan only ever raises hunger, so any decrease means Eat.
            if agent.hunger < hunger_before {
                flash_timer = EAT_FLASH_SECS;
            }
        }

        draw_agent(&agent, flash_timer > 0.0);

        next_frame().await;
    }
}

/// Draws the agent as a circle plus its hunger as a number and a 0-100 bar.
/// Read-only: takes `&Agent` and never mutates or advances simulation state.
fn draw_agent(agent: &Agent, eating: bool) {
    clear_background(Color::from_rgba(24, 24, 28, 255));

    let agent_color = if eating { YELLOW } else { SKYBLUE };
    draw_circle(80.0, 120.0, 28.0, agent_color);
    draw_text(agent.name, 55.0, 170.0, 20.0, WHITE);

    let (bar_x, bar_y, bar_w, bar_h) = (160.0, 110.0, 260.0, 22.0);
    draw_text(
        format!("hunger: {:.1}", agent.hunger),
        bar_x,
        bar_y - 12.0,
        22.0,
        WHITE,
    );
    draw_rectangle_lines(bar_x, bar_y, bar_w, bar_h, 2.0, WHITE);
    let fill_w = bar_w * (agent.hunger / HUNGER_MAX).clamp(0.0, 1.0);
    draw_rectangle(
        bar_x,
        bar_y,
        fill_w,
        bar_h,
        if eating { YELLOW } else { ORANGE },
    );

    if eating {
        draw_text("ATE!", bar_x, bar_y + bar_h + 26.0, 24.0, YELLOW);
    }
}
