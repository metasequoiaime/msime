//! The C ABI other IME hosts link against.
//!
//! Every function here is a thin shim: it validates the pointers and lengths it
//! was handed, calls into the crate's own logic, and hands back an owned JSON
//! string the caller frees with `msime_client_string_free`. Keeping them in one
//! file separates "what the library does" from "how a foreign caller reaches
//! it", which is the boundary the header in include/ describes.
//!
//! `#[no_mangle]` exports the symbol regardless of which module it sits in, so
//! moving these out of the crate root does not change the ABI.

use crate::*;
use lru::LruCache;
use std::num::NonZeroUsize;

pub(crate) fn serialized_runtime_view(session: &HostSession) -> Result<Value, String> {
    serde_json::to_value(session.runtime.view()).map_err(|error| error.to_string())
}

pub(crate) unsafe fn with_bounded_bytes<T>(
    pointer: *const u8,
    length: usize,
    maximum: usize,
    invalid: &'static str,
    operation: impl FnOnce(&[u8]) -> Result<T, String>,
) -> Result<T, String> {
    if pointer.is_null() || length > maximum {
        return Err(invalid.into());
    }
    operation(unsafe { std::slice::from_raw_parts(pointer, length) })
}

pub(crate) fn absolute_path(value: &str) -> bool {
    Path::new(value).is_absolute()
}

pub(crate) fn parse_absolute_socket_path(bytes: &[u8]) -> Result<&str, String> {
    parse_absolute_path(
        bytes,
        "socket path is not UTF-8",
        "socket path must be absolute",
    )
}

pub(crate) fn parse_absolute_path<'a>(
    bytes: &'a [u8],
    invalid_utf8: &'static str,
    non_absolute: &'static str,
) -> Result<&'a str, String> {
    let path = std::str::from_utf8(bytes).map_err(|_| invalid_utf8.to_owned())?;
    absolute_path(path)
        .then_some(path)
        .ok_or_else(|| non_absolute.to_owned())
}

// Shared by the session and host modules below, so it lives in the parent.
/// The file name the reranking model is published under inside the resource set.
pub(crate) const SENTENCE_MODEL_FILE: &str = "sentence-model.safetensors";

/// The larger model, published beside the first, run only once typing settles.
///
/// A separate file rather than a preset flag inside one, because the two are wanted at once: the
/// small one on every keystroke and this one when the user pauses. Installations that ship only
/// the small one keep today's behaviour exactly.
pub(crate) const SETTLED_MODEL_FILE: &str = "sentence-model-desktop.safetensors";

/// Largest model file a host will read into memory. The shipped settled model is about 25 MiB;
/// this leaves room for a larger compatible model without allowing an arbitrary configured path to
/// make startup allocate unbounded memory.
pub(crate) const MAX_SENTENCE_MODEL_BYTES: u64 = 64 * 1024 * 1024;
const SENTENCE_MODEL_CACHE_CAPACITY: usize = 8;

static SENTENCE_MODELS: OnceLock<Mutex<LruCache<PathBuf, Option<Arc<SentenceModel>>>>> =
    OnceLock::new();

/// The candidate reranking model, loaded once per path and shared by every session using it.
///
/// The small model is part of the verified resource set. Prepared dictionaries contain only the
/// Engine's writable dictionary files, so the default must resolve from `resources` instead.
/// Shares `sentence_model`'s cache by going through it, so two sessions on the same resources load
/// the twenty five megabytes once between them rather than once each.
pub(crate) fn sentence_model_settled(
    resources: &str,
    configured: Option<&str>,
) -> Option<Arc<SentenceModel>> {
    let path = match configured {
        Some(path) => PathBuf::from(path),
        None => Path::new(resources).join(SETTLED_MODEL_FILE),
    };
    // Absence is the normal case — most installations ship one model — so it is checked rather
    // than reported. A host that named a path and got nothing gets the same silence: a second
    // model is not worth failing a session over.
    if !path.is_file() {
        return None;
    }
    sentence_model(resources, path.to_str())
}

pub(crate) fn sentence_model(
    resources: &str,
    configured: Option<&str>,
) -> Option<Arc<SentenceModel>> {
    let path = sentence_model_path(resources, configured);
    let cache = SENTENCE_MODELS.get_or_init(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(SENTENCE_MODEL_CACHE_CAPACITY).unwrap(),
        ))
    });
    let mut cache = cache.lock().ok()?;
    // Keyed by path: two sessions may legitimately be pointed at different models, and a cache that
    // remembered only the first would silently serve one of them the other's weights.
    if let Some(cached) = cache.get(&path) {
        return cached.clone();
    }
    let loaded = std::fs::File::open(&path)
        .ok()
        .and_then(|file| crate::bounded_file::read(file, MAX_SENTENCE_MODEL_BYTES).ok())
        .and_then(|bytes| match SentenceModel::load(&bytes) {
            Ok(model) => Some(Arc::new(model)),
            Err(error) => {
                // A corrupt or mismatched model is worth saying out loud: the input method keeps
                // working without it, so nothing else would ever reveal that it is not running.
                eprintln!("msime: ignoring {}: {error}", path.display());
                None
            }
        });
    cache.put(path, loaded.clone());
    loaded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sentence_model_cache_is_bounded_across_paths() {
        let mut directories = Vec::new();
        for index in 0..=SENTENCE_MODEL_CACHE_CAPACITY {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(format!("model-{index}.safetensors"));
            std::fs::write(&path, b"not a model").unwrap();
            assert!(sentence_model("", path.to_str()).is_none());
            directories.push(directory);
        }
        let cache = SENTENCE_MODELS.get().unwrap().lock().unwrap();
        assert!(cache.len() <= SENTENCE_MODEL_CACHE_CAPACITY);
    }
}

pub(crate) fn sentence_model_path(resources: &str, configured: Option<&str>) -> PathBuf {
    configured
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(resources).join(SENTENCE_MODEL_FILE))
}

pub mod candidates;
pub mod host;
pub mod input;
pub mod lifecycle;
pub mod mcp;
pub mod moderation;
pub mod plugins;
pub mod providers;
pub mod reporting;
pub mod session;
pub mod translation;
pub mod voice;

// Every export has always been reachable at the crate root; the domain split
// below is for readers, not for callers, so each module is flattened back out.
pub use candidates::*;
pub use host::*;
pub use input::*;
pub use lifecycle::*;
pub use mcp::*;
pub use plugins::*;
pub use providers::*;
pub use session::*;
pub use translation::*;
pub use voice::*;
