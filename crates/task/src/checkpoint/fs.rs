use super::CheckpointStore;
use std::path::{Path, PathBuf};
use tracers_core::TraceErr;

/// A [`CheckpointStore`] backed by a single file on disk.
pub struct FileCheckpointStore {
    path: PathBuf,
}

impl FileCheckpointStore {
    /// Point a store at `path`. Doesn't touch the filesystem until
    /// `save`/`load` is called.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::conformance::assert_checkpoint_store_contract;

    #[test]
    fn file_checkpoint_store_conforms_to_checkpoint_store_contract() {
        let path =
            std::env::temp_dir().join(format!("tracers-conformance-{}.json", uuid::Uuid::new_v4()));
        let store = FileCheckpointStore::new(&path);
        assert_checkpoint_store_contract(&store);
        std::fs::remove_file(&path).ok();
    }
}
