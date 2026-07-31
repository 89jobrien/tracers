use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// A single unit of reasoning. Every `observe`, `branch`, and `emit` in a
/// trace:: agent produces a `Step` that is appended to the `Trace<T>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: Uuid,
    pub name: String,
    pub confidence: Option<f64>,
    pub duration: Option<Duration>,
    pub started_at: DateTime<Utc>,
    pub outcome: StepOutcome,
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

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes = Some(note.into());
        self
    }

    pub fn rejected(mut self, reason: impl Into<String>) -> Self {
        self.outcome = StepOutcome::Rejected {
            reason: reason.into(),
        };
        self
    }

    pub fn failed(mut self, message: impl Into<String>) -> Self {
        self.outcome = StepOutcome::Failed {
            message: message.into(),
        };
        self
    }

    pub fn is_rejected(&self) -> bool {
        matches!(self.outcome, StepOutcome::Rejected { .. })
    }

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
    pub fn taken(label: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            outcome: BranchOutcome::Taken,
            confidence: None,
        }
    }

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

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }
}
