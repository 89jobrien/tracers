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

    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn into_value(self) -> Option<T> {
        self.value
    }

    pub fn error(&self) -> Option<&TraceErr> {
        self.error.as_ref()
    }

    pub fn is_ok(&self) -> bool {
        self.value.is_some()
    }

    pub fn trace_ref(&self) -> TraceRef {
        TraceRef(self.id)
    }

    // ── Step management ───────────────────────────────────────────────────────

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
