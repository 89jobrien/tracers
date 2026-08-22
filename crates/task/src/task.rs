use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use trace_lang_core::{ApprovalDecision, ApprovalRequest, TraceErr, TraceRef};
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
    /// Determines scheduling order via `TaskRegistry::all_by_priority`.
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
    /// Stopped, waiting on a human decision. Not a delegation to a
    /// "reviewer agent" — nothing runs until an [`ApprovalDecision`]
    /// arrives through some external channel, which may be days later.
    /// The request carries the partial trace that reached the pause, so
    /// this state links back to its execution exactly like `Done` and
    /// `Failed` do.
    Paused(ApprovalRequest),
}

/// Task scheduling priority.
///
/// Variant declaration order is load-bearing: `derive(PartialOrd, Ord)`
/// ranks variants by position (`Low < Normal < High < Critical`), and
/// `TaskRegistry::all_by_priority` relies on that ordering via
/// `Reverse(t.priority)` to sort tasks highest-priority-first. Reordering
/// these variants silently changes scheduling behavior.
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
    /// Construct a `Task` with `Pending` status and `Normal` priority.
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

    /// Builder: attach a goal description.
    pub fn with_goal(mut self, goal: impl Into<String>) -> Self {
        self.goal = Some(goal.into());
        self
    }

    /// Builder: set the scheduling priority.
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Builder: attach a confidence estimate, clamped into `[0.0, 1.0]`.
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

    /// Assign the task to an agent and transition to `Running`.
    ///
    /// Every status transition (`assign_to`, `complete`, `fail`) bumps
    /// `updated_at`; `complete` and `fail` additionally clear
    /// `assigned_to` — a consistent side effect that isn't obvious from
    /// the signatures alone.
    pub fn assign_to(&mut self, agent: impl Into<String>) {
        self.assigned_to = Some(agent.into());
        self.status = TaskStatus::Running;
        self.updated_at = Utc::now();
    }

    /// Transition to `Done`, linking the completed execution trace.
    pub fn complete(&mut self, trace_ref: TraceRef) {
        self.status = TaskStatus::Done(trace_ref);
        self.assigned_to = None;
        self.updated_at = Utc::now();
    }

    /// Transition to `Failed`, carrying the error and partial trace.
    pub fn fail(&mut self, error: TraceErr, trace_ref: TraceRef) {
        self.status = TaskStatus::Failed {
            error,
            trace: trace_ref,
        };
        self.assigned_to = None;
        self.updated_at = Utc::now();
    }

    /// Stop and wait for a human, carrying the question and the partial
    /// trace that led to it.
    ///
    /// Unlike `complete` and `fail`, this *keeps* `assigned_to`: the work
    /// is suspended, not finished, and the agent that raised the question
    /// is still the one that owns it.
    pub fn pause(&mut self, request: ApprovalRequest) {
        self.status = TaskStatus::Paused(request);
        self.updated_at = Utc::now();
    }

    /// Apply a human's decision to a paused task.
    ///
    /// An approval returns the task to `Pending` — schedulable again, with
    /// `assigned_to` intact so a scheduler can hand it back to the agent
    /// that asked. A rejection is terminal: the task transitions to
    /// `Failed` with [`TraceErr::ApprovalDenied`], pointing at the same
    /// partial trace the approver saw.
    ///
    /// Errors if the task is not paused — resuming something that never
    /// stopped would silently discard whatever state it *is* in.
    pub fn resume(&mut self, decision: ApprovalDecision) -> Result<(), TraceErr> {
        let TaskStatus::Paused(request) = &self.status else {
            return Err(TraceErr::other(format!(
                "cannot resume task {}: it is not paused",
                self.id
            )));
        };

        match decision {
            ApprovalDecision::Approved { .. } => {
                self.status = TaskStatus::Pending;
            }
            ApprovalDecision::Rejected { by, reason } => {
                let trace = request.trace.clone();
                self.status = TaskStatus::Failed {
                    error: TraceErr::approval_denied(by, reason),
                    trace,
                };
                self.assigned_to = None;
            }
        }
        self.updated_at = Utc::now();
        Ok(())
    }

    // ── Status helpers ────────────────────────────────────────────────────────

    /// True if the task's status is `Pending`.
    pub fn is_pending(&self) -> bool {
        self.status == TaskStatus::Pending
    }

    /// True if the task's status is `Done`.
    pub fn is_done(&self) -> bool {
        matches!(self.status, TaskStatus::Done(_))
    }

    /// True if the task's status is `Failed`.
    pub fn is_failed(&self) -> bool {
        matches!(self.status, TaskStatus::Failed { .. })
    }

    /// True if the task is stopped waiting on a human.
    pub fn is_paused(&self) -> bool {
        matches!(self.status, TaskStatus::Paused(_))
    }

    /// The question this task is waiting on, if it is paused.
    pub fn approval_request(&self) -> Option<&ApprovalRequest> {
        match &self.status {
            TaskStatus::Paused(request) => Some(request),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering_is_low_to_critical() {
        assert!(Priority::Low < Priority::Normal);
        assert!(Priority::Normal < Priority::High);
        assert!(Priority::High < Priority::Critical);
    }

    #[test]
    fn new_task_starts_pending_with_normal_priority() {
        let t = Task::new("do the thing");
        assert!(t.is_pending());
        assert_eq!(t.priority, Priority::Normal);
        assert!(t.assigned_to.is_none());
    }

    #[test]
    fn assign_to_transitions_to_running_and_sets_assignee() {
        let mut t = Task::new("x");
        t.assign_to("agent-1");
        assert_eq!(t.status, TaskStatus::Running);
        assert_eq!(t.assigned_to.as_deref(), Some("agent-1"));
    }

    #[test]
    fn complete_transitions_to_done_and_clears_assignee() {
        let mut t = Task::new("x");
        t.assign_to("agent-1");
        let trace_ref = TraceRef(Uuid::new_v4());
        t.complete(trace_ref);
        assert!(t.is_done());
        assert!(t.assigned_to.is_none());
    }

    #[test]
    fn fail_transitions_to_failed_and_clears_assignee() {
        let mut t = Task::new("x");
        t.assign_to("agent-1");
        let trace_ref = TraceRef(Uuid::new_v4());
        t.fail(TraceErr::other("boom"), trace_ref);
        assert!(t.is_failed());
        assert!(t.assigned_to.is_none());
    }

    fn paused_task() -> Task {
        let mut task = Task::new("refund");
        task.assign_to("Refunder");
        task.pause(ApprovalRequest::new(
            "approve this refund?",
            TraceRef(Uuid::new_v4()),
        ));
        task
    }

    #[test]
    fn pause_keeps_the_assignee_unlike_complete_and_fail() {
        let task = paused_task();
        assert!(task.is_paused());
        assert_eq!(task.assigned_to.as_deref(), Some("Refunder"));
        assert_eq!(
            task.approval_request().map(|r| r.question.as_str()),
            Some("approve this refund?")
        );
    }

    #[test]
    fn resuming_with_an_approval_makes_the_task_schedulable_again() {
        let mut task = paused_task();
        task.resume(ApprovalDecision::approve("joe")).unwrap();

        assert!(task.is_pending());
        assert!(!task.is_paused());
        // The agent that asked keeps the task, so a scheduler can hand it back.
        assert_eq!(task.assigned_to.as_deref(), Some("Refunder"));
    }

    #[test]
    fn resuming_with_a_rejection_fails_the_task_against_the_reviewed_trace() {
        let mut task = paused_task();
        let reviewed = task.approval_request().unwrap().trace.clone();

        task.resume(ApprovalDecision::reject("joe", "over the limit"))
            .unwrap();

        assert!(task.is_failed());
        assert!(task.assigned_to.is_none());
        assert_eq!(
            task.status,
            TaskStatus::Failed {
                error: TraceErr::approval_denied("joe", "over the limit"),
                trace: reviewed,
            }
        );
    }

    #[test]
    fn resuming_a_task_that_was_never_paused_is_an_error() {
        let mut task = Task::new("x");
        assert!(task.resume(ApprovalDecision::approve("joe")).is_err());
        // ... and leaves the task exactly as it was.
        assert!(task.is_pending());
    }

    #[test]
    fn a_paused_task_round_trips_through_json() {
        // Pausing is only useful if the decision can arrive after the
        // process that asked has exited.
        let task = paused_task();
        let json = serde_json::to_string(&task).expect("serializes");
        let restored: Task = serde_json::from_str(&json).expect("deserializes");

        assert!(restored.is_paused());
        assert_eq!(restored.status, task.status);
    }

    #[test]
    fn depends_on_appends_dependency_ids() {
        let dep = Uuid::new_v4();
        let t = Task::new("x").depends_on(dep);
        assert_eq!(t.depends_on, vec![dep]);
    }

    #[test]
    fn with_confidence_clamps_into_unit_range() {
        let t = Task::new("x").with_confidence(5.0);
        assert_eq!(t.confidence, Some(1.0));
    }

    proptest::proptest! {
        #[test]
        fn task_with_confidence_is_always_in_unit_range(raw in -1000.0f64..1000.0) {
            let confidence = Task::new("x").with_confidence(raw).confidence.unwrap();
            proptest::prop_assert!((0.0..=1.0).contains(&confidence));
        }
    }
}
