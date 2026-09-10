//! Milestone 2 UI: a thin macroquad rendering pass over `continental::Agent`'s
//! tick loop. It reads `Agent` state each frame through its public accessors
//! and draws it — it adds no decision logic of its own. Both needs (hunger,
//! energy) are drawn now.

use continental::{Agent, ENERGY_MAX, HUNGER_MAX};
use macroquad::prelude::*;

/// Seconds of real time between ticks, so needs visibly change over time
/// instead of the whole run flashing by in one frame.
const TICK_INTERVAL_SECS: f32 = 1.0;
/// How long an action's flash stays on screen after the tick that caused it.
const ACTION_FLASH_SECS: f32 = 0.4;

const BAR_X: f32 = 160.0;
const BAR_W: f32 = 260.0;
const BAR_H: f32 = 22.0;
const HUNGER_BAR_Y: f32 = 78.0;
const ENERGY_BAR_Y: f32 = 168.0;

fn window_conf() -> Conf {
    Conf {
        window_title: "Continental — milestone 2".to_owned(),
        window_width: 480,
        window_height: 260,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut agent = Agent::new("Agent-0");
    let mut tick_timer = 0.0;
    let mut eat_flash_timer = 0.0;
    let mut rest_flash_timer = 0.0;

    loop {
        let dt = get_frame_time();
        tick_timer += dt;
        eat_flash_timer = (eat_flash_timer - dt).max(0.0);
        rest_flash_timer = (rest_flash_timer - dt).max(0.0);

        if tick_timer >= TICK_INTERVAL_SECS {
            tick_timer -= TICK_INTERVAL_SECS;
            let hunger_before = agent.hunger();
            let energy_before = agent.energy();
            agent.tick();
            // tick() doesn't report which Action it picked, so infer which
            // one ran from its one observable effect: Eat is the only thing
            // that can drop hunger, Rest the only thing that can raise
            // energy — Idle + replan only ever push both the other way.
            if agent.hunger() < hunger_before {
                eat_flash_timer = ACTION_FLASH_SECS;
            }
            if agent.energy() > energy_before {
                rest_flash_timer = ACTION_FLASH_SECS;
            }
        }

        draw_agent(&agent, eat_flash_timer > 0.0, rest_flash_timer > 0.0);

        next_frame().await;
    }
}

/// Draws the agent as a circle plus each need as a number and a 0-100 bar.
/// Read-only: takes `&Agent` and never mutates or advances simulation state.
fn draw_agent(agent: &Agent, eating: bool, resting: bool) {
    clear_background(Color::from_rgba(24, 24, 28, 255));

    let agent_color = if eating {
        YELLOW
    } else if resting {
        GREEN
    } else {
        SKYBLUE
    };
    draw_circle(80.0, 130.0, 28.0, agent_color);
    draw_text(agent.name(), 55.0, 175.0, 20.0, WHITE);

    draw_need_bar(
        "hunger",
        agent.hunger(),
        HUNGER_MAX,
        HUNGER_BAR_Y,
        ORANGE,
        eating,
        "ATE!",
    );
    draw_need_bar(
        "energy",
        agent.energy(),
        ENERGY_MAX,
        ENERGY_BAR_Y,
        GREEN,
        resting,
        "RESTED!",
    );
}

/// Draws one need's label, value, and a 0-`max` bar at `bar_y`. While
/// `flashing`, the bar switches to yellow and `flash_label` appears beneath
/// it — the same visible-moment treatment milestone 1 used for Eat, now
/// shared by both needs instead of duplicated per need.
fn draw_need_bar(
    label: &str,
    value: f32,
    max: f32,
    bar_y: f32,
    normal_color: Color,
    flashing: bool,
    flash_label: &str,
) {
    draw_text(
        format!("{label}: {value:.1}"),
        BAR_X,
        bar_y - 12.0,
        22.0,
        WHITE,
    );
    draw_rectangle_lines(BAR_X, bar_y, BAR_W, BAR_H, 2.0, WHITE);
    let fill_w = BAR_W * (value / max).clamp(0.0, 1.0);
    draw_rectangle(
        BAR_X,
        bar_y,
        fill_w,
        BAR_H,
        if flashing { YELLOW } else { normal_color },
    );

    if flashing {
        draw_text(flash_label, BAR_X, bar_y + BAR_H + 26.0, 24.0, YELLOW);
    }
}
