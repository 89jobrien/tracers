use crate::cost::StepCost;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// A single unit of reasoning. Every `observe`, `branch`, and `emit` in a
/// trace:: agent produces a `Step` that is appended to the `Trace<T>`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Step {
    /// Unique identifier assigned via `Uuid::new_v4()` in `Step::named`.
    pub id: Uuid,
    pub name: String,
    /// Confidence score in `[0.0, 1.0]`, clamped by `with_confidence`.
    pub confidence: Option<f64>,
    pub duration: Option<Duration>,
    pub started_at: DateTime<Utc>,
    pub outcome: StepOutcome,
    /// Candidate paths considered for this step. Populated by `speculate`
    /// with one `Branch` per candidate — the winner marked `Taken`, the
    /// rest `Rejected`.
    pub branches: Vec<Branch>,
    pub notes: Option<String>,
    /// What this step cost to produce, if the caller recorded it.
    /// `#[serde(default)]` so checkpoints written before the cost ledger
    /// existed still deserialize.
    #[serde(default)]
    pub cost: Option<StepCost>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepOutcome {
    /// Step completed and produced a value.
    Taken,
    /// Step was explicitly abandoned via `reject()`.
    Rejected { reason: String },
    /// Step failed with an error.
    Failed { message: String },
}

impl Step {
    /// Create a new step with the given name, timestamped now.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            confidence: None,
            duration: None,
            started_at: Utc::now(),
            outcome: StepOutcome::Taken,
            branches: Vec::new(),
            notes: None,
            cost: None,
        }
    }

    /// Builder: attach a confidence score, clamped into `[0.0, 1.0]`.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Builder: record how long the step took.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Builder: attach a free-text note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes = Some(note.into());
        self
    }

    /// Builder: record what this step cost — tokens consumed and, if the
    /// caller knew the provider's price, dollars. Rolls up through
    /// [`crate::Trace::total_cost`] and orders
    /// [`crate::Trace::priciest_steps`].
    pub fn with_cost(mut self, cost: StepCost) -> Self {
        self.cost = Some(cost);
        self
    }

    /// Builder: transition the step's outcome to `Rejected` with a reason.
    pub fn rejected(mut self, reason: impl Into<String>) -> Self {
        self.outcome = StepOutcome::Rejected {
            reason: reason.into(),
        };
        self
    }

    /// Builder: transition the step's outcome to `Failed` with a message.
    pub fn failed(mut self, message: impl Into<String>) -> Self {
        self.outcome = StepOutcome::Failed {
            message: message.into(),
        };
        self
    }

    /// True if this step's outcome is `Rejected`.
    pub fn is_rejected(&self) -> bool {
        matches!(self.outcome, StepOutcome::Rejected { .. })
    }

    /// True if this step's outcome is `Failed`.
    pub fn is_failed(&self) -> bool {
        matches!(self.outcome, StepOutcome::Failed { .. })
    }
}

/// A branch is a path that was *considered* — either taken or rejected.
/// `speculate { A: .., B: .., C: .. }` produces one `Branch` per arm,
/// with the winner marked `BranchOutcome::Taken` and losers `Rejected`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Branch {
    pub id: Uuid,
    pub label: String,
    pub outcome: BranchOutcome,
    /// Confidence score in `[0.0, 1.0]`, e.g. `speculate`'s per-candidate
    /// mean confidence.
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BranchOutcome {
    /// This branch was selected.
    Taken,
    /// This branch was considered but discarded.
    Rejected { reason: String },
}

impl Branch {
    /// Construct a `Branch` marked `Taken` — the winning candidate.
    pub fn taken(label: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            outcome: BranchOutcome::Taken,
            confidence: None,
        }
    }

    /// Construct a `Branch` marked `Rejected` with a reason — a losing candidate.
    pub fn rejected(label: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            outcome: BranchOutcome::Rejected {
                reason: reason.into(),
            },
            confidence: None,
        }
    }

    /// Builder: attach a confidence score, clamped into `[0.0, 1.0]`.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_confidence_clamps_into_unit_range() {
        assert_eq!(Step::named("a").with_confidence(1.5).confidence, Some(1.0));
        assert_eq!(Step::named("a").with_confidence(-0.5).confidence, Some(0.0));
    }

    #[test]
    fn step_rejected_and_failed_flip_outcome_and_helpers() {
        let rejected = Step::named("a").rejected("nope");
        assert!(rejected.is_rejected());
        assert!(!rejected.is_failed());

        let failed = Step::named("b").failed("boom");
        assert!(failed.is_failed());
        assert!(!failed.is_rejected());

        let taken = Step::named("c");
        assert!(!taken.is_rejected());
        assert!(!taken.is_failed());
    }

    #[test]
    fn with_cost_attaches_a_cost_and_defaults_to_none() {
        assert_eq!(Step::named("a").cost, None);
        let step = Step::named("a").with_cost(StepCost::new(10, 5));
        assert_eq!(step.cost.unwrap().total_tokens(), 15);
    }

    #[test]
    fn step_deserializes_from_a_checkpoint_written_before_the_cost_field() {
        // Regression guard for the `#[serde(default)]` on `cost`: a step
        // serialized by <= v0.2.1 has no `cost` key at all.
        let legacy = serde_json::json!({
            "id": uuid::Uuid::new_v4(),
            "name": "legacy",
            "confidence": null,
            "duration": null,
            "started_at": chrono::Utc::now(),
            "outcome": "Taken",
            "branches": [],
            "notes": null,
        });
        let step: Step = serde_json::from_value(legacy).expect("legacy step deserializes");
        assert_eq!(step.name, "legacy");
        assert_eq!(step.cost, None);
    }

    #[test]
    fn branch_confidence_clamps_into_unit_range() {
        assert_eq!(
            Branch::taken("a").with_confidence(2.0).confidence,
            Some(1.0)
        );
    }

    #[test]
    fn branch_taken_and_rejected_set_expected_outcome() {
        let taken = Branch::taken("a");
        assert_eq!(taken.outcome, BranchOutcome::Taken);

        let rejected = Branch::rejected("b", "worse option");
        assert_eq!(
            rejected.outcome,
            BranchOutcome::Rejected {
                reason: "worse option".to_string()
            }
        );
    }

    proptest::proptest! {
        #[test]
        fn step_with_confidence_is_always_in_unit_range(raw in -1000.0f64..1000.0) {
            let confidence = Step::named("a").with_confidence(raw).confidence.unwrap();
            proptest::prop_assert!((0.0..=1.0).contains(&confidence));
        }

        #[test]
        fn branch_with_confidence_is_always_in_unit_range(raw in -1000.0f64..1000.0) {
            let confidence = Branch::taken("a").with_confidence(raw).confidence.unwrap();
            proptest::prop_assert!((0.0..=1.0).contains(&confidence));
        }
    }
}
