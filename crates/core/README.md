# trace-lang-core

[![crates.io](https://img.shields.io/crates/v/trace-lang-core.svg)](https://crates.io/crates/trace-lang-core)
[![docs.rs](https://docs.rs/trace-lang-core/badge.svg)](https://docs.rs/trace-lang-core)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://github.com/89jobrien/trace-lang)

Core `Trace<T>` type — reasoning provenance as a first-class value.

Every computation in a `trace::` program returns `Trace<T>` rather than a bare `T`.
The trace carries the full causal chain: steps taken, branches rejected, confidence
at each decision point, and RAII-style span timing. `trace-lang-core` has no dependency
on any other crate in this workspace — everything else (`trace-lang-task`,
`trace-lang-agent`, `trace-lang-runtime`) is built on top of it.

## Install

```toml
[dependencies]
trace-lang-core = { path = "../core" }  # or a version once published
```

## Quick start

```rust
use trace_lang_core::{Trace, Step, TraceErr};

let mut t = Trace::new("hello world");
t.push_step(Step::named("greet").with_confidence(0.97));

assert_eq!(t.value(), Some(&"hello world"));
assert_eq!(t.causal_chain().len(), 1);
```

## Types

### `Trace<T>`

The core type. Wraps an `Option<T>` value (or a `TraceErr` on failure) alongside
the ordered list of `Step`s that produced it.

| method | returns | purpose |
| --- | --- | --- |
| `Trace::new(value)` | `Self` | construct a successful trace |
| `Trace::failed(err)` | `Self` | construct a failed trace |
| `Trace::merge(lhs, rhs)` | `Self` | concatenate two traces' step chains; left value wins |
| `value()` / `into_value()` | `Option<&T>` / `Option<T>` | borrow or take the carried value |
| `error()` | `Option<&TraceErr>` | borrow the failure, if any |
| `is_ok()` | `bool` | true iff the trace carries a value |
| `trace_ref()` | `TraceRef` | stable id for storage in a `Task` |
| `push_step(step)` | — | append to the causal chain (the primary mutation point) |
| `causal_chain()` | `&[Step]` | every step in execution order |
| `rejected_branches()` | `Vec<&Step>` | steps explicitly rejected via `Step::rejected` |
| `all_branches()` | `Vec<&Branch>` | every `Branch` across all steps (from `speculate`) |
| `bottlenecks()` | `Vec<&Step>` | steps sorted slowest-first |
| `low_confidence()` | `Vec<&Step>` | steps below confidence `0.7` |
| `low_confidence_below(threshold)` | `Vec<&Step>` | steps below an arbitrary threshold |
| `total_cost()` | `StepCost` | summed tokens and dollars across every step that recorded a cost |
| `priciest_steps()` | `Vec<&Step>` | steps sorted by dollars, then tokens, descending |

`Trace<T>` implements `From<Trace<T>> for Result<T, TraceErr>`, so `let v: T = trace?;`
works in any function returning `Result<_, TraceErr>` — the same ergonomics as `Result`
itself, but with a logged causal chain attached.

### `Step`

A single unit of reasoning — one `observe`, `branch`, or `emit`. Carries a name,
an optional confidence score (clamped to `[0.0, 1.0]`), an optional duration, a
timestamp, an `outcome` (`Taken` / `Rejected { reason }` / `Failed { message }`),
and any `Branch`es considered during that step (populated by `speculate`).

```rust
use trace_lang_core::Step;
use std::time::Duration;

let step = Step::named("search")
    .with_confidence(0.85)
    .with_duration(Duration::from_millis(120))
    .with_note("used cached index");
```

### `StepCost`

What a step cost to produce: `input_tokens`, `output_tokens`, and an optional
`dollars`. Implements `Add`/`AddAssign`/`Sum` (token counts saturate; `dollars` is
`Some` iff at least one operand recorded one, so a partially-priced trace reports
the spend it does know about rather than `None`).

```rust
use trace_lang_core::{Step, StepCost};

let step = Step::named("summarize")
    .with_cost(StepCost::new(1_200, 340).with_dollars(0.0042));
```

The dollar figure is recorded at step time rather than computed lazily against a
pricing table: a checkpoint written today should still report what the run actually
cost after the provider reprices. `Step::cost` is `#[serde(default)]`, so
checkpoints written before the ledger existed still deserialize.

### `Branch`

A path that was *considered* during a step. `speculate` produces one `Branch` per
candidate, marking the winner `Taken` and the rest `Rejected { reason }`.
`speculate_race` adds a third outcome: `Cancelled { reason }`, for a candidate
dropped before it finished. A cancelled branch carries **no** confidence — it never
reported one, and `0.0` would say it was bad rather than unfinished.

### `TraceGraph`

Lineage *between* traces. `causal_chain()` explains one run; `TraceGraph` explains
which run caused which — the same idea as `Task::depends_on`, one level down.

| method | purpose |
| --- | --- |
| `record_node(node)` | insert or replace a `TraceNode` |
| `record_edge(producer, consumer)` | record that one trace's output fed another; ignores self-edges and duplicates, and creates unlabelled placeholders for unknown refs |
| `node(&trace_ref)` / `edges()` / `len()` / `is_empty()` | accessors |
| `downstream_of(&trace_ref)` | every trace transitively caused by this one, breadth-first |
| `upstream_of(&trace_ref)` | every trace that transitively fed into this one |
| `critical_path()` | the chain that dominates latency — ranked by summed node duration, tie-broken by hop count |

`TraceNode::from_trace(&trace, label)` builds a node timed by summing the trace's
step durations. Edges are recorded explicitly rather than inferred: automatic edges
would need producer identity threaded through every `spawn`/`delegate` call, and a
partially-populated lineage graph is worse than an empty one.

```rust
use trace_lang_core::{TraceGraph, TraceNode};

let mut graph = TraceGraph::new();
graph.record_node(TraceNode::from_trace(&fetch, "Fetcher"));
graph.record_node(TraceNode::from_trace(&summarize, "Summarizer"));
graph.record_edge(fetch.trace_ref(), summarize.trace_ref());

assert_eq!(graph.downstream_of(&fetch.trace_ref())[0].label, "Summarizer");
```

### `ApprovalRequest` / `ApprovalDecision`

A question a pipeline stopped to ask a person, and the answer. Used by
`EscalationAction::RequireApproval` in `trace-lang-agent` and
`TaskStatus::Paused` in `trace-lang-task`.

An `ApprovalRequest` carries the question, arbitrary JSON context, a stable id for
an external channel to correlate against, `requested_at`/`age()`, and the
`TraceRef` of the partial trace that reached the pause — an approver is never asked
to decide without the provenance that led there.

A lifecycle hook can't see its own trace, so `ApprovalRequest::unattached(question)`
leaves the `TraceRef` blank and `spawn`/`delegate` stamp the real one before handing
the escalation back. `attach` is one-way: a request that already names its trace is
left alone.

`ApprovalDecision` is `approve(by)` / `approve_with_note(by, note)` /
`reject(by, reason)`. Both variants record *who* decided — an approval nobody is
accountable for is not much of an approval.

### `TraceErr`

Every error in `trace::` is a named `TraceErr` variant — there are no silent panics.
Implements `std::error::Error` (via `thiserror`) and `miette::Diagnostic`, so errors
carry a diagnostic code (`trace::budget_exhausted`, `trace::low_confidence`, etc.) and
render with `miette`'s fancy formatting.

| variant | meaning |
| --- | --- |
| `Rejected(String)` | a step called `reject()` |
| `ToolFailed { tool, message }` | an external tool call failed |
| `BudgetExhausted { steps }` | agent exceeded its declared step budget |
| `DelegationFailed { trace_id, message }` | a delegated agent returned an error |
| `LowConfidence { score, threshold }` | a step's confidence fell below threshold |
| `Timeout { duration }` | a step exceeded its time limit |
| `ApprovalDenied { by, reason }` | a human rejected an `ApprovalRequest` |
| `ContractViolated { message }` | a step succeeded but broke an invariant it declared |
| `Serde(String)` | serialization/deserialization failure |
| `Other(String)` | catch-all |

### `Span`

RAII-style timing helper. Timing begins on `Span::start(name)` and is captured on
`finish()`.

> **Not auto-wired:** `Span` does not populate `Step::duration` on its own —
> callers must pair `Span::finish()`'s return value with `Step::with_duration`
> themselves.

```rust
use trace_lang_core::Span;

let span = Span::start("search");
// ... do work ...
let duration = span.finish();
```

## Serialization

Every public type derives `Serialize + Deserialize` — this is a compile-time
constraint, not a convention, since `trace-lang-task` checkpoints traces alongside
`TaskRegistry`.

The workspace enables serde_json's `float_roundtrip` feature, which is **off by
default**. Without it, serde_json's fast float parser loses an ULP on some
extreme-exponent `f64`s, so a `confidence` or `dollars` value drifts a little on
every save/load cycle. The `trace_roundtrip` fuzz target found this; a unit test in
`src/trace.rs` pins both offending values bit-for-bit. A trace is a record, not an
approximation of one.

## Testing

Unit tests live alongside each module (`#[cfg(test)] mod tests`). `Step` and
`Branch`'s confidence-clamping is additionally covered by `proptest` property tests
verifying `with_confidence` always lands in `[0.0, 1.0]` regardless of input, and
`TraceGraph::critical_path` by one asserting it always returns a real, non-repeating
walk for any edge set — including cyclic ones.

```bash
cargo test  -p trace-lang-core
cargo bench -p trace-lang-core   # criterion; benches/trace.rs
```

Benchmarks cover the per-step cost of `push_step`, the inspection queries at
10/100/1,000 steps, serde round-trips (on the hot path, since `TaskRegistry::save`
runs after every transition), and `critical_path`. Fuzz targets live in the
workspace's `fuzz/` directory.

```bash
cargo test -p trace-lang-core
```
