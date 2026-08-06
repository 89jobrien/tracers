use crate::checkpoint::CheckpointStore;
use crate::task::Task;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracers_core::TraceErr;
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
        trace_ref: tracers_core::TraceRef,
        store: &impl CheckpointStore,
    ) -> Result<(), TraceErr> {
        if let Some(task) = self.tasks.get_mut(&id) {
            task.complete(trace_ref);
        }
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
            .complete(tracers_core::TraceRef(Uuid::new_v4()));

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
            .complete(tracers_core::TraceRef(Uuid::new_v4()));
        registry
            .get_mut(failed_id)
            .unwrap()
            .fail(TraceErr::other("x"), tracers_core::TraceRef(Uuid::new_v4()));

        assert_eq!(registry.pending().len(), 1);
        assert_eq!(registry.done().len(), 1);
        assert_eq!(registry.failed().len(), 1);
        assert_eq!(registry.total(), 3);
    }
}
