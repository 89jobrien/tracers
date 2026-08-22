use futures::StreamExt;
use futures::stream::FuturesUnordered;
use serde::Serialize;
use std::sync::Arc;
use trace_lang_agent::{Agent, spawn};
use trace_lang_core::{Branch, Step, Trace};

// TODO: thread-parallel variant via `tokio::spawn` (see matching TODO in
// join.rs).
// TODO: also see the deferred "shared/global step budget across concurrent
// branches" item in CLAUDE.md — `AgentContext::budget` is per-run only.

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
/// use trace_lang_agent::{Agent, AgentContext};
/// use trace_lang_core::{Step, Trace};
/// use trace_lang_runtime::speculate;
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

    let scores: Vec<f64> = results
        .iter()
        .map(|(_, trace)| confidence_of(trace))
        .collect();
    let winner_idx = first_max_index(&scores);

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

/// Race several candidate agents against the same input and stop as soon
/// as one clears `threshold`, cancelling the rest.
///
/// [`speculate`] runs every candidate to completion before choosing. That
/// is right when you genuinely want to compare all of them, and wasteful
/// when a "good enough" answer arrives early — you have already paid the
/// full latency and cost of every losing branch by the time you look.
/// `speculate_race` makes that trade a parameter rather than a choice
/// between two APIs: a high `threshold` behaves almost like `speculate`, a
/// low one behaves almost like "take the first plausible answer".
///
/// If no candidate clears the bar, every candidate finishes and the winner
/// is chosen exactly as `speculate` chooses it — highest mean confidence,
/// ties going to the first candidate in order.
///
/// The recorded `speculate_race` step distinguishes three outcomes, which
/// is the point of racing rather than comparing:
///
/// - the winner is `BranchOutcome::Taken`
/// - a candidate that finished and lost is `Rejected`, with its score
/// - a candidate still in flight when the race ended is `Cancelled`, with
///   **no** confidence — it never reported one, and recording `0.0` would
///   claim it was bad rather than unfinished
///
/// `threshold` is clamped into `[0.0, 1.0]`. One threshold covers every
/// candidate: it expresses the caller's bar for "good enough to stop
/// paying for alternatives", not a property of any one agent — an agent's
/// own reliability already shows up in the confidence it reports.
///
/// # Panics
///
/// Panics if `candidates` is empty — there is nothing to race.
///
/// ```rust
/// use trace_lang_agent::{Agent, AgentContext};
/// use trace_lang_core::{Step, Trace};
/// use trace_lang_runtime::speculate_race;
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
///     ("good-enough".to_string(), Arc::new(Guess("good enough", 0.85))),
///     ("cautious".to_string(), Arc::new(Guess("cautious", 0.40))),
/// ];
///
/// // 0.8 is cleared by the first candidate, so the race can stop early.
/// let trace = speculate_race(candidates, (), 0.8).await;
/// assert_eq!(trace.value(), Some(&"good enough"));
/// # });
/// ```
pub async fn speculate_race<I, O>(
    candidates: Vec<(String, Arc<dyn Agent<Input = I, Output = O>>)>,
    input: I,
    threshold: f64,
) -> Trace<O>
where
    I: Clone + Send,
    O: Clone + Serialize + Send,
{
    assert!(
        !candidates.is_empty(),
        "speculate_race requires at least one candidate"
    );
    // Clamped so a negative threshold can't let a *failed* candidate
    // (scored -1.0) win the race by default.
    let threshold = threshold.clamp(0.0, 1.0);

    let mut racing = FuturesUnordered::new();
    for (index, (_, agent)) in candidates.iter().enumerate() {
        let agent = Arc::clone(agent);
        let input = input.clone();
        racing.push(async move {
            let outcome = spawn(agent.as_ref(), input).await;
            (index, outcome.trace)
        });
    }

    // Keyed by candidate index, not completion order, so the recorded
    // branches read in the order the caller declared them.
    let mut finished: Vec<Option<Trace<O>>> = (0..candidates.len()).map(|_| None).collect();
    let mut early_winner: Option<usize> = None;

    while let Some((index, trace)) = racing.next().await {
        let cleared = confidence_of(&trace) >= threshold;
        finished[index] = Some(trace);
        if cleared {
            early_winner = Some(index);
            break;
        }
    }
    // Dropping the stream cancels whatever is still in flight.
    drop(racing);

    let scores: Vec<f64> = finished
        .iter()
        .map(|t| t.as_ref().map(confidence_of).unwrap_or(f64::NEG_INFINITY))
        .collect();
    let winner_idx = early_winner.unwrap_or_else(|| first_max_index(&scores));
    let winner_label = candidates[winner_idx].0.clone();

    let mut step = Step::named("speculate_race").with_note(format!(
        "stopped at the first candidate above {threshold:.2}"
    ));
    for (index, (label, _)) in candidates.iter().enumerate() {
        let branch = match (&finished[index], index == winner_idx) {
            (_, true) => Branch::taken(label.clone()).with_confidence(scores[index]),
            (Some(trace), false) => {
                let reason = trace.error().map(|e| e.to_string()).unwrap_or_else(|| {
                    format!(
                        "scored {:.2}, below the {threshold:.2} threshold",
                        scores[index]
                    )
                });
                Branch::rejected(label.clone(), reason).with_confidence(scores[index])
            }
            (None, false) => Branch::cancelled(
                label.clone(),
                format!("still running when {winner_label} cleared the {threshold:.2} threshold"),
            ),
        };
        step.branches.push(branch);
    }

    let mut winning = finished
        .into_iter()
        .nth(winner_idx)
        .flatten()
        .expect("the winning candidate always finished");
    winning.push_step(step);
    winning
}

/// Index of the first strictly-greatest score in `scores`.
///
/// Deliberately not `Iterator::max_by`: it returns the *last* element on
/// a tie, but `speculate` wants ties to keep the first candidate in
/// `candidates` order — a fold that only replaces on strictly greater
/// score gives us that.
///
/// # Panics
///
/// Panics if `scores` is empty.
fn first_max_index(scores: &[f64]) -> usize {
    assert!(!scores.is_empty(), "scores must not be empty");
    let mut winner_idx = 0usize;
    let mut best_score = f64::NEG_INFINITY;
    for (i, &score) in scores.iter().enumerate() {
        if score > best_score {
            best_score = score;
            winner_idx = i;
        }
    }
    winner_idx
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
    use trace_lang_agent::AgentContext;
    use trace_lang_core::TraceErr;

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

    type Candidates = Vec<(String, Arc<dyn Agent<Input = (), Output = &'static str>>)>;

    #[tokio::test]
    async fn ties_keep_first_candidate_in_order() {
        let candidates: Candidates = vec![
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
        let candidates: Candidates = vec![
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

    // ── speculate_race ───────────────────────────────────────────────────────

    /// Finishes only after a real delay, so a race against an instant
    /// candidate has a deterministic loser.
    struct Slow(&'static str, f64);

    #[async_trait]
    impl Agent for Slow {
        type Input = ();
        type Output = &'static str;

        fn name(&self) -> &str {
            self.0
        }
        fn goal(&self) -> &str {
            "eventually produce a very good answer"
        }

        async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
            ctx.record_step().unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let mut t = Trace::new(self.0);
            t.push_step(Step::named("deliberate").with_confidence(self.1));
            t
        }
    }

    fn branch<'a>(trace: &'a Trace<&'static str>, label: &str) -> &'a trace_lang_core::Branch {
        trace
            .all_branches()
            .into_iter()
            .find(|b| b.label == label)
            .expect("every candidate is recorded as a branch")
    }

    #[tokio::test]
    async fn a_candidate_over_the_threshold_wins_and_the_rest_are_cancelled() {
        let candidates: Candidates = vec![
            (
                "slow".to_string(),
                Arc::new(Slow("slow but excellent", 0.99)),
            ),
            (
                "quick".to_string(),
                Arc::new(Scored("quick and fine", 0.85)),
            ),
        ];

        let trace = speculate_race(candidates, (), 0.8).await;

        // The better answer loses on purpose: it had not arrived yet.
        assert_eq!(trace.value(), Some(&"quick and fine"));
        assert!(branch(&trace, "quick").is_taken());

        let cancelled = branch(&trace, "slow");
        assert!(cancelled.is_cancelled());
        assert!(
            !cancelled.is_rejected(),
            "cancelled is not the same as judged and rejected"
        );
        assert_eq!(
            cancelled.confidence, None,
            "a cancelled candidate never reported a score"
        );
    }

    #[tokio::test]
    async fn when_nothing_clears_the_bar_every_candidate_runs_and_the_best_wins() {
        let candidates: Candidates = vec![
            ("weak".to_string(), Arc::new(Scored("weak answer", 0.30))),
            (
                "strong".to_string(),
                Arc::new(Scored("strong answer", 0.70)),
            ),
        ];

        // Nothing reaches 0.99, so this degrades to `speculate`'s semantics.
        let trace = speculate_race(candidates, (), 0.99).await;

        assert_eq!(trace.value(), Some(&"strong answer"));
        assert!(branch(&trace, "strong").is_taken());
        assert!(branch(&trace, "weak").is_rejected());
        assert_eq!(branch(&trace, "weak").confidence, Some(0.30));
        assert!(trace.all_branches().iter().all(|b| !b.is_cancelled()));
    }

    #[tokio::test]
    async fn ties_keep_the_first_candidate_when_nothing_clears_the_bar() {
        // Same tie-break rule as `speculate` — see `first_max_index`.
        let candidates: Candidates = vec![
            ("first".to_string(), Arc::new(Scored("first answer", 0.5))),
            ("second".to_string(), Arc::new(Scored("second answer", 0.5))),
        ];

        let trace = speculate_race(candidates, (), 0.9).await;
        assert_eq!(trace.value(), Some(&"first answer"));
    }

    #[tokio::test]
    async fn a_failed_candidate_never_wins_the_race_however_low_the_threshold() {
        let candidates: Candidates = vec![
            ("broken".to_string(), Arc::new(AlwaysFails("broken"))),
            ("ok".to_string(), Arc::new(Scored("ok answer", 0.10))),
        ];

        let trace = speculate_race(candidates, (), 0.0).await;

        assert_eq!(trace.value(), Some(&"ok answer"));
        assert!(branch(&trace, "broken").is_rejected());
    }

    #[tokio::test]
    async fn a_negative_threshold_is_clamped_so_a_failure_still_cannot_win() {
        let candidates: Candidates = vec![
            ("broken".to_string(), Arc::new(AlwaysFails("broken"))),
            ("ok".to_string(), Arc::new(Scored("ok answer", 0.10))),
        ];

        let trace = speculate_race(candidates, (), -5.0).await;
        assert_eq!(trace.value(), Some(&"ok answer"));
    }

    #[tokio::test]
    async fn every_candidate_failing_still_returns_a_trace_with_branches() {
        let candidates: Candidates = vec![
            ("a".to_string(), Arc::new(AlwaysFails("a"))),
            ("b".to_string(), Arc::new(AlwaysFails("b"))),
        ];

        let trace = speculate_race(candidates, (), 0.5).await;
        assert!(!trace.is_ok());
        assert_eq!(trace.all_branches().len(), 2);
    }

    #[tokio::test]
    #[should_panic(expected = "speculate_race requires at least one candidate")]
    async fn racing_no_candidates_panics() {
        let candidates: Vec<(String, Arc<dyn Agent<Input = (), Output = ()>>)> = vec![];
        let _ = speculate_race(candidates, (), 0.5).await;
    }

    // ── first_max_index: unit ────────────────────────────────────────────────

    #[test]
    fn first_max_index_picks_the_single_max() {
        assert_eq!(first_max_index(&[0.1, 0.9, 0.5]), 1);
    }

    #[test]
    fn first_max_index_keeps_first_candidate_on_tie() {
        assert_eq!(first_max_index(&[0.5, 0.5, 0.5]), 0);
        assert_eq!(first_max_index(&[0.1, 0.9, 0.9]), 1);
    }

    #[test]
    fn first_max_index_handles_single_element() {
        assert_eq!(first_max_index(&[0.3]), 0);
    }

    #[test]
    #[should_panic(expected = "scores must not be empty")]
    fn first_max_index_panics_on_empty_scores() {
        first_max_index(&[]);
    }

    // ── first_max_index: property ────────────────────────────────────────────
    // Regression coverage for the documented max_by tie-break bug: for any
    // non-empty score vector, the returned index must point at a value tied
    // for the maximum, and it must be the *first* such index.

    proptest::proptest! {
        #[test]
        fn first_max_index_always_points_at_a_maximal_score(
            scores in proptest::collection::vec(-1.0f64..=1.0, 1..20)
        ) {
            let idx = first_max_index(&scores);
            let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            proptest::prop_assert_eq!(scores[idx], max);
        }

        #[test]
        fn first_max_index_is_the_earliest_maximal_index(
            scores in proptest::collection::vec(-1.0f64..=1.0, 1..20)
        ) {
            let idx = first_max_index(&scores);
            let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let earliest = scores.iter().position(|&s| s == max).unwrap();
            proptest::prop_assert_eq!(idx, earliest);
        }
    }
}

// ── first_max_index: model check ────────────────────────────────────────────
// Proves the tie-break invariant exhaustively within bounds, rather than just
// for sampled inputs, given this exact bug shipped once already (see
// CLAUDE.md's `max_by` note) and was only caught by a hand-written unit test.
#[cfg(kani)]
mod kani_proofs {
    use super::first_max_index;

    #[kani::proof]
    #[kani::unwind(5)]
    fn first_max_index_never_indexes_out_of_bounds() {
        let len: usize = kani::any();
        kani::assume(len > 0 && len <= 4);
        let mut scores = Vec::with_capacity(len);
        for _ in 0..len {
            let s: f64 = kani::any();
            kani::assume(s.is_finite());
            scores.push(s);
        }
        let idx = first_max_index(&scores);
        assert!(idx < scores.len());
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn first_max_index_on_two_element_tie_keeps_first() {
        let a: f64 = kani::any();
        kani::assume(a.is_finite());
        let scores = vec![a, a];
        assert_eq!(first_max_index(&scores), 0);
    }
}
