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

- [x] Single agent, single need (hunger), tick loop
- [x] Second need (energy) + priority comparison between needs
- [ ] A world object (e.g. `House`) agents can own
- [ ] Ownership constraint: an agent can only use a resource it owns (e.g. rest only works
      in your own house) — deliberately leans into Rust ownership/borrowing rather than
      papering over it with `Rc<RefCell<>>` shortcuts
- [ ] Career/job as agent state (enum), gating which production action is available
- [ ] Production actions (farmer produces food, builder produces houses)
- [ ] Scale to multiple agents with real contention over shared resources
- [ ] (stretch, later) replace flat priority scoring with something closer to production
      rules / working memory, SOAR-inspired

## Building and running

```
cargo run
```

builds and opens a window showing the agent ticking once a second: hunger and energy each
drawn as a number and a bar, with a brief flash when the agent eats or rests.

```
cargo test
```

runs the unit tests covering the decision loop's logic.

## License

MIT — see [LICENSE](LICENSE).
