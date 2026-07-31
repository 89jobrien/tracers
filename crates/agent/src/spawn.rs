use crate::agent::Agent;
use crate::context::AgentContext;
use crate::hooks::EscalationAction;
use tracers_core::{Trace, TraceErr};

/// The result of running an agent via [`spawn`] or [`delegate`].
///
/// Bundles the produced `Trace<Output>` with the `AgentContext` it ran
/// under (so callers can inspect `delegation_chain` and
/// `steps_taken`) and the `escalation` a lifecycle hook recommended,
/// if any.
///
/// Generic over the output type alone (not the concrete `Agent`), so
/// the same outcome type works whether the agent was invoked through a
/// concrete `&A` or through a `&dyn Agent<Input = I, Output = O>` —
/// see `tracers-runtime`'s `AgentRegistry` for the latter.
pub struct SpawnOutcome<O> {
    pub trace: Trace<O>,
    pub context: AgentContext,
    pub escalation: EscalationAction,
}

/// Launch `agent` with a fresh [`AgentContext`], then evaluate its
/// lifecycle hooks against the resulting trace:
///
/// - a `BudgetExhausted` error consults [`Agent::on_budget_exceeded`]
/// - any other error consults [`Agent::on_step_failure`]
/// - a successful trace with any step below
///   [`Agent::confidence_threshold`] consults [`Agent::on_low_confidence`]
///
/// The escalation is returned for the caller to act on — `spawn` does
/// not perform delegation itself, keeping the decision explicit at the
/// call site.
///
/// `A: ?Sized` so this also accepts `&dyn Agent<Input = I, Output = O>`
/// trait objects, not just concrete sized agent types.
pub async fn spawn<A: Agent + ?Sized>(agent: &A, input: A::Input) -> SpawnOutcome<A::Output> {
    let mut ctx = AgentContext::new(agent.name(), agent.budget());
    let trace = agent.run(input, &mut ctx).await;
    let escalation = evaluate(agent, &trace);
    SpawnOutcome {
        trace,
        context: ctx,
        escalation,
    }
}

/// Transfer execution to `agent`, preserving the delegation chain from
/// `from`. The returned [`AgentContext::delegation_chain`] includes
/// every agent that touched the task so far, so
/// `Trace::causal_chain()` can be reconstructed across the full
/// handoff — not just the delegatee's own steps.
pub async fn delegate<A: Agent + ?Sized>(
    agent: &A,
    input: A::Input,
    from: &AgentContext,
) -> SpawnOutcome<A::Output> {
    let mut ctx = AgentContext {
        agent_name: agent.name().to_string(),
        steps_taken: 0,
        budget: agent.budget(),
        delegation_chain: from.extend_chain(agent.name()),
    };
    let trace = agent.run(input, &mut ctx).await;
    let escalation = evaluate(agent, &trace);
    SpawnOutcome {
        trace,
        context: ctx,
        escalation,
    }
}

/// Shared hook-evaluation logic for `spawn` and `delegate`.
fn evaluate<A: Agent + ?Sized>(agent: &A, trace: &Trace<A::Output>) -> EscalationAction {
    if let Some(err) = trace.error() {
        return match err {
            TraceErr::BudgetExhausted { .. } => agent.on_budget_exceeded(),
            _ => agent.on_step_failure(),
        };
    }

    if !trace
        .low_confidence_below(agent.confidence_threshold())
        .is_empty()
    {
        return agent.on_low_confidence();
    }

    EscalationAction::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tracers_core::Step;

    /// An agent whose budget is exhausted on the second `record_step()`
    /// call. Escalates to `Delegate("Fallback")`.
    struct BudgetLimited;

    #[async_trait]
    impl Agent for BudgetLimited {
        type Input = ();
        type Output = ();

        fn name(&self) -> &str {
            "BudgetLimited"
        }
        fn goal(&self) -> &str {
            "demonstrate budget exhaustion"
        }
        fn budget(&self) -> Option<usize> {
            Some(1)
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<()> {
            if let Err(e) = ctx.record_step() {
                return Trace::failed(e);
            }
            if let Err(e) = ctx.record_step() {
                return Trace::failed(e);
            }
            Trace::new(())
        }

        fn on_budget_exceeded(&self) -> EscalationAction {
            EscalationAction::Delegate("Fallback".to_string())
        }
    }

    /// An agent that always produces one low-confidence step.
    /// Escalates to `Delegate("Reviewer")`.
    struct Uncertain;

    #[async_trait]
    impl Agent for Uncertain {
        type Input = ();
        type Output = &'static str;

        fn name(&self) -> &str {
            "Uncertain"
        }
        fn goal(&self) -> &str {
            "demonstrate low-confidence escalation"
        }
        fn confidence_threshold(&self) -> f64 {
            0.8
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
            ctx.record_step().unwrap();
            let mut trace = Trace::new("guess");
            trace.push_step(Step::named("guess").with_confidence(0.4));
            trace
        }

        fn on_low_confidence(&self) -> EscalationAction {
            EscalationAction::Delegate("Reviewer".to_string())
        }
    }

    /// A trivial agent used to verify delegation chain propagation.
    struct ChainProbe;

    #[async_trait]
    impl Agent for ChainProbe {
        type Input = ();
        type Output = ();

        fn name(&self) -> &str {
            "ChainProbe"
        }
        fn goal(&self) -> &str {
            "record its own name into the delegation chain"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<()> {
            ctx.record_step().unwrap();
            Trace::new(())
        }
    }

    #[tokio::test]
    async fn budget_exhaustion_triggers_escalation() {
        let outcome = spawn(&BudgetLimited, ()).await;
        assert!(!outcome.trace.is_ok());
        assert_eq!(
            outcome.escalation,
            EscalationAction::Delegate("Fallback".to_string())
        );
    }

    #[tokio::test]
    async fn low_confidence_triggers_escalation() {
        let outcome = spawn(&Uncertain, ()).await;
        assert!(outcome.trace.is_ok());
        assert_eq!(
            outcome.escalation,
            EscalationAction::Delegate("Reviewer".to_string())
        );
    }

    #[tokio::test]
    async fn healthy_run_has_no_escalation() {
        let outcome = spawn(&ChainProbe, ()).await;
        assert!(outcome.trace.is_ok());
        assert_eq!(outcome.escalation, EscalationAction::None);
    }

    #[tokio::test]
    async fn delegate_extends_the_chain() {
        let root = spawn(&ChainProbe, ()).await;
        assert_eq!(root.context.delegation_chain, vec!["ChainProbe"]);

        let handed_off = delegate(&ChainProbe, (), &root.context).await;
        assert_eq!(
            handed_off.context.delegation_chain,
            vec!["ChainProbe", "ChainProbe"]
        );
    }

    #[tokio::test]
    async fn context_tracks_remaining_budget() {
        let mut ctx = AgentContext::new("probe", Some(3));
        assert_eq!(ctx.budget_remaining(), Some(3));
        ctx.record_step().unwrap();
        assert_eq!(ctx.budget_remaining(), Some(2));
        assert!(!ctx.is_budget_exhausted());
    }
}
