//! The `TraceOutcome` port — anything `assert_trace!` can inspect.
//! Implemented for every outcome type in the workspace that carries a
//! `Trace<O>` and an `AgentContext`-derived delegation chain, so the
//! assertion primitives never need to know which concrete outcome type
//! they're looking at.

use serde::Serialize;
use tracers_agent::SpawnOutcome;
use tracers_core::Trace;
use tracers_runtime::RunOutcome;

/// Port: anything `assert_trace!` can inspect.
///
/// `O: Clone + Serialize` mirrors `Trace<T>`'s own impl block bound
/// (`crates/core/src/trace.rs`) — every method this port needs
/// (`causal_chain()`, `error()`) is only defined for `Trace<T>` under that
/// bound, so the port carries the same constraint rather than deferring
/// the error to every call site.
pub trait TraceOutcome<O: Clone + Serialize> {
    fn trace(&self) -> &Trace<O>;
    fn delegation_chain(&self) -> &[String];
}

impl<O: Clone + Serialize> TraceOutcome<O> for SpawnOutcome<O> {
    fn trace(&self) -> &Trace<O> {
        &self.trace
    }
    fn delegation_chain(&self) -> &[String] {
        &self.context.delegation_chain
    }
}

impl<O: Clone + Serialize> TraceOutcome<O> for RunOutcome<O> {
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
pub fn assert_trace_outcome_contract<O: Clone + Serialize, T: TraceOutcome<O>>(outcome: &T) {
    let chain_a = outcome.delegation_chain().to_vec();
    let chain_b = outcome.delegation_chain().to_vec();
    assert_eq!(
        chain_a, chain_b,
        "delegation_chain() must be stable across calls"
    );
    assert!(
        !outcome.trace().causal_chain().is_empty() || outcome.trace().error().is_some(),
        "trace() must reflect either a recorded step or a recorded error"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracers_agent::spawn;
    use tracers_runtime::fixtures::{Expert, Guesser};

    #[tokio::test]
    async fn spawn_outcome_exposes_trace_and_delegation_chain() {
        let outcome = spawn(&Expert, ()).await;
        assert_eq!(outcome.trace().value(), Some(&"expert answer"));
        assert_eq!(outcome.delegation_chain(), &["Expert".to_string()]);
    }

    #[tokio::test]
    async fn run_outcome_exposes_trace_and_delegation_chain() {
        use tracers_runtime::{AgentRegistry, run_with_escalation};

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

        let registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
        let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;
        assert_trace_outcome_contract(&outcome);
    }
}
