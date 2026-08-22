//! `TaskRegistry` end to end: a dependency-gated task graph that survives
//! a crash because every transition checkpoints.
//!
//! ```text
//! cargo run -p trace-lang-examples --example task_pipeline
//! ```

use trace_lang_core::{TraceErr, TraceRef};
use trace_lang_examples::{fact, heading, scratch_path};
use trace_lang_task::{FileCheckpointStore, Priority, Task, TaskRegistry};
use uuid::Uuid;

fn main() -> Result<(), TraceErr> {
    let path = scratch_path("task-pipeline");
    let store = FileCheckpointStore::new(&path);

    // Four tasks, three of them gated behind dependencies. `depends_on`
    // takes ids, so the graph is data — nothing about the ordering lives in
    // control flow.
    let survey = Task::new("survey the existing schema")
        .with_goal("understand what we already store")
        .with_priority(Priority::High);
    let design = Task::new("design the migration").depends_on(survey.id);
    let write = Task::new("write the migration").depends_on(design.id);
    let changelog = Task::new("update the changelog").with_priority(Priority::Low);

    let (survey_id, design_id, write_id) = (survey.id, design.id, write.id);
    let mut registry = TaskRegistry::from(vec![survey, design, write, changelog]);
    registry.save(&store)?;

    heading("scheduling order");
    for task in registry.all_by_priority() {
        fact(&task.title, task.priority);
    }

    heading("what can start right now");
    // Only the two tasks with no unmet dependencies are ready — the other
    // two are blocked, and the registry knows it without being told.
    for task in ready_titles(&registry) {
        fact("ready", task);
    }

    heading("run the first task");
    registry
        .get_mut(survey_id)
        .expect("survey task is in the registry")
        .assign_to("Surveyor");
    registry.save(&store)?;
    fact(
        "status",
        format!("{:?}", registry.get(survey_id).map(|t| &t.status)),
    );

    // A real agent would hand back its own `trace.trace_ref()` here. The
    // point is that `Done` cannot be constructed without one: no output is
    // ever detached from the execution that produced it.
    registry.complete(survey_id, TraceRef(Uuid::new_v4()), &store)?;
    fact("completed", "survey the existing schema");

    heading("the graph re-opens");
    for task in ready_titles(&registry) {
        fact("ready", task);
    }

    heading("crash, then resume from the checkpoint");
    // Every transition above already wrote the full registry through the
    // `CheckpointStore` port. Dropping the in-memory registry loses nothing.
    drop(registry);
    let mut resumed = TaskRegistry::load(&store)?;
    fact("tasks restored", resumed.total());
    fact("done", resumed.done().len());
    fact("still pending", resumed.pending().len());
    for task in ready_titles(&resumed) {
        fact("ready", task);
    }

    heading("a task can also fail");
    resumed
        .get_mut(design_id)
        .expect("design task is in the registry")
        .fail(
            TraceErr::tool_failed("psql", "connection refused"),
            TraceRef(Uuid::new_v4()),
        );
    resumed.save(&store)?;
    fact("failed", resumed.failed().len());
    // A failed dependency is not a satisfied one, so the work behind it
    // stays blocked rather than quietly running against missing input.
    fact(
        "still blocked",
        resumed
            .get(write_id)
            .map(|t| t.title.clone())
            .unwrap_or_default(),
    );

    std::fs::remove_file(&path).ok();
    Ok(())
}

fn ready_titles(registry: &TaskRegistry) -> Vec<String> {
    let mut titles: Vec<String> = registry
        .ready_tasks()
        .iter()
        .map(|t| t.title.clone())
        .collect();
    // `ready_tasks` reads from a HashMap, so sort for a stable printout.
    titles.sort();
    titles
}
