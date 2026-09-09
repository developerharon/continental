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

1. Single agent, single need (hunger), tick loop — **DONE**, see [src/main.rs](src/main.rs)
2. Second need (energy) + priority comparison between needs — **next up**
3. A world object (e.g. `House`) agents can own
4. Ownership constraint: an agent can only use a resource it owns (e.g. rest only works in
   your own house). This step is deliberately where Rust ownership/borrowing is meant to
   get interesting — don't paper over it with `Rc<RefCell<>>` shortcuts without flagging
   the tradeoff explicitly and discussing it first.
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
- Don't add a dependency without discussing it first — the crate currently has zero
  dependencies (see [Cargo.toml](Cargo.toml)); adding one is a decision, not a default.

## Layout

Single binary crate, no dependencies yet:
- [Cargo.toml](Cargo.toml) — package manifest, edition 2024, no deps
- [src/main.rs](src/main.rs) — entry point; holds the `Agent` type and milestone 1's
  sense/evaluate/select/act/replan tick loop (single agent, hunger need only)

No other modules, tests, or supporting files exist yet. As agent/world/need types are
added, prefer splitting them into modules under `src/` rather than growing `main.rs`
indefinitely — but don't pre-create module structure ahead of the code that needs it.

## Commands

- Build: `cargo build`
- Run: `cargo run`
- Release build: `cargo build --release`
- Test (all): `cargo test`
- Test (single): `cargo test <test_name>`
- Lint: `cargo clippy`
- Format: `cargo fmt`

## CI

GitHub Actions ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs on every push
and PR to `master`: `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo build` +
`cargo test`. Match this locally before pushing — run `cargo fmt`, then
`cargo clippy --all-targets --all-features -- -D warnings`, then `cargo test`. No
deployment step exists (or is planned) — this is CI only.
