#![no_main]
//! Feed the parsers arbitrary bytes and prove they return an error rather
//! than panicking.
//!
//! A `TaskRegistry` checkpoint is read back after a crash — which is to say,
//! at exactly the moment the file is most likely to be truncated or corrupt.
//! "Errors are `TraceErr` variants, never panics" (CLAUDE.md) has to hold on
//! the deserialization path too, or crash recovery becomes a second crash.

use libfuzzer_sys::fuzz_target;
use trace_lang_core::{Trace, TraceErr};
use trace_lang_task::{CheckpointStore, Task, TaskRegistry};

/// A store that hands back whatever bytes the fuzzer produced.
struct HostileStore {
    blob: String,
}

impl CheckpointStore for HostileStore {
    fn load(&self) -> Result<String, TraceErr> {
        Ok(self.blob.clone())
    }

    fn save(&self, _data: &str) -> Result<(), TraceErr> {
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    // Whatever this is, loading it must produce a Result — never a panic,
    // and never a partially-populated registry.
    let store = HostileStore {
        blob: text.to_string(),
    };
    if let Ok(registry) = TaskRegistry::load(&store) {
        // If it parsed, every query over it must also hold up.
        let _ = registry.ready_tasks();
        let _ = registry.all_by_priority();
        let _ = registry.paused();
        let _ = registry.total();

        // And it must survive a re-save/re-load: a checkpoint that parses
        // once but not twice is worse than one that never parsed.
        let json = serde_json::to_string(&registry).expect("a parsed registry re-serializes");
        let again: TaskRegistry =
            serde_json::from_str(&json).expect("and parses back the second time");
        assert_eq!(again.total(), registry.total());
    }

    // The same for the two types a checkpoint is built out of.
    let _ = serde_json::from_str::<Task>(text);
    let _ = serde_json::from_str::<Trace<String>>(text);
});
