# CLAUDE.md — trace-lang

Agent context for working in this repository.

## what this repo is

`trace-lang` is two things:

1. A **language design** — `trace::`, a programming language where `Trace<T>` is a first-class type carrying full execution provenance. The design lives in `README.md` and the tutorial artifacts.

2. A **Rust reference implementation** — the crates under `crates/` are real and usable: five libraries implementing the core types plus a `trace` CLI. They compile today, are covered by tests, benches, and fuzz targets, and can be depended on.

## crate map

```
crates/
  core/    — Trace<T>, Step, Span, Branch, TraceErr
  task/    — Task, TaskStatus, Priority, TaskRegistry
  agent/   — Agent trait, spawn/delegate, lifecycle escalation hooks
  runtime/ — AgentRegistry, run_with_escalation, join_all, speculate, speculate_race
  cli/     — `trace` binary: list/show/chain/diff over checkpoint files
```

Plus, outside `crates/`:

```
examples/ — runnable end-to-end walkthroughs (publish = false, in the workspace)
fuzz/     — cargo-fuzz targets (its own workspace; nightly only)
```

## key design decisions

- `Trace<T>` is not a logging concern — it is a _value type_ that carries provenance as data
- `TaskStatus::Done(TraceRef)` links every completed task to its execution trace
- `TaskRegistry::save()` is called after every state transition — crash recovery is always possible
- `TaskRegistry` persists through the `CheckpointStore` trait (`crates/task/src/checkpoint/`), never
  `std::fs` directly — `FileCheckpointStore` is one adapter among possible others (S3, a database, an
  in-memory buffer for tests), keeping the domain crate free of concrete I/O per the hexagonal rule below
- Errors are `TraceErr` variants, never panics. The `?` operator propagates `TraceErr` through traces
- All types are `Serialize + Deserialize` — this is a compile-time constraint, not a convention
- The workspace enables serde_json's `float_roundtrip` feature, which is **off by default**.
  Without it serde_json's fast float parser loses an ULP on some extreme-exponent `f64`s, so a
  `Step::confidence` or `StepCost::dollars` drifts slightly on every checkpoint save/load cycle.
  Found by the `trace_roundtrip` fuzz target; pinned by a unit test in `crates/core/src/trace.rs`.
  A trace is a record, not an approximation of one — do not drop the feature
- `StepCost` records dollars at step time rather than computing them lazily against a pricing
  table: what a run cost is a fact about the past, and must not change when a provider reprices
- `TraceGraph` edges are recorded explicitly by the caller, not inferred. Automatic edges would
  need producer identity threaded through every `spawn`/`delegate` call, and a
  partially-populated lineage graph is worse than an empty one
- `Contract` violations produce `TraceErr::ContractViolated`, distinct from `ToolFailed`, so
  `on_step_failure` can tell "the tool broke" (retry) from "the tool worked and returned
  something forbidden" (escalate). Checking is explicit inside `Agent::run` — there is no global
  toggle, so a hot-loop agent simply doesn't call it
- `TaskStatus::Paused(ApprovalRequest)` needs no storage of its own: `TaskRegistry` already
  checkpoints every transition. `pause` keeps `assigned_to` (unlike `complete`/`fail`) because
  the work is suspended, not finished. An approval returns the task to `Pending`; a rejection is
  terminal, failing against the same partial trace the approver saw
- A lifecycle hook is `fn(&self) -> EscalationAction` and cannot see its trace, so an
  `ApprovalRequest` it raises is built `unattached` and `spawn`/`delegate` stamp the real
  `TraceRef` on the way out — a caller must never get a question about a run it can't look up
- `Agent::run` uses `async-trait` for object-safety; lifecycle hooks (`on_low_confidence`,
  `on_budget_exceeded`, `on_step_failure`) return an `EscalationAction` rather than performing
  the escalation themselves — `spawn`/`delegate` evaluate hooks, callers act on them
- `delegate()` extends `AgentContext::delegation_chain` rather than replacing it, so a
  multi-agent handoff is always reconstructable from the context alone
- `spawn`/`delegate` in `trace-lang-agent` take `A: Agent + ?Sized`, so `trace-lang-runtime`'s
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
- `speculate_race` cancels losing candidates by dropping a `FuturesUnordered`, and records them
  as `BranchOutcome::Cancelled` with **no** confidence — a cancelled candidate never reported a
  score, and writing `0.0` would claim it was bad rather than unfinished. Its `threshold` is
  clamped to `[0.0, 1.0]` so a negative value can't let a failed candidate (scored `-1.0`) win
- `run_with_escalation` returns `Emit` and `RequireApproval` in `RunOutcome::unresolved`. Only
  `Delegate` is resolvable by a registry; dropping the others silently lost a hook's decision

## workspace commands

```bash
cargo check --workspace          # fast feedback
cargo test --workspace           # run all tests (also builds every example)
cargo clippy --workspace         # lint
cargo doc --workspace --open     # browse docs
cargo bench -p trace-lang-core   # criterion, crates/core/benches/
mise run fuzz                    # both fuzz targets, 60s each (nightly + cargo-fuzz)
mise run changelog               # regenerate CHANGELOG.md via git-cliff
taskit inspect                   # thresholds: clippy, tests, versions, TODO count
```

MSRV is pinned at `rust-version = "1.88"` in the root manifest and enforced by
the `msrv` job in `.github/workflows/ci.yml`, which reads that exact line.
1.87 rejects the let-chains in `crates/agent/src/context.rs`.

## conventions

- Hexagonal architecture: keep domain logic in `trace-lang-core` and `trace-lang-task` free of any runtime/IO
- `trace-lang-core` must not depend on `trace-lang-task` (the dependency flows one way)
- Builder pattern for all public types (see `Task::with_priority()`, `Step::with_confidence()`)
- No `unwrap()` in library code — use `?` or explicit error handling
- Anything ordered that a person or a diff will read must be deterministic. `TaskRegistry` is a
  `HashMap`, so `trace-cli` and `TraceGraph::critical_path` sort explicitly rather than
  inheriting iteration order
- Tie-breaking keeps the *first* candidate. `Iterator::max_by` returns the last on a tie, so
  `speculate`, `speculate_race`, `Trace::priciest_steps` and `TraceGraph::critical_path` all use
  a manual fold or a stable sort

## planned crates

None currently — `trace-cli` shipped; see the crate map above.

## deferred (explicitly out of scope for now)

These two are the only remaining `TODO` markers in the tree — `taskit.toml`'s
`max_todo_fixme` is set to the count they cost, and should come down, not up.

- **Thread-parallel `join_all`/`speculate`/`speculate_race`** — the current implementations are
  concurrent-on-one-task (`futures::future::join_all`, `FuturesUnordered`). A `tokio::spawn`-based
  variant needs `'static` agents (typically `Arc<dyn Agent<...>>` everywhere), which is a bigger
  API change.
- **Shared/global step budget across concurrent branches** — `AgentContext::budget` is
  per-run only. A `SharedBudget` that caps total steps across all `speculate` candidates or
  `join_all` invocations would need a new field threaded through `AgentContext` and
  `record_step()`.

Everything else FEATURES.md proposed is implemented: the step cost ledger (#3), pausable traces
(#5), contracts (#6), `TraceGraph` (#7), and `speculate_race` (#8). `Trace<T, Trust>` (#1),
confidence decay (#2), and deterministic replay (#4) remain design proposals.

## related repos

- `agent_trace` — the personal crate that inspired `Trace<T>`; design reference
- `doob` — agent-first task CLI; `trace-lang-task` could serve as its storage model
- `orca-strait` — parallel TDD orchestrator; could consume `TaskRegistry::ready_tasks()`
- `atelier` — Claude Code workflow plugin; drives this repo's development sessions
