use crate::agent::Agent;
use crate::context::AgentContext;
use crate::hooks::EscalationAction;
use trace_lang_core::{Trace, TraceErr};

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
/// see `trace-lang-runtime`'s `AgentRegistry` for the latter.
#[derive(Debug)]
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
///
/// A hook signature is `fn(&self) -> EscalationAction` — it cannot see the
/// trace it is reacting to. So an `ApprovalRequest` raised by a hook comes
/// back unattached, and this is where it gets stamped with the trace that
/// produced it: a caller must never receive a question about a run it
/// cannot look up.
fn evaluate<A: Agent + ?Sized>(agent: &A, trace: &Trace<A::Output>) -> EscalationAction {
    let action = if let Some(err) = trace.error() {
        match err {
            TraceErr::BudgetExhausted { .. } => agent.on_budget_exceeded(),
            _ => agent.on_step_failure(),
        }
    } else if !trace
        .low_confidence_below(agent.confidence_threshold())
        .is_empty()
    {
        agent.on_low_confidence()
    } else {
        EscalationAction::None
    };

    match action {
        EscalationAction::RequireApproval(mut request) => {
            request.attach(trace.trace_ref());
            EscalationAction::RequireApproval(request)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use trace_lang_core::Step;

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
    /// A hook cannot see the trace it is reacting to, so `spawn` must stamp
    /// the request on the way out — otherwise a caller gets a question it
    /// cannot trace back to a run.
    struct AsksAHuman;

    #[async_trait]
    impl Agent for AsksAHuman {
        type Input = ();
        type Output = ();

        fn name(&self) -> &str {
            "AsksAHuman"
        }
        fn goal(&self) -> &str {
            "refuse to proceed without a person"
        }

        async fn run(&self, _input: (), _ctx: &mut AgentContext) -> Trace<()> {
            Trace::failed(TraceErr::other("needs sign-off"))
        }

        fn on_step_failure(&self) -> EscalationAction {
            EscalationAction::RequireApproval(trace_lang_core::ApprovalRequest::unattached(
                "proceed?",
            ))
        }
    }

    #[tokio::test]
    async fn spawn_stamps_an_approval_request_with_the_trace_that_raised_it() {
        let outcome = spawn(&AsksAHuman, ()).await;
        let request = outcome
            .escalation
            .approval_request()
            .expect("the hook asked for approval");

        assert!(request.is_attached());
        assert_eq!(request.trace, outcome.trace.trace_ref());
    }

    #[tokio::test]
    async fn delegate_also_stamps_the_approval_request() {
        let from = AgentContext::new("Caller", None);
        let outcome = delegate(&AsksAHuman, (), &from).await;
        let request = outcome
            .escalation
            .approval_request()
            .expect("the hook asked for approval");

        assert_eq!(request.trace, outcome.trace.trace_ref());
        assert_eq!(
            outcome.context.delegation_chain,
            vec!["Caller", "AsksAHuman"]
        );
    }
}
