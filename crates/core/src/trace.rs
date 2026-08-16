use crate::error::TraceErr;
use crate::step::{Branch, Step};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// TODO: add a `TraceGraph` type (docs/ideas/FEATURES.md #7) for cross-trace
// lineage — `{ nodes: HashMap<TraceRef, TraceNode>, edges: Vec<(TraceRef,
// TraceRef)> }` with `record_edge`/`downstream_of`/`upstream_of`/
// `critical_path()`. Distinct from `Task::depends_on` in tracers-task,
// which is the same idea one level up.

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
    fn from_trace_for_result_maps_ok_and_err() {
        let ok: Result<i32, TraceErr> = Trace::new(5).into();
        assert_eq!(ok, Ok(5));

        let err: Result<i32, TraceErr> = Trace::failed(TraceErr::other("x")).into();
        assert!(err.is_err());
    }
}
