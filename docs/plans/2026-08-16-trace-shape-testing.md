# Plan: Trace-shape testing (`trace-test`)

## Goal

Add a `trace-test` crate with an `assert_trace!` macro so agent tests can assert the
*shape* of an execution (steps, confidence, escalation, absence) instead of only the
final `Trace` value. See `docs/designs/2026-08-16-trace-shape-testing-design.md`.

## Architecture

- Crates affected: `tracers-runtime` (new `test-support` feature + `fixtures` module,
  `async-trait` dependency promoted), new `crates/trace-test`.
- New traits/types: `TraceOutcome<O>` (port, `crates/trace-test/src/outcome.rs`),
  `TraceAssertionError` (`crates/trace-test/src/assertion.rs`).
- Data flow: `spawn()`/`run_with_escalation()` → `SpawnOutcome<O>`/`RunOutcome<O>` →
  `assert_trace!` macro → `contains_step`/`confidence_below`/`escalates_to`/`never_step`
  → `Result<(), TraceAssertionError>`, panicking with a rendered causal chain on `Err`.

## Tech Stack

- Rust edition 2024, workspace-pinned `serde`/`thiserror`/`async-trait`/`proptest`/`tokio`.
- `miette = { version = "7", features = ["fancy"] }` in `trace-test` (matches
  `crates/core/Cargo.toml`'s existing unpinned declaration — not in
  `[workspace.dependencies]`).

## Tasks

### Task 1: promote `async-trait` to an optional dependency in `tracers-runtime`

**Crate**: `tracers-runtime`
**File(s)**: `crates/runtime/Cargo.toml`
**Run**: `cargo check -p tracers-runtime --features test-support`

1. Edit `crates/runtime/Cargo.toml` from:
   ```toml
   [dependencies]
   tracers-core  = { path = "../core" }
   tracers-agent = { path = "../agent" }
   serde         = { workspace = true }
   futures       = { workspace = true }

   [dev-dependencies]
   tokio       = { workspace = true }
   async-trait = { workspace = true }
   proptest    = { workspace = true }

   [lints.rust]
   unexpected_cfgs = { level = "allow", check-cfg = ["cfg(kani)"] }
   ```
   to:
   ```toml
   [dependencies]
   tracers-core  = { path = "../core" }
   tracers-agent = { path = "../agent" }
   serde         = { workspace = true }
   futures       = { workspace = true }
   async-trait   = { workspace = true, optional = true }

   [dev-dependencies]
   tokio       = { workspace = true }
   proptest    = { workspace = true }

   [features]
   test-support = ["dep:async-trait"]

   [lints.rust]
   unexpected_cfgs = { level = "allow", check-cfg = ["cfg(kani)"] }
   ```
2. Run: `cargo check -p tracers-runtime` (no features) → succeeds, `async-trait` unused
   is fine since nothing in `src/` references it yet.
3. Run: `cargo check -p tracers-runtime --features test-support` → succeeds.
4. Run: `git branch --show-current`
   Verify output is `main`. Stop immediately if not.
   Commit: `git commit -m "feat(runtime): add test-support feature, promote async-trait to optional dep"`

### Task 2: move `Guesser`/`Careful`/`Expert` into `crates/runtime/src/fixtures.rs`

**Crate**: `tracers-runtime`
**File(s)**: `crates/runtime/src/fixtures.rs` (new), `crates/runtime/src/lib.rs`,
`crates/runtime/tests/escalation_wiring.rs`
**Run**: `cargo nextest run -p tracers-runtime`

1. Create `crates/runtime/src/fixtures.rs`:
   ```rust
   //! Real agent fixtures shared between this crate's own integration tests
   //! and downstream crates' tests (via the `test-support` feature) — a
   //! `Guesser -> Careful -> Expert` chain exercising a low-confidence
   //! escalation followed by a budget-exhaustion escalation before a third
   //! agent finally succeeds. Moved out of `tests/escalation_wiring.rs` so
   //! `trace-test`'s integration test can drive the same proven flow instead
   //! of a fixture written to make its own macro look good.

   use async_trait::async_trait;
   use tracers_agent::{Agent, AgentContext, EscalationAction};
   use tracers_core::{Step, Trace};

   /// Always produces a low-confidence step, escalating to "Careful".
   pub struct Guesser;

   #[async_trait]
   impl Agent for Guesser {
       type Input = ();
       type Output = &'static str;

       fn name(&self) -> &str {
           "Guesser"
       }
       fn goal(&self) -> &str {
           "produce a low-confidence first guess"
       }
       fn confidence_threshold(&self) -> f64 {
           0.9
       }

       async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
           ctx.record_step().unwrap();
           let mut t = Trace::new("shaky guess");
           t.push_step(Step::named("guess").with_confidence(0.2));
           t
       }

       fn on_low_confidence(&self) -> EscalationAction {
           EscalationAction::Delegate("Careful".to_string())
       }
   }

   /// Exhausts its one-step budget immediately, escalating to "Expert".
   pub struct Careful;

   #[async_trait]
   impl Agent for Careful {
       type Input = ();
       type Output = &'static str;

       fn name(&self) -> &str {
           "Careful"
       }
       fn goal(&self) -> &str {
           "run out of budget while double-checking"
       }
       fn budget(&self) -> Option<usize> {
           Some(1)
       }

       async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
           ctx.record_step().unwrap();
           if let Err(e) = ctx.record_step() {
               return Trace::failed(e);
           }
           Trace::new("careful answer")
       }

       fn on_budget_exceeded(&self) -> EscalationAction {
           EscalationAction::Delegate("Expert".to_string())
       }
   }

   /// Succeeds cleanly with no further escalation.
   pub struct Expert;

   #[async_trait]
   impl Agent for Expert {
       type Input = ();
       type Output = &'static str;

       fn name(&self) -> &str {
           "Expert"
       }
       fn goal(&self) -> &str {
           "settle the task with high confidence"
       }

       async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
           ctx.record_step().unwrap();
           let mut t = Trace::new("expert answer");
           t.push_step(Step::named("verify").with_confidence(0.95));
           t
       }
   }
   ```
2. Edit `crates/runtime/src/lib.rs`, add after the existing `pub mod speculate;` line:
   ```rust
   pub mod execute;
   pub mod join;
   pub mod registry;
   pub mod speculate;

   /// Real agent fixtures for shared use across this crate's tests and, via
   /// the `test-support` feature, downstream crates' tests (see
   /// `crates/task/src/checkpoint/mod.rs`'s `conformance` module for the
   /// identical precedent).
   #[cfg(any(test, feature = "test-support"))]
   pub mod fixtures;

   pub use execute::{RunOutcome, run_with_escalation};
   pub use join::join_all;
   pub use registry::AgentRegistry;
   pub use speculate::speculate;
   ```
3. Edit `crates/runtime/tests/escalation_wiring.rs`: delete the `Guesser`, `Careful`,
   `Expert` struct/impl blocks (everything between the file's doc comment block and the
   first `#[tokio::test]`), and change the imports from:
   ```rust
   use async_trait::async_trait;
   use std::sync::Arc;
   use tracers_agent::{Agent, AgentContext, EscalationAction};
   use tracers_core::{Step, Trace, TraceErr};
   use tracers_runtime::{AgentRegistry, run_with_escalation};
   ```
   to:
   ```rust
   use std::sync::Arc;
   use tracers_agent::EscalationAction;
   use tracers_core::TraceErr;
   use tracers_runtime::fixtures::{Careful, Expert, Guesser};
   use tracers_runtime::{AgentRegistry, run_with_escalation};
   ```
   (drop `async_trait`, `Agent`, `Step`, `Trace` imports — no longer directly used in this
   file; the three `#[tokio::test]` fns below are unchanged.)
4. Run: `cargo nextest run -p tracers-runtime` → all tests pass, including the 3
   `escalation_wiring` tests.
5. Run: `cargo clippy -p tracers-runtime -- -D warnings` → zero warnings.
6. Run: `git branch --show-current`
   Verify output is `main`. Stop immediately if not.
   Commit: `git commit -m "refactor(runtime): move escalation_wiring fixtures into src/fixtures.rs"`

### Task 3: create `crates/trace-test` crate skeleton and register it in the workspace

**Crate**: `tracers-trace-test`
**File(s)**: `crates/trace-test/Cargo.toml`, `crates/trace-test/src/lib.rs`,
`Cargo.toml` (root), `taskit.toml`
**Run**: `cargo check -p tracers-trace-test`

1. Create `crates/trace-test/Cargo.toml`:
   ```toml
   [package]
   name        = "tracers-trace-test"
   description = "Trace-shape assertions for trace:: agent tests — assert_trace! over Trace/AgentContext shape, not just final output"
   version.workspace    = true
   edition.workspace    = true
   authors.workspace    = true
   license.workspace    = true
   repository.workspace = true

   [dependencies]
   tracers-core    = { path = "../core" }
   tracers-agent   = { path = "../agent" }
   tracers-runtime = { path = "../runtime" }
   thiserror       = { workspace = true }
   miette          = { version = "7", features = ["fancy"] }

   [dev-dependencies]
   tokio       = { workspace = true }
   proptest    = { workspace = true }
   tracers-runtime = { path = "../runtime", features = ["test-support"] }

   [features]
   test-support = []
   ```
   Note: `tracers-runtime` appears in both `[dependencies]` (for `RunOutcome`) and
   `[dev-dependencies]` with `test-support` enabled (for `fixtures` in the integration
   test) — Cargo merges these into one dependency with the union of features when
   building tests, which is the correct outcome here.
2. Create `crates/trace-test/src/lib.rs`:
   ```rust
   //! `trace-test` — assert the *shape* of an agent execution, not just its
   //! final output. `assert_trace!` inspects `Trace::causal_chain()` and
   //! `AgentContext::delegation_chain` via the `TraceOutcome` port, so it
   //! works uniformly over `SpawnOutcome<O>` and `RunOutcome<O>`.

   pub mod assertion;
   pub mod outcome;

   pub use assertion::{TraceAssertionError, confidence_below, contains_step, escalates_to, never_step};
   pub use outcome::TraceOutcome;
   ```
3. Create empty placeholder modules so the crate compiles before Tasks 4-6 fill them in:
   `crates/trace-test/src/outcome.rs`:
   ```rust
   //! The `TraceOutcome` port — placeholder, filled in by the next task.
   ```
   `crates/trace-test/src/assertion.rs`:
   ```rust
   //! `assert_trace!` and the four assertion primitives — placeholder,
   //! filled in by the next task.
   ```
   (Task 3's `lib.rs` re-exports above will fail to compile against these placeholders —
   that's expected and resolved within this same task by deferring the re-export lines
   until Task 5/6. For this task, write `lib.rs` as just:
   ```rust
   pub mod assertion;
   pub mod outcome;
   ```
   without the `pub use` re-export line yet; Task 6 adds it.)
4. Edit root `Cargo.toml`, `[workspace] members` array — add `"crates/trace-test"` after
   `"crates/runtime"`:
   ```toml
   members = [
       "crates/core",
       "crates/task",
       "crates/agent",
       "crates/runtime",
       "crates/trace-test",
       "xtask",
       # TODO: "crates/cli" — planned `trace-cli`, a doob-style CLI for
       # inspecting trace checkpoints (see CLAUDE.md "planned crates").
   ]
   ```
5. Edit `taskit.toml`, `[workspace] crates` array — add an entry after the `runtime`
   entry:
   ```toml
   crates = [
     { dir = "crates/core", pkg = "tracers-core" },
     { dir = "crates/task", pkg = "tracers-task" },
     { dir = "crates/agent", pkg = "tracers-agent" },
     { dir = "crates/runtime", pkg = "tracers-runtime" },
     { dir = "crates/trace-test", pkg = "tracers-trace-test" },
   ]
   ```
   Then add two `[[workspace.propagation]]` entries after the existing
   `tracers-core -> [tracers-task, tracers-agent, tracers-runtime]` entry:
   ```toml
   [[workspace.propagation]]
   source = "tracers-core"
   dependents = ["tracers-trace-test"]

   [[workspace.propagation]]
   source = "tracers-agent"
   dependents = ["tracers-trace-test"]

   [[workspace.propagation]]
   source = "tracers-runtime"
   dependents = ["tracers-trace-test"]
   ```
6. Run: `cargo check -p tracers-trace-test` → succeeds (two placeholder modules, no
   re-exports yet).
7. Run: `cargo check --workspace` → succeeds (workspace member registered correctly).
8. Run: `git branch --show-current`
   Verify output is `main`. Stop immediately if not.
   Commit: `git commit -m "feat(trace-test): scaffold new crate and register in workspace"`

### Task 4: implement `TraceOutcome` port + adapters (TDD)

**Crate**: `tracers-trace-test`
**File(s)**: `crates/trace-test/src/outcome.rs`
**Run**: `cargo nextest run -p tracers-trace-test`

1. Write failing test in `crates/trace-test/src/outcome.rs` (appended at the bottom, in a
   `#[cfg(test)] mod tests` block):
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use tracers_agent::{AgentContext, spawn};
       use tracers_core::Trace;
       use tracers_runtime::fixtures::Expert;

       #[tokio::test]
       async fn spawn_outcome_exposes_trace_and_delegation_chain() {
           let outcome = spawn(&Expert, ()).await;
           assert_eq!(outcome.trace().value(), Some(&"expert answer"));
           assert_eq!(outcome.delegation_chain(), &["Expert".to_string()]);
       }
   }
   ```
   Run: `cargo nextest run -p tracers-trace-test --features test-support -- outcome::tests`
   Expected: FAIL (compile error — `TraceOutcome` trait and impls don't exist yet, and
   `tracers_runtime::fixtures` isn't visible without the `test-support` feature on the
   `[dev-dependencies]` entry, which Task 3 already added).
2. Implement `crates/trace-test/src/outcome.rs`:
   ```rust
   //! The `TraceOutcome` port — anything `assert_trace!` can inspect.
   //! Implemented for every outcome type in the workspace that carries a
   //! `Trace<O>` and an `AgentContext`-derived delegation chain, so the
   //! assertion primitives never need to know which concrete outcome type
   //! they're looking at.

   use tracers_agent::SpawnOutcome;
   use tracers_core::Trace;
   use tracers_runtime::RunOutcome;

   /// Port: anything `assert_trace!` can inspect.
   pub trait TraceOutcome<O> {
       fn trace(&self) -> &Trace<O>;
       fn delegation_chain(&self) -> &[String];
   }

   impl<O> TraceOutcome<O> for SpawnOutcome<O> {
       fn trace(&self) -> &Trace<O> {
           &self.trace
       }
       fn delegation_chain(&self) -> &[String] {
           &self.context.delegation_chain
       }
   }

   impl<O> TraceOutcome<O> for RunOutcome<O> {
       fn trace(&self) -> &Trace<O> {
           &self.trace
       }
       fn delegation_chain(&self) -> &[String] {
           &self.context.delegation_chain
       }
   }

   /// Exercise a `TraceOutcome` impl against the shared contract: `trace()`
   /// and `delegation_chain()` both return non-panicking, stable views —
   /// calling them twice returns the same data. Gated behind `test-support`
   /// so downstream crates can assert new `TraceOutcome` impls conform
   /// without depending on this crate's `#[cfg(test)]` code (same pattern as
   /// `tracers_task::checkpoint::conformance::assert_checkpoint_store_contract`).
   #[cfg(any(test, feature = "test-support"))]
   pub fn assert_trace_outcome_contract<O, T: TraceOutcome<O>>(outcome: &T) {
       let chain_a = outcome.delegation_chain().to_vec();
       let chain_b = outcome.delegation_chain().to_vec();
       assert_eq!(chain_a, chain_b, "delegation_chain() must be stable across calls");
       assert!(
           !outcome.trace().causal_chain().is_empty() || outcome.trace().error().is_some(),
           "trace() must reflect either a recorded step or a recorded error"
       );
   }
   ```
   Then add the test module from step 1 back at the bottom of the file (it references
   `super::*`, i.e. `TraceOutcome`, which now exists).
3. Run: `cargo nextest run -p tracers-trace-test --features test-support -- outcome::tests`
   Expected: PASS.
4. Write a second test for `RunOutcome` and for the conformance fn, appended to the same
   `mod tests` block:
   ```rust
   #[tokio::test]
   async fn run_outcome_exposes_trace_and_delegation_chain() {
       use tracers_runtime::{AgentRegistry, run_with_escalation};
       use tracers_runtime::fixtures::Guesser;

       let registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
       let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;
       assert_eq!(outcome.delegation_chain(), &["Guesser".to_string()]);
   }

   #[tokio::test]
   async fn spawn_outcome_satisfies_trace_outcome_contract() {
       let outcome = spawn(&Expert, ()).await;
       assert_trace_outcome_contract(&outcome);
   }

   #[tokio::test]
   async fn run_outcome_satisfies_trace_outcome_contract() {
       use tracers_runtime::{AgentRegistry, run_with_escalation};
       use tracers_runtime::fixtures::Guesser;

       let registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
       let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;
       assert_trace_outcome_contract(&outcome);
   }
   ```
5. Run: `cargo nextest run -p tracers-trace-test --features test-support` → all 4 tests
   pass.
6. Run: `cargo clippy -p tracers-trace-test --features test-support -- -D warnings` →
   zero warnings.
7. Run: `git branch --show-current`
   Verify output is `main`. Stop immediately if not.
   Commit: `git commit -m "feat(trace-test): implement TraceOutcome port for SpawnOutcome/RunOutcome"`

### Task 5: implement `TraceAssertionError` and the four assertion primitives (TDD)

**Crate**: `tracers-trace-test`
**File(s)**: `crates/trace-test/src/assertion.rs`
**Run**: `cargo nextest run -p tracers-trace-test --features test-support`

1. Write failing tests in `crates/trace-test/src/assertion.rs`:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use tracers_agent::spawn;
       use tracers_runtime::fixtures::{Expert, Guesser};

       #[tokio::test]
       async fn contains_step_passes_when_step_present() {
           let outcome = spawn(&Expert, ()).await;
           assert!(contains_step(&outcome, "verify").is_ok());
       }

       #[tokio::test]
       async fn contains_step_fails_when_step_absent() {
           let outcome = spawn(&Expert, ()).await;
           assert!(matches!(
               contains_step(&outcome, "nonexistent"),
               Err(TraceAssertionError::MissingStep { .. })
           ));
       }

       #[tokio::test]
       async fn confidence_below_passes_when_below_threshold() {
           let outcome = spawn(&Guesser, ()).await;
           assert!(confidence_below(&outcome, "guess", 0.5).is_ok());
       }

       #[tokio::test]
       async fn confidence_below_fails_when_at_or_above_threshold() {
           let outcome = spawn(&Expert, ()).await;
           assert!(matches!(
               confidence_below(&outcome, "verify", 0.5),
               Err(TraceAssertionError::ConfidenceNotBelow { .. })
           ));
       }

       #[tokio::test]
       async fn escalates_to_passes_when_agent_in_delegation_chain() {
           use tracers_runtime::{AgentRegistry, run_with_escalation};
           let mut registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
           registry.register(std::sync::Arc::new(tracers_runtime::fixtures::Careful));
           registry.register(std::sync::Arc::new(Expert));
           let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;
           assert!(escalates_to(&outcome, "Careful").is_ok());
       }

       #[tokio::test]
       async fn escalates_to_fails_when_agent_never_ran() {
           let outcome = spawn(&Expert, ()).await;
           assert!(matches!(
               escalates_to(&outcome, "NeverRan"),
               Err(TraceAssertionError::DidNotEscalateTo { .. })
           ));
       }

       #[tokio::test]
       async fn never_step_passes_when_step_absent() {
           let outcome = spawn(&Expert, ()).await;
           assert!(never_step(&outcome, "publish").is_ok());
       }

       #[tokio::test]
       async fn never_step_fails_when_step_present() {
           let outcome = spawn(&Expert, ()).await;
           assert!(matches!(
               never_step(&outcome, "verify"),
               Err(TraceAssertionError::UnexpectedStep { .. })
           ));
       }
   }
   ```
   Run: `cargo nextest run -p tracers-trace-test --features test-support -- assertion::tests`
   Expected: FAIL (compile error — nothing implemented yet).
2. Implement `crates/trace-test/src/assertion.rs` (above the `#[cfg(test)]` block from
   step 1):
   ```rust
   //! `assert_trace!` and the four shape-assertion primitives it expands to.
   //! Failures render `TraceAssertionError` via `miette`, embedding the
   //! actual causal chain so a failure is debuggable without re-running
   //! under a debugger — the same rich-diagnostics style as
   //! `tracers_core::TraceErr` (see `crates/core/src/error.rs`).

   use crate::outcome::TraceOutcome;
   use miette::Diagnostic;
   use thiserror::Error;

   fn chain_summary<O, T: TraceOutcome<O>>(outcome: &T) -> String {
       outcome
           .trace()
           .causal_chain()
           .iter()
           .map(|s| match s.confidence {
               Some(c) => format!("{}({:.2})", s.name, c),
               None => s.name.clone(),
           })
           .collect::<Vec<_>>()
           .join(" -> ")
   }

   /// Every way an `assert_trace!` block can fail.
   #[derive(Debug, Error, Diagnostic)]
   pub enum TraceAssertionError {
       #[error("expected step {name:?}, causal chain was: {chain_summary}")]
       #[diagnostic(
           code(trace_test::missing_step),
           help("check the step name matches exactly what Step::named() was called with")
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
           help("a step with this name ran when the test asserted it never should")
       )]
       UnexpectedStep { name: String, chain_summary: String },
   }

   /// Assert `outcome`'s causal chain contains a step named `name`.
   pub fn contains_step<O, T: TraceOutcome<O>>(
       outcome: &T,
       name: &str,
   ) -> Result<(), TraceAssertionError> {
       if outcome.trace().causal_chain().iter().any(|s| s.name == name) {
           Ok(())
       } else {
           Err(TraceAssertionError::MissingStep {
               name: name.to_string(),
               chain_summary: chain_summary(outcome),
           })
       }
   }

   /// Assert the step named `name` has a confidence strictly below `threshold`.
   pub fn confidence_below<O, T: TraceOutcome<O>>(
       outcome: &T,
       name: &str,
       threshold: f64,
   ) -> Result<(), TraceAssertionError> {
       let step = outcome.trace().causal_chain().iter().find(|s| s.name == name);
       match step.and_then(|s| s.confidence) {
           Some(c) if c < threshold => Ok(()),
           actual => Err(TraceAssertionError::ConfidenceNotBelow {
               name: name.to_string(),
               actual,
               threshold,
               chain_summary: chain_summary(outcome),
           }),
       }
   }

   /// Assert `agent_name` appears in `outcome`'s delegation chain.
   pub fn escalates_to<O, T: TraceOutcome<O>>(
       outcome: &T,
       agent_name: &str,
   ) -> Result<(), TraceAssertionError> {
       if outcome.delegation_chain().iter().any(|n| n == agent_name) {
           Ok(())
       } else {
           Err(TraceAssertionError::DidNotEscalateTo {
               expected: agent_name.to_string(),
               actual_chain: outcome.delegation_chain().to_vec(),
           })
       }
   }

   /// Assert `outcome`'s causal chain does NOT contain a step named `name`.
   pub fn never_step<O, T: TraceOutcome<O>>(
       outcome: &T,
       name: &str,
   ) -> Result<(), TraceAssertionError> {
       if outcome.trace().causal_chain().iter().any(|s| s.name == name) {
           Err(TraceAssertionError::UnexpectedStep {
               name: name.to_string(),
               chain_summary: chain_summary(outcome),
           })
       } else {
           Ok(())
       }
   }
   ```
3. Run: `cargo nextest run -p tracers-trace-test --features test-support -- assertion::tests`
   Expected: all 8 tests PASS.
4. Run: `cargo clippy -p tracers-trace-test --features test-support -- -D warnings` →
   zero warnings.
5. Run: `git branch --show-current`
   Verify output is `main`. Stop immediately if not.
   Commit: `git commit -m "feat(trace-test): implement TraceAssertionError and the four assertion primitives"`

### Task 6: property test for `confidence_below`

**Crate**: `tracers-trace-test`
**File(s)**: `crates/trace-test/src/assertion.rs`
**Run**: `cargo nextest run -p tracers-trace-test --features test-support`

1. Append to the `#[cfg(test)] mod tests` block in `crates/trace-test/src/assertion.rs`:
   ```rust
   struct FakeOutcome(tracers_core::Trace<()>);

   impl TraceOutcome<()> for FakeOutcome {
       fn trace(&self) -> &tracers_core::Trace<()> {
           &self.0
       }
       fn delegation_chain(&self) -> &[String] {
           &[]
       }
   }

   proptest::proptest! {
       #[test]
       fn confidence_below_matches_manual_comparison(
           confidence in proptest::option::of(-10.0f64..10.0),
           threshold in -10.0f64..10.0,
       ) {
           let mut trace = tracers_core::Trace::new(());
           let mut step = tracers_core::Step::named("probe");
           step.confidence = confidence.map(|c| c.clamp(0.0, 1.0));
           trace.push_step(step);
           let outcome = FakeOutcome(trace);

           let expected = matches!(confidence.map(|c| c.clamp(0.0, 1.0)), Some(c) if c < threshold);
           let actual = confidence_below(&outcome, "probe", threshold).is_ok();
           proptest::prop_assert_eq!(actual, expected);
       }
   }
   ```
   Run: `cargo nextest run -p tracers-trace-test --features test-support -- confidence_below_matches_manual_comparison`
   Expected: FAIL initially only if `FakeOutcome` name collides or `Step.confidence` field
   isn't public — it is (`crates/core/src/step.rs` has `pub confidence: Option<f64>`), so
   this should compile and PASS on first run given the implementation from Task 5 is
   already correct; if it fails, the counterexample printed by `proptest` identifies a
   real bug in `confidence_below` to fix before proceeding.
2. Run: `cargo nextest run -p tracers-trace-test --features test-support` → all tests
   (unit + property) pass.
3. Run: `git branch --show-current`
   Verify output is `main`. Stop immediately if not.
   Commit: `git commit -m "test(trace-test): add property test for confidence_below"`

### Task 7: implement `assert_trace!` macro and wire up `lib.rs` re-exports

**Crate**: `tracers-trace-test`
**File(s)**: `crates/trace-test/src/assertion.rs`, `crates/trace-test/src/lib.rs`
**Run**: `cargo nextest run -p tracers-trace-test --features test-support`

1. Write failing test, appended to `crates/trace-test/src/assertion.rs`'s test module:
   ```rust
   #[tokio::test]
   async fn assert_trace_macro_runs_all_checks_and_panics_on_first_failure() {
       let outcome = spawn(&Expert, ()).await;
       // All checks pass: macro must not panic.
       assert_trace!(&outcome, {
           contains_step("verify");
           confidence_below("verify", 1.0);
           never_step("publish");
       });
   }

   #[tokio::test]
   #[should_panic(expected = "expected step")]
   async fn assert_trace_macro_panics_when_a_check_fails() {
       let outcome = spawn(&Expert, ()).await;
       assert_trace!(&outcome, {
           contains_step("nonexistent");
       });
   }
   ```
   Run: `cargo nextest run -p tracers-trace-test --features test-support -- assert_trace_macro`
   Expected: FAIL (compile error — macro doesn't exist yet).
2. Add the macro to the top of `crates/trace-test/src/assertion.rs`, immediately after
   the module doc comment and before `use crate::outcome::TraceOutcome;`:
   ```rust
   /// Assert the shape of an agent execution — which steps ran, at what
   /// confidence, whether it escalated to a specific agent, whether some
   /// step never fired. Each check is one of `contains_step(name)`,
   /// `confidence_below(name, threshold)`, `escalates_to(agent_name)`,
   /// `never_step(name)`. Panics with the rendered `TraceAssertionError` on
   /// the first check that fails.
   #[macro_export]
   macro_rules! assert_trace {
       ($outcome:expr, { $($check:ident($($arg:expr),+ $(,)?));+ $(;)? }) => {
           $(
               if let Err(e) = $crate::assertion::$check($outcome, $($arg),+) {
                   panic!("{:?}", miette::Report::new(e));
               }
           )+
       };
   }
   ```
3. Edit `crates/trace-test/src/lib.rs` to its final form:
   ```rust
   //! `trace-test` — assert the *shape* of an agent execution, not just its
   //! final output. `assert_trace!` inspects `Trace::causal_chain()` and
   //! `AgentContext::delegation_chain` via the `TraceOutcome` port, so it
   //! works uniformly over `SpawnOutcome<O>` and `RunOutcome<O>`.

   pub mod assertion;
   pub mod outcome;

   pub use assertion::{TraceAssertionError, confidence_below, contains_step, escalates_to, never_step};
   pub use outcome::TraceOutcome;
   ```
4. Run: `cargo nextest run -p tracers-trace-test --features test-support -- assert_trace_macro`
   Expected: both tests PASS.
5. Run: `cargo nextest run -p tracers-trace-test --features test-support` → full crate
   test suite green.
6. Run: `cargo clippy -p tracers-trace-test --features test-support -- -D warnings` →
   zero warnings.
7. Run: `git branch --show-current`
   Verify output is `main`. Stop immediately if not.
   Commit: `git commit -m "feat(trace-test): implement assert_trace! macro"`

### Task 8: integration test reusing the real `Guesser -> Careful -> Expert` fixture

**Crate**: `tracers-trace-test`
**File(s)**: `crates/trace-test/tests/escalation_shape.rs` (new)
**Run**: `cargo nextest run -p tracers-trace-test --features test-support`

1. Write failing test — create `crates/trace-test/tests/escalation_shape.rs`:
   ```rust
   //! Integration test: `assert_trace!` against the real
   //! `Guesser -> Careful -> Expert` escalation chain from
   //! `tracers_runtime::fixtures` — the same flow proven end-to-end by
   //! `crates/runtime/tests/escalation_wiring.rs`, now also asserted on
   //! shape (not just final value) via `trace-test`.

   use std::sync::Arc;
   use tracers_runtime::fixtures::{Careful, Expert, Guesser};
   use tracers_runtime::{AgentRegistry, run_with_escalation};
   use tracers_trace_test::assert_trace;

   #[tokio::test]
   async fn full_escalation_chain_has_expected_shape() {
       let mut registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
       registry.register(Arc::new(Careful));
       registry.register(Arc::new(Expert));

       let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;

       assert_trace!(&outcome, {
           contains_step("verify");
           confidence_below("verify", 1.0);
           escalates_to("Careful");
           escalates_to("Expert");
           never_step("publish");
       });
   }

   #[tokio::test]
   #[should_panic(expected = "expected escalation to")]
   async fn escalates_to_fails_for_an_agent_never_reached() {
       let registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
       let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;

       assert_trace!(&outcome, {
           escalates_to("NeverRegistered");
       });
   }
   ```
   Run: `cargo nextest run -p tracers-trace-test --features test-support --test escalation_shape`
   Expected: depends on whether Tasks 1-7 landed correctly — if the crate builds, both
   tests should PASS immediately since they exercise already-implemented, already-tested
   code; this task's role is proving the integration surface (dev-dependency on
   `tracers-runtime`'s `test-support` feature, `use tracers_trace_test::assert_trace;`
   from outside the crate) actually works, not finding a new bug.
2. Run: `cargo nextest run -p tracers-trace-test --features test-support --test escalation_shape`
   Expected: both tests PASS.
3. Run: `cargo nextest run --workspace` → entire workspace green.
4. Run: `cargo clippy --workspace --features tracers-trace-test/test-support -- -D warnings`
   → zero warnings (or `cargo clippy --workspace -- -D warnings` if the feature flag
   syntax isn't accepted workspace-wide; fall back to per-crate clippy calls from
   Tasks 1-7 as the authoritative check).
5. Run: `git branch --show-current`
   Verify output is `main`. Stop immediately if not.
   Commit: `git commit -m "test(trace-test): add integration test against the real escalation chain fixture"`

### Task 9: final workspace verification and cleanup

**Crate**: workspace-wide
**File(s)**: none (verification only)
**Run**: `taskit ci --fail-fast`

1. Run: `cargo check --workspace --all-features` → succeeds.
2. Run: `cargo fmt --all -- --check` → no diff. If it fails, run `cargo fmt --all` and
   re-check.
3. Run: `taskit check-protocol-drift` — expected to pass with no update needed (no
   `[[protocol.surfaces]]` file was touched by this plan); if it unexpectedly reports
   drift, investigate before proceeding — do not blindly `--update`.
4. Run: `taskit ci --fail-fast` → full gated pipeline green (self-check, fmt --check,
   lint, compile-tests, test, check-deps, check-protocol-drift).
5. Run: `git branch --show-current`
   Verify output is `main`. Stop immediately if not.
   Commit only if step 2's `cargo fmt --all` produced a diff:
   `git commit -m "chore: cargo fmt"` (skip this commit if there was no diff).
