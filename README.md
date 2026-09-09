# Continental

Continental is an agent-based colony sim written in Rust. The goal is **not** a playable
game — it's a demonstration that city-level behavior can emerge from individually legible
per-agent decision loops. Every agent's behavior should be explainable from its current
state at any tick.

## Core mechanic

Each agent runs a decision loop every tick:

```
sense -> evaluate -> select -> act -> replan
```

Needs (hunger, energy, work quota, etc.) are plain numbers that decay over time. Priority
between needs/actions is decided by comparing or scoring those numbers — **never** by
randomness. RNG is only used for world variation (e.g. resource spawn locations) or genuine
tiebreaks between equal-priority actions.

## Roadmap

Built piece by piece, in order — each step compiles and runs before moving to the next:

1. Single agent, single need (hunger), tick loop — **DONE**, see `src/main.rs`
2. Second need (energy) + priority comparison between needs — **next up**
3. A world object (e.g. `House`) agents can own
4. Ownership constraint: an agent can only use a resource it owns (e.g. rest only works in
   your own house) — deliberately leans into Rust ownership/borrowing rather than papering
   over it with `Rc<RefCell<>>` shortcuts
5. Career/job as agent state (enum), gating which production action is available
6. Production actions (farmer produces food, builder produces houses)
7. Scale to multiple agents with real contention over shared resources
8. (stretch, later) replace flat priority scoring with something closer to production
   rules / working memory, SOAR-inspired

## Building and running

```
cargo build
cargo run
```

Tests:

```
cargo test
```

## CI

GitHub Actions runs `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo build` +
`cargo test` on every push and PR to `master` (see `.github/workflows/ci.yml`).

## Project layout

Single binary crate, no dependencies yet:
- `Cargo.toml` — package manifest, edition 2024, no deps
- `src/main.rs` — entry point; holds the `Agent` type and milestone 1's
  sense/evaluate/select/act/replan tick loop (single agent, hunger need only)
