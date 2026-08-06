use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// A single unit of reasoning. Every `observe`, `branch`, and `emit` in a
/// trace:: agent produces a `Step` that is appended to the `Trace<T>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
