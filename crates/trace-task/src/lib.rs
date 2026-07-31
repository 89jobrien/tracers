//! `trace-task` — serializable task management for trace:: agentic pipelines.
//!
//! Tasks in trace:: are not strings or loose IDs. They are structured,
//! versioned values with identity, status, priority, and dependency edges
//! baked in. Every `TaskStatus::Done` carries a `TraceRef` — a stable
//! pointer to the execution trace that produced it.
//!
//! The `TaskRegistry` is the runtime task graph: it tracks ready tasks
//! (all dependencies satisfied), serializes checkpoints after every
//! completion, and can restore from disk to resume a crashed pipeline.
//!
//! # Example
//!
//! ```rust
//! use trace_task::{Task, TaskRegistry, Priority};
//!
//! let mut registry = TaskRegistry::new();
//!
//! let t1 = Task::new("fetch requirements").with_priority(Priority::High);
//! let t2 = Task::new("plan architecture").depends_on(t1.id);
//!
//! registry.insert(t1);
//! registry.insert(t2);
//!
//! // Only t1 is ready — t2 blocks on it.
//! let ready = registry.ready_tasks();
//! assert_eq!(ready.len(), 1);
//! ```

pub mod registry;
pub mod task;

pub use registry::TaskRegistry;
pub use task::{Priority, Task, TaskStatus};
