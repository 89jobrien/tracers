use crate::task::Task;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracers_core::TraceErr;
use uuid::Uuid;

/// The runtime task graph.
///
/// `TaskRegistry` owns all tasks, resolves dependency ordering, and
/// serializes checkpoints to disk after every state transition. A
/// crashed executor can call `TaskRegistry::load()` to resume exactly
/// where it left off — no task is ever re-run unnecessarily.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRegistry {
    tasks: HashMap<Uuid, Task>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    /// Restore a registry from a checkpoint file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, TraceErr> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| TraceErr::other(format!("could not read checkpoint: {e}")))?;
        serde_json::from_str(&raw).map_err(|e| TraceErr::Serde(e.to_string()))
    }

    // ── Mutation ──────────────────────────────────────────────────────────────

    pub fn insert(&mut self, task: Task) {
        self.tasks.insert(task.id, task);
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut Task> {
        self.tasks.get_mut(&id)
    }

    pub fn get(&self, id: Uuid) -> Option<&Task> {
        self.tasks.get(&id)
    }

    /// Mark a task as complete and persist a checkpoint.
    pub fn complete(
        &mut self,
        id: Uuid,
        trace_ref: tracers_core::TraceRef,
        checkpoint: impl AsRef<Path>,
    ) -> Result<(), TraceErr> {
        if let Some(task) = self.tasks.get_mut(&id) {
            task.complete(trace_ref);
        }
        self.save(checkpoint)
    }

    // ── Querying ──────────────────────────────────────────────────────────────

    /// Tasks whose dependencies are all `Done` and whose status is `Pending`.
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

    pub fn pending(&self) -> Vec<&Task> {
        self.tasks.values().filter(|t| t.is_pending()).collect()
    }

    pub fn done(&self) -> Vec<&Task> {
        self.tasks.values().filter(|t| t.is_done()).collect()
    }

    pub fn failed(&self) -> Vec<&Task> {
        self.tasks.values().filter(|t| t.is_failed()).collect()
    }

    pub fn total(&self) -> usize {
        self.tasks.len()
    }

    // ── Serialization ─────────────────────────────────────────────────────────

    /// Persist the full registry to disk. Called after every task
    /// transition so the pipeline is always resumable.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), TraceErr> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| TraceErr::Serde(e.to_string()))?;
        std::fs::write(path, json)
            .map_err(|e| TraceErr::other(format!("could not write checkpoint: {e}")))
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
