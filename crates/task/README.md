# tracers-task

Serializable task management for `trace::` agentic pipelines.

Tasks in `trace::` are not strings or loose IDs. They are structured, versioned
values with identity, status, priority, and dependency edges baked in. Every
`TaskStatus::Done` carries a `TraceRef` — a stable pointer to the execution trace
that produced it, from `tracers-core`. The `TaskRegistry` is the runtime task
graph: it tracks ready tasks (all dependencies satisfied), checkpoints itself
after every completion, and can restore from disk to resume a crashed pipeline.

## Install

```toml
[dependencies]
tracers-task = { path = "../task" }
```

## Quick start

```rust
use tracers_task::{Task, TaskRegistry, Priority};

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
`fail(error, trace_ref)` → `Failed`. `complete` and `fail` both clear `assigned_to`
as a side effect. Status checks: `is_pending()`, `is_done()`, `is_failed()`.

### `TaskStatus`

```rust
enum TaskStatus {
    Pending,
    Running,
    Done(TraceRef),
    Failed { error: TraceErr, trace: TraceRef },
}
```

`Done` and `Failed` both carry a `TraceRef` from `tracers-core` — no terminal state
is ever detached from the execution that produced it.

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
| `ready_tasks()` | `Vec<&Task>` | pending tasks whose dependencies are all `Done` |
| `all_by_priority()` | `Vec<&Task>` | all tasks, `Critical` first |
| `pending()` / `done()` / `failed()` | `Vec<&Task>` | partition by status |
| `total()` | `usize` | task count |
| `save(store)` | `Result<(), TraceErr>` | serialize the whole registry through `store` |
| `TaskRegistry::load(store)` | `Result<Self, TraceErr>` | restore from `store` |

A task with an empty `depends_on` is vacuously ready as soon as it's `Pending`.

## Checkpointing

The registry checkpoints after every task completion:

```rust
use tracers_task::FileCheckpointStore;

let store = FileCheckpointStore::new("./checkpoint.trace.json");

// after each task completes, save state
registry.complete(task.id, trace_ref, &store)?;

// resume a crashed executor — no task is re-run unnecessarily
let registry = TaskRegistry::load(&store)?;
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
use tracers_task::CheckpointStore;
use tracers_core::TraceErr;
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
tracers-task = { path = "../task", features = ["test-support"] }
```

```rust
use tracers_task::checkpoint::conformance::assert_checkpoint_store_contract;

#[test]
fn my_store_conforms() {
    let store = MyCheckpointStore::new(/* ... */);
    assert_checkpoint_store_contract(&store);
}
```

## Testing

```bash
cargo test -p tracers-task
cargo test -p tracers-task --features test-support
```

Includes `proptest` coverage for confidence clamping, round-tripping tasks through
the registry by id, and `all_by_priority`'s sort invariant.
