//! `assert_trace!` and the four shape-assertion primitives it expands to.
//! Failures render `TraceAssertionError` via `miette`, embedding the
//! actual causal chain so a failure is debuggable without re-running
//! under a debugger — the same rich-diagnostics style as
//! `tracers_core::TraceErr` (see `crates/core/src/error.rs`).

use crate::outcome::TraceOutcome;
use miette::Diagnostic;
use serde::Serialize;
use thiserror::Error;

fn chain_summary<O: Clone + Serialize, T: TraceOutcome<O>>(outcome: &T) -> String {
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

    #[error(
        "step {name:?} confidence {actual:?} is not below {threshold}, causal chain was: {chain_summary}"
    )]
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
        help(
            "check the agent's on_low_confidence/on_budget_exceeded hook returns Delegate(expected)"
        )
    )]
    DidNotEscalateTo {
        expected: String,
        actual_chain: Vec<String>,
    },

    #[error("step {name:?} was not expected to run, causal chain was: {chain_summary}")]
    #[diagnostic(
        code(trace_test::unexpected_step),
        help("a step with this name ran when the test asserted it never should")
    )]
    UnexpectedStep { name: String, chain_summary: String },
}

/// Assert `outcome`'s causal chain contains a step named `name`.
pub fn contains_step<O: Clone + Serialize, T: TraceOutcome<O>>(
    outcome: &T,
    name: &str,
) -> Result<(), TraceAssertionError> {
    if outcome
        .trace()
        .causal_chain()
        .iter()
        .any(|s| s.name == name)
    {
        Ok(())
    } else {
        Err(TraceAssertionError::MissingStep {
            name: name.to_string(),
            chain_summary: chain_summary(outcome),
        })
    }
}

/// Assert the step named `name` has a confidence strictly below `threshold`.
pub fn confidence_below<O: Clone + Serialize, T: TraceOutcome<O>>(
    outcome: &T,
    name: &str,
    threshold: f64,
) -> Result<(), TraceAssertionError> {
    let step = outcome
        .trace()
        .causal_chain()
        .iter()
        .find(|s| s.name == name);
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
pub fn escalates_to<O: Clone + Serialize, T: TraceOutcome<O>>(
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
pub fn never_step<O: Clone + Serialize, T: TraceOutcome<O>>(
    outcome: &T,
    name: &str,
) -> Result<(), TraceAssertionError> {
    if outcome
        .trace()
        .causal_chain()
        .iter()
        .any(|s| s.name == name)
    {
        Err(TraceAssertionError::UnexpectedStep {
            name: name.to_string(),
            chain_summary: chain_summary(outcome),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracers_agent::spawn;
    use tracers_runtime::fixtures::{Careful, Expert, Guesser};

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
        registry.register(std::sync::Arc::new(Careful));
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
}
