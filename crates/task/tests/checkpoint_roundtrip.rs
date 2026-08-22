//! Integration test: a `TaskRegistry` checkpointed through a real
//! `FileCheckpointStore` survives a simulated crash — a fresh registry
//! restored from the same file resumes exactly where the original left
//! off, with dependency-gated readiness intact.

use trace_lang_core::{ApprovalDecision, ApprovalRequest, TraceErr, TraceRef};
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

#[test]
fn a_paused_task_survives_a_crash_and_resumes_on_a_decision() {
    // The claim FEATURES.md #5 makes for putting human-in-the-loop *inside*
    // trace:: rather than around it: the pause needs no new storage
    // mechanism, because `TaskRegistry` already checkpoints every
    // transition. This test is that claim, executed.
    let path = temp_checkpoint_path();
    let store = FileCheckpointStore::new(&path);

    let refund = Task::new("issue refund").with_priority(Priority::Critical);
    let refund_id = refund.id;
    let notify = Task::new("notify customer").depends_on(refund_id);
    let notify_id = notify.id;

    let mut registry = TaskRegistry::from(vec![refund, notify]);
    registry.get_mut(refund_id).unwrap().assign_to("Refunder");

    let partial_trace = TraceRef(Uuid::new_v4());
    registry
        .pause(
            refund_id,
            ApprovalRequest::new("approve a $4,000 refund?", partial_trace.clone())
                .with_context(serde_json::json!({ "amount_usd": 4000 })),
            &store,
        )
        .expect("pausing must checkpoint");

    // "Crash": the process that asked the question exits.
    drop(registry);

    // Days later, an approval channel reads the inbox from disk.
    let mut resumed = TaskRegistry::load(&store).expect("must restore from checkpoint");
    let waiting = resumed.paused();
    assert_eq!(waiting.len(), 1);
    let request = waiting[0]
        .approval_request()
        .expect("paused task has a request");
    assert_eq!(request.question, "approve a $4,000 refund?");
    assert_eq!(request.context["amount_usd"], 4000);
    assert_eq!(
        request.trace, partial_trace,
        "the approver must see the trace that led to the question"
    );
    assert!(
        resumed.ready_tasks().is_empty(),
        "a paused task blocks its dependents"
    );

    resumed
        .resume(refund_id, ApprovalDecision::approve("joe"), &store)
        .expect("resuming must checkpoint");

    // A second crash: the approval itself is durable, not just the pause.
    drop(resumed);
    let mut after_approval = TaskRegistry::load(&store).expect("must restore after resume");
    assert!(after_approval.paused().is_empty());
    assert_eq!(after_approval.ready_tasks().len(), 1);
    assert_eq!(after_approval.ready_tasks()[0].id, refund_id);

    // Finishing the approved work unblocks the dependent task as usual.
    after_approval
        .complete(refund_id, TraceRef(Uuid::new_v4()), &store)
        .expect("completing must checkpoint");
    assert_eq!(after_approval.ready_tasks()[0].id, notify_id);

    std::fs::remove_file(&path).ok();
}

#[test]
fn a_rejected_approval_fails_the_task_against_the_trace_the_human_saw() {
    let path = temp_checkpoint_path();
    let store = FileCheckpointStore::new(&path);

    let task = Task::new("delete production database");
    let task_id = task.id;
    let mut registry = TaskRegistry::from(vec![task]);

    let partial_trace = TraceRef(Uuid::new_v4());
    registry
        .pause(
            task_id,
            ApprovalRequest::new("really?", partial_trace.clone()),
            &store,
        )
        .expect("pausing must checkpoint");
    registry
        .resume(
            task_id,
            ApprovalDecision::reject("joe", "absolutely not"),
            &store,
        )
        .expect("resuming must checkpoint");

    let restored = TaskRegistry::load(&store).expect("must restore");
    let failed = restored.get(task_id).expect("task survives");
    assert!(failed.is_failed());
    assert_eq!(
        failed.status,
        trace_lang_task::TaskStatus::Failed {
            error: TraceErr::approval_denied("joe", "absolutely not"),
            trace: partial_trace,
        }
    );

    std::fs::remove_file(&path).ok();
}
