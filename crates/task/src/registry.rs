use crate::checkpoint::CheckpointStore;
use crate::task::Task;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use trace_lang_core::{ApprovalDecision, ApprovalRequest, TraceErr};
use uuid::Uuid;

/// The runtime task graph.
///
/// `TaskRegistry` owns all tasks and resolves dependency ordering. It
/// checkpoints itself through a [`CheckpointStore`] after every state
/// transition, so a crashed executor can restore from the same store to
/// resume exactly where it left off — no task is ever re-run unnecessarily.
/// `TaskRegistry` has no idea whether that store is a file, a database, or
/// an in-memory buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRegistry {
    tasks: HashMap<Uuid, Task>,
}

impl TaskRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    /// Restore a registry from a checkpoint store.
    pub fn load(store: &impl CheckpointStore) -> Result<Self, TraceErr> {
        let raw = store.load()?;
        serde_json::from_str(&raw).map_err(|e| TraceErr::Serde(e.to_string()))
    }

    // ── Mutation ──────────────────────────────────────────────────────────────

    /// Insert a task, overwriting any existing task with the same `id`.
    pub fn insert(&mut self, task: Task) {
        self.tasks.insert(task.id, task);
    }

    /// Mutably borrow a task by id.
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut Task> {
        self.tasks.get_mut(&id)
    }

    /// Borrow a task by id.
    pub fn get(&self, id: Uuid) -> Option<&Task> {
        self.tasks.get(&id)
    }

    /// Mark a task as complete and persist a checkpoint.
    pub fn complete(
        &mut self,
        id: Uuid,
        trace_ref: trace_lang_core::TraceRef,
        store: &impl CheckpointStore,
    ) -> Result<(), TraceErr> {
        if let Some(task) = self.tasks.get_mut(&id) {
            task.complete(trace_ref);
        }
        self.save(store)
    }

    /// Stop a task to wait on a human decision, and persist a checkpoint.
    ///
    /// The checkpoint is the whole point: the pipeline can exit and the
    /// decision can arrive days later against a registry restored from
    /// disk. Errors if `id` is unknown, rather than silently checkpointing
    /// a pause that never happened.
    pub fn pause(
        &mut self,
        id: Uuid,
        request: ApprovalRequest,
        store: &impl CheckpointStore,
    ) -> Result<(), TraceErr> {
        let task = self
            .tasks
            .get_mut(&id)
            .ok_or_else(|| TraceErr::other(format!("cannot pause unknown task {id}")))?;
        task.pause(request);
        self.save(store)
    }

    /// Apply a human's decision to a paused task and persist a checkpoint.
    ///
    /// Errors if `id` is unknown or the task is not paused; in neither case
    /// is a checkpoint written.
    pub fn resume(
        &mut self,
        id: Uuid,
        decision: ApprovalDecision,
        store: &impl CheckpointStore,
    ) -> Result<(), TraceErr> {
        let task = self
            .tasks
            .get_mut(&id)
            .ok_or_else(|| TraceErr::other(format!("cannot resume unknown task {id}")))?;
        task.resume(decision)?;
        self.save(store)
    }

    // ── Querying ──────────────────────────────────────────────────────────────

    /// Tasks whose dependencies are all `Done` and whose status is `Pending`.
    /// A task with an empty `depends_on` is vacuously satisfied.
    pub fn ready_tasks(&self) -> Vec<&Task> {
        self.tasks
            .values()
            .filter(|t| t.is_pending() && self.dependencies_satisfied(t))
            .collect()
    }

    /// All tasks, sorted by priority descending.
    pub fn all_by_priority(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.values().collect();
        tasks.sort_by_key(|t| std::cmp::Reverse(t.priority));
        tasks
    }

    /// All tasks with status `Pending`.
    pub fn pending(&self) -> Vec<&Task> {
        self.tasks.values().filter(|t| t.is_pending()).collect()
    }

    /// All tasks with status `Done`.
    pub fn done(&self) -> Vec<&Task> {
        self.tasks.values().filter(|t| t.is_done()).collect()
    }

    /// All tasks with status `Failed`.
    pub fn failed(&self) -> Vec<&Task> {
        self.tasks.values().filter(|t| t.is_failed()).collect()
    }

    /// All tasks stopped waiting on a human decision — the inbox an
    /// approval channel (CLI, Slack, web form) reads from.
    pub fn paused(&self) -> Vec<&Task> {
        self.tasks.values().filter(|t| t.is_paused()).collect()
    }

    /// Total number of tasks in the registry.
    pub fn total(&self) -> usize {
        self.tasks.len()
    }

    // ── Serialization ─────────────────────────────────────────────────────────

    /// Persist the full registry through `store`. Called after every task
    /// transition so the pipeline is always resumable.
    pub fn save(&self, store: &impl CheckpointStore) -> Result<(), TraceErr> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| TraceErr::Serde(e.to_string()))?;
        store.save(&json)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn dependencies_satisfied(&self, task: &Task) -> bool {
        task.depends_on.iter().all(|dep_id| {
            self.tasks
                .get(dep_id)
                .map(|dep| dep.is_done())
                .unwrap_or(false)
        })
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<Task>> for TaskRegistry {
    fn from(tasks: Vec<Task>) -> Self {
        let mut registry = Self::new();
        for task in tasks {
            registry.insert(task);
        }
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Priority;
    use std::cell::RefCell;
    use trace_lang_core::TraceRef;

    /// The smallest possible `CheckpointStore` — enough to assert that a
    /// transition checkpointed, without touching the filesystem.
    #[derive(Default)]
    struct MemoryStore {
        blob: RefCell<Option<String>>,
    }

    impl MemoryStore {
        fn saved(&self) -> bool {
            self.blob.borrow().is_some()
        }
    }

    impl CheckpointStore for MemoryStore {
        fn load(&self) -> Result<String, TraceErr> {
            self.blob
                .borrow()
                .clone()
                .ok_or_else(|| TraceErr::other("nothing checkpointed yet"))
        }

        fn save(&self, data: &str) -> Result<(), TraceErr> {
            *self.blob.borrow_mut() = Some(data.to_string());
            Ok(())
        }
    }

    fn a_request() -> ApprovalRequest {
        ApprovalRequest::new("proceed?", TraceRef(Uuid::new_v4()))
    }

    #[test]
    fn task_with_no_dependencies_is_ready_when_pending() {
        let registry = TaskRegistry::from(vec![Task::new("solo")]);
        assert_eq!(registry.ready_tasks().len(), 1);
    }

    #[test]
    fn task_is_not_ready_until_dependency_is_done() {
        let dep = Task::new("dep");
        let dep_id = dep.id;
        let dependent = Task::new("dependent").depends_on(dep_id);

        let mut registry = TaskRegistry::from(vec![dep, dependent]);
        assert_eq!(registry.ready_tasks().len(), 1);
        assert_eq!(registry.ready_tasks()[0].title, "dep");

        registry
            .get_mut(dep_id)
            .unwrap()
            .complete(trace_lang_core::TraceRef(Uuid::new_v4()));

        let ready_titles: Vec<_> = registry.ready_tasks().iter().map(|t| &t.title).collect();
        assert_eq!(ready_titles, vec!["dependent"]);
    }

    #[test]
    fn all_by_priority_sorts_critical_first() {
        let registry = TaskRegistry::from(vec![
            Task::new("low").with_priority(Priority::Low),
            Task::new("critical").with_priority(Priority::Critical),
            Task::new("normal"),
        ]);
        let ordered: Vec<_> = registry
            .all_by_priority()
            .iter()
            .map(|t| &t.title)
            .collect();
        assert_eq!(ordered, vec!["critical", "normal", "low"]);
    }

    #[test]
    fn pending_done_failed_partition_by_status() {
        let mut registry = TaskRegistry::from(vec![
            Task::new("pending"),
            Task::new("done"),
            Task::new("failed"),
        ]);
        let done_id = registry
            .pending()
            .iter()
            .find(|t| t.title == "done")
            .unwrap()
            .id;
        let failed_id = registry
            .pending()
            .iter()
            .find(|t| t.title == "failed")
            .unwrap()
            .id;

        registry
            .get_mut(done_id)
            .unwrap()
            .complete(trace_lang_core::TraceRef(Uuid::new_v4()));
        registry.get_mut(failed_id).unwrap().fail(
            TraceErr::other("x"),
            trace_lang_core::TraceRef(Uuid::new_v4()),
        );

        assert_eq!(registry.pending().len(), 1);
        assert_eq!(registry.done().len(), 1);
        assert_eq!(registry.failed().len(), 1);
        assert_eq!(registry.total(), 3);
    }

    #[test]
    fn a_paused_task_leaves_the_ready_queue_and_joins_the_approval_inbox() {
        let store = MemoryStore::default();
        let mut registry = TaskRegistry::from(vec![Task::new("refund")]);
        let id = registry.pending()[0].id;
        assert_eq!(registry.ready_tasks().len(), 1);

        registry.pause(id, a_request(), &store).unwrap();

        assert!(registry.ready_tasks().is_empty());
        assert_eq!(registry.paused().len(), 1);
        assert!(store.saved(), "pausing must checkpoint");
    }

    #[test]
    fn approving_returns_the_task_to_the_ready_queue() {
        let store = MemoryStore::default();
        let mut registry = TaskRegistry::from(vec![Task::new("refund")]);
        let id = registry.pending()[0].id;
        registry.pause(id, a_request(), &store).unwrap();

        registry
            .resume(id, ApprovalDecision::approve("joe"), &store)
            .unwrap();

        assert!(registry.paused().is_empty());
        assert_eq!(registry.ready_tasks().len(), 1);
    }

    #[test]
    fn rejecting_fails_the_task_and_unblocks_nothing() {
        let store = MemoryStore::default();
        let blocker = Task::new("refund");
        let blocker_id = blocker.id;
        let dependent = Task::new("notify customer").depends_on(blocker_id);
        let mut registry = TaskRegistry::from(vec![blocker, dependent]);
        registry.pause(blocker_id, a_request(), &store).unwrap();

        registry
            .resume(
                blocker_id,
                ApprovalDecision::reject("joe", "too large"),
                &store,
            )
            .unwrap();

        assert_eq!(registry.failed().len(), 1);
        // A rejected dependency is not a satisfied one.
        assert!(registry.ready_tasks().is_empty());
    }

    #[test]
    fn pausing_or_resuming_an_unknown_task_errors_without_checkpointing() {
        let store = MemoryStore::default();
        let mut registry = TaskRegistry::new();
        let missing = Uuid::new_v4();

        assert!(registry.pause(missing, a_request(), &store).is_err());
        assert!(
            registry
                .resume(missing, ApprovalDecision::approve("joe"), &store)
                .is_err()
        );
        assert!(!store.saved());
    }

    #[test]
    fn resuming_a_task_that_is_not_paused_errors_without_checkpointing() {
        let store = MemoryStore::default();
        let mut registry = TaskRegistry::from(vec![Task::new("running")]);
        let id = registry.pending()[0].id;

        assert!(
            registry
                .resume(id, ApprovalDecision::approve("joe"), &store)
                .is_err()
        );
        assert!(!store.saved());
    }

    proptest::proptest! {
        #[test]
        fn insert_then_get_always_round_trips_by_id(title in "[a-z]{1,20}") {
            let task = Task::new(title.clone());
            let id = task.id;
            let mut registry = TaskRegistry::new();
            registry.insert(task);
            let fetched = registry.get(id).unwrap();
            proptest::prop_assert_eq!(&fetched.title, &title);
            proptest::prop_assert_eq!(registry.total(), 1);
        }

        #[test]
        fn all_by_priority_is_always_sorted_non_increasing(
            priorities in proptest::collection::vec(0u8..4, 1..20)
        ) {
            let to_priority = |n: u8| match n {
                0 => Priority::Low,
                1 => Priority::Normal,
                2 => Priority::High,
                _ => Priority::Critical,
            };
            let mut registry = TaskRegistry::new();
            for (i, p) in priorities.iter().enumerate() {
                registry.insert(Task::new(format!("t{i}")).with_priority(to_priority(*p)));
            }
            let ordered = registry.all_by_priority();
            for window in ordered.windows(2) {
                proptest::prop_assert!(window[0].priority >= window[1].priority);
            }
        }
    }
}
