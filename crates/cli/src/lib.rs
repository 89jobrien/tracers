//! `trace` — a checkpoint inspector for trace:: task graphs.
//!
//! A `TaskRegistry` checkpoint is a complete, serialized picture of a
//! pipeline: every task, its status, its dependencies, and the `TraceRef`
//! linking each terminal state back to the execution that produced it. This
//! crate is the reader for that file.
//!
//! ```text
//! trace list   checkpoint.trace.json
//! trace show   checkpoint.trace.json 1a2b3c4d
//! trace chain  checkpoint.trace.json trace::9f8e...
//! trace diff   before.trace.json after.trace.json
//! ```
//!
//! Every command takes `--json`, because the point of an agent-first CLI is
//! that an agent can read the output.
//!
//! Checkpoints are read through [`trace_lang_task::CheckpointStore`], never
//! `std::fs` directly, so pointing `trace` at some other backing store is a
//! matter of swapping the adapter.

pub mod report;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use trace_lang_core::{TraceErr, TraceRef};
use trace_lang_task::{FileCheckpointStore, Task, TaskRegistry, TaskStatus};
use uuid::Uuid;

use report::{ChainReport, DiffReport, StatusChange, TaskSummary, short, status_label, trace_of};

/// Inspect trace:: checkpoint files.
#[derive(Debug, Parser)]
#[command(name = "trace", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List the tasks in a checkpoint.
    List {
        /// Path to a `.trace.json` checkpoint.
        checkpoint: PathBuf,
        /// Show only tasks in this state.
        #[arg(long, value_enum)]
        status: Option<StatusFilter>,
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Show one task in full, including its dependencies and trace link.
    Show {
        /// Path to a `.trace.json` checkpoint.
        checkpoint: PathBuf,
        /// Task id, or any unambiguous prefix of one.
        task: String,
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Find the task a trace belongs to, and what it waited on.
    Chain {
        /// Path to a `.trace.json` checkpoint.
        checkpoint: PathBuf,
        /// A trace reference — `trace::<uuid>`, a bare uuid, or a prefix.
        trace: String,
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Compare two checkpoints of the same pipeline.
    Diff {
        /// The earlier checkpoint.
        before: PathBuf,
        /// The later checkpoint.
        after: PathBuf,
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StatusFilter {
    Pending,
    Running,
    Done,
    Failed,
    Paused,
}

impl StatusFilter {
    fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Paused => "paused",
        }
    }
}

/// Run one command and print its output.
pub fn run(cli: Cli) -> Result<(), TraceErr> {
    match cli.command {
        Command::List {
            checkpoint,
            status,
            json,
        } => {
            let registry = load(&checkpoint)?;
            let rows = list(&registry, status);
            emit(json, &rows, || render_list(&rows))
        }
        Command::Show {
            checkpoint,
            task,
            json,
        } => {
            let registry = load(&checkpoint)?;
            let found = resolve_task(&registry, &task)?;
            if json {
                println!("{}", to_json(found)?);
            } else {
                println!("{}", render_show(&registry, found));
            }
            Ok(())
        }
        Command::Chain {
            checkpoint,
            trace,
            json,
        } => {
            let registry = load(&checkpoint)?;
            let report = chain(&registry, &trace)?;
            emit(json, &report, || render_chain(&report))
        }
        Command::Diff {
            before,
            after,
            json,
        } => {
            let report = diff(&load(&before)?, &load(&after)?);
            emit(json, &report, || render_diff(&report))
        }
    }
}

fn emit<T: serde::Serialize>(
    json: bool,
    value: &T,
    text: impl FnOnce() -> String,
) -> Result<(), TraceErr> {
    if json {
        println!("{}", to_json(value)?);
    } else {
        println!("{}", text());
    }
    Ok(())
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String, TraceErr> {
    serde_json::to_string_pretty(value).map_err(|e| TraceErr::Serde(e.to_string()))
}

/// Read a checkpoint through the `CheckpointStore` port.
pub fn load(path: &std::path::Path) -> Result<TaskRegistry, TraceErr> {
    TaskRegistry::load(&FileCheckpointStore::new(path))
}

// ── Queries ─────────────────────────────────────────────────────────────────

/// Every task, highest priority first, optionally filtered by status.
///
/// Ties are broken by title and then id. `TaskRegistry` stores tasks in a
/// `HashMap`, so without that the order of equal-priority tasks would
/// change between runs — which makes CLI output impossible to diff and
/// makes a passing test a coincidence.
pub fn list(registry: &TaskRegistry, status: Option<StatusFilter>) -> Vec<TaskSummary> {
    let mut tasks: Vec<&Task> = registry
        .all_by_priority()
        .into_iter()
        .filter(|t| status.is_none_or(|s| status_label(&t.status) == s.label()))
        .collect();
    tasks.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    tasks.into_iter().map(TaskSummary::from).collect()
}

/// Find a task by id or by any unambiguous prefix of one.
///
/// An ambiguous prefix is an error rather than a silent first-match: acting
/// on the wrong task because two ids shared four characters is exactly the
/// kind of failure this tool exists to prevent.
pub fn resolve_task<'a>(registry: &'a TaskRegistry, needle: &str) -> Result<&'a Task, TraceErr> {
    let needle = normalize(needle);
    if needle.is_empty() {
        return Err(TraceErr::other("no task id given"));
    }

    let matches: Vec<&Task> = registry
        .all_by_priority()
        .into_iter()
        .filter(|t| normalize(&t.id.to_string()).starts_with(&needle))
        .collect();

    match matches.as_slice() {
        [task] => Ok(task),
        [] => Err(TraceErr::other(format!("no task matches {needle:?}"))),
        many => Err(TraceErr::other(format!(
            "{needle:?} is ambiguous — {} tasks match: {}",
            many.len(),
            many.iter()
                .map(|t| short(&t.id))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Find the task a trace belongs to, then walk its dependency edges.
///
/// `TaskRegistry` links tasks to traces one way — a status points at a
/// `TraceRef`. Walking back the other way is a scan, which is fine for a
/// checkpoint file and honest about what the data model actually stores.
pub fn chain(registry: &TaskRegistry, needle: &str) -> Result<ChainReport, TraceErr> {
    let needle = normalize(needle.trim_start_matches("trace::"));
    if needle.is_empty() {
        return Err(TraceErr::other("no trace reference given"));
    }

    let matches: Vec<&Task> = registry
        .all_by_priority()
        .into_iter()
        .filter(|t| {
            trace_of(&t.status).is_some_and(|r| normalize(&r.0.to_string()).starts_with(&needle))
        })
        .collect();

    let task = match matches.as_slice() {
        [task] => *task,
        [] => {
            return Err(TraceErr::other(format!(
                "no task in this checkpoint points at trace {needle:?}"
            )));
        }
        many => {
            return Err(TraceErr::other(format!(
                "{needle:?} is ambiguous — {} tasks match",
                many.len()
            )));
        }
    };

    Ok(ChainReport {
        trace: trace_of(&task.status)
            .cloned()
            .unwrap_or(TraceRef(Uuid::nil())),
        task: TaskSummary::from(task),
        upstream: upstream_of(registry, task),
    })
}

/// Every task `task` transitively waited on, breadth-first.
fn upstream_of(registry: &TaskRegistry, task: &Task) -> Vec<TaskSummary> {
    let mut seen: HashSet<Uuid> = HashSet::from([task.id]);
    let mut queue: VecDeque<Uuid> = task.depends_on.iter().copied().collect();
    let mut out = Vec::new();

    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        // A dependency id with no task behind it is a dangling edge — skip
        // it rather than inventing a placeholder the checkpoint never had.
        if let Some(dep) = registry.get(id) {
            out.push(TaskSummary::from(dep));
            queue.extend(dep.depends_on.iter().copied());
        }
    }
    out
}

/// What changed between two checkpoints, keyed by task id.
pub fn diff(before: &TaskRegistry, after: &TaskRegistry) -> DiffReport {
    let by_id = |registry: &TaskRegistry| -> HashMap<Uuid, Task> {
        registry
            .all_by_priority()
            .into_iter()
            .map(|t| (t.id, t.clone()))
            .collect()
    };
    let (old, new) = (by_id(before), by_id(after));

    let mut report = DiffReport::default();

    for task in after.all_by_priority() {
        match old.get(&task.id) {
            None => report.added.push(TaskSummary::from(task)),
            Some(previous) => {
                let (from, to) = (status_label(&previous.status), status_label(&task.status));
                // Compare labels, not the statuses themselves: a task that
                // failed twice with different errors did not change state.
                if from != to {
                    report.changed.push(StatusChange {
                        id: task.id,
                        title: task.title.clone(),
                        from: from.to_string(),
                        to: to.to_string(),
                    });
                }
            }
        }
    }

    for task in before.all_by_priority() {
        if !new.contains_key(&task.id) {
            report.removed.push(TaskSummary::from(task));
        }
    }

    // Same reason `list` sorts: a `HashMap` walk is not a stable order, and
    // a diff whose lines move between runs is not much of a diff.
    report
        .added
        .sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.cmp(&b.id)));
    report
        .removed
        .sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.cmp(&b.id)));
    report
        .changed
        .sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.cmp(&b.id)));

    report
}

fn normalize(raw: &str) -> String {
    raw.trim().to_lowercase().replace('-', "")
}

// ── Rendering ───────────────────────────────────────────────────────────────

fn render_list(rows: &[TaskSummary]) -> String {
    if rows.is_empty() {
        return "no matching tasks".to_string();
    }
    let mut out = format!(
        "{:<8}  {:<8}  {:<8}  {}",
        "id", "status", "priority", "title"
    );
    for row in rows {
        out.push('\n');
        out.push_str(&row.row());
    }
    out.push_str(&format!("\n\n{} task(s)", rows.len()));
    out
}

fn render_show(registry: &TaskRegistry, task: &Task) -> String {
    let mut out = String::new();
    let mut line = |label: &str, value: String| {
        out.push_str(&format!("{label:<14}{value}\n"));
    };

    line("id", task.id.to_string());
    line("title", task.title.clone());
    if let Some(goal) = &task.goal {
        line("goal", goal.clone());
    }
    line("status", status_label(&task.status).to_string());
    line("priority", task.priority.to_string());
    line(
        "assigned to",
        task.assigned_to.clone().unwrap_or_else(|| "—".to_string()),
    );
    line(
        "confidence",
        task.confidence
            .map(|c| format!("{c:.2}"))
            .unwrap_or_else(|| "—".to_string()),
    );
    line("created", task.created_at.to_rfc3339());
    line("updated", task.updated_at.to_rfc3339());

    if let Some(trace) = trace_of(&task.status) {
        line("trace", trace.to_string());
    }
    match &task.status {
        TaskStatus::Failed { error, .. } => line("error", error.to_string()),
        TaskStatus::Paused(request) => {
            line("waiting on", request.question.clone());
            line("asked", request.requested_at.to_rfc3339());
            if !request.context.is_null() {
                line("context", request.context.to_string());
            }
        }
        _ => {}
    }

    if task.depends_on.is_empty() {
        line("depends on", "—".to_string());
    } else {
        out.push_str("depends on\n");
        for id in &task.depends_on {
            let described = match registry.get(*id) {
                Some(dep) => format!("{} [{}]", dep.title, status_label(&dep.status)),
                None => "<not in this checkpoint>".to_string(),
            };
            out.push_str(&format!("  {}  {described}\n", short(id)));
        }
    }
    out.trim_end().to_string()
}

fn render_chain(report: &ChainReport) -> String {
    let mut out = format!(
        "{}\n  produced by  {} [{}]\n",
        report.trace, report.task.title, report.task.status
    );
    if report.upstream.is_empty() {
        out.push_str("\nnothing upstream — this task had no dependencies");
        return out;
    }
    out.push_str("\nupstream, nearest first\n");
    for task in &report.upstream {
        let trace = task
            .trace
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "no trace yet".to_string());
        out.push_str(&format!(
            "  {:<8}  {:<8}  {:<28}  {trace}\n",
            short(&task.id),
            task.status,
            task.title
        ));
    }
    out.trim_end().to_string()
}

fn render_diff(report: &DiffReport) -> String {
    if report.is_empty() {
        return "no changes".to_string();
    }
    let mut out = String::new();
    for task in &report.added {
        out.push_str(&format!("+ {:<8}  {}\n", short(&task.id), task.title));
    }
    for task in &report.removed {
        out.push_str(&format!("- {:<8}  {}\n", short(&task.id), task.title));
    }
    for change in &report.changed {
        out.push_str(&format!(
            "~ {:<8}  {}: {} → {}\n",
            short(&change.id),
            change.title,
            change.from,
            change.to
        ));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use trace_lang_core::{ApprovalRequest, TraceErr as CoreErr};

    fn a_trace() -> TraceRef {
        TraceRef(Uuid::new_v4())
    }

    /// survey → design → write, plus an unrelated low-priority task.
    fn a_registry() -> (TaskRegistry, Uuid, Uuid, Uuid) {
        let survey = Task::new("survey").with_priority(trace_lang_task::Priority::High);
        let design = Task::new("design").depends_on(survey.id);
        let write = Task::new("write").depends_on(design.id);
        let (s, d, w) = (survey.id, design.id, write.id);
        (
            TaskRegistry::from(vec![survey, design, write, Task::new("changelog")]),
            s,
            d,
            w,
        )
    }

    #[test]
    fn list_returns_every_task_highest_priority_first() {
        let (registry, _, _, _) = a_registry();
        let rows = list(&registry, None);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].title, "survey");
        assert_eq!(rows[0].priority, "high");
    }

    #[test]
    fn list_is_ordered_deterministically_within_a_priority() {
        // `TaskRegistry` is a HashMap, so this must hold no matter which
        // order insertion happened to produce.
        let registry = TaskRegistry::from(vec![
            Task::new("zebra"),
            Task::new("apple"),
            Task::new("mango"),
            Task::new("urgent").with_priority(trace_lang_task::Priority::Critical),
        ]);
        let rows = list(&registry, None);
        let titles: Vec<&str> = rows.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["urgent", "apple", "mango", "zebra"]);
    }

    #[test]
    fn diff_output_is_ordered_deterministically() {
        let before = TaskRegistry::new();
        let after = TaskRegistry::from(vec![
            Task::new("zebra"),
            Task::new("apple"),
            Task::new("mango"),
        ]);
        let report = diff(&before, &after);
        let titles: Vec<&str> = report.added.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn list_filters_by_status() {
        let (mut registry, survey_id, _, _) = a_registry();
        registry.get_mut(survey_id).unwrap().complete(a_trace());

        assert_eq!(list(&registry, Some(StatusFilter::Done)).len(), 1);
        assert_eq!(list(&registry, Some(StatusFilter::Pending)).len(), 3);
        assert_eq!(list(&registry, Some(StatusFilter::Paused)).len(), 0);
    }

    #[test]
    fn resolve_task_accepts_a_full_id_or_a_prefix() {
        let (registry, survey_id, _, _) = a_registry();
        assert_eq!(
            resolve_task(&registry, &survey_id.to_string())
                .unwrap()
                .title,
            "survey"
        );
        assert_eq!(
            resolve_task(&registry, &short(&survey_id)).unwrap().title,
            "survey"
        );
        // Hyphens and case are cosmetic in a UUID, so neither should matter.
        assert_eq!(
            resolve_task(&registry, &survey_id.to_string().to_uppercase())
                .unwrap()
                .title,
            "survey"
        );
    }

    #[test]
    fn an_ambiguous_prefix_is_an_error_not_a_lucky_guess() {
        let (registry, _, _, _) = a_registry();
        // Every uuid starts with the empty string; a bare "" and a
        // one-character prefix are the two ways to hit this in practice.
        let err = resolve_task(&registry, "").unwrap_err();
        assert!(err.to_string().contains("no task id given"));

        // Force a real collision rather than relying on random uuids.
        let shared = Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000001").unwrap();
        let other = Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000002").unwrap();
        let mut a = Task::new("a");
        a.id = shared;
        let mut b = Task::new("b");
        b.id = other;
        let colliding = TaskRegistry::from(vec![a, b]);

        let err = resolve_task(&colliding, "aaaaaaaa").unwrap_err();
        assert!(err.to_string().contains("ambiguous"), "{err}");
        // The full id still resolves.
        assert_eq!(
            resolve_task(&colliding, &shared.to_string()).unwrap().title,
            "a"
        );
    }

    #[test]
    fn an_unknown_task_is_an_error() {
        let (registry, _, _, _) = a_registry();
        assert!(resolve_task(&registry, "ffffffff").is_err());
    }

    #[test]
    fn chain_finds_the_task_behind_a_trace_and_walks_its_dependencies() {
        let (mut registry, survey_id, design_id, write_id) = a_registry();
        registry.get_mut(survey_id).unwrap().complete(a_trace());
        registry.get_mut(design_id).unwrap().complete(a_trace());

        let target = a_trace();
        registry.get_mut(write_id).unwrap().complete(target.clone());

        let report = chain(&registry, &target.to_string()).unwrap();
        assert_eq!(report.task.title, "write");
        let upstream: Vec<&str> = report.upstream.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(upstream, vec!["design", "survey"]);
        // Each upstream task carries its own trace, so the chain is
        // followable all the way back.
        assert!(report.upstream.iter().all(|t| t.trace.is_some()));
    }

    #[test]
    fn chain_accepts_the_displayed_trace_form_and_a_bare_prefix() {
        let (mut registry, survey_id, _, _) = a_registry();
        let target = a_trace();
        registry
            .get_mut(survey_id)
            .unwrap()
            .complete(target.clone());

        // `TraceRef` displays as `trace::<uuid>`; both that and the bare
        // uuid are things a person will paste in.
        assert!(chain(&registry, &format!("trace::{}", target.0)).is_ok());
        assert!(chain(&registry, &target.0.to_string()).is_ok());
        assert!(chain(&registry, &target.0.to_string()[..8]).is_ok());
    }

    #[test]
    fn chain_reaches_a_paused_task_through_its_pending_approval() {
        let (mut registry, survey_id, _, _) = a_registry();
        let target = a_trace();
        registry
            .get_mut(survey_id)
            .unwrap()
            .pause(ApprovalRequest::new("ok?", target.clone()));

        let report = chain(&registry, &target.to_string()).unwrap();
        assert_eq!(report.task.status, "paused");
        assert!(report.upstream.is_empty());
    }

    #[test]
    fn chain_on_an_unknown_trace_is_an_error() {
        let (registry, _, _, _) = a_registry();
        assert!(chain(&registry, &a_trace().to_string()).is_err());
        assert!(chain(&registry, "trace::").is_err());
    }

    #[test]
    fn upstream_skips_a_dependency_the_checkpoint_does_not_contain() {
        let ghost = Uuid::new_v4();
        let orphan = Task::new("orphan").depends_on(ghost);
        let mut registry = TaskRegistry::from(vec![orphan]);
        let id = registry.pending()[0].id;
        let target = a_trace();
        registry.get_mut(id).unwrap().complete(target.clone());

        let report = chain(&registry, &target.to_string()).unwrap();
        assert!(report.upstream.is_empty());
    }

    #[test]
    fn diff_reports_additions_removals_and_status_moves() {
        let (before, survey_id, _, _) = a_registry();

        let mut after = before.clone();
        after.get_mut(survey_id).unwrap().complete(a_trace());
        after.insert(Task::new("brand new"));

        let report = diff(&before, &after);
        assert_eq!(report.added.len(), 1);
        assert_eq!(report.added[0].title, "brand new");
        assert!(report.removed.is_empty());
        assert_eq!(report.changed.len(), 1);
        assert_eq!(report.changed[0].from, "pending");
        assert_eq!(report.changed[0].to, "done");
        assert!(!report.is_empty());
    }

    #[test]
    fn diff_reports_a_task_that_disappeared() {
        let (before, _, _, _) = a_registry();
        let after = TaskRegistry::new();

        let report = diff(&before, &after);
        assert_eq!(report.removed.len(), 4);
        assert!(report.added.is_empty());
    }

    #[test]
    fn diff_of_a_checkpoint_against_itself_is_empty() {
        let (registry, _, _, _) = a_registry();
        assert!(diff(&registry, &registry).is_empty());
    }

    #[test]
    fn a_task_that_failed_twice_differently_is_not_a_status_change() {
        let (before, survey_id, _, _) = a_registry();
        let mut before = before;
        before
            .get_mut(survey_id)
            .unwrap()
            .fail(CoreErr::other("first"), a_trace());

        let mut after = before.clone();
        after
            .get_mut(survey_id)
            .unwrap()
            .fail(CoreErr::other("second"), a_trace());

        assert!(diff(&before, &after).is_empty());
    }

    // ── Rendering ────────────────────────────────────────────────────────────

    #[test]
    fn rendering_an_empty_result_says_so_rather_than_printing_a_bare_header() {
        assert_eq!(render_list(&[]), "no matching tasks");
        assert_eq!(render_diff(&DiffReport::default()), "no changes");
    }

    #[test]
    fn show_renders_a_paused_task_with_the_question_and_its_dependencies() {
        let (mut registry, survey_id, design_id, _) = a_registry();
        registry.get_mut(design_id).unwrap().pause(
            ApprovalRequest::new("proceed?", a_trace())
                .with_context(serde_json::json!({ "risk": "high" })),
        );

        let rendered = render_show(&registry, registry.get(design_id).unwrap());
        assert!(rendered.contains("paused"));
        assert!(rendered.contains("proceed?"));
        assert!(rendered.contains("\"risk\""));
        // The dependency is named and its state shown, not just its id.
        assert!(rendered.contains(&short(&survey_id)));
        assert!(rendered.contains("survey [pending]"));
    }

    #[test]
    fn show_names_a_dependency_that_is_missing_from_the_checkpoint() {
        let orphan = Task::new("orphan").depends_on(Uuid::new_v4());
        let registry = TaskRegistry::from(vec![orphan]);
        let task = registry.pending()[0];

        assert!(render_show(&registry, task).contains("<not in this checkpoint>"));
    }

    #[test]
    fn chain_renders_a_task_with_no_dependencies_explicitly() {
        let (mut registry, survey_id, _, _) = a_registry();
        let target = a_trace();
        registry
            .get_mut(survey_id)
            .unwrap()
            .complete(target.clone());

        let rendered = render_chain(&chain(&registry, &target.to_string()).unwrap());
        assert!(rendered.contains("nothing upstream"));
    }

    #[test]
    fn every_report_serializes_for_the_json_flag() {
        let (mut registry, survey_id, _, _) = a_registry();
        let target = a_trace();
        registry
            .get_mut(survey_id)
            .unwrap()
            .complete(target.clone());

        assert!(to_json(&list(&registry, None)).is_ok());
        assert!(to_json(&chain(&registry, &target.to_string()).unwrap()).is_ok());
        assert!(to_json(&diff(&TaskRegistry::new(), &registry)).is_ok());
        assert!(to_json(registry.get(survey_id).unwrap()).is_ok());
    }

    #[test]
    fn the_cli_definition_is_internally_consistent() {
        // clap can only catch a malformed command tree at runtime.
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
