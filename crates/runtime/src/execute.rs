use crate::registry::AgentRegistry;
use serde::Serialize;
use trace_lang_agent::{Agent, AgentContext, EscalationAction, SpawnOutcome, delegate, spawn};
use trace_lang_core::Trace;

/// Outcome of [`run_with_escalation`] — a run that may have hopped
/// across multiple agents via delegation before settling.
#[derive(Debug)]
pub struct RunOutcome<O> {
    pub trace: Trace<O>,
    pub context: AgentContext,
    /// `Some` if the run stopped with an escalation still pending:
    /// `max_hops` was reached, the escalation named an agent the registry
    /// doesn't recognize, or the escalation is one no agent can discharge
    /// (`RequireApproval`, `Emit`). `None` means the final agent in the
    /// chain produced no further escalation.
    pub unresolved: Option<EscalationAction>,
}

impl<O> RunOutcome<O> {
    /// The question this run stopped to ask a human, if it stopped on a
    /// `RequireApproval` escalation.
    ///
    /// This is the handoff point between `trace-lang-runtime` and
    /// `trace-lang-task`: park the work with
    /// `TaskRegistry::pause(id, request, store)` and resume it when a
    /// decision arrives.
    pub fn approval_request(&self) -> Option<&trace_lang_core::ApprovalRequest> {
        self.unresolved.as_ref().and_then(|e| e.approval_request())
    }
}

/// Run `agent`, and if its lifecycle hooks recommend delegating to
/// another agent, resolve that delegation against `registry` and keep
/// going — up to `max_hops` handoffs — until a run produces no further
/// escalation, the registry can't resolve the named target, or the hop
/// limit is reached.
///
/// Only `Delegate` is resolvable here. An escalation that needs a human
/// (`RequireApproval`) or that aborts (`Emit`) comes back in
/// [`RunOutcome::unresolved`] for the caller to act on — the runtime
/// cannot discharge either, and dropping them would silently lose the
/// escalation the hook asked for.
///
/// `input` must be `Clone`: each hop re-runs the *same* task against a
/// new agent. That is the point of escalation — retry the original
/// task with a different agent, not continue from partial output.
///
/// ```rust
/// use trace_lang_runtime::{AgentRegistry, run_with_escalation};
/// use trace_lang_agent::{Agent, AgentContext, EscalationAction};
/// use trace_lang_core::Trace;
/// use async_trait::async_trait;
/// use std::sync::Arc;
///
/// struct Junior;
/// #[async_trait]
/// impl Agent for Junior {
///     type Input = u32;
///     type Output = u32;
///     fn name(&self) -> &str { "Junior" }
///     fn goal(&self) -> &str { "attempt the task, escalate on failure" }
///     async fn run(&self, input: u32, ctx: &mut AgentContext) -> Trace<u32> {
///         ctx.record_step().unwrap();
///         Trace::failed(trace_lang_core::TraceErr::other("out of my depth"))
///     }
///     fn on_step_failure(&self) -> EscalationAction {
///         EscalationAction::Delegate("Senior".to_string())
///     }
/// }
///
/// struct Senior;
/// #[async_trait]
/// impl Agent for Senior {
///     type Input = u32;
///     type Output = u32;
///     fn name(&self) -> &str { "Senior" }
///     fn goal(&self) -> &str { "handle what Junior escalated" }
///     async fn run(&self, input: u32, ctx: &mut AgentContext) -> Trace<u32> {
///         ctx.record_step().unwrap();
///         Trace::new(input * 2)
///     }
/// }
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let mut registry: AgentRegistry<u32, u32> = AgentRegistry::new();
/// registry.register(Arc::new(Senior));
///
/// let outcome = run_with_escalation(&Junior, 21, &registry, 3).await;
/// assert_eq!(outcome.trace.value(), Some(&42));
/// assert!(outcome.unresolved.is_none());
/// assert_eq!(outcome.context.delegation_chain, vec!["Junior", "Senior"]);
/// # });
/// ```
pub async fn run_with_escalation<I, O>(
    agent: &dyn Agent<Input = I, Output = O>,
    input: I,
    registry: &AgentRegistry<I, O>,
    max_hops: usize,
) -> RunOutcome<O>
where
    I: Clone + Send,
    O: Clone + Serialize + Send,
{
    let SpawnOutcome {
        mut trace,
        mut context,
        mut escalation,
    } = spawn(agent, input.clone()).await;

    let mut hops = 0usize;

    loop {
        let target_name = match &escalation {
            EscalationAction::Delegate(name) => name.clone(),
            other => {
                let unresolved = other.needs_a_human().then(|| escalation.clone());
                return RunOutcome {
                    trace,
                    context,
                    unresolved,
                };
            }
        };

        if hops >= max_hops {
            return RunOutcome {
                trace,
                context,
                unresolved: Some(escalation),
            };
        }

        let Some(next_agent) = registry.get(&target_name) else {
            return RunOutcome {
                trace,
                context,
                unresolved: Some(escalation),
            };
        };

        let outcome = delegate(next_agent.as_ref(), input.clone(), &context).await;
        trace = outcome.trace;
        context = outcome.context;
        escalation = outcome.escalation;
        hops += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Arc;
    use trace_lang_core::TraceErr;

    /// Always fails and always escalates to "Loop" — used to force the
    /// delegation loop to keep going so we can test `max_hops`.
    struct AlwaysEscalates;

    #[async_trait]
    impl Agent for AlwaysEscalates {
        type Input = ();
        type Output = ();

        fn name(&self) -> &str {
            "AlwaysEscalates"
        }
        fn goal(&self) -> &str {
            "never succeed, always delegate onward"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<()> {
            ctx.record_step().unwrap();
            Trace::failed(TraceErr::other("still stuck"))
        }

        fn on_step_failure(&self) -> EscalationAction {
            EscalationAction::Delegate("AlwaysEscalates".to_string())
        }
    }

    /// Fails and escalates to a name that is never registered.
    struct DeadEnd;

    #[async_trait]
    impl Agent for DeadEnd {
        type Input = ();
        type Output = ();

        fn name(&self) -> &str {
            "DeadEnd"
        }
        fn goal(&self) -> &str {
            "escalate to an agent nobody registered"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<()> {
            ctx.record_step().unwrap();
            Trace::failed(TraceErr::other("nobody to ask"))
        }

        fn on_step_failure(&self) -> EscalationAction {
            EscalationAction::Delegate("Nobody".to_string())
        }
    }

    #[tokio::test]
    async fn max_hops_stops_an_infinite_delegation_loop() {
        let mut registry: AgentRegistry<(), ()> = AgentRegistry::new();
        registry.register(Arc::new(AlwaysEscalates));

        let outcome = run_with_escalation(&AlwaysEscalates, (), &registry, 3).await;

        assert!(!outcome.trace.is_ok());
        assert_eq!(
            outcome.unresolved,
            Some(EscalationAction::Delegate("AlwaysEscalates".to_string()))
        );
        // initial run + 3 delegated hops = 4 agents total in the chain
        assert_eq!(outcome.context.delegation_chain.len(), 4);
    }

    #[tokio::test]
    async fn unresolved_target_stops_the_loop_immediately() {
        let registry: AgentRegistry<(), ()> = AgentRegistry::new();

        let outcome = run_with_escalation(&DeadEnd, (), &registry, 10).await;

        assert!(!outcome.trace.is_ok());
        assert_eq!(
            outcome.unresolved,
            Some(EscalationAction::Delegate("Nobody".to_string()))
        );
        assert_eq!(outcome.context.delegation_chain, vec!["DeadEnd"]);
    }

    /// Stops mid-run to ask a human — an escalation no registry can
    /// resolve, however many agents it holds.
    struct NeedsApproval;

    #[async_trait]
    impl Agent for NeedsApproval {
        type Input = ();
        type Output = ();

        fn name(&self) -> &str {
            "NeedsApproval"
        }
        fn goal(&self) -> &str {
            "refuse to act without a human decision"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<()> {
            ctx.record_step().unwrap();
            Trace::failed(TraceErr::other("this needs sign-off"))
        }

        fn on_step_failure(&self) -> EscalationAction {
            let partial: Trace<()> = Trace::failed(TraceErr::other("this needs sign-off"));
            EscalationAction::RequireApproval(trace_lang_core::ApprovalRequest::new(
                "approve this?",
                partial.trace_ref(),
            ))
        }
    }

    /// Aborts outright rather than escalating to anyone.
    struct Aborts;

    #[async_trait]
    impl Agent for Aborts {
        type Input = ();
        type Output = ();

        fn name(&self) -> &str {
            "Aborts"
        }
        fn goal(&self) -> &str {
            "fail terminally without delegating"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<()> {
            ctx.record_step().unwrap();
            Trace::failed(TraceErr::other("unrecoverable"))
        }

        fn on_step_failure(&self) -> EscalationAction {
            EscalationAction::Emit(TraceErr::other("giving up"))
        }
    }

    #[tokio::test]
    async fn an_approval_escalation_comes_back_unresolved_for_the_caller_to_park() {
        let mut registry: AgentRegistry<(), ()> = AgentRegistry::new();
        registry.register(Arc::new(Available));

        let outcome = run_with_escalation(&NeedsApproval, (), &registry, 10).await;

        // No agent can discharge this, so the runtime must hand it back
        // rather than reporting the run as cleanly finished.
        let request = outcome
            .approval_request()
            .expect("the approval request must survive back to the caller");
        assert_eq!(request.question, "approve this?");
        assert_eq!(outcome.context.delegation_chain, vec!["NeedsApproval"]);
    }

    #[tokio::test]
    async fn an_emit_escalation_comes_back_unresolved_rather_than_being_dropped() {
        let registry: AgentRegistry<(), ()> = AgentRegistry::new();

        let outcome = run_with_escalation(&Aborts, (), &registry, 10).await;

        assert_eq!(
            outcome.unresolved,
            Some(EscalationAction::Emit(TraceErr::other("giving up")))
        );
        assert!(outcome.approval_request().is_none());
    }

    /// A registered agent that would happily run, to prove the approval
    /// path is not merely "the registry was empty".
    struct Available;

    #[async_trait]
    impl Agent for Available {
        type Input = ();
        type Output = ();

        fn name(&self) -> &str {
            "Anyone"
        }
        fn goal(&self) -> &str {
            "be available, and still not be a substitute for a human"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<()> {
            ctx.record_step().unwrap();
            Trace::new(())
        }
    }

    /// A trivial agent that always succeeds with no escalation.
    struct Healthy;

    #[async_trait]
    impl Agent for Healthy {
        type Input = ();
        type Output = &'static str;

        fn name(&self) -> &str {
            "Healthy"
        }
        fn goal(&self) -> &str {
            "succeed without ever escalating"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
            ctx.record_step().unwrap();
            Trace::new("done")
        }
    }

    #[tokio::test]
    async fn no_escalation_returns_immediately_with_a_single_hop_chain() {
        let registry: AgentRegistry<(), &'static str> = AgentRegistry::new();

        let outcome = run_with_escalation(&Healthy, (), &registry, 10).await;

        assert_eq!(outcome.trace.value(), Some(&"done"));
        assert!(outcome.unresolved.is_none());
        assert_eq!(outcome.context.delegation_chain, vec!["Healthy"]);
    }

    #[tokio::test]
    async fn zero_max_hops_stops_before_the_first_delegation() {
        let mut registry: AgentRegistry<(), ()> = AgentRegistry::new();
        registry.register(Arc::new(AlwaysEscalates));

        let outcome = run_with_escalation(&AlwaysEscalates, (), &registry, 0).await;

        assert_eq!(
            outcome.unresolved,
            Some(EscalationAction::Delegate("AlwaysEscalates".to_string()))
        );
        // No hops occurred — only the initial run is in the chain.
        assert_eq!(outcome.context.delegation_chain, vec!["AlwaysEscalates"]);
    }
}
