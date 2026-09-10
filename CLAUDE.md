# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

Continental is an agent-based colony sim written in Rust. The goal is **not** a playable
game — it's a demonstration that city-level behavior can emerge from individually legible
per-agent decision loops. Every agent's behavior should be explainable from its current
state at any tick. "Is this fun" is not a design criterion; "can I explain why the agent
did that" is.

## Roadmap / current milestone

Built piece by piece, in this order. Each step must compile and run before moving to the
next — do not jump ahead to a later milestone while working on an earlier one, even if the
later step seems easy or related.

1. Single agent, single need (hunger), tick loop — **DONE**, see [src/agent.rs](src/agent.rs)
2. Second need (energy) + priority comparison between needs — **DONE**, see [src/agent.rs](src/agent.rs)
3. A world object (e.g. `House`) agents can own — **DONE**, see [src/house.rs](src/house.rs)
4. Ownership constraint: an agent can only use a resource it owns (e.g. rest only works in
   your own house) — **DONE**, see [src/agent.rs](src/agent.rs). `home` is now
   `Option<House>` — agents start without one, `claim_house` grants it, and `select` only
   ever picks Rest if `home.is_some()`. Still a plain owned field, no `Rc<RefCell<>>`: at
   one-agent scale that's honest, not a shortcut. The real tension this step was flagged
   for — sharing a house safely once *multiple* agents can each own/contend for houses —
   is deferred to step 7 on purpose, where it'll need an actual design conversation
   (registry + id/reference? something else?) rather than being pre-solved here.
5. Career/job as agent state (enum), gating which production action is available —
   **DONE**, see [src/career.rs](src/career.rs). Data-model only, same as step 3's House:
   `Career::production_action()` says what's available, but nothing calls it from `tick`
   and there's no way to actually perform one yet. That's step 6.
6. Production actions (farmer produces food, builder produces houses) — **DONE**, see
   [src/agent.rs](src/agent.rs). `select` falls back to `Action::Produce(...)` when
   neither survival need is due and a career allows it; `act` applies the effect —
   Farm adds to `self.food` (a plain stockpile, nothing consumes it yet), Build produces
   a `House` but does **not** self-claim it. `act` hands a built house back to its
   caller instead, because deciding who gets it is step 7's job, not this one's.
7. Scale to multiple agents with real contention over shared resources — **DONE**, see
   [src/world.rs](src/world.rs). This is where step 4's deferred tension actually got
   resolved: `World` holds `Vec<Agent>` plus a pool of unclaimed houses, and is the one
   neutral party that moves a `House` between "in the pool" and "owned by an agent" via
   plain `Vec::pop`/`push` — never two owners at once, so no `Rc<RefCell<>>` was needed.
   Contention resolves by fixed agent order each tick (`World::tick`): a houseless agent
   claims from the pool before acting, so when houses are scarcer than demand, whichever
   agent comes first in the list gets one and the rest don't — deterministic, explainable
   from agent order, never randomness. `main.rs` still only demos a single `Agent`
   directly (not through `World`) — multi-agent is proven in [src/world.rs](src/world.rs)'s
   tests, not yet in the running UI. That's a UI follow-up, not a logic gap.
8. (stretch, later) replace flat priority scoring with something closer to production
   rules / working memory, SOAR-inspired — **DONE (partial, deliberately)**, see
   `Agent::select`. `select` is now an ordered list of condition-action rules (first
   match wins) instead of a flat match — closer to SOAR-style production rules in
   *mechanism*, and behaviorally identical to what it replaced (every prior test still
   passes unchanged). This does **not** attempt real SOAR concepts — a working-memory
   fact store, impasses/subgoaling, chunking/learning. Those are a substantial,
   speculative undertaking that a one-line stretch-goal description doesn't justify
   committing to unilaterally; if genuinely wanted, discuss the design first rather than
   expanding this silently.

All eight roadmap items are now done. Milestones 6-8 were implemented together in one
batch at the user's explicit request (an exception to the usual one-at-a-time cadence),
each still verified compiling/tested/linted before moving to the next internally. Where
this repo goes next isn't yet defined — ask before assuming a direction.

## Standing architecture convention: the decision loop

Any new agent behavior follows the same per-tick loop:

```
sense -> evaluate -> select -> act -> replan
```

- **sense**: read current world/agent state
- **evaluate**: score or compare candidate needs/actions from that state
- **select**: pick the winning action
- **act**: apply its effect
- **replan**: re-sense/re-evaluate before the next tick (no stale plans carried on faith)

Needs (hunger, energy, work quota, etc.) are plain numbers that decay over time. Priority
between needs/actions is decided by comparing or scoring those numbers — never by
randomness.

## Hard constraint: no RNG in decision logic

Any agent's behavior must be explainable from its current state. Do not use RNG to choose
between needs, actions, or priorities. RNG is only acceptable for:
- world variation (e.g. resource spawn locations)
- genuine tiebreaks between two *equal*-priority actions

If you're reaching for `rand` inside a decision/priority function, stop and reconsider —
that's almost certainly the wrong place for it.

## Working style

- Small, compiling steps. Every change should build and run before moving on.
- Don't implement later milestones early, even opportunistically.
- Don't add a dependency without discussing it first — `macroquad` (rendering) is
  currently the only one (see [Cargo.toml](Cargo.toml)); adding another is a decision,
  not a default.

## Layout

The package builds both a library and a binary from the same crate name, so `main.rs`
can depend on the sim logic via plain `use continental::...` — no `[lib]`/`[[bin]]`
section needed in Cargo.toml, Cargo infers this from the two entry points existing.

- [Cargo.toml](Cargo.toml) — package manifest, edition 2024; `macroquad` is the only dep
- [src/lib.rs](src/lib.rs) — library root; re-exports the public sim API (`Agent`,
  `House`, `Career`, `ProductionAction`, `World`, `HUNGER_MAX`, `ENERGY_MAX`)
- [src/agent.rs](src/agent.rs) — `Agent`, `Action` (including `Produce`), `Urgency`, and
  the sense/evaluate/select/act/replan tick loop (`select` as an ordered rule list, see
  roadmap step 8), plus its unit tests (`#[cfg(test)] mod tests` at the bottom of the
  file). Also owns the ownership constraint: `home: Option<House>`, granted via
  `claim_house`, gates Rest in `select`. `tick`/`act` return `Option<House>` — a produced
  house that `Agent` hands to its caller rather than keeping for itself. `tick` records
  what it did to a capped `log: VecDeque<String>` (`LOG_CAPACITY`, oldest dropped first)
  instead of printing — this crate does no I/O at all now; reading/displaying that log
  (`Agent::log()`) is `main.rs`'s job, same as any other piece of `Agent` state
- [src/house.rs](src/house.rs) — `House`, the first world object an agent can own. Fields
  get added only when a milestone actually needs them (still empty)
- [src/career.rs](src/career.rs) — `Career` (an agent's job) and `ProductionAction`
  (what a career unlocks). `Career::production_action()` is a pure query — not wired
  into the decision loop; `Agent` just carries a `career: Career` field
- [src/world.rs](src/world.rs) — `World`: `Vec<Agent>` plus the shared pool of unclaimed
  houses, and the tick loop that brokers contention over it (see roadmap step 7). The
  only place multiple agents exist together so far — `main.rs` doesn't use `World` yet
- [src/main.rs](src/main.rs) — macroquad UI only: reads `Agent` state each frame through
  its public accessors (`name()`, `hunger()`, `energy()`, `tick()`) and draws it. No
  decision logic lives here — `select`/`act`/`evaluate`/`replan` are private to
  `agent.rs` on purpose, so the UI can't reach past the public API by accident. Still
  drives a single `Agent` directly, not a `World` — multi-agent isn't in the running demo.
  Grid objects are clickable (`hit_test` + `Selected`) and the right-side panel shows
  whichever one is selected instead of always showing the one agent — `Selected` is a
  plain two-variant enum for now (one agent, one house); once there's more than one of a
  kind it'll need to carry an identifier instead of just a case. Selecting the agent also
  shows its recent-activity log (`Agent::log()`) in the panel — the terminal itself only
  prints one line at startup; there's no per-tick terminal output any more

As more need/world/agent types are added, keep following this pattern — one module per
concern under `src/`, tests colocated with the code they cover — rather than growing any
one file indefinitely. Don't pre-create module structure ahead of the code that needs it.

## Testing

Unit tests live next to the logic they cover ([src/agent.rs](src/agent.rs),
[src/career.rs](src/career.rs), [src/world.rs](src/world.rs)), in a
`#[cfg(test)] mod tests { use super::*; ... }` block, not a separate `tests/`
directory — this lets tests reach private fields/methods directly (e.g. setting
`agent.hunger`, calling `agent.select(...)` on a hand-built `Urgency`, or seeding
`world.available_houses` directly to test contention without depending on production
timing) instead of needing everything under test to be `pub`. Favor this style for new
sim-logic tests too.

When adding a new need/action, test at minimum: decay applies correctly when idle, the
action triggers exactly at its threshold, its effect clamps at the need's bounds, and —
once there's more than one need competing — that priority comparison and any tiebreak are
covered explicitly (see `higher_urgency_wins_when_both_needs_are_due` and
`equal_urgency_breaks_the_tie_toward_hunger_deterministically` for the existing pattern).
If an action has a precondition beyond its threshold (e.g. Rest requiring ownership), test
both sides of it explicitly — that the action is unavailable without the precondition
*and* available once it's met (see `resting_is_unavailable_without_a_house` /
`resting_is_available_once_a_house_is_claimed`), not just the happy path.

## Commands

- Build: `cargo build`
- Run: `cargo run`
- Release build: `cargo build --release`
- Test (all): `cargo test`
- Test (single): `cargo test <test_name>` (e.g. `cargo test eats_when_hunger_crosses_threshold`)
- Lint: `cargo clippy`
- Format: `cargo fmt`

## CI

GitHub Actions ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs on every push
and PR to `master`: `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo build` +
`cargo test`. Match this locally before pushing — run `cargo fmt`, then
`cargo clippy --all-targets --all-features -- -D warnings`, then `cargo test`. No
deployment step exists (or is planned) — this is CI only.
