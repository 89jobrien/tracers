//! Integration test: a `TaskRegistry` checkpointed through a real
//! `FileCheckpointStore` survives a simulated crash — a fresh registry
//! restored from the same file resumes exactly where the original left
//! off, with dependency-gated readiness intact.

use trace_lang_core::TraceRef;
use trace_lang_task::{FileCheckpointStore, Priority, Task, TaskRegistry};
use uuid::Uuid;

fn temp_checkpoint_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("tracers-integration-{}.json", Uuid::new_v4()))
}

#[test]
fn registry_resumes_from_checkpoint_after_simulated_crash() {
    let path = temp_checkpoint_path();
    let store = FileCheckpointStore::new(&path);

    let setup = Task::new("gather requirements").with_priority(Priority::High);
    let setup_id = setup.id;
    let dependent = Task::new("write design doc").depends_on(setup_id);
    let dependent_id = dependent.id;

    let mut registry = TaskRegistry::new();
    registry.insert(setup);
    registry.insert(dependent);
    registry.save(&store).expect("initial checkpoint must save");

    // "Crash": the original `registry` is dropped without further action.
    drop(registry);

    // Resume: a fresh registry restored from the same store picks up
    // exactly where the crashed one left off.
    let mut resumed = TaskRegistry::load(&store).expect("must restore from checkpoint");
    assert_eq!(resumed.total(), 2);
    let ready_titles: Vec<_> = resumed
        .ready_tasks()
        .iter()
        .map(|t| t.title.clone())
        .collect();
    assert_eq!(ready_titles, vec!["gather requirements"]);

    // Complete the ready task and checkpoint again through the resumed
    // registry — this is the "every state transition is checkpointed"
    // invariant CLAUDE.md documents for `TaskRegistry`.
    resumed
        .complete(setup_id, TraceRef(Uuid::new_v4()), &store)
        .expect("completing a task must save a checkpoint");

    // Simulate a second crash and resume again.
    drop(resumed);
    let resumed_again = TaskRegistry::load(&store).expect("must restore after second save");
    assert!(resumed_again.get(setup_id).unwrap().is_done());
    let ready_titles: Vec<_> = resumed_again
        .ready_tasks()
        .iter()
        .map(|t| t.title.clone())
        .collect();
    assert_eq!(ready_titles, vec!["write design doc"]);
    assert_eq!(
        resumed_again.get(dependent_id).unwrap().title,
        "write design doc"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn loading_before_any_save_fails_rather_than_returning_an_empty_registry() {
    let path = temp_checkpoint_path();
    let store = FileCheckpointStore::new(&path);

    let result = TaskRegistry::load(&store);
    assert!(
        result.is_err(),
        "loading a registry with no prior checkpoint must fail, not silently return empty"
    );
}
