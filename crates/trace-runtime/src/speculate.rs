use serde::Serialize;
use std::sync::Arc;
use trace_agent::{spawn, Agent};
use trace_core::{Branch, Step, Trace};

/// Run several candidate agents concurrently against the same input,
/// pick a winner, and record the outcome as a single `speculate` step
/// whose [`Branch`]es show which candidate was taken and which were
/// rejected — and why.
///
/// Confidence per candidate is the mean of that run's step
/// confidences (steps with no recorded confidence are ignored, and a
/// candidate with zero scored steps counts as `0.0`). A candidate that
/// produced a `TraceErr` is scored `-1.0` so it never outranks a
/// successful candidate; its rejection reason is the error's `Display`
/// text. Ties keep the first candidate in `candidates` order.
///
/// # Panics
///
/// Panics if `candidates` is empty — there is nothing to speculate
/// over.
///
/// ```rust
/// use trace_agent::{Agent, AgentContext};
/// use trace_core::{Step, Trace};
/// use trace_runtime::speculate;
/// use async_trait::async_trait;
/// use std::sync::Arc;
///
/// struct Guess(&'static str, f64);
/// #[async_trait]
/// impl Agent for Guess {
///     type Input = ();
///     type Output = &'static str;
///     fn name(&self) -> &str { self.0 }
///     fn goal(&self) -> &str { "produce a candidate answer" }
///     async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
///         ctx.record_step().unwrap();
///         let mut t = Trace::new(self.0);
///         t.push_step(Step::named("guess").with_confidence(self.1));
///         t
///     }
/// }
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let candidates: Vec<(String, Arc<dyn Agent<Input = (), Output = &'static str>>)> = vec![
///     ("cautious".to_string(), Arc::new(Guess("cautious answer", 0.6))),
///     ("confident".to_string(), Arc::new(Guess("confident answer", 0.9))),
/// ];
///
/// let trace = speculate(candidates, ()).await;
/// assert_eq!(trace.value(), Some(&"confident answer"));
/// assert_eq!(trace.rejected_branches().len(), 0); // rejection lives on the speculate step's branches
/// assert_eq!(trace.all_branches().len(), 2);
/// # });
/// ```
pub async fn speculate<I, O>(
    candidates: Vec<(String, Arc<dyn Agent<Input = I, Output = O>>)>,
    input: I,
) -> Trace<O>
where
    I: Clone + Send,
    O: Clone + Serialize + Send,
{
    assert!(
        !candidates.is_empty(),
        "speculate requires at least one candidate"
    );

    let futures = candidates.iter().map(|(label, agent)| {
        let label = label.clone();
        let input = input.clone();
        let agent = Arc::clone(agent);
        async move {
            let outcome = spawn(agent.as_ref(), input).await;
            (label, outcome.trace)
        }
    });

    let results: Vec<(String, Trace<O>)> = futures::future::join_all(futures).await;

    // Deliberately not `Iterator::max_by`: it returns the *last* element
    // on a tie, but we want ties to keep the first candidate in
    // `candidates` order — a fold that only replaces on strictly
    // greater confidence gives us that.
    let mut winner_idx = 0usize;
    let mut best_score = f64::NEG_INFINITY;
    for (i, (_, trace)) in results.iter().enumerate() {
        let score = confidence_of(trace);
        if score > best_score {
            best_score = score;
            winner_idx = i;
        }
    }

    let mut step = Step::named("speculate");
    for (i, (label, trace)) in results.iter().enumerate() {
        let conf = confidence_of(trace);
        let branch = if i == winner_idx {
            Branch::taken(label.clone())
        } else {
            let reason = trace
                .error()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "lower confidence than winner".to_string());
            Branch::rejected(label.clone(), reason)
        };
        step.branches.push(branch.with_confidence(conf));
    }

    let mut winning = results
        .into_iter()
        .nth(winner_idx)
        .map(|(_, t)| t)
        .expect("winner_idx is always within results' bounds");
    winning.push_step(step);
    winning
}

/// Mean confidence across a trace's scored steps. `-1.0` if the trace
/// failed outright; `0.0` if it succeeded but recorded no confidence.
fn confidence_of<O>(trace: &Trace<O>) -> f64
where
    O: Clone + Serialize,
{
    if trace.error().is_some() {
        return -1.0;
    }
    let scores: Vec<f64> = trace
        .causal_chain()
        .iter()
        .filter_map(|s| s.confidence)
        .collect();
    if scores.is_empty() {
        return 0.0;
    }
    scores.iter().sum::<f64>() / scores.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use trace_agent::AgentContext;
    use trace_core::TraceErr;

    struct Scored(&'static str, f64);

    #[async_trait]
    impl Agent for Scored {
        type Input = ();
        type Output = &'static str;

        fn name(&self) -> &str {
            self.0
        }
        fn goal(&self) -> &str {
            "produce a scored candidate answer"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
            ctx.record_step().unwrap();
            let mut t = Trace::new(self.0);
            t.push_step(Step::named("guess").with_confidence(self.1));
            t
        }
    }

    struct AlwaysFails(&'static str);

    #[async_trait]
    impl Agent for AlwaysFails {
        type Input = ();
        type Output = &'static str;

        fn name(&self) -> &str {
            self.0
        }
        fn goal(&self) -> &str {
            "always fail"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
            ctx.record_step().unwrap();
            Trace::failed(TraceErr::other("nope"))
        }
    }

    #[tokio::test]
    async fn ties_keep_first_candidate_in_order() {
        let candidates: Vec<(String, Arc<dyn Agent<Input = (), Output = &'static str>>)> = vec![
            ("first".to_string(), Arc::new(Scored("first answer", 0.5))),
            ("second".to_string(), Arc::new(Scored("second answer", 0.5))),
        ];

        let trace = speculate(candidates, ()).await;
        assert_eq!(trace.value(), Some(&"first answer"));

        let branches = trace.all_branches();
        assert_eq!(branches.len(), 2);
    }

    #[tokio::test]
    async fn all_candidates_failing_still_returns_a_trace_with_branches() {
        let candidates: Vec<(String, Arc<dyn Agent<Input = (), Output = &'static str>>)> = vec![
            ("a".to_string(), Arc::new(AlwaysFails("a"))),
            ("b".to_string(), Arc::new(AlwaysFails("b"))),
        ];

        let trace = speculate(candidates, ()).await;
        assert!(!trace.is_ok());
        assert_eq!(trace.all_branches().len(), 2);
    }

    #[tokio::test]
    #[should_panic(expected = "speculate requires at least one candidate")]
    async fn empty_candidates_panics() {
        let candidates: Vec<(String, Arc<dyn Agent<Input = (), Output = ()>>)> = vec![];
        let _ = speculate(candidates, ()).await;
    }
}
