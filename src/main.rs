//! Milestone 3/4 UI: a thin macroquad rendering pass over `continental::Agent`'s
//! tick loop. It reads `Agent` state each frame through its public accessors
//! and draws it — it adds no decision logic of its own. The agent sits on a
//! 10x10 grid instead of a free pixel position. The grid's House square is
//! still just a static placeholder (milestone 3) — the ownership constraint
//! milestone 4 actually added lives in `agent.rs`; this demo agent claims a
//! *separate* House (unconnected to the grid square) so it can still rest.
//!
//! Grid objects are clickable: clicking the agent or the house selects it
//! (`Selected`), and the right-side panel — previously a fixed spot for the
//! one agent's bars — now shows whichever thing is selected, or a prompt
//! when nothing is. With only one agent and one house today, `Selected` is
//! a plain enum; once there's more than one of a kind (multiple agents via
//! `World`, more world objects), it'll need to carry an identifier instead.

use continental::{Agent, ENERGY_MAX, HUNGER_MAX, House};
use macroquad::prelude::*;

/// Something on the grid the player can click to see details about in the
/// side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selected {
    Agent,
    House,
}

/// Seconds of real time between ticks, so needs visibly change over time
/// instead of the whole run flashing by in one frame.
const TICK_INTERVAL_SECS: f32 = 1.0;
/// How long an action's flash stays on screen after the tick that caused it.
const ACTION_FLASH_SECS: f32 = 0.4;
/// How fast each bar's on-screen fill eases toward the agent's real value —
/// see `ease_toward`. Tuned so a tick's change visibly catches up well
/// within the ~1s gap before the next tick.
const BAR_ANIM_SPEED: f32 = 6.0;

const BAR_X: f32 = 360.0;
const BAR_W: f32 = 260.0;
const BAR_H: f32 = 22.0;
const HUNGER_BAR_Y: f32 = 78.0;
const ENERGY_BAR_Y: f32 = 168.0;

/// Grid the agent and world objects sit on. Coordinates are (col, row),
/// each in `0..GRID_COLS`/`0..GRID_ROWS`.
const GRID_COLS: i32 = 10;
const GRID_ROWS: i32 = 10;
const CELL_SIZE: f32 = 32.0;
const GRID_X: f32 = 20.0;
const GRID_Y: f32 = 20.0;

/// The agent's fixed position on the grid. No movement yet — that's a
/// later milestone — so this is just where it's drawn every frame.
const AGENT_GRID_POS: (i32, i32) = (2, 3);
/// Where the House placeholder sits. Still just a static square on the
/// grid, not connected to `agent.home()` or any ownership logic — wiring
/// the grid itself up to ownership (e.g. recoloring this square once it's
/// actually claimed) is future UI work, not part of this pass.
const HOUSE_GRID_POS: (i32, i32) = (7, 6);

/// Radius the agent is drawn at and hit-tested against for clicks.
const AGENT_RADIUS: f32 = 12.0;
/// Side length the house square is drawn at and hit-tested against.
const HOUSE_SIZE: f32 = CELL_SIZE - 8.0;

fn window_conf() -> Conf {
    Conf {
        window_title: "Continental".to_owned(),
        window_width: 640,
        window_height: 400,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut agent = Agent::new("Agent-0");
    // Milestone 4 made owning a house a precondition for Rest, and agents
    // now start without one. Grant this demo agent one up front so it can
    // still rest, same as before — the grid's gray House square (below)
    // isn't this one; it's still just an unconnected placeholder.
    agent.claim_house(House::new());
    let mut tick_timer = 0.0;
    let mut eat_flash_timer = 0.0;
    let mut rest_flash_timer = 0.0;
    // Bar fills displayed on screen. These ease toward the agent's real
    // hunger()/energy() every frame (see `ease_toward`) instead of jumping
    // the moment a tick changes them, so e.g. Rest visibly fills the energy
    // bar up rather than snapping it. The printed numbers stay exact — only
    // the bar fill is smoothed.
    let mut displayed_hunger = agent.hunger();
    let mut displayed_energy = agent.energy();
    // Nothing selected until the player clicks something — see `Selected`.
    let mut selected: Option<Selected> = None;

    loop {
        let dt = get_frame_time();
        tick_timer += dt;
        eat_flash_timer = (eat_flash_timer - dt).max(0.0);
        rest_flash_timer = (rest_flash_timer - dt).max(0.0);

        if is_mouse_button_pressed(MouseButton::Left) {
            // Replaces whatever was selected, including with nothing —
            // clicking empty grid space (or anywhere else) deselects.
            selected = hit_test(mouse_position());
        }

        if tick_timer >= TICK_INTERVAL_SECS {
            tick_timer -= TICK_INTERVAL_SECS;
            let hunger_before = agent.hunger();
            let energy_before = agent.energy();
            // This demo has no World to broker a produced house to whoever
            // needs it (see world.rs) — with only one agent there's no one
            // else it could go to, so it just claims its own output. Would
            // silently vanish here without this if the demo agent's career
            // were ever set to Builder.
            if let Some(house) = agent.tick() {
                agent.claim_house(house);
            }
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

        displayed_hunger = ease_toward(displayed_hunger, agent.hunger(), dt, BAR_ANIM_SPEED);
        displayed_energy = ease_toward(displayed_energy, agent.energy(), dt, BAR_ANIM_SPEED);

        draw_scene(
            &agent,
            displayed_hunger,
            displayed_energy,
            eat_flash_timer > 0.0,
            rest_flash_timer > 0.0,
            selected,
        );

        next_frame().await;
    }
}

/// Frame-rate-independent exponential ease of `current` toward `target`:
/// closes roughly `1 - e^(-speed * dt)` of the remaining gap each frame, so
/// the same `speed` produces the same catch-up time regardless of frame rate.
fn ease_toward(current: f32, target: f32, dt: f32, speed: f32) -> f32 {
    current + (target - current) * (1.0 - (-speed * dt).exp())
}

/// Converts a grid cell `(col, row)` to the screen-space center point of
/// that cell.
fn grid_to_screen((col, row): (i32, i32)) -> (f32, f32) {
    (
        GRID_X + (col as f32 + 0.5) * CELL_SIZE,
        GRID_Y + (row as f32 + 0.5) * CELL_SIZE,
    )
}

/// Which selectable grid object, if any, `pos` (screen space, e.g. from
/// `mouse_position()`) falls within — checked in front-to-back draw order.
/// Only two objects exist to check against today; once there are more
/// (multiple agents, more world objects) this is where iterating them goes.
fn hit_test((x, y): (f32, f32)) -> Option<Selected> {
    let (agent_x, agent_y) = grid_to_screen(AGENT_GRID_POS);
    if (x - agent_x).hypot(y - agent_y) <= AGENT_RADIUS {
        return Some(Selected::Agent);
    }

    let (house_x, house_y) = grid_to_screen(HOUSE_GRID_POS);
    let half = HOUSE_SIZE / 2.0;
    if (x - house_x).abs() <= half && (y - house_y).abs() <= half {
        return Some(Selected::House);
    }

    None
}

/// Draws the grid the agent and world objects sit on: an outer border plus
/// internal lines at each cell boundary.
fn draw_grid() {
    let grid_w = CELL_SIZE * GRID_COLS as f32;
    let grid_h = CELL_SIZE * GRID_ROWS as f32;

    for col in 1..GRID_COLS {
        let x = GRID_X + col as f32 * CELL_SIZE;
        draw_line(x, GRID_Y, x, GRID_Y + grid_h, 1.0, DARKGRAY);
    }
    for row in 1..GRID_ROWS {
        let y = GRID_Y + row as f32 * CELL_SIZE;
        draw_line(GRID_X, y, GRID_X + grid_w, y, 1.0, DARKGRAY);
    }

    draw_rectangle_lines(GRID_X, GRID_Y, grid_w, grid_h, 2.0, WHITE);
}

/// Draws the House placeholder at `pos`: a gray square signaling "exists,
/// unclaimed". Purely decorative — not connected to `agent.home()` or any
/// ownership logic, and there's no "walk to house" here. `agent.rs` has the
/// real ownership constraint now (milestone 4); wiring this square up to
/// it is separate UI work, not done in this pass. Draws a highlight ring
/// when `selected` is true, from clicking it — see `hit_test`.
fn draw_house(pos: (i32, i32), selected: bool) {
    let (cx, cy) = grid_to_screen(pos);
    let half = HOUSE_SIZE / 2.0;
    draw_rectangle(cx - half, cy - half, HOUSE_SIZE, HOUSE_SIZE, GRAY);
    if selected {
        draw_rectangle_lines(
            cx - half - 3.0,
            cy - half - 3.0,
            HOUSE_SIZE + 6.0,
            HOUSE_SIZE + 6.0,
            2.0,
            WHITE,
        );
    }
}

/// Draws the grid, the house, the agent, and the right-side details panel.
/// Read-only: takes `&Agent` and never mutates or advances simulation state.
/// `displayed_hunger`/`displayed_energy` are the eased bar-fill values from
/// the render loop — the numeric labels still read the agent's real state.
/// `selected` is whatever the player last clicked (see `Selected`,
/// `hit_test`) — it decides what the details panel shows and which grid
/// object (if any) gets a highlight ring.
fn draw_scene(
    agent: &Agent,
    displayed_hunger: f32,
    displayed_energy: f32,
    eating: bool,
    resting: bool,
    selected: Option<Selected>,
) {
    clear_background(Color::from_rgba(24, 24, 28, 255));

    draw_grid();
    draw_house(HOUSE_GRID_POS, selected == Some(Selected::House));

    let agent_color = if eating {
        YELLOW
    } else if resting {
        GREEN
    } else {
        SKYBLUE
    };
    let (agent_x, agent_y) = grid_to_screen(AGENT_GRID_POS);
    draw_circle(agent_x, agent_y, AGENT_RADIUS, agent_color);
    if selected == Some(Selected::Agent) {
        draw_circle_lines(agent_x, agent_y, AGENT_RADIUS + 4.0, 2.0, WHITE);
    }
    draw_text(
        agent.name(),
        GRID_X,
        GRID_Y + CELL_SIZE * GRID_ROWS as f32 + 24.0,
        20.0,
        WHITE,
    );

    draw_details_panel(
        agent,
        displayed_hunger,
        displayed_energy,
        eating,
        resting,
        selected,
    );
}

/// Draws the right-side panel: whichever object is `selected`'s details, or
/// a prompt to click something when nothing is. With just one agent and
/// one house today this is a 3-way match; once there's more than one of a
/// kind, `Selected` needs an identifier and this needs to look it up.
fn draw_details_panel(
    agent: &Agent,
    displayed_hunger: f32,
    displayed_energy: f32,
    eating: bool,
    resting: bool,
    selected: Option<Selected>,
) {
    match selected {
        Some(Selected::Agent) => {
            draw_need_bar(
                NeedBar {
                    label: "hunger",
                    label_value: agent.hunger(),
                    bar_value: displayed_hunger,
                    max: HUNGER_MAX,
                },
                HUNGER_BAR_Y,
                ORANGE,
                eating,
                "ATE!",
            );
            draw_need_bar(
                NeedBar {
                    label: "energy",
                    label_value: agent.energy(),
                    bar_value: displayed_energy,
                    max: ENERGY_MAX,
                },
                ENERGY_BAR_Y,
                GREEN,
                resting,
                "RESTED!",
            );
        }
        Some(Selected::House) => {
            draw_text("House", BAR_X, HUNGER_BAR_Y - 12.0, 24.0, WHITE);
            draw_text("status: unclaimed", BAR_X, HUNGER_BAR_Y + 20.0, 20.0, GRAY);
        }
        None => {
            draw_text(
                "Click an agent or house",
                BAR_X,
                HUNGER_BAR_Y - 12.0,
                20.0,
                GRAY,
            );
            draw_text(
                "to see its details.",
                BAR_X,
                HUNGER_BAR_Y + 14.0,
                20.0,
                GRAY,
            );
        }
    }
}

/// One need's display inputs, grouped to keep `draw_need_bar`'s parameter
/// list from growing every time a need gains another displayed value.
#[derive(Clone, Copy)]
struct NeedBar<'a> {
    label: &'a str,
    /// The agent's real, unsmoothed value — shown as the numeric label.
    label_value: f32,
    /// The eased display value the bar is filled to (see `ease_toward`).
    bar_value: f32,
    max: f32,
}

/// Draws one need's label, a 0-`max` bar, at `bar_y`. While `flashing`, the
/// bar switches to yellow and `flash_label` appears beneath it — the same
/// visible-moment treatment milestone 1 used for Eat, now shared by both
/// needs instead of duplicated per need.
fn draw_need_bar(
    need: NeedBar,
    bar_y: f32,
    normal_color: Color,
    flashing: bool,
    flash_label: &str,
) {
    draw_text(
        format!("{}: {:.1}", need.label, need.label_value),
        BAR_X,
        bar_y - 12.0,
        22.0,
        WHITE,
    );
    draw_rectangle_lines(BAR_X, bar_y, BAR_W, BAR_H, 2.0, WHITE);
    let fill_w = BAR_W * (need.bar_value / need.max).clamp(0.0, 1.0);
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
