//! Integration test: `tracers-agent`'s lifecycle hooks wired through
//! `tracers-runtime`'s `AgentRegistry` and `run_with_escalation`, chaining
//! two real delegation hops across a low-confidence escalation and a
//! budget-exhaustion escalation before a third agent finally succeeds.

use async_trait::async_trait;
use std::sync::Arc;
use tracers_agent::{Agent, AgentContext, EscalationAction};
use tracers_core::{Step, Trace, TraceErr};
use tracers_runtime::{AgentRegistry, run_with_escalation};

/// Always produces a low-confidence step, escalating to "Careful".
struct Guesser;

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
struct Careful;

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
struct Expert;

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
