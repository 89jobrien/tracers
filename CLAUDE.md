# CLAUDE.md — trace-lang

Agent context for working in this repository.

## what this repo is

`trace-lang` is two things:

1. A **language design** — `trace::`, a programming language where `Trace<T>` is a first-class type carrying full execution provenance. The design lives in `README.md` and the tutorial artifacts.

2. A **Rust reference implementation** — `trace-core` and `trace-task` are real, usable crates that implement the core types. They compile today and can be depended on.

## crate map

```
crates/
  trace-core/    — Trace<T>, Step, Span, Branch, TraceErr
  trace-task/    — Task, TaskStatus, Priority, TaskRegistry
  trace-agent/   — Agent trait, spawn/delegate, lifecycle escalation hooks
  trace-runtime/ — AgentRegistry, run_with_escalation, join_all, speculate
```

## key design decisions

- `Trace<T>` is not a logging concern — it is a _value type_ that carries provenance as data
- `TaskStatus::Done(TraceRef)` links every completed task to its execution trace
- `TaskRegistry::save()` is called after every state transition — crash recovery is always possible
- Errors are `TraceErr` variants, never panics. The `?` operator propagates `TraceErr` through traces
- All types are `Serialize + Deserialize` — this is a compile-time constraint, not a convention
- `Agent::run` uses `async-trait` for object-safety; lifecycle hooks (`on_low_confidence`,
  `on_budget_exceeded`, `on_step_failure`) return an `EscalationAction` rather than performing
  the escalation themselves — `spawn`/`delegate` evaluate hooks, callers act on them
- `delegate()` extends `AgentContext::delegation_chain` rather than replacing it, so a
  multi-agent handoff is always reconstructable from the context alone
- `spawn`/`delegate` in `trace-agent` take `A: Agent + ?Sized`, so `trace-runtime`'s
  `AgentRegistry<I, O>` can call them with `&dyn Agent<Input = I, Output = O>` — no logic
  duplicated between the sized and trait-object paths
- `AgentRegistry<I, O>` is keyed by `Agent::name()` and constrains all registered agents to
  one `Input`/`Output` contract — a delegation target must accept the same task shape as the
  agent that escalated to it
- `speculate`'s winner selection is a manual fold, not `Iterator::max_by` — `max_by` returns
  the _last_ element on a tie, which silently breaks "first candidate wins ties" semantics.
  Caught by a unit test (`ties_keep_first_candidate_in_order`), not by inspection — a reminder
  to actually run tests rather than eyeball async/iterator code
- `join_all`/`speculate` use `futures::future::join_all` (concurrent on one task), not
  `tokio::spawn` (parallel across threads) — true parallelism needs `'static` agents behind
  `Arc`, tracked as a follow-up rather than silently assumed

## workspace commands

```bash
cargo check --workspace          # fast feedback
cargo test --workspace           # run all tests
cargo clippy --workspace         # lint
cargo doc --workspace --open     # browse docs
```

## conventions

- Hexagonal architecture: keep domain logic in `trace-core` and `trace-task` free of any runtime/IO
- `trace-core` must not depend on `trace-task` (the dependency flows one way)
- Builder pattern for all public types (see `Task::with_priority()`, `Step::with_confidence()`)
- No `unwrap()` in library code — use `?` or explicit error handling

## planned crates

| crate       | description                                       | status  |
| ----------- | ------------------------------------------------- | ------- |
| `trace-cli` | `doob`-style CLI for inspecting trace checkpoints | planned |

## deferred (explicitly out of scope for now)

- **Thread-parallel `join_all`/`speculate`** — current implementation is concurrent-on-one-task
  via `futures::future::join_all`. A `tokio::spawn`-based variant needs `'static` agents
  (typically `Arc<dyn Agent<...>>` everywhere), which is a bigger API change.
- **Shared/global step budget across concurrent branches** — `AgentContext::budget` is
  per-run only. A `SharedBudget` that caps total steps across all `speculate` candidates or
  `join_all` invocations would need a new field threaded through `AgentContext` and
  `record_step()`.

## related repos

- `agent_trace` — the personal crate that inspired `Trace<T>`; design reference
- `doob` — agent-first task CLI; `trace-task` could serve as its storage model
- `orca-strait` — parallel TDD orchestrator; could consume `TaskRegistry::ready_tasks()`
- `atelier` — Claude Code workflow plugin; drives this repo's development sessions
