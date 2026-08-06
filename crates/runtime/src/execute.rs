use crate::registry::AgentRegistry;
use serde::Serialize;
use tracers_agent::{Agent, AgentContext, EscalationAction, SpawnOutcome, delegate, spawn};
use tracers_core::Trace;

/// Outcome of [`run_with_escalation`] — a run that may have hopped
/// across multiple agents via delegation before settling.
pub struct RunOutcome<O> {
    pub trace: Trace<O>,
    pub context: AgentContext,
    /// `Some` if the run stopped with an escalation still pending —
    /// either `max_hops` was reached, or the escalation named an agent
    /// the registry doesn't recognize. `None` means the final agent in
    /// the chain produced no further escalation.
    pub unresolved: Option<EscalationAction>,
}

/// Run `agent`, and if its lifecycle hooks recommend delegating to
/// another agent, resolve that delegation against `registry` and keep
/// going — up to `max_hops` handoffs — until a run produces no further
/// escalation, the registry can't resolve the named target, or the hop
/// limit is reached.
///
/// `input` must be `Clone`: each hop re-runs the *same* task against a
/// new agent. That is the point of escalation — retry the original
/// task with a different agent, not continue from partial output.
///
/// ```rust
/// use tracers_runtime::{AgentRegistry, run_with_escalation};
/// use tracers_agent::{Agent, AgentContext, EscalationAction};
/// use tracers_core::Trace;
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
///         Trace::failed(tracers_core::TraceErr::other("out of my depth"))
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
            _ => {
                return RunOutcome {
                    trace,
                    context,
                    unresolved: None,
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
    use tracers_core::TraceErr;

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
