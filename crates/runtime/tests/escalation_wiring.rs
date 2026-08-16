//! Integration test: `tracers-agent`'s lifecycle hooks wired through
//! `tracers-runtime`'s `AgentRegistry` and `run_with_escalation`, chaining
//! two real delegation hops across a low-confidence escalation and a
//! budget-exhaustion escalation before a third agent finally succeeds.

use std::sync::Arc;
use tracers_agent::EscalationAction;
use tracers_core::TraceErr;
use tracers_runtime::fixtures::{Careful, Expert, Guesser};
use tracers_runtime::{AgentRegistry, run_with_escalation};

#[tokio::test]
async fn escalation_chain_hops_through_two_registered_agents_to_success() {
    let mut registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
    registry.register(Arc::new(Careful));
    registry.register(Arc::new(Expert));

    let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;

    assert_eq!(outcome.trace.value(), Some(&"expert answer"));
    assert!(outcome.unresolved.is_none());
    assert_eq!(
        outcome.context.delegation_chain,
        vec!["Guesser", "Careful", "Expert"]
    );

    // The final trace only carries the winning agent's own steps — the
    // causal chain does not retroactively absorb earlier agents' traces.
    assert_eq!(outcome.trace.causal_chain().len(), 1);
}

#[tokio::test]
async fn escalation_chain_stops_unresolved_when_target_is_never_registered() {
    // Empty registry: Guesser escalates to "Careful", which nothing resolves.
    let registry: AgentRegistry<(), &'static str> = AgentRegistry::new();

    let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;

    // Guesser's own run succeeds — it's the low-confidence escalation,
    // not the trace itself, that's left unresolved.
    assert_eq!(outcome.trace.value(), Some(&"shaky guess"));
    assert_eq!(
        outcome.unresolved,
        Some(EscalationAction::Delegate("Careful".to_string()))
    );
    assert_eq!(outcome.context.delegation_chain, vec!["Guesser"]);
}

#[tokio::test]
async fn budget_exhaustion_error_variant_survives_the_hop() {
    let mut registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
    registry.register(Arc::new(Careful));
    // No "Expert" registered — the chain should stop unresolved right
    // after Careful exhausts its budget.

    let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;

    assert!(!outcome.trace.is_ok());
    assert!(matches!(
        outcome.trace.error(),
        Some(TraceErr::BudgetExhausted { steps: 2 })
    ));
    assert_eq!(
        outcome.unresolved,
        Some(EscalationAction::Delegate("Expert".to_string()))
    );
    assert_eq!(outcome.context.delegation_chain, vec!["Guesser", "Careful"]);
}
