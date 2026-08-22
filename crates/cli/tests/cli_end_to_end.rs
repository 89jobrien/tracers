//! End-to-end tests for the `trace` binary itself.
//!
//! The library tests in `src/lib.rs` cover the queries against in-memory
//! registries. These cover the parts only a real invocation exercises:
//! argument parsing, reading a checkpoint off disk, exit codes, and the
//! shape of what actually lands on stdout.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use trace_lang_core::{ApprovalRequest, TraceErr, TraceRef};
use trace_lang_task::{FileCheckpointStore, Priority, Task, TaskRegistry};
use uuid::Uuid;

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("trace-cli-{name}-{}.trace.json", Uuid::new_v4()))
}

/// survey (done) → design (paused) → write (pending), plus a stray task.
fn fixture(path: &Path) -> (Uuid, Uuid, TraceRef, TraceRef) {
    let store = FileCheckpointStore::new(path);

    let survey = Task::new("survey the schema").with_priority(Priority::High);
    let design = Task::new("design the migration").depends_on(survey.id);
    let write = Task::new("write the migration").depends_on(design.id);
    let (survey_id, design_id) = (survey.id, design.id);

    let mut registry = TaskRegistry::from(vec![survey, design, write, Task::new("changelog")]);

    let survey_trace = TraceRef(Uuid::new_v4());
    registry
        .get_mut(survey_id)
        .expect("survey is in the registry")
        .complete(survey_trace.clone());

    let design_trace = TraceRef(Uuid::new_v4());
    registry
        .get_mut(design_id)
        .expect("design is in the registry")
        .assign_to("Designer");
    registry
        .pause(
            design_id,
            ApprovalRequest::new("approve the destructive migration?", design_trace.clone())
                .with_context(serde_json::json!({ "drops_columns": 2 })),
            &store,
        )
        .expect("pausing checkpoints");

    (survey_id, design_id, survey_trace, design_trace)
}

fn trace_cmd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trace"))
        .args(args)
        .output()
        .expect("the trace binary runs")
}

fn stdout_of(args: &[&str]) -> String {
    let output = trace_cmd(args);
    assert!(
        output.status.success(),
        "`trace {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout is utf-8")
}

#[test]
fn list_prints_every_task_and_filters_by_status() {
    let path = scratch("list");
    fixture(&path);
    let file = path.to_string_lossy().to_string();

    let all = stdout_of(&["list", &file]);
    assert!(all.contains("survey the schema"));
    assert!(all.contains("design the migration"));
    assert!(all.contains("4 task(s)"));

    let paused = stdout_of(&["list", &file, "--status", "paused"]);
    assert!(paused.contains("design the migration"));
    assert!(!paused.contains("survey the schema"));
    assert!(paused.contains("1 task(s)"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn list_json_is_machine_readable() {
    let path = scratch("list-json");
    fixture(&path);

    let raw = stdout_of(&["list", &path.to_string_lossy(), "--json"]);
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("--json emits valid JSON");
    let rows = parsed.as_array().expect("a JSON array of tasks");
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().any(|r| r["status"] == "paused"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn show_resolves_a_task_by_id_prefix_and_names_its_dependencies() {
    let path = scratch("show");
    let (_survey_id, design_id, _, _) = fixture(&path);
    let prefix = design_id.simple().to_string()[..8].to_string();

    let rendered = stdout_of(&["show", &path.to_string_lossy(), &prefix]);
    assert!(rendered.contains("design the migration"));
    assert!(rendered.contains("paused"));
    assert!(rendered.contains("approve the destructive migration?"));
    assert!(rendered.contains("drops_columns"));
    // The dependency is resolved to a title and a state, not left as a uuid.
    assert!(rendered.contains("survey the schema [done]"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn chain_walks_back_from_a_trace_to_what_produced_it() {
    let path = scratch("chain");
    let (_, _, _, design_trace) = fixture(&path);

    let rendered = stdout_of(&["chain", &path.to_string_lossy(), &design_trace.to_string()]);
    assert!(rendered.contains("design the migration"));
    assert!(rendered.contains("survey the schema"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn diff_reports_what_moved_between_two_checkpoints() {
    let before_path = scratch("diff-before");
    let after_path = scratch("diff-after");
    let (survey_id, design_id, _, _) = fixture(&before_path);

    // Approve the paused task and add a new one.
    let after_store = FileCheckpointStore::new(&after_path);
    let mut after =
        TaskRegistry::load(&FileCheckpointStore::new(&before_path)).expect("the fixture loads");
    after
        .resume(
            design_id,
            trace_lang_core::ApprovalDecision::approve("joe"),
            &after_store,
        )
        .expect("resuming checkpoints");
    after.insert(Task::new("backfill"));
    after.save(&after_store).expect("saving works");

    let rendered = stdout_of(&[
        "diff",
        &before_path.to_string_lossy(),
        &after_path.to_string_lossy(),
    ]);
    assert!(
        rendered.contains("+ "),
        "an added task is reported: {rendered}"
    );
    assert!(rendered.contains("backfill"));
    assert!(rendered.contains("paused → pending"));
    // The untouched task is not mentioned at all.
    assert!(!rendered.contains(&survey_id.simple().to_string()[..8]));

    std::fs::remove_file(&before_path).ok();
    std::fs::remove_file(&after_path).ok();
}

#[test]
fn a_missing_checkpoint_fails_with_a_diagnostic_rather_than_a_panic() {
    let output = trace_cmd(&["list", "/definitely/not/a/checkpoint.json"]);
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not read checkpoint"),
        "the underlying error must survive to the user: {stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "a missing file is a user error, not a crash: {stderr}"
    );
}

#[test]
fn an_unknown_task_id_fails_with_a_message_naming_what_was_searched_for() {
    let path = scratch("unknown");
    fixture(&path);

    let output = trace_cmd(&["show", &path.to_string_lossy(), "ffffffff"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ffffffff"), "{stderr}");

    std::fs::remove_file(&path).ok();
}

#[test]
fn the_error_type_the_cli_surfaces_is_a_diagnostic() {
    // `main` returns `miette::Result`, which requires this conversion to
    // exist — a compile-time guard that the fancy rendering stays wired up.
    fn assert_diagnostic<T: miette::Diagnostic + Send + Sync + 'static>() {}
    assert_diagnostic::<TraceErr>();
}
