use tracers_agent::{Agent, SpawnOutcome, spawn};

// TODO: thread-parallel variant via `tokio::spawn` — needs `'static` agents
// (`Arc<dyn Agent<...>>` everywhere), a bigger API change than this fn alone.
// See CLAUDE.md "deferred" and the matching TODO in speculate.rs.

/// Run `agent` concurrently against every input in `inputs`, collecting
/// one [`SpawnOutcome`] per input in the original order.
///
/// This concurrently polls all invocations on the current task (via
/// `futures::future::join_all`) rather than distributing them across
/// OS threads. For true multi-threaded parallelism, wrap the agent in
/// an `Arc` and dispatch each input via `tokio::spawn` instead — that
/// variant is tracked as future work.
///
/// ```rust
/// use tracers_agent::{Agent, AgentContext};
/// use tracers_core::Trace;
/// use tracers_runtime::join_all;
/// use async_trait::async_trait;
///
/// struct Doubler;
/// #[async_trait]
/// impl Agent for Doubler {
///     type Input = u32;
///     type Output = u32;
///     fn name(&self) -> &str { "Doubler" }
///     fn goal(&self) -> &str { "double a number" }
///     async fn run(&self, input: u32, ctx: &mut AgentContext) -> Trace<u32> {
///         ctx.record_step().unwrap();
///         Trace::new(input * 2)
///     }
/// }
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let outcomes = join_all(&Doubler, vec![1, 2, 3]).await;
/// let values: Vec<_> = outcomes.iter().map(|o| *o.trace.value().unwrap()).collect();
/// assert_eq!(values, vec![2, 4, 6]);
/// # });
/// ```
pub async fn join_all<A>(agent: &A, inputs: Vec<A::Input>) -> Vec<SpawnOutcome<A::Output>>
where
    A: Agent + ?Sized,
{
    let futures = inputs.into_iter().map(|input| spawn(agent, input));
    futures::future::join_all(futures).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tracers_agent::AgentContext;
    use tracers_core::Trace;

    struct Doubler;

    #[async_trait]
    impl Agent for Doubler {
        type Input = u32;
        type Output = u32;
        fn name(&self) -> &str {
            "Doubler"
        }
        fn goal(&self) -> &str {
            "double a number"
        }
        async fn run(&self, input: u32, ctx: &mut AgentContext) -> Trace<u32> {
            ctx.record_step().unwrap();
            Trace::new(input * 2)
        }
    }

    #[tokio::test]
    async fn empty_inputs_produce_no_outcomes() {
        let outcomes = join_all(&Doubler, vec![]).await;
        assert!(outcomes.is_empty());
    }

    #[tokio::test]
    async fn outcomes_preserve_input_order() {
        let outcomes = join_all(&Doubler, vec![1, 2, 3, 4]).await;
        let values: Vec<_> = outcomes.iter().map(|o| *o.trace.value().unwrap()).collect();
        assert_eq!(values, vec![2, 4, 6, 8]);
    }

    #[tokio::test]
    async fn each_outcome_gets_its_own_fresh_context() {
        let outcomes = join_all(&Doubler, vec![10, 20]).await;
        for outcome in &outcomes {
            assert_eq!(outcome.context.steps_taken, 1);
            assert_eq!(outcome.context.delegation_chain, vec!["Doubler"]);
        }
    }
}
