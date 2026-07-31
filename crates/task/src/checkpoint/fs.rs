use super::CheckpointStore;
use std::path::{Path, PathBuf};
use tracers_core::TraceErr;

/// A [`CheckpointStore`] backed by a single file on disk.
pub struct FileCheckpointStore {
    path: PathBuf,
}

impl FileCheckpointStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl CheckpointStore for FileCheckpointStore {
    fn load(&self) -> Result<String, TraceErr> {
        std::fs::read_to_string(&self.path)
            .map_err(|e| TraceErr::other(format!("could not read checkpoint: {e}")))
    }

    fn save(&self, data: &str) -> Result<(), TraceErr> {
        std::fs::write(&self.path, data)
            .map_err(|e| TraceErr::other(format!("could not write checkpoint: {e}")))
    }
}
