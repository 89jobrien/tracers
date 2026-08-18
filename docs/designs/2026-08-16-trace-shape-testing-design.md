# Design: Trace-shape testing (`trace-test`)

## Goal

Give this repo a way to assert the *shape* of an agent execution — which steps ran, at
what confidence, whether it escalated to a specific agent, whether some step never fired
— instead of only the final `Trace` output value, catching "right answer, wrong
reasoning" bugs nothing in the test suite currently catches.

## Approved Approach

New `crates/trace-test` workspace crate providing a declarative `assert_trace!` macro
over four primitives (`contains_step`, `confidence_below`, `escalates_to`, `never_step`),
per `docs/designs/proposals/round3-trace-shape-testing.md`.

## Crate Ownership

- **Owner crate**: `tracers-trace-test` (package name), directory `crates/trace-test` —
  a dev-dependency-only testing DSL, kept out of any production crate's `[dependencies]`
  so it never appears in the runtime dependency graph. Doesn't belong in `tracers-core`
  or `tracers-agent` because it depends on *both* (via the `TraceOutcome` port covering
  `SpawnOutcome` from `tracers-agent` and `RunOutcome` from `tracers-runtime`), and
  `tracers-core` must not depend on `tracers-task`/`tracers-agent` per this workspace's
  one-way dependency rule (CLAUDE.md) — a testing crate that spans both is a new leaf,
  not an addition to an existing one.
- **Affected crates**: `tracers-runtime` (gains a `test-support`-gated `fixtures` module
  and a dependency-type change, see Risk); `crates/trace-test` is the only new crate.

## Public API

### Traits

```rust
// crates/trace-test/src/outcome.rs

/// Port: anything `assert_trace!` can inspect. Implemented for every
/// outcome type in the workspace that carries a `Trace<O>` and an
/// `AgentContext`-derived delegation chain.
pub trait TraceOutcome<O> {
    fn trace(&self) -> &Trace<O>;
    fn delegation_chain(&self) -> &[String];
}
```

### Types

```rust
// crates/trace-test/src/outcome.rs

impl<O> TraceOutcome<O> for tracers_agent::SpawnOutcome<O> {
    fn trace(&self) -> &Trace<O>;
    fn delegation_chain(&self) -> &[String];
}

impl<O> TraceOutcome<O> for tracers_runtime::RunOutcome<O> {
    fn trace(&self) -> &Trace<O>;
    fn delegation_chain(&self) -> &[String];
}
```

```rust
// crates/trace-test/src/assertion.rs

/// Every way an `assert_trace!` block can fail. Mirrors `TraceErr`'s
/// rich-diagnostics style (`crates/core/src/error.rs`): every variant
/// carries a `code()` and a `help()`, and the `Display` message embeds
/// the actual `causal_chain()` (step names + confidences) so a failure
/// is debuggable without re-running under a debugger.
#[derive(Debug, Error, Diagnostic)]
pub enum TraceAssertionError {
    #[error("expected step {name:?}, causal chain was: {chain_summary}")]
    #[diagnostic(
        code(trace_test::missing_step),
        help("check the step name matches exactly what `Step::named()` was called with")
    )]
    MissingStep { name: String, chain_summary: String },

    #[error("step {name:?} confidence {actual:?} is not below {threshold}, causal chain was: {chain_summary}")]
    #[diagnostic(
        code(trace_test::confidence_not_below),
        help("either the step's confidence is too high, or the step never ran")
    )]
    ConfidenceNotBelow {
        name: String,
        actual: Option<f64>,
        threshold: f64,
        chain_summary: String,
    },

    #[error("expected escalation to {expected:?}, delegation chain was: {actual_chain:?}")]
    #[diagnostic(
        code(trace_test::did_not_escalate),
        help("check the agent's on_low_confidence/on_budget_exceeded hook returns Delegate(expected)")
    )]
    DidNotEscalateTo { expected: String, actual_chain: Vec<String> },

    #[error("step {name:?} was not expected to run, causal chain was: {chain_summary}")]
    #[diagnostic(
        code(trace_test::unexpected_step),
        help("a step named {name:?} ran when the test asserted it never should")
    )]
    UnexpectedStep { name: String, chain_summary: String },
}
```

### Functions

```rust
// crates/trace-test/src/assertion.rs

pub fn contains_step<O, T: TraceOutcome<O>>(
    outcome: &T,
    name: &str,
) -> Result<(), TraceAssertionError>;

pub fn confidence_below<O, T: TraceOutcome<O>>(
    outcome: &T,
    name: &str,
    threshold: f64,
) -> Result<(), TraceAssertionError>;

pub fn escalates_to<O, T: TraceOutcome<O>>(
    outcome: &T,
    agent_name: &str,
) -> Result<(), TraceAssertionError>;

pub fn never_step<O, T: TraceOutcome<O>>(
    outcome: &T,
    name: &str,
) -> Result<(), TraceAssertionError>;
```

```rust
// crates/trace-test/src/outcome.rs — test-support surface for the
// conformance suite (both this crate's own tests and any future
// TraceOutcome impl elsewhere), gated `#[cfg(any(test, feature = "test-support"))]`
// per the crates/task/src/checkpoint/conformance.rs precedent.

pub fn assert_trace_outcome_contract<O, T: TraceOutcome<O>>(outcome: &T);
```

```rust
// crates/trace-test/src/assertion.rs — macro export, not a fn, listed
// for completeness of the public surface.

#[macro_export]
macro_rules! assert_trace {
    ($outcome:expr, { $($check:ident($($arg:expr),+));+ $(;)? }) => { ... };
}
```

## Data Flow

1. **Source**: a test calls `spawn()`/`delegate()` (→ `SpawnOutcome<O>`) or
   `run_with_escalation()` (→ `RunOutcome<O>`) against a real `Agent` impl.
2. **Transform**: `assert_trace!(outcome, { ... })` expands to a sequence of calls into
   `contains_step`/`confidence_below`/`escalates_to`/`never_step`, each generic over
   `T: TraceOutcome<O>`, reading `outcome.trace().causal_chain()` and
   `outcome.delegation_chain()`.
3. **Sink**: each primitive returns `Result<(), TraceAssertionError>`; the macro
   `.unwrap()`s (or equivalent panic-on-`Err`) so a failure surfaces as a standard
   `#[test]` panic, with `TraceAssertionError`'s `Display` (via `miette`) rendering the
   causal chain inline.

## Hexagonal Boundaries

- **Port** (trait): `TraceOutcome<O>` in `crates/trace-test/src/outcome.rs`
- **Adapters** (impl): `impl<O> TraceOutcome<O> for SpawnOutcome<O>` and
  `impl<O> TraceOutcome<O> for RunOutcome<O>`, both in `crates/trace-test/src/outcome.rs`
  (adapters live with the port here since neither `tracers-agent` nor `tracers-runtime`
  should know `trace-test` exists — the port crate owns both impls, not the outcome
  crates)

## Testing Plan

Per the seven-dimension model, four dimensions apply:

- **Unit** (`crates/trace-test/src/assertion.rs`, `#[cfg(test)]`): one pass case + one
  fail case per primitive, against a hand-built `Trace`/fake `TraceOutcome` fixture.
- **Conformance** (`crates/trace-test/src/outcome.rs`,
  `assert_trace_outcome_contract<O, T: TraceOutcome<O>>`, gated
  `#[cfg(any(test, feature = "test-support"))]`): run once against a `SpawnOutcome`
  fixture, once against a `RunOutcome` fixture.
- **Property** (`crates/trace-test/src/assertion.rs`, `proptest`): `confidence_below`'s
  result always matches a manual `<` comparison, for arbitrary `f64` threshold × arbitrary
  `Option<f64>` step confidence.
- **Integration** (`crates/trace-test/tests/escalation_shape.rs`): drives the real
  `Guesser → Careful → Expert` chain (moved to `tracers_runtime::fixtures`, dev-dependency
  on `tracers-runtime` with `features = ["test-support"]`) through `run_with_escalation`,
  asserting `assert_trace!(outcome, { contains_step("guess"); confidence_below("guess",
  0.9); escalates_to("Careful"); never_step("publish"); })`-shaped checks against the
  same flow `escalation_wiring.rs` already proves works end to end.

Not applicable, with reasons:

- **Fuzz**: no `unsafe`, no external byte/string parsing — `trace-test` only reads
  already-typed `Trace`/`AgentContext` structs it did not construct from raw input.
- **Model Check**: no arithmetic on untrusted input, no unsafe invariants;
  `confidence_below`'s float comparison is already covered by the property test above.
- **Regression**: deferred — no bug found yet. First one gets a permanent regression test
  per the workspace's testing-philosophy policy.

## Out of Scope

- The `#[trace_test]` attribute macro from the original proposal doc — only the
  `assert_trace!` block macro is approved for this design.
- Any proc-macro sub-crate or `syn`/`quote` dependency.
- Any change to `TraceErr` itself — `TraceAssertionError` is a new, separate type.
- `speculate_race` / other round-3 proposals — tracked separately.

## Risk

- [ ] **Breaking API changes**: no — nothing existing changes signature.
- [x] **New external dependency**: `miette` in `trace-test`, matching `tracers-core`'s
  existing unpinned `{ version = "7", features = ["fancy"] }` declaration (not in
  `[workspace.dependencies]` today).
- [x] **Feature flag required**: yes — `tracers-runtime`'s new `test-support` feature.
  **Dependency-type correction found during context mapping**: `async-trait` must move
  from `tracers-runtime`'s `[dev-dependencies]` to an optional `[dependencies]` entry
  (`async-trait = { workspace = true, optional = true }`, `test-support =
  ["dep:async-trait"]`), because `fixtures.rs` lives in `src/` and needs `async_trait`
  available when an *external* crate (`trace-test`) enables `test-support` — dev-deps are
  never visible to downstream feature consumers, only to the declaring crate's own
  `cfg(test)` builds.
