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

#[cfg(unix)]
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

/// 每个路径一个加载槽。全局锁只用来取槽，读文件和解析在锁外的槽里做：后台加载约 25 MB 的落定重排模型时，输入线程上新建会话取另一个模型不会被它卡住；同一路径的并发请求仍然只加载一次，后来的等先到的那次。
type SentenceModelSlot = Arc<OnceLock<Option<Arc<SentenceModel>>>>;

static SENTENCE_MODELS: OnceLock<Mutex<LruCache<PathBuf, SentenceModelSlot>>> = OnceLock::new();

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

fn sentence_model_cache() -> &'static Mutex<LruCache<PathBuf, SentenceModelSlot>> {
    SENTENCE_MODELS.get_or_init(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(SENTENCE_MODEL_CACHE_CAPACITY).unwrap(),
        ))
    })
}

pub(crate) fn sentence_model(
    resources: &str,
    configured: Option<&str>,
) -> Option<Arc<SentenceModel>> {
    let path = sentence_model_path(resources, configured);
    let cache = sentence_model_cache();
    // Keyed by path: two sessions may legitimately be pointed at different models, and a cache that
    // remembered only the first would silently serve one of them the other's weights.
    let slot = {
        let mut cache = cache.lock().ok()?;
        Arc::clone(cache.get_or_insert(path.clone(), || Arc::new(OnceLock::new())))
    };
    slot.get_or_init(|| {
        // 文件在却读不了（符号链接、FIFO、不可信的硬链接、超过上限、没有权限）也要说出来，否则在诊断日志里和「没装模型」分不开（#10341）。文件不存在仍是常态，不报告：键盘模型只有带它的版本才有，落定重排模型在调用前已经查过文件在不在。
        let unreadable = |error: std::io::Error| {
            if error.kind() != std::io::ErrorKind::NotFound {
                crate::diagnostics::report(
                    "sentence model unreadable",
                    &format!("{}: {error}", path.display()),
                );
            }
        };
        crate::bounded_file::open_private(&path)
            .and_then(|file| crate::bounded_file::read(file, MAX_SENTENCE_MODEL_BYTES))
            .map_err(unreadable)
            .ok()
            .and_then(|bytes| match SentenceModel::load(&bytes) {
                Ok(model) => Some(Arc::new(model)),
                Err(error) => {
                    // A corrupt or mismatched model is worth saying out loud: the input method keeps
                    // working without it, so nothing else would ever reveal that it is not running.
                    crate::diagnostics::report(
                        "sentence model ignored",
                        &format!("{}: {error}", path.display()),
                    );
                    None
                }
            })
    })
    .clone()
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

    #[test]
    fn a_model_still_loading_does_not_block_other_paths() {
        let directory = tempfile::tempdir().unwrap();
        let loading = directory.path().join("loading.safetensors");
        let other = directory.path().join("other.safetensors");
        std::fs::write(&other, b"not a model").unwrap();
        // 占住 `loading` 的槽并停在加载中，模拟后台线程正在读约 25 MB 的落定重排模型。
        let slot = Arc::clone(
            sentence_model_cache()
                .lock()
                .unwrap()
                .get_or_insert(loading.clone(), || Arc::new(OnceLock::new())),
        );
        let (started, wait_started) = std::sync::mpsc::channel();
        let (release, wait_release) = std::sync::mpsc::channel::<()>();
        let loader = std::thread::spawn(move || {
            slot.get_or_init(|| {
                started.send(()).unwrap();
                wait_release.recv().unwrap();
                None
            })
            .clone()
        });
        wait_started.recv().unwrap();

        // 加载在锁外进行：别的路径照常取用（这里是一份坏文件，得到 None），不等那次加载。
        assert!(sentence_model("", other.to_str()).is_none());

        release.send(()).unwrap();
        assert!(loader.join().unwrap().is_none());
    }
}

pub(crate) fn sentence_model_path(resources: &str, configured: Option<&str>) -> PathBuf {
    configured
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(resources).join(SENTENCE_MODEL_FILE))
}

/// The per-keystroke reranker, only while `enabled`: the keyboard model switch (`neural_keyboard`, 整句联想「增强」) is on and the edition ships the model (`keyboard_reranker_enabled`). It is the only layer that switch mounts; the Engine's own keyboard-model rerank stays off.
///
/// The switch is opt-in because this model runs inside every `character` call, on the thread the host types on: one decision costs a few milliseconds, and the first one after a commit, which has to read the new context from scratch, about 20 ms on a desktop CPU and five to ten times that on a phone. Attaching it whenever the file was present ran it for every touch user on 标准 and 关闭 too, which is what made fast typing on Android fall behind and drop keys. The model is not loaded at all while the switch is off.
///
/// 编译时打开了 `onnx-rerank`、资源目录里又有 [`RERANK_MODEL_DIRECTORY`] 时用那个模型（ONNX Runtime 后端），读不了就退回随包的句子模型。
pub(crate) fn keyboard_reranker(
    enabled: bool,
    resources: &str,
    configured: Option<&str>,
) -> Option<Box<dyn msime_input_runtime::CandidateScorer>> {
    if !enabled {
        return None;
    }
    #[cfg(feature = "onnx-rerank")]
    if let Some(scorer) = onnx_reranker(Path::new(resources)) {
        return Some(scorer);
    }
    sentence_model(resources, configured).map(|model| {
        Box::new(Reranker::new(model)) as Box<dyn msime_input_runtime::CandidateScorer>
    })
}

/// 资源目录里放 ONNX Runtime 重排模型的子目录：chinese-ime-lm 的 `neural/choice/export_step.py` 导出的 `lm_step.ort`、`model.json` 和 `vocab.json`。
#[cfg(feature = "onnx-rerank")]
pub(crate) const RERANK_MODEL_DIRECTORY: &str = "rerank-model";

/// 一次图运行用几个线程。苹果的 A 系列芯片只有两个大核，四线程在 iPhone XR 和 iPhone 17 上都比两线程慢；Pixel 3 XL 上四线程略快，但两线程已在按键预算之内。
#[cfg(feature = "onnx-rerank")]
const RERANK_MODEL_THREADS: usize = 2;

#[cfg(feature = "onnx-rerank")]
fn onnx_reranker(resources: &Path) -> Option<Box<dyn msime_input_runtime::CandidateScorer>> {
    let directory = resources.join(RERANK_MODEL_DIRECTORY);
    let model = std::fs::read(directory.join("lm_step.ort")).ok()?;
    let info = std::fs::read_to_string(directory.join("model.json")).ok()?;
    let vocabulary = std::fs::read_to_string(directory.join("vocab.json")).ok()?;
    msime_onnx_rerank::OnnxScorer::load(&model, &info, &vocabulary, RERANK_MODEL_THREADS)
        .ok()
        .map(|scorer| Box::new(scorer) as Box<dyn msime_input_runtime::CandidateScorer>)
}

pub mod account;
pub mod ai_skin;
pub mod android_data;
pub mod candidates;
pub mod chat_stream;
pub mod cloud_dictionary;
pub mod cloud_sync;
pub mod community;
pub mod credentials;
pub mod dictionary_sync;
pub mod google;
#[cfg(not(target_os = "android"))]
pub mod handwriting;
pub mod harmony_settings;
pub mod host;
pub mod input;
pub mod ios_settings;
pub mod lifecycle;
pub mod mcp;
pub mod moderation;
pub mod pack_downloads;
pub mod plugins;
pub mod providers;
pub mod reporting;
pub mod session;
pub mod session_store;
pub mod settled;
pub mod skin_image;
pub mod spacing;
pub mod translation;
pub mod user_words;
pub mod voice;
#[cfg(windows)]
pub mod windows_handwriting;
#[cfg(windows)]
pub mod windows_voice;

#[cfg(test)]
mod preference_schema_tests;

// Every export has always been reachable at the crate root; the domain split
// below is for readers, not for callers, so each module is flattened back out.
pub use android_data::*;
pub use candidates::*;
pub use credentials::*;
pub use dictionary_sync::*;
pub use host::*;
pub use input::*;
pub use lifecycle::*;
pub use mcp::*;
pub use pack_downloads::*;
pub use plugins::*;
pub use providers::*;
pub use session::*;
pub use settled::*;
pub use skin_image::*;
pub use translation::*;
pub use user_words::*;
pub use voice::*;
#[cfg(windows)]
pub use windows_handwriting::*;
#[cfg(windows)]
pub use windows_voice::*;
