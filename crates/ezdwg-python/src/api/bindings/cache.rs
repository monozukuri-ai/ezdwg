// ============================================================================
// Shared path-keyed decode cache (Phase 1).
//
// One FileKey scheme for every process-global table cache. Today only the
// LAYER table uses it (see layer.rs); future tables (layer-states, styles,
// …) should store slots on the same key instead of inventing a new static.
//
// clear_decode_cache() is the Rust half of ezdwg.clear_decode_caches() /
// raw.clear_decode_cache() so long-running batch jobs can drop both the
// Python @lru_cache helpers and the native FIFO entries.
// ============================================================================

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Shared capacity for path-keyed FIFO caches that live behind FileKey.
///
/// FIXME: fixed-capacity, oldest-evicted -- fine for "one Document, a
/// handful of files" in one process; not a real LRU, not tuned for heavier
/// use. Raise or replace with a proper LRU when more tables share this.
const DECODE_CACHE_CAPACITY: usize = 8;

/// Identity of a DWG file for cache lookup.
///
/// When `modified` is `None` (mtime unavailable) the key still compares by
/// path+size, but callers should treat a miss as permanent for that process
/// if they cannot re-stat -- the layer cache never inserts a hit-able entry
/// in that case either (see file_key).
#[derive(Clone, PartialEq, Eq, Hash)]
struct FileKey {
    path: String,
    size: u64,
    modified: Option<Duration>,
}

fn file_key(path: &str) -> FileKey {
    let meta = std::fs::metadata(path).ok();
    FileKey {
        path: path.to_string(),
        size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        modified: meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()),
    }
}

/// FIFO (path, size, mtime) → Arc<T> store. Used by layer.rs today; other
/// table caches should reuse the same type rather than copy-pasting a static.
struct PathFifoCache<T> {
    entries: Mutex<Vec<(FileKey, Arc<T>)>>,
}

impl<T> PathFifoCache<T> {
    fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::with_capacity(DECODE_CACHE_CAPACITY)),
        }
    }

    fn get(&self, key: &FileKey) -> Option<Arc<T>> {
        let cache = self.entries.lock().unwrap();
        cache
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    fn insert(&self, key: FileKey, value: Arc<T>) {
        let mut cache = self.entries.lock().unwrap();
        if cache.iter().any(|(k, _)| k == &key) {
            return;
        }
        if cache.len() >= DECODE_CACHE_CAPACITY {
            cache.remove(0);
        }
        cache.push((key, value));
    }

    fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

/// Drop every native path-keyed decode cache entry.
///
/// Called from Python `clear_decode_caches()` so batch converters release
/// both the pure-Python `@lru_cache` maps and the Rust LAYER (and future
/// table) FIFO. Layer-specific clear lives in layer.rs and is invoked here
/// so this function stays the single public entry point.
#[pyfunction]
fn clear_decode_cache() -> PyResult<()> {
    clear_layer_records_cache();
    Ok(())
}
