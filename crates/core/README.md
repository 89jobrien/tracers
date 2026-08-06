# tracers-core

Core `Trace<T>` type — reasoning provenance as a first-class value.

Every computation in a `trace::` program returns `Trace<T>` rather than a bare `T`.
The trace carries the full causal chain: steps taken, branches rejected, confidence
at each decision point, and RAII-style span timing. `tracers-core` has no dependency
on any other crate in this workspace — everything else (`tracers-task`,
`tracers-agent`, `tracers-runtime`) is built on top of it.

## Install

```toml
[dependencies]
tracers-core = { path = "../core" }  # or a version once published
```

## Quick start

```rust
use tracers_core::{Trace, Step, TraceErr};

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

`Trace<T>` implements `From<Trace<T>> for Result<T, TraceErr>`, so `let v: T = trace?;`
works in any function returning `Result<_, TraceErr>` — the same ergonomics as `Result`
itself, but with a logged causal chain attached.

### `Step`

A single unit of reasoning — one `observe`, `branch`, or `emit`. Carries a name,
an optional confidence score (clamped to `[0.0, 1.0]`), an optional duration, a
timestamp, an `outcome` (`Taken` / `Rejected { reason }` / `Failed { message }`),
and any `Branch`es considered during that step (populated by `speculate`).

```rust
use tracers_core::Step;
use std::time::Duration;

let step = Step::named("search")
    .with_confidence(0.85)
    .with_duration(Duration::from_millis(120))
    .with_note("used cached index");
```

### `Branch`

A path that was *considered* during a step — either taken or rejected. `speculate`
produces one `Branch` per candidate, marking the winner `Taken` and the rest
`Rejected { reason }`.

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
| `Serde(String)` | serialization/deserialization failure |
| `Other(String)` | catch-all |

### `Span`

RAII-style timing helper. Timing begins on `Span::start(name)` and is captured on
`finish()`.

> **Not auto-wired:** `Span` does not populate `Step::duration` on its own —
> callers must pair `Span::finish()`'s return value with `Step::with_duration`
> themselves.

```rust
use tracers_core::Span;

let span = Span::start("search");
// ... do work ...
let duration = span.finish();
```

## Serialization

Every public type derives `Serialize + Deserialize` — this is a compile-time
constraint, not a convention, since `tracers-task` checkpoints traces alongside
`TaskRegistry`.

## Testing

Unit tests live alongside each module (`#[cfg(test)] mod tests`). `Step` and
`Branch`'s confidence-clamping is additionally covered by `proptest` property tests
verifying `with_confidence` always lands in `[0.0, 1.0]` regardless of input.

```bash
cargo test -p tracers-core
```
