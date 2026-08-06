//! The `CheckpointStore` port — persistence is an infrastructure concern,
//! not something `TaskRegistry` should know how to do itself.

mod fs;

/// Shared contract tests for `CheckpointStore` implementations — used by
/// this crate's own tests and by downstream crates' tests (via the
/// `test-support` feature) to verify their own impls conform.
#[cfg(any(test, feature = "test-support"))]
pub mod conformance;

pub use fs::FileCheckpointStore;

use tracers_core::TraceErr;

/// A place a [`crate::TaskRegistry`] can be checkpointed to and restored from.
///
/// `TaskRegistry` depends on this trait, never on a concrete storage
/// mechanism — implement it for disk, S3, a database, or an in-memory
/// buffer for tests.
pub trait CheckpointStore {
    /// Load the raw serialized checkpoint blob. Must return `Err`, not an
    /// empty `Ok` or a panic, if no checkpoint has ever been saved.
    fn load(&self) -> Result<String, TraceErr>;
    /// Persist `data` as the checkpoint blob. Full-overwrite semantics —
    /// implementations must not append or merge with a prior checkpoint.
    fn save(&self, data: &str) -> Result<(), TraceErr>;
}
