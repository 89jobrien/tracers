use trace_agent::{spawn, Agent, SpawnOutcome};

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
/// use trace_agent::{Agent, AgentContext};
/// use trace_core::Trace;
/// use trace_runtime::join_all;
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
