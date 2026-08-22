use crate::cost::StepCost;
use crate::error::TraceErr;
use crate::step::{Branch, Step};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A stable reference to a completed trace, safe to store in a `Task`
/// or serialize to a checkpoint file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceRef(pub Uuid);

impl std::fmt::Display for TraceRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "trace::{}", self.0)
    }
}

/// The core type. Every agent `step` returns `Trace<T>` rather than bare `T`.
///
/// `Trace<T>` wraps the output value with:
/// - a full `causal_chain()` of every step taken
/// - `rejected_branches()` — alternatives that were considered and discarded
/// - `bottlenecks()` — steps sorted by duration
/// - `low_confidence()` — steps below a threshold
///
/// Traces are serializable so they can be checkpointed alongside `TaskRegistry`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace<T> {
    /// Unique identifier, set on construction and exposed by `trace_ref()`
    /// as a stable `TraceRef` for storage in a `Task`.
    pub id: Uuid,
    value: Option<T>,
    error: Option<TraceErr>,
    steps: Vec<Step>,
}

impl<T: Clone + Serialize> Trace<T> {
    /// Construct a successful trace carrying `value`.
    pub fn new(value: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            value: Some(value),
            error: None,
            steps: Vec::new(),
        }
    }

    /// Construct a failed trace carrying `err`.
    pub fn failed(err: TraceErr) -> Self {
        Self {
            id: Uuid::new_v4(),
            value: None,
            error: Some(err),
            steps: Vec::new(),
        }
    }

    /// Merge two traces into one, concatenating their causal chains.
    /// The left-hand value wins.
    pub fn merge(mut lhs: Self, rhs: Self) -> Self {
        lhs.steps.extend(rhs.steps);
        lhs
    }

    // ── Value access ──────────────────────────────────────────────────────────

    /// Borrow the carried value, if the trace succeeded.
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Consume the trace and take ownership of the carried value, if any.
    pub fn into_value(self) -> Option<T> {
        self.value
    }

    /// Borrow the trace's error, if it failed.
    pub fn error(&self) -> Option<&TraceErr> {
        self.error.as_ref()
    }

    /// True iff the trace carries a value, i.e. it didn't fail.
    pub fn is_ok(&self) -> bool {
        self.value.is_some()
    }

    /// Wrap this trace's `id` into a `TraceRef` for storage in a `Task`.
    pub fn trace_ref(&self) -> TraceRef {
        TraceRef(self.id)
    }

    // ── Step management ───────────────────────────────────────────────────────

    /// Append a step to the causal chain. The primary mutation point agents
    /// call after each unit of work.
    pub fn push_step(&mut self, step: Step) {
        self.steps.push(step);
    }

    // ── Introspection ─────────────────────────────────────────────────────────

    /// Every step in the order they were executed.
    pub fn causal_chain(&self) -> &[Step] {
        &self.steps
    }

    /// Steps that were explicitly rejected via `reject()`.
    pub fn rejected_branches(&self) -> Vec<&Step> {
        self.steps.iter().filter(|s| s.is_rejected()).collect()
    }

    /// All `Branch` values across all steps (from `speculate {}`).
    pub fn all_branches(&self) -> Vec<&Branch> {
        self.steps.iter().flat_map(|s| &s.branches).collect()
    }

    /// Steps sorted slowest-first — useful for identifying bottlenecks.
    pub fn bottlenecks(&self) -> Vec<&Step> {
        let mut steps: Vec<&Step> = self.steps.iter().collect();
        steps.sort_by(|a, b| {
            b.duration
                .unwrap_or_default()
                .cmp(&a.duration.unwrap_or_default())
        });
        steps
    }

    /// Total cost across every step that recorded one.
    ///
    /// Token counts sum unconditionally; `dollars` is `Some` iff at least
    /// one step recorded a dollar figure, and sums only those steps — a
    /// partially-priced trace reports the spend it actually knows about
    /// rather than silently reporting `None` or pretending unpriced steps
    /// were free.
    pub fn total_cost(&self) -> StepCost {
        self.steps.iter().filter_map(|s| s.cost).sum()
    }

    /// Steps sorted priciest-first — the cost analogue of [`Self::bottlenecks`].
    ///
    /// Ordering is by recorded dollars descending, tie-broken by total
    /// tokens descending, so the method behaves sensibly whether every step
    /// carries a price (dollar ordering) or none do (pure token ordering).
    /// Steps with no recorded cost sort last. The sort is stable, so steps
    /// of equal cost keep their execution order.
    pub fn priciest_steps(&self) -> Vec<&Step> {
        let mut steps: Vec<&Step> = self.steps.iter().collect();
        steps.sort_by(|a, b| {
            let (a, b) = (a.cost.unwrap_or_default(), b.cost.unwrap_or_default());
            b.dollars
                .unwrap_or(0.0)
                .partial_cmp(&a.dollars.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.total_tokens().cmp(&a.total_tokens()))
        });
        steps
    }

    /// Steps whose confidence score is below `threshold` (default 0.7).
    pub fn low_confidence(&self) -> Vec<&Step> {
        self.low_confidence_below(0.7)
    }

    /// Steps whose confidence score is below an arbitrary `threshold`. Used
    /// by `spawn`'s escalation check against `Agent::confidence_threshold`.
    pub fn low_confidence_below(&self, threshold: f64) -> Vec<&Step> {
        self.steps
            .iter()
            .filter(|s| s.confidence.map(|c| c < threshold).unwrap_or(false))
            .collect()
    }
}

// Allow `trace?` propagation just like `Result`.
impl<T: Clone + Serialize> From<Trace<T>> for Result<T, TraceErr> {
    fn from(t: Trace<T>) -> Self {
        match t.value {
            Some(v) => Ok(v),
            None => Err(t.error.unwrap_or_else(|| TraceErr::other("empty trace"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::StepCost;
    use crate::step::Step;
    use std::time::Duration;

    #[test]
    fn new_trace_is_ok_and_carries_value() {
        let t = Trace::new(42);
        assert!(t.is_ok());
        assert_eq!(t.value(), Some(&42));
        assert!(t.error().is_none());
    }

    #[test]
    fn failed_trace_is_not_ok_and_carries_error() {
        let t: Trace<i32> = Trace::failed(TraceErr::other("boom"));
        assert!(!t.is_ok());
        assert_eq!(t.value(), None);
        assert!(t.error().is_some());
    }

    #[test]
    fn into_value_consumes_and_returns_owned_value() {
        let t = Trace::new(String::from("hi"));
        assert_eq!(t.into_value(), Some(String::from("hi")));
    }

    #[test]
    fn trace_ref_wraps_the_trace_id() {
        let t = Trace::new(1);
        assert_eq!(t.trace_ref(), TraceRef(t.id));
    }

    #[test]
    fn push_step_appends_to_causal_chain() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("a"));
        t.push_step(Step::named("b"));
        let chain = t.causal_chain();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].name, "a");
        assert_eq!(chain[1].name, "b");
    }

    #[test]
    fn merge_concatenates_causal_chains_and_keeps_lhs_value() {
        let mut lhs = Trace::new(1);
        lhs.push_step(Step::named("a"));
        let mut rhs = Trace::new(2);
        rhs.push_step(Step::named("b"));

        let merged = Trace::merge(lhs, rhs);
        assert_eq!(merged.value(), Some(&1));
        assert_eq!(merged.causal_chain().len(), 2);
    }

    #[test]
    fn rejected_branches_filters_only_rejected_steps() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("ok"));
        t.push_step(Step::named("rej").rejected("nope"));
        assert_eq!(t.rejected_branches().len(), 1);
        assert_eq!(t.rejected_branches()[0].name, "rej");
    }

    #[test]
    fn bottlenecks_sorts_slowest_first() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("fast").with_duration(Duration::from_millis(10)));
        t.push_step(Step::named("slow").with_duration(Duration::from_millis(100)));
        let sorted = t.bottlenecks();
        assert_eq!(sorted[0].name, "slow");
        assert_eq!(sorted[1].name, "fast");
    }

    #[test]
    fn total_cost_sums_only_the_steps_that_recorded_one() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("cheap").with_cost(StepCost::new(10, 5).with_dollars(0.01)));
        t.push_step(Step::named("free"));
        t.push_step(Step::named("pricey").with_cost(StepCost::new(100, 50).with_dollars(0.10)));

        let total = t.total_cost();
        assert_eq!(total.input_tokens, 110);
        assert_eq!(total.output_tokens, 55);
        assert_eq!(total.dollars.map(|d| (d * 100.0).round()), Some(11.0));
    }

    #[test]
    fn total_cost_of_a_trace_with_no_recorded_costs_is_zero() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("a"));
        assert!(t.total_cost().is_zero());
    }

    #[test]
    fn priciest_steps_sorts_by_dollars_then_tokens() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("free"));
        t.push_step(Step::named("cheap").with_cost(StepCost::new(1, 1).with_dollars(0.01)));
        t.push_step(Step::named("pricey").with_cost(StepCost::new(1, 1).with_dollars(0.50)));

        let names: Vec<&str> = t.priciest_steps().iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["pricey", "cheap", "free"]);
    }

    #[test]
    fn priciest_steps_falls_back_to_tokens_when_nothing_is_priced() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("small").with_cost(StepCost::new(1, 1)));
        t.push_step(Step::named("big").with_cost(StepCost::new(500, 500)));

        let names: Vec<&str> = t.priciest_steps().iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["big", "small"]);
    }

    #[test]
    fn priciest_steps_keeps_execution_order_on_a_tie() {
        // Mirrors `speculate`'s "first candidate wins ties" rule — the sort
        // must be stable, not merely correct on distinct costs.
        let mut t = Trace::new(1);
        t.push_step(Step::named("first").with_cost(StepCost::new(10, 10).with_dollars(0.05)));
        t.push_step(Step::named("second").with_cost(StepCost::new(10, 10).with_dollars(0.05)));

        assert_eq!(t.priciest_steps()[0].name, "first");
    }

    #[test]
    fn low_confidence_uses_default_threshold_of_0_7() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("weak").with_confidence(0.5));
        t.push_step(Step::named("strong").with_confidence(0.9));
        let weak = t.low_confidence();
        assert_eq!(weak.len(), 1);
        assert_eq!(weak[0].name, "weak");
    }

    #[test]
    fn low_confidence_below_respects_arbitrary_threshold() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("a").with_confidence(0.3));
        t.push_step(Step::named("b").with_confidence(0.6));
        assert_eq!(t.low_confidence_below(0.5).len(), 1);
        assert_eq!(t.low_confidence_below(0.7).len(), 2);
    }

    #[test]
    fn a_float_field_survives_a_checkpoint_round_trip_bit_for_bit() {
        // Regression guard for serde_json's `float_roundtrip` feature, which
        // the workspace manifest enables and which is off by default.
        // Without it these two values each come back one ULP off, so a
        // checkpoint drifts slightly on every save/load cycle — a trace is
        // supposed to be a record, not an approximation of one.
        //
        // Found by the `trace_roundtrip` fuzz target within seconds of first
        // running it, not by inspection.
        let confidence = 1.5626343493868385e-307_f64;
        let dollars = 5.986173235317172e-212_f64;

        let mut t = Trace::new(1);
        t.push_step(Step::named("scored").with_confidence(confidence));
        t.push_step(Step::named("priced").with_cost(StepCost::new(1, 1).with_dollars(dollars)));

        let json = serde_json::to_string(&t).expect("a trace serializes");
        let restored: Trace<i32> = serde_json::from_str(&json).expect("and deserializes");

        assert_eq!(
            restored.causal_chain()[0].confidence.map(f64::to_bits),
            Some(confidence.to_bits()),
        );
        assert_eq!(
            restored.causal_chain()[1]
                .cost
                .and_then(|c| c.dollars)
                .map(f64::to_bits),
            Some(dollars.to_bits()),
        );
        assert_eq!(
            json,
            serde_json::to_string(&restored).expect("re-serializes"),
            "a checkpoint must be byte-identical after a round trip"
        );
    }

    #[test]
    fn from_trace_for_result_maps_ok_and_err() {
        let ok: Result<i32, TraceErr> = Trace::new(5).into();
        assert_eq!(ok, Ok(5));

        let err: Result<i32, TraceErr> = Trace::failed(TraceErr::other("x")).into();
        assert!(err.is_err());
    }
}
