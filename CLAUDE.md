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
3. A world object (e.g. `House`) agents can own — **DONE**, see [src/house.rs](src/house.rs).
   Ownership only, so far — `home: House` lives directly on `Agent` as an owned field, and
   nothing checks it before acting yet. That's step 4's job.
4. Ownership constraint: an agent can only use a resource it owns (e.g. rest only works in
   your own house) — **next up**. This step is deliberately where Rust ownership/borrowing
   is meant to get interesting — don't paper over it with `Rc<RefCell<>>` shortcuts without
   flagging the tradeoff explicitly and discussing it first.
5. Career/job as agent state (enum), gating which production action is available
6. Production actions (farmer produces food, builder produces houses)
7. Scale to multiple agents with real contention over shared resources
8. (stretch, later) replace flat priority scoring with something closer to production
   rules / working memory, SOAR-inspired

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
  `House`, `HUNGER_MAX`, `ENERGY_MAX`)
- [src/agent.rs](src/agent.rs) — `Agent`, `Action`, `Urgency`, and the
  sense/evaluate/select/act/replan tick loop, plus its unit tests
  (`#[cfg(test)] mod tests` at the bottom of the file)
- [src/house.rs](src/house.rs) — `House`, the first world object an agent can own. Fields
  get added only when a milestone actually needs them (still empty as of milestone 3)
- [src/main.rs](src/main.rs) — macroquad UI only: reads `Agent` state each frame through
  its public accessors (`name()`, `hunger()`, `energy()`, `tick()`) and draws it. No
  decision logic lives here — `select`/`act`/`evaluate`/`replan` are private to
  `agent.rs` on purpose, so the UI can't reach past the public API by accident.

As more need/world/agent types are added, keep following this pattern — one module per
concern under `src/`, tests colocated with the code they cover — rather than growing any
one file indefinitely. Don't pre-create module structure ahead of the code that needs it.

## Testing

Unit tests live next to the logic they cover (currently just [src/agent.rs](src/agent.rs)),
in a `#[cfg(test)] mod tests { use super::*; ... }` block, not a separate `tests/`
directory — this lets tests reach private fields/methods directly (e.g. setting
`agent.hunger` or calling `agent.select(...)` on a hand-built `Urgency`) instead of
needing everything under test to be `pub`. Favor this style for new sim-logic tests too.

When adding a new need/action, test at minimum: decay applies correctly when idle, the
action triggers exactly at its threshold, its effect clamps at the need's bounds, and —
once there's more than one need competing — that priority comparison and any tiebreak are
covered explicitly (see `higher_urgency_wins_when_both_needs_are_due` and
`equal_urgency_breaks_the_tie_toward_hunger_deterministically` for the existing pattern).

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
