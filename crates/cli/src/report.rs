//! The shapes `trace` prints.
//!
//! Every command builds a report value and then renders it — as a table for
//! a person, or as JSON for whatever is calling the CLI. Keeping the report
//! separate from the rendering is what makes both possible without two
//! implementations of the same query.

use serde::Serialize;
use trace_lang_core::TraceRef;
use trace_lang_task::{Task, TaskStatus};
use uuid::Uuid;

/// The first eight characters of a UUID — enough to identify a task by eye
/// in a checkpoint of any realistic size, and enough for `show` to resolve.
pub fn short(id: &Uuid) -> String {
    id.simple().to_string()[..8].to_string()
}

/// A one-word status label, and the trace it points at if it has one.
pub fn status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Done(_) => "done",
        TaskStatus::Failed { .. } => "failed",
        TaskStatus::Paused(_) => "paused",
    }
}

/// The execution trace a terminal or paused status points back at.
pub fn trace_of(status: &TaskStatus) -> Option<&TraceRef> {
    match status {
        TaskStatus::Done(trace) => Some(trace),
        TaskStatus::Failed { trace, .. } => Some(trace),
        TaskStatus::Paused(request) => Some(&request.trace),
        TaskStatus::Pending | TaskStatus::Running => None,
    }
}

/// Just enough of a task to identify it in a list or a chain.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskSummary {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub assigned_to: Option<String>,
    pub trace: Option<TraceRef>,
}

impl From<&Task> for TaskSummary {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id,
            title: task.title.clone(),
            status: status_label(&task.status).to_string(),
            priority: task.priority.to_string(),
            assigned_to: task.assigned_to.clone(),
            trace: trace_of(&task.status).cloned(),
        }
    }
}

impl TaskSummary {
    /// One aligned row: `id  status  priority  title`.
    pub fn row(&self) -> String {
        format!(
            "{:<8}  {:<8}  {:<8}  {}",
            short(&self.id),
            self.status,
            self.priority,
            self.title
        )
    }
}

/// What `chain` found: a trace, the task it belongs to, and everything that
/// task waited on, transitively.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChainReport {
    pub trace: TraceRef,
    pub task: TaskSummary,
    /// Nearest dependency first, then its dependencies, and so on.
    pub upstream: Vec<TaskSummary>,
}

/// What changed between two checkpoints.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DiffReport {
    pub added: Vec<TaskSummary>,
    pub removed: Vec<TaskSummary>,
    pub changed: Vec<StatusChange>,
}

impl DiffReport {
    /// True if the two checkpoints describe the same task graph in the same
    /// state.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// A task present in both checkpoints whose status moved.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusChange {
    pub id: Uuid,
    pub title: String,
    pub from: String,
    pub to: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use trace_lang_core::{ApprovalRequest, TraceErr};

    fn a_trace() -> TraceRef {
        TraceRef(Uuid::new_v4())
    }

    #[test]
    fn short_is_the_first_eight_hex_characters() {
        let id = Uuid::parse_str("1234abcd-0000-0000-0000-000000000000").unwrap();
        assert_eq!(short(&id), "1234abcd");
    }

    #[test]
    fn every_status_has_a_label_and_the_terminal_ones_carry_a_trace() {
        let trace = a_trace();
        let cases: Vec<(TaskStatus, &str, bool)> = vec![
            (TaskStatus::Pending, "pending", false),
            (TaskStatus::Running, "running", false),
            (TaskStatus::Done(trace.clone()), "done", true),
            (
                TaskStatus::Failed {
                    error: TraceErr::other("x"),
                    trace: trace.clone(),
                },
                "failed",
                true,
            ),
            (
                TaskStatus::Paused(ApprovalRequest::new("?", trace.clone())),
                "paused",
                true,
            ),
        ];

        for (status, label, has_trace) in cases {
            assert_eq!(status_label(&status), label);
            assert_eq!(trace_of(&status).is_some(), has_trace, "{label}");
        }
    }

    #[test]
    fn a_summary_carries_the_trace_a_done_task_points_at() {
        let mut task = Task::new("ship it");
        let trace = a_trace();
        task.complete(trace.clone());

        let summary = TaskSummary::from(&task);
        assert_eq!(summary.status, "done");
        assert_eq!(summary.trace, Some(trace));
        assert!(summary.row().contains("ship it"));
    }

    #[test]
    fn an_empty_diff_reports_itself_as_empty() {
        assert!(DiffReport::default().is_empty());
        assert!(
            !DiffReport {
                added: vec![TaskSummary::from(&Task::new("new"))],
                ..Default::default()
            }
            .is_empty()
        );
    }
}
