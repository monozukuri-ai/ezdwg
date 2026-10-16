// ============================================================================
// Shared path-keyed decode cache (Phase 1).
//
// One FileKey scheme for every process-global table cache. The LAYER table
// uses it directly (see layer.rs); every other full-file pass that is asked
// for more than once per read stores a slot in FileDecodeState below instead
// of inventing a new static.
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
/// A missing timestamp or failed stat disables caching for that read.
#[derive(Clone, PartialEq, Eq, Hash)]
struct FileKey {
    path: std::path::PathBuf,
    size: u64,
    modified: Option<Duration>,
}

fn file_key(path: &str) -> Option<FileKey> {
    let canonical = std::fs::canonicalize(path).ok()?;
    let meta = std::fs::metadata(&canonical).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(FileKey {
        path: canonical,
        size: meta.len(),
        modified: Some(modified),
    })
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
        cache.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
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
    clear_file_decode_state_cache();
    Ok(())
}

// --- Per-file decoded tables shared by the bindings -------------------------
//
// One FIFO entry per FileKey holds a slot for every full-file pass that more
// than one public function (or one read of a Document) asks for. Reading a
// 163k-object drawing used to walk the object map seven times for
// `list_object_headers_with_type` and twice for `decode_block_header_names`
// and `decode_entity_linetypes`; each now decodes once per (path, size,
// mtime). Slots fill lazily; a call with `limit` never fills a slot (a partial
// decode is not the table) but is served from one that is already full.

type DecodeSlot<T> = Mutex<Option<Arc<Vec<T>>>>;

/// Lazily filled decode results of one file. Add a slot here instead of a new
/// process-global static when another pass turns out to run repeatedly.
#[derive(Default)]
struct FileDecodeState {
    /// `list_object_headers_with_type`: handle/offset/size/type of every object.
    object_headers_with_type: DecodeSlot<ObjectHeaderWithTypeRow>,
    /// `decode_block_header_names`: BLOCK_HEADER handle -> block name.
    block_header_names: DecodeSlot<BlockHeaderNameRow>,
    /// `decode_entity_linetypes`: common entity linetype data of every entity.
    entity_linetypes: DecodeSlot<EntityLinetypeRow>,
}

fn file_decode_state_cache() -> &'static PathFifoCache<FileDecodeState> {
    static CACHE: OnceLock<PathFifoCache<FileDecodeState>> = OnceLock::new();
    CACHE.get_or_init(PathFifoCache::new)
}

/// Called from clear_decode_cache() above.
fn clear_file_decode_state_cache() {
    file_decode_state_cache().clear();
}

/// The shared decode state of `path`; `None` when the file cannot be stat'ed
/// (then nothing is cached for this read, as with the LAYER cache).
fn file_decode_state(path: &str) -> Option<Arc<FileDecodeState>> {
    let key = file_key(path)?;
    let cache = file_decode_state_cache();
    if let Some(state) = cache.get(&key) {
        return Some(state);
    }
    cache.insert(key.clone(), Arc::new(FileDecodeState::default()));
    // insert() keeps an entry another thread put in first; share that one.
    cache.get(&key)
}

/// Rows of one full-file pass, decoded once per (path, size, mtime).
///
/// `limit` is applied to cached rows on the way out; on a cold slot a limited
/// call runs `decode(limit)` and leaves the slot empty.
fn cached_rows<T: Clone, E>(
    slot: fn(&FileDecodeState) -> &DecodeSlot<T>,
    path: &str,
    limit: Option<usize>,
    decode: impl FnOnce(Option<usize>) -> Result<Vec<T>, E>,
) -> Result<Vec<T>, E> {
    let state = file_decode_state(path);
    if let Some(state) = state.as_ref() {
        let cached = slot(state).lock().unwrap().clone();
        if let Some(rows) = cached {
            return Ok(take_rows(&rows, limit));
        }
    }
    if limit.is_some() {
        return decode(limit);
    }
    let rows = Arc::new(decode(None)?);
    if let Some(state) = state {
        *slot(&state).lock().unwrap() = Some(rows.clone());
    }
    Ok(take_rows(&rows, None))
}

fn take_rows<T: Clone>(rows: &[T], limit: Option<usize>) -> Vec<T> {
    match limit {
        Some(limit) => rows.iter().take(limit).cloned().collect(),
        None => rows.to_vec(),
    }
}

#[cfg(test)]
mod file_decode_state_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn cached_rows_decodes_once_per_file_and_serves_limits_from_the_slot() {
        let path = std::env::temp_dir().join(format!(
            "ezdwg-file-decode-state-{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, b"not a dwg; only its identity matters here").unwrap();
        let path = path.to_string_lossy().into_owned();
        let calls = AtomicUsize::new(0);
        // A plain error type keeps PyErr (and the Python C API) out of the test binary.
        let decode = |limit: Option<usize>| -> Result<Vec<BlockHeaderNameRow>, ()> {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok((0..5u64)
                .take(limit.unwrap_or(usize::MAX))
                .map(|h| (h, format!("B{h}")))
                .collect())
        };
        let slot: fn(&FileDecodeState) -> &DecodeSlot<BlockHeaderNameRow> =
            |state| &state.block_header_names;

        // A limited call on a cold slot decodes with the limit and caches nothing.
        assert_eq!(cached_rows(slot, &path, Some(2), decode).unwrap().len(), 2);
        assert_eq!(cached_rows(slot, &path, Some(2), decode).unwrap().len(), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // A full call fills the slot; later full and limited calls are hits.
        assert_eq!(cached_rows(slot, &path, None, decode).unwrap().len(), 5);
        assert_eq!(cached_rows(slot, &path, None, decode).unwrap().len(), 5);
        assert_eq!(
            cached_rows(slot, &path, Some(3), decode).unwrap(),
            vec![(0, "B0".to_string()), (1, "B1".to_string()), (2, "B2".to_string())]
        );
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        // Clearing the decode cache forces a new decode.
        clear_file_decode_state_cache();
        assert_eq!(cached_rows(slot, &path, None, decode).unwrap().len(), 5);
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_rewritten_file_does_not_hit_the_old_slot() {
        let path = std::env::temp_dir().join(format!(
            "ezdwg-file-decode-state-rewrite-{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, b"first").unwrap();
        let path = path.to_string_lossy().into_owned();
        let decode_a = |_: Option<usize>| -> Result<Vec<BlockHeaderNameRow>, ()> {
            Ok(vec![(1, "A".to_string())])
        };
        let decode_b = |_: Option<usize>| -> Result<Vec<BlockHeaderNameRow>, ()> {
            Ok(vec![(2, "B".to_string())])
        };
        let slot: fn(&FileDecodeState) -> &DecodeSlot<BlockHeaderNameRow> =
            |state| &state.block_header_names;
        assert_eq!(cached_rows(slot, &path, None, decode_a).unwrap()[0].1, "A");
        // Different size => different FileKey, so the stale rows are not served.
        std::fs::write(&path, b"second, longer").unwrap();
        assert_eq!(cached_rows(slot, &path, None, decode_b).unwrap()[0].1, "B");
        std::fs::remove_file(&path).ok();
    }
}
