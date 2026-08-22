# trace-lang-task

[![crates.io](https://img.shields.io/crates/v/trace-lang-task.svg)](https://crates.io/crates/trace-lang-task)
[![docs.rs](https://docs.rs/trace-lang-task/badge.svg)](https://docs.rs/trace-lang-task)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://github.com/89jobrien/trace-lang)

Serializable task management for `trace::` agentic pipelines.

Tasks in `trace::` are not strings or loose IDs. They are structured, versioned
values with identity, status, priority, and dependency edges baked in. Every
`TaskStatus::Done` carries a `TraceRef` — a stable pointer to the execution trace
that produced it, from `trace-lang-core`. The `TaskRegistry` is the runtime task
graph: it tracks ready tasks (all dependencies satisfied), checkpoints itself
after every completion, and can restore from disk to resume a crashed pipeline.

## Install

```toml
[dependencies]
trace-lang-task = { path = "../task" }
```

## Quick start

```rust
use trace_lang_task::{Task, TaskRegistry, Priority};

let mut registry = TaskRegistry::new();

let t1 = Task::new("fetch requirements").with_priority(Priority::High);
let t2 = Task::new("plan architecture").depends_on(t1.id);

registry.insert(t1);
registry.insert(t2);

// Only t1 is ready — t2 blocks on it.
let ready = registry.ready_tasks();
assert_eq!(ready.len(), 1);
```

## Types

### `Task`

A serializable, dependency-aware unit of work.

| field | type | notes |
| --- | --- | --- |
| `id` | `Uuid` | assigned on `Task::new` |
| `title` | `String` | |
| `goal` | `Option<String>` | set via `with_goal` |
| `status` | `TaskStatus` | starts `Pending` |
| `priority` | `Priority` | starts `Normal` |
| `confidence` | `Option<f64>` | clamped `[0.0, 1.0]` via `with_confidence` |
| `depends_on` | `Vec<Uuid>` | must all reach `Done` before this task is ready |
| `assigned_to` | `Option<String>` | set by `assign_to`, cleared by `complete`/`fail` |
| `created_at` / `updated_at` | `DateTime<Utc>` | `updated_at` bumps on every status transition |

Builder methods: `with_goal`, `with_priority`, `with_confidence`, `depends_on`.

Status transitions: `assign_to(agent)` → `Running`; `complete(trace_ref)` → `Done`;
`fail(error, trace_ref)` → `Failed`; `pause(request)` → `Paused`;
`resume(decision)` → `Pending` on approval, `Failed` on rejection. `complete`,
`fail`, and a rejected `resume` all clear `assigned_to`; `pause` deliberately does
not, because the work is suspended rather than finished and still belongs to the
agent that raised the question. Status checks: `is_pending()`, `is_done()`,
`is_failed()`, `is_paused()`, plus `approval_request()`.

`resume` errors rather than doing anything if the task is not paused — resuming
something that never stopped would silently discard whatever state it is in.

### `TaskStatus`

```rust
enum TaskStatus {
    Pending,
    Running,
    Done(TraceRef),
    Failed { error: TraceErr, trace: TraceRef },
    Paused(ApprovalRequest),
}
```

`Done` and `Failed` both carry a `TraceRef` from `trace-lang-core` — no terminal state
is ever detached from the execution that produced it. `Paused` carries an
`ApprovalRequest`, which carries the partial trace that reached the pause, so the
same rule holds while the work is stopped.

### `Priority`

```rust
enum Priority { Low, Normal, High, Critical }
```

Derives `PartialOrd`/`Ord` by declaration order (`Low < Normal < High < Critical`).
`TaskRegistry::all_by_priority` relies on this ordering — reordering the variants
would silently change scheduling behavior.

### `TaskRegistry`

Owns all tasks in a `HashMap<Uuid, Task>` and resolves dependency ordering.

| method | returns | purpose |
| --- | --- | --- |
| `new()` | `Self` | empty registry |
| `insert(task)` | — | insert or overwrite by id |
| `get(id)` / `get_mut(id)` | `Option<&Task>` / `Option<&mut Task>` | lookup by id |
| `complete(id, trace_ref, store)` | `Result<(), TraceErr>` | mark done, then checkpoint via `store` |
| `pause(id, request, store)` | `Result<(), TraceErr>` | stop for a human decision, then checkpoint |
| `resume(id, decision, store)` | `Result<(), TraceErr>` | apply that decision, then checkpoint |
| `ready_tasks()` | `Vec<&Task>` | pending tasks whose dependencies are all `Done` |
| `all_by_priority()` | `Vec<&Task>` | all tasks, `Critical` first (`HashMap` order within a priority — sort if you are printing it) |
| `pending()` / `done()` / `failed()` / `paused()` | `Vec<&Task>` | partition by status |
| `total()` | `usize` | task count |
| `save(store)` | `Result<(), TraceErr>` | serialize the whole registry through `store` |
| `TaskRegistry::load(store)` | `Result<Self, TraceErr>` | restore from `store` |

A task with an empty `depends_on` is vacuously ready as soon as it's `Pending`.

## Checkpointing

The registry checkpoints after every task completion:

```rust
use trace_lang_task::FileCheckpointStore;

let store = FileCheckpointStore::new("./checkpoint.trace.json");

// after each task completes, save state
registry.complete(task.id, trace_ref, &store)?;

// resume a crashed executor — no task is re-run unnecessarily
let registry = TaskRegistry::load(&store)?;
```

The same mechanism carries human-in-the-loop pauses, which is the whole argument
for putting them in the library rather than around it — no approval queue, no
side table, no bespoke resume path:

```rust
use trace_lang_core::{ApprovalDecision, ApprovalRequest};

registry.pause(task_id, request, &store)?;   // checkpointed; the process may exit

let mut inbox = TaskRegistry::load(&store)?; // days later
for task in inbox.paused() {
    println!("{}", task.approval_request().expect("paused").question);
}
inbox.resume(task_id, ApprovalDecision::approve("joe"), &store)?;
```

`TaskRegistry` depends only on the `CheckpointStore` trait, never on a concrete
storage mechanism:

```rust
pub trait CheckpointStore {
    fn load(&self) -> Result<String, TraceErr>;
    fn save(&self, data: &str) -> Result<(), TraceErr>;
}
```

`FileCheckpointStore` (a single file on disk) is the only adapter shipped today,
but any backend — S3, a database, an in-memory buffer for tests — can implement
the same trait without touching registry code. `save` has full-overwrite semantics;
implementations must not append or merge with a prior checkpoint, and `load` must
return `Err` (not an empty `Ok` or a panic) if nothing has been saved yet.

For example, an in-memory store for tests:

```rust
use trace_lang_task::CheckpointStore;
use trace_lang_core::TraceErr;
use std::sync::Mutex;

struct MemoryCheckpointStore {
    blob: Mutex<Option<String>>,
}

impl CheckpointStore for MemoryCheckpointStore {
    fn load(&self) -> Result<String, TraceErr> {
        self.blob
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| TraceErr::other("no checkpoint saved yet"))
    }

    fn save(&self, data: &str) -> Result<(), TraceErr> {
        *self.blob.lock().unwrap() = Some(data.to_string());
        Ok(())
    }
}
```

### Testing your own `CheckpointStore`

Enable the `test-support` feature to reuse this crate's conformance suite against
your own adapter:

```toml
[dev-dependencies]
trace-lang-task = { path = "../task", features = ["test-support"] }
```

```rust
use trace_lang_task::checkpoint::conformance::assert_checkpoint_store_contract;

#[test]
fn my_store_conforms() {
    let store = MyCheckpointStore::new(/* ... */);
    assert_checkpoint_store_contract(&store);
}
```

## Testing

```bash
cargo test -p trace-lang-task
cargo test -p trace-lang-task --features test-support
```

Includes `proptest` coverage for confidence clamping, round-tripping tasks through
the registry by id, and `all_by_priority`'s sort invariant.
