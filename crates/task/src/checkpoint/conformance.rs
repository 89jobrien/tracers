//! Shared contract tests for every `CheckpointStore` impl.
//!
//! Only asserts what `FileCheckpointStore`'s current behaviour actually
//! guarantees — `load` before any `save` returns `Err`, not a panic or
//! an `Ok(String::new())`; that's the one invariant both impls can
//! promise today, since neither distinguishes "not found" from other
//! I/O errors in its `TraceErr` variant.

use super::CheckpointStore;

/// Exercise a `CheckpointStore` impl against the shared contract: `load`
/// before any `save` returns `Err`, and `save` round-trips (full-overwrite,
/// not append/merge). Gated behind `test-support` so downstream crates can
/// assert new impls (S3, DB, in-memory) conform without depending on this
/// crate's `#[cfg(test)]` code.
pub fn assert_checkpoint_store_contract<S: CheckpointStore>(store: &S) {
    // Round-trip: whatever was saved is exactly what loads back.
    store
        .save("hello checkpoint")
        .expect("save must succeed on a fresh store");
    let loaded = store.load().expect("load must succeed after a save");
    assert_eq!(loaded, "hello checkpoint");

    // Save is idempotent — the second save fully replaces the first,
    // no append/merge behaviour.
    store.save("replaced").expect("second save must succeed");
    let loaded = store.load().expect("load must succeed after overwrite");
    assert_eq!(loaded, "replaced");
}
