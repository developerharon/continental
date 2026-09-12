//! macroquad UI over `continental`'s `World`. It reads sim state each frame
//! through public accessors and draws it — it adds no decision logic of its
//! own.
//!
//! Two agents now run side by side through a real `World` (previously this
//! demo drove a single bare `Agent` directly — `World`'s multi-agent
//! contention was proven only in its own tests, and wiring it into the
//! running UI was flagged as the follow-up that finally happens here).
//! Agents actually walk the grid: each has its own `House` (position it
//! owns, where Rest happens) and there's one shared `Restaurant` (where Eat
//! happens) — an agent that isn't already standing wherever its selected
//! action requires moves exactly one cell closer per tick instead of
//! teleporting or acting from a distance (see `Agent::tick` in the sim
//! crate). Both houses and the restaurant are real, positioned world
//! objects now, not the unconnected placeholder square this file used to
//! draw.
//!
//! Grid objects are still clickable (`hit_test` + `Selected`), and the
//! right-side panel shows whichever thing is selected. With more than one
//! agent and more than one house, `Selected` now carries the index of
//! which one (into `world.agents()`) instead of being a bare two-variant
//! enum, same as the code previously flagged it would need to.
//!
//! Deliberately left alone here: houses sitting unclaimed in `World`'s
//! pool aren't drawn — neither demo agent has a `Builder` career, so the
//! pool never has anything in it. That's for whenever production gets
//! wired into this same UI.

use continental::{Agent, ENERGY_MAX, HUNGER_MAX, House, Restaurant, World};
use macroquad::prelude::*;

/// Something on the grid the player can click to see details about in the
/// side panel. Carries the clicked object's index into `world.agents()` —
/// a house is identified by the index of the agent that owns it, since
/// there's exactly one house per agent and no unowned ones are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selected {
    Agent(usize),
    House(usize),
    Restaurant,
}

/// Per-agent display-only state the render loop tracks alongside `World`'s
/// real `Agent`s: eased bar fills (see `ease_toward`) and action-flash
/// timers. Indexed in parallel with `world.agents()`.
struct AgentVisual {
    eat_flash: f32,
    rest_flash: f32,
    /// Eases toward the agent's real `hunger()`/`energy()` every frame
    /// instead of jumping the moment a tick changes them, so e.g. Rest
    /// visibly fills the energy bar up rather than snapping it. The
    /// printed numbers in the panel still read the agent's real state —
    /// only the bar fill is smoothed.
    displayed_hunger: f32,
    displayed_energy: f32,
}

/// Seconds of real time between ticks, so needs and movement visibly
/// change over time instead of the whole run flashing by in one frame.
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

/// Where the selected agent's recent-activity log starts, below the bars.
const LOG_HEADER_Y: f32 = 232.0;
const LOG_LINE_Y: f32 = 254.0;
const LOG_LINE_HEIGHT: f32 = 18.0;
const LOG_FONT_SIZE: f32 = 14.0;
/// How many of the agent's most recent log lines to show at once. The
/// agent itself remembers more than this (`LOG_CAPACITY` in agent.rs) —
/// this is just how much fits in the panel, a display choice, not a
/// storage limit.
const LOG_LINES_SHOWN: usize = 6;

/// Grid the agents and world objects sit on. Coordinates are (col, row),
/// each in `0..GRID_COLS`/`0..GRID_ROWS`.
const GRID_COLS: i32 = 10;
const GRID_ROWS: i32 = 10;
const CELL_SIZE: f32 = 32.0;
const GRID_X: f32 = 20.0;
const GRID_Y: f32 = 20.0;

/// Each agent's starting position, its own (distinct) house's position,
/// and its name — paired up positionally, so `AGENT_START_POSITIONS[i]`
/// is where `HOUSE_POSITIONS[i]`'s owner starts. Chosen far enough from
/// both the restaurant and the agent's own house that a run visibly shows
/// walking rather than starting already there.
const AGENT_NAMES: [&str; 2] = ["Agent-0", "Agent-1"];
const AGENT_START_POSITIONS: [(i32, i32); 2] = [(1, 1), (8, 1)];
const HOUSE_POSITIONS: [(i32, i32); 2] = [(1, 8), (8, 8)];
/// Where the one shared `Restaurant` sits — reachable by every agent,
/// regardless of who owns what house.
const RESTAURANT_POSITION: (i32, i32) = (4, 4);

/// Radius an agent is drawn at and hit-tested against for clicks.
const AGENT_RADIUS: f32 = 12.0;
/// Side length a house/restaurant square is drawn at and hit-tested
/// against.
const SQUARE_SIZE: f32 = CELL_SIZE - 8.0;

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
    println!("Continental is running. Close the window to quit.");

    let mut agents: Vec<Agent> = AGENT_NAMES
        .iter()
        .zip(AGENT_START_POSITIONS)
        .map(|(name, position)| Agent::new(name, position))
        .collect();
    for (agent, house_position) in agents.iter_mut().zip(HOUSE_POSITIONS) {
        agent.claim_house(House::new(house_position));
    }
    let mut world = World::new(agents, Restaurant::new(RESTAURANT_POSITION));

    let mut visuals: Vec<AgentVisual> = world
        .agents()
        .iter()
        .map(|agent| AgentVisual {
            eat_flash: 0.0,
            rest_flash: 0.0,
            displayed_hunger: agent.hunger(),
            displayed_energy: agent.energy(),
        })
        .collect();

    let mut tick_timer = 0.0;
    // Nothing selected until the player clicks something — see `Selected`.
    let mut selected: Option<Selected> = None;

    loop {
        let dt = get_frame_time();
        tick_timer += dt;

        if is_mouse_button_pressed(MouseButton::Left) {
            // Replaces whatever was selected, including with nothing —
            // clicking empty grid space (or anywhere else) deselects.
            selected = hit_test(&world, mouse_position());
        }

        if tick_timer >= TICK_INTERVAL_SECS {
            tick_timer -= TICK_INTERVAL_SECS;
            let hunger_before: Vec<f32> = world.agents().iter().map(Agent::hunger).collect();
            let energy_before: Vec<f32> = world.agents().iter().map(Agent::energy).collect();
            // World brokers house contention and passes each agent the
            // shared restaurant's position — this file adds no decision
            // logic, just observes the result.
            world.tick();
            // tick() doesn't report which Action it picked (or whether it
            // moved instead of acting), so infer which one ran from its
            // one observable effect: Eat is the only thing that can drop
            // hunger, Rest the only thing that can raise energy — walking,
            // Idle, and replan only ever push both the other way.
            for (i, agent) in world.agents().iter().enumerate() {
                if agent.hunger() < hunger_before[i] {
                    visuals[i].eat_flash = ACTION_FLASH_SECS;
                }
                if agent.energy() > energy_before[i] {
                    visuals[i].rest_flash = ACTION_FLASH_SECS;
                }
            }
        }

        for (i, agent) in world.agents().iter().enumerate() {
            let visual = &mut visuals[i];
            visual.eat_flash = (visual.eat_flash - dt).max(0.0);
            visual.rest_flash = (visual.rest_flash - dt).max(0.0);
            visual.displayed_hunger =
                ease_toward(visual.displayed_hunger, agent.hunger(), dt, BAR_ANIM_SPEED);
            visual.displayed_energy =
                ease_toward(visual.displayed_energy, agent.energy(), dt, BAR_ANIM_SPEED);
        }

        draw_scene(&world, &visuals, selected);

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
/// `mouse_position()`) falls within — checked in front-to-back draw order
/// (agents are drawn on top, so checked first), against every agent's
/// *current* (possibly mid-walk) position rather than a fixed spot.
fn hit_test(world: &World, (x, y): (f32, f32)) -> Option<Selected> {
    for (i, agent) in world.agents().iter().enumerate() {
        let (agent_x, agent_y) = grid_to_screen(agent.position());
        if (x - agent_x).hypot(y - agent_y) <= AGENT_RADIUS {
            return Some(Selected::Agent(i));
        }
    }

    let half = SQUARE_SIZE / 2.0;
    for (i, agent) in world.agents().iter().enumerate() {
        if let Some(house) = agent.home() {
            let (house_x, house_y) = grid_to_screen(house.position());
            if (x - house_x).abs() <= half && (y - house_y).abs() <= half {
                return Some(Selected::House(i));
            }
        }
    }

    let (rest_x, rest_y) = grid_to_screen(world.restaurant().position());
    if (x - rest_x).abs() <= half && (y - rest_y).abs() <= half {
        return Some(Selected::Restaurant);
    }

    None
}

/// Draws `name` as a small nameplate centered under a grid object's point
/// `(x, y)` — e.g. an agent's circle center. Uses `measure_text` to center
/// it horizontally rather than guessing an offset, so it stays centered
/// regardless of how long the name is.
fn draw_agent_label(name: &str, x: f32, y: f32) {
    const FONT_SIZE: f32 = 16.0;
    let dims = measure_text(name, None, FONT_SIZE as u16, 1.0);
    draw_text(
        name,
        x - dims.width / 2.0,
        y + AGENT_RADIUS + dims.height + 4.0,
        FONT_SIZE,
        WHITE,
    );
}

/// Draws the grid the agents and world objects sit on: an outer border plus
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

/// Draws one `SQUARE_SIZE` world-object square at `pos` in `color`, with a
/// highlight ring when `selected` is true — shared by houses and the
/// restaurant, which differ only in color and what selecting them shows.
fn draw_square(pos: (i32, i32), color: Color, selected: bool) {
    let (cx, cy) = grid_to_screen(pos);
    let half = SQUARE_SIZE / 2.0;
    draw_rectangle(cx - half, cy - half, SQUARE_SIZE, SQUARE_SIZE, color);
    if selected {
        draw_rectangle_lines(
            cx - half - 3.0,
            cy - half - 3.0,
            SQUARE_SIZE + 6.0,
            SQUARE_SIZE + 6.0,
            2.0,
            WHITE,
        );
    }
}

/// Draws the grid, every house, the restaurant, every agent, and the
/// right-side details panel. Read-only: takes `&World` and never mutates
/// or advances simulation state. `selected` is whatever the player last
/// clicked (see `Selected`, `hit_test`) — it decides what the details
/// panel shows and which grid object (if any) gets a highlight ring.
fn draw_scene(world: &World, visuals: &[AgentVisual], selected: Option<Selected>) {
    clear_background(Color::from_rgba(24, 24, 28, 255));

    draw_grid();

    for (i, agent) in world.agents().iter().enumerate() {
        if let Some(house) = agent.home() {
            draw_square(house.position(), GRAY, selected == Some(Selected::House(i)));
        }
    }
    draw_square(
        world.restaurant().position(),
        ORANGE,
        selected == Some(Selected::Restaurant),
    );

    for (i, agent) in world.agents().iter().enumerate() {
        let visual = &visuals[i];
        let agent_color = if visual.eat_flash > 0.0 {
            YELLOW
        } else if visual.rest_flash > 0.0 {
            GREEN
        } else {
            SKYBLUE
        };
        let (agent_x, agent_y) = grid_to_screen(agent.position());
        draw_circle(agent_x, agent_y, AGENT_RADIUS, agent_color);
        if selected == Some(Selected::Agent(i)) {
            draw_circle_lines(agent_x, agent_y, AGENT_RADIUS + 4.0, 2.0, WHITE);
        }
        draw_agent_label(agent.name(), agent_x, agent_y);
    }

    draw_details_panel(world, visuals, selected);
}

/// Draws the right-side panel: whichever object is `selected`'s details, or
/// a prompt to click something when nothing is.
fn draw_details_panel(world: &World, visuals: &[AgentVisual], selected: Option<Selected>) {
    match selected {
        Some(Selected::Agent(i)) => {
            let agent = &world.agents()[i];
            let visual = &visuals[i];
            draw_need_bar(
                NeedBar {
                    label: "hunger",
                    label_value: agent.hunger(),
                    bar_value: visual.displayed_hunger,
                    max: HUNGER_MAX,
                },
                HUNGER_BAR_Y,
                ORANGE,
                visual.eat_flash > 0.0,
                "ATE!",
            );
            draw_need_bar(
                NeedBar {
                    label: "energy",
                    label_value: agent.energy(),
                    bar_value: visual.displayed_energy,
                    max: ENERGY_MAX,
                },
                ENERGY_BAR_Y,
                GREEN,
                visual.rest_flash > 0.0,
                "RESTED!",
            );
            draw_agent_log(agent);
        }
        Some(Selected::House(i)) => {
            let agent = &world.agents()[i];
            draw_text(
                format!("{}'s House", agent.name()),
                BAR_X,
                HUNGER_BAR_Y - 12.0,
                24.0,
                WHITE,
            );
            if let Some(house) = agent.home() {
                draw_text(
                    format!("position: {:?}", house.position()),
                    BAR_X,
                    HUNGER_BAR_Y + 20.0,
                    20.0,
                    GRAY,
                );
            }
        }
        Some(Selected::Restaurant) => {
            draw_text("Restaurant", BAR_X, HUNGER_BAR_Y - 12.0, 24.0, WHITE);
            draw_text(
                format!("status: shared, at {:?}", world.restaurant().position()),
                BAR_X,
                HUNGER_BAR_Y + 20.0,
                20.0,
                GRAY,
            );
        }
        None => {
            draw_text(
                "Click an agent, house,",
                BAR_X,
                HUNGER_BAR_Y - 12.0,
                20.0,
                GRAY,
            );
            draw_text("or the restaurant.", BAR_X, HUNGER_BAR_Y + 14.0, 20.0, GRAY);
        }
    }
}

/// Draws the selected agent's `LOG_LINES_SHOWN` most recent log lines
/// (newest first), below its need bars. This is the replacement for the
/// terminal spam `Agent::tick` used to produce directly — see module docs.
fn draw_agent_log(agent: &Agent) {
    draw_text("recent activity:", BAR_X, LOG_HEADER_Y, 18.0, GRAY);

    for (i, line) in agent.log().rev().take(LOG_LINES_SHOWN).enumerate() {
        let y = LOG_LINE_Y + i as f32 * LOG_LINE_HEIGHT;
        draw_text(line, BAR_X, y, LOG_FONT_SIZE, LIGHTGRAY);
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
