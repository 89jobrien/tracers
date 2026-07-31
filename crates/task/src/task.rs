use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracers_core::{TraceErr, TraceRef};
use uuid::Uuid;

/// A serializable, dependency-aware unit of work.
///
/// Tasks are first-class values in trace:: — not opaque strings or
/// fire-and-forget futures. Every field is serializable so the full
/// task graph can be checkpointed and resumed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub goal: Option<String>,
    pub status: TaskStatus,
    pub priority: Priority,
    /// Confidence estimate (0.0–1.0). Populated once an agent picks up
    /// the task and assesses it.
    pub confidence: Option<f64>,
    /// IDs of tasks that must reach `Done` before this one becomes ready.
    pub depends_on: Vec<Uuid>,
    /// Which agent is currently running this task.
    pub assigned_to: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The lifecycle of a task. `Done` and `Failed` carry a `TraceRef` so
/// every terminal state links back to the execution that produced it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    /// Waiting for dependencies or an available agent.
    Pending,
    /// Actively being executed by `assigned_to`.
    Running,
    /// Completed successfully. Carries a pointer to the execution trace.
    Done(TraceRef),
    /// Failed. Carries the error and a pointer to the partial trace.
    Failed { error: TraceErr, trace: TraceRef },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Normal => write!(f, "normal"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}

impl Task {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            goal: None,
            status: TaskStatus::Pending,
            priority: Priority::Normal,
            confidence: None,
            depends_on: Vec::new(),
            assigned_to: None,
            created_at: now,
            updated_at: now,
        }
    }

    // ── Builder methods ───────────────────────────────────────────────────────

    pub fn with_goal(mut self, goal: impl Into<String>) -> Self {
        self.goal = Some(goal.into());
        self
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Declare that this task must not start until `dep_id` is `Done`.
    pub fn depends_on(mut self, dep_id: Uuid) -> Self {
        self.depends_on.push(dep_id);
        self
    }

    // ── Status transitions ────────────────────────────────────────────────────

    pub fn assign_to(&mut self, agent: impl Into<String>) {
        self.assigned_to = Some(agent.into());
        self.status = TaskStatus::Running;
        self.updated_at = Utc::now();
    }

    pub fn complete(&mut self, trace_ref: TraceRef) {
        self.status = TaskStatus::Done(trace_ref);
        self.assigned_to = None;
        self.updated_at = Utc::now();
    }

    pub fn fail(&mut self, error: TraceErr, trace_ref: TraceRef) {
        self.status = TaskStatus::Failed {
            error,
            trace: trace_ref,
        };
        self.assigned_to = None;
        self.updated_at = Utc::now();
    }

    // ── Status helpers ────────────────────────────────────────────────────────

    pub fn is_pending(&self) -> bool {
        self.status == TaskStatus::Pending
    }

    pub fn is_done(&self) -> bool {
        matches!(self.status, TaskStatus::Done(_))
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.status, TaskStatus::Failed { .. })
    }
}
