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
/// 编译时打开了 `onnx-rerank`、随包资源目录旁边又有 [`RERANK_MODEL_DIRECTORY`] 时用那个模型（ONNX Runtime 后端），读不了就退回随包的句子模型。`bundle` 是随包资源目录（`EngineOptions::helpcode_fallback_resources`）：`resources` 换成下载的词库代次时，ONNX 模型仍在安装包里，按它去找；为 `None` 时 `resources` 本身就是随包的那个。
///
/// 开关打开时写一行诊断，说挂上的是哪一个（见 [`report_keyboard_reranker`]）。
pub(crate) fn keyboard_reranker(
    enabled: bool,
    resources: &str,
    bundle: Option<&str>,
    configured: Option<&str>,
) -> Option<Box<dyn msime_input_runtime::CandidateScorer>> {
    #[cfg(feature = "onnx-rerank")]
    let root = rerank_models_root(resources, bundle);
    if !enabled {
        #[cfg(feature = "onnx-rerank")]
        if let Some(root) = root.as_deref() {
            onnx_rerankers().release(root);
        }
        return None;
    }
    #[cfg(feature = "onnx-rerank")]
    if let Some(attached) = root.as_deref().and_then(onnx_reranker) {
        report_keyboard_reranker(attached.category, &attached.detail);
        return Some(Box::new(SharedOnnxScorer(attached.scorer)));
    }
    #[cfg(not(feature = "onnx-rerank"))]
    let _ = bundle;
    let scorer = sentence_model(resources, configured).map(|model| {
        Box::new(Reranker::new(model)) as Box<dyn msime_input_runtime::CandidateScorer>
    });
    let path = sentence_model_path(resources, configured);
    match &scorer {
        Some(_) => report_keyboard_reranker(
            "keyboard reranker bundled model",
            &path.display().to_string(),
        ),
        None => report_keyboard_reranker("keyboard reranker missing", &path.display().to_string()),
    }
    scorer
}

/// 「增强」开着时逐键重排挂的是哪一个，写一行诊断。类别本身就说明了档位（`keyboard reranker onnx large`、`keyboard reranker onnx small`、可用内存不够加载大的时的 `keyboard reranker onnx small (low memory)`、`keyboard reranker bundled model`、`keyboard reranker missing`），只留类别的宿主（macOS、iOS）也看得出来；细节是模型目录、大模型的自测毫秒数、可用内存，以及是不是与之前的会话共用（`(shared)`）。只在会话建立和开关打开时写，不在按键路径上写。
fn report_keyboard_reranker(category: &'static str, detail: &str) {
    crate::diagnostics::report(category, detail);
}

/// 放 ONNX 重排模型的目录：随包资源目录的上一级，与 `settled-model/`、`language-dictionaries/` 一样是资源目录的兄弟。不放进资源目录本身，因为那里必须与词库锁逐项相符（`ResourceStore::verify` 只放过 `helpcodes/`），多一个目录整套资源就校验失败。
#[cfg(feature = "onnx-rerank")]
pub(crate) fn rerank_models_root(resources: &str, bundle: Option<&str>) -> Option<PathBuf> {
    let bundled = bundle.filter(|path| !path.is_empty()).unwrap_or(resources);
    Path::new(bundled).parent().map(Path::to_path_buf)
}

/// 资源目录旁边放 ONNX Runtime 重排模型的子目录：chinese-ime-lm 的 `neural/choice/export_step.py` 导出的 `lm_step.ort`、`model.json` 和 `vocab.json`。这是大的那个（8x448，ctx-v1 有上下文 91.9%、无上下文 82.7%）。
#[cfg(feature = "onnx-rerank")]
pub(crate) const RERANK_MODEL_DIRECTORY: &str = "rerank-model";

/// 小的那个（6x256，跳过全字表 softmax，ctx-v1 有上下文 90.4%、无上下文 83.0%），给大模型在按键预算里跑不动的机器。
#[cfg(feature = "onnx-rerank")]
pub(crate) const SMALL_RERANK_MODEL_DIRECTORY: &str = "rerank-model-small";

/// 大模型打一页重负载（[`msime_onnx_rerank::OnnxScorer::calibrate`]）超过这么多毫秒就换小模型。2026-11-01 在真机上量的自测值：iPhone 17 是 3.8 到 4.8 ms，Mac 是 4 到 7 ms（机器很忙时），iPhone XR 是 21.9 到 23.4 ms，Pixel 3 XL（四线程）是 30.4 ms。前两台用大模型时整键 p95 在 4 ms 上下；后两台用大模型是 13 到 27 ms，用小模型是 4.4 到 4.9 和 9.3 ms。
#[cfg(feature = "onnx-rerank")]
pub(crate) const LARGE_RERANK_MODEL_BUDGET_MS: f64 = 10.0;

/// 进程可用内存（iOS 的 `os_proc_available_memory`）少于这么多时不加载大模型，直接用小的。
///
/// iOS 键盘扩展有硬上限（2026-11-02 的 iPhone 17 上是 176 MB），超过就被系统结束。2026-11-02 在 iOS 模拟器里用 `msime-onnx-rerank` 的 `footprint` 示例量 phys_footprint（比加载前多出的部分，按 iOS 的设置：内存池关着，ONNX Runtime 自己读文件）：8x448 加载时峰值 74 MiB，加载后 40 MiB，打字时 46 到 47 MiB；6x256 峰值 17 MiB，打字时 11 到 12 MiB。门槛是大模型的加载峰值再留 46 MiB：键盘界面在会话建立之后才画出来，iPhone 17 上那一段是 20 MB 左右，打字时词库页面和候选缓存还会涨。
#[cfg(feature = "onnx-rerank")]
pub(crate) const LARGE_RERANK_MODEL_MIN_AVAILABLE: u64 = 120 << 20;

/// 一次图运行用几个线程。安卓手机多是四个大核加四个小核：Pixel 3 XL 上用 `rerank_latency` 在真实运行时里量，手机模型每键整键 p95 两线程 10.8 ms、三线程 10.0 ms、四线程 9.3 ms，所以安卓用四线程。苹果的 A 系列芯片只有两个大核，四线程在 iPhone XR 和 iPhone 17 上都比两线程慢；桌面两线程已在预算之内，不多占核。
#[cfg(all(feature = "onnx-rerank", target_os = "android"))]
const RERANK_MODEL_THREADS: usize = 4;
#[cfg(all(feature = "onnx-rerank", not(target_os = "android")))]
const RERANK_MODEL_THREADS: usize = 2;

/// ONNX Runtime 的 CPU 内存池只在 iOS 上关掉：键盘扩展有硬上限，内存池留着的那几十 MB 在那里是被系统结束和不被结束的差别；其他平台没有这样的上限，留着内存池省下每次运行重新要内存的时间。数字见 `msime_onnx_rerank::OnnxScorer::load`。
#[cfg(all(feature = "onnx-rerank", target_os = "ios"))]
const RERANK_MEMORY_ARENA: bool = false;
#[cfg(all(feature = "onnx-rerank", not(target_os = "ios")))]
const RERANK_MEMORY_ARENA: bool = true;

/// 进程此刻还能用多少内存，只有 iOS 说得出（`os_proc_available_memory`，模拟器上是 0，当作不知道）；不知道时不按内存挑档位。
#[cfg(all(feature = "onnx-rerank", target_os = "ios"))]
fn available_memory() -> Option<u64> {
    extern "C" {
        fn os_proc_available_memory() -> usize;
    }
    // SAFETY: 不带参数、只读进程的内存额度，iOS 13 起都有。
    let available = unsafe { os_proc_available_memory() };
    (available > 0).then_some(available as u64)
}

#[cfg(all(feature = "onnx-rerank", not(target_os = "ios")))]
fn available_memory() -> Option<u64> {
    None
}

/// 挂上的 ONNX 重排模型，连同诊断行的类别与细节。`scorer` 是进程里共用的那一份，各会话拿它的引用。
#[cfg(feature = "onnx-rerank")]
pub(crate) struct AttachedOnnxReranker {
    pub(crate) scorer: Arc<Mutex<msime_onnx_rerank::OnnxScorer>>,
    pub(crate) category: &'static str,
    pub(crate) detail: String,
}

/// 模型文件 `lm_step.ort` 的长度与修改时间，用来认出缓存里的重排器和自测结果是不是还对应磁盘上的文件；文件不在时是 `None`。
#[cfg(feature = "onnx-rerank")]
type ModelStamp = Option<(u64, std::time::SystemTime)>;

#[cfg(feature = "onnx-rerank")]
fn model_stamp(directory: &Path) -> ModelStamp {
    let metadata = std::fs::metadata(directory.join("lm_step.ort")).ok()?;
    Some((metadata.len(), metadata.modified().ok()?))
}

/// 已加载的那一个：模型目录（大的或小的）、定档位时大小两个模型的文件戳（任一个变了就重新定）、重排器本身和它的诊断类别。
#[cfg(feature = "onnx-rerank")]
struct LoadedOnnxReranker {
    directory: PathBuf,
    stamps: [ModelStamp; 2],
    scorer: Arc<Mutex<msime_onnx_rerank::OnnxScorer>>,
    category: &'static str,
    detail: String,
}

/// 进程里的 ONNX 重排器：同一时刻只加载一个模型，各会话共用；大模型的自测在进程里只做一次。
///
/// iOS 每弹出一次键盘就建一个新会话。原来每个会话各自加载、自测一份大模型，2026-11-02 在 iPhone 17 上键盘扩展每弹出一次占用就涨二十多 MB，连续弹出五六次就碰到 176 MB 的上限被系统结束。重排器的缓存按上文分开，换会话、换上文时自己从头算，所以共用不影响打分。
///
/// 档位在进程里第一次挂上时定下（自测速度，加上 iOS 上的可用内存，见 [`OnnxRerankers::attach`]），之后的会话沿用，不因为某一刻的内存多少在大小模型之间来回换；「增强」关掉（[`OnnxRerankers::release`]）或模型文件换了以后再定一次，自测结果留着不重测。内存警告时不放开：挂着它的会话还在用，放开也省不下内存，下一个会话还要再加载一遍。
#[cfg(feature = "onnx-rerank")]
#[derive(Default)]
pub(crate) struct OnnxRerankers {
    loaded: Option<LoadedOnnxReranker>,
    /// 大模型的自测结果：模型根目录、当时大模型的文件戳和毫秒数。
    calibrated: Option<(PathBuf, ModelStamp, f64)>,
}

#[cfg(feature = "onnx-rerank")]
fn onnx_rerankers() -> std::sync::MutexGuard<'static, OnnxRerankers> {
    static RERANKERS: OnceLock<Mutex<OnnxRerankers>> = OnceLock::new();
    RERANKERS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 装了两个 ONNX 模型时按自测速度和可用内存在两者之间选，只装了一个就用那一个，见 [`OnnxRerankers::attach`]。`root` 是放两个模型目录的地方（[`rerank_models_root`]）。
#[cfg(feature = "onnx-rerank")]
fn onnx_reranker(root: &Path) -> Option<AttachedOnnxReranker> {
    onnx_rerankers().attach(root, LARGE_RERANK_MODEL_BUDGET_MS, available_memory())
}

/// 共用重排器在一个会话里的那一份：每次打分时加锁。同一时刻通常只有一个会话在打字，锁几乎不会等。
#[cfg(feature = "onnx-rerank")]
struct SharedOnnxScorer(Arc<Mutex<msime_onnx_rerank::OnnxScorer>>);

#[cfg(feature = "onnx-rerank")]
impl msime_input_runtime::CandidateScorer for SharedOnnxScorer {
    fn context_length(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .context_length()
    }

    fn scores_prediction_pool_within_keystroke_budget(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .scores_prediction_pool_within_keystroke_budget()
    }

    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .log_probabilities(context, candidates)
    }

    fn static_evidence_weight(&self) -> f64 {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .static_evidence_weight()
    }
}

#[cfg(feature = "onnx-rerank")]
impl OnnxRerankers {
    /// 给一个新会话挂重排器。已经为 `root` 加载过、两个模型文件都没换时直接共用（细节标 `(shared)`）；否则定档位并加载：
    ///
    /// - 只装了一个模型就用那一个；大的读不了就用小的。
    /// - 两个都装了：`available`（字节）少于 [`LARGE_RERANK_MODEL_MIN_AVAILABLE`] 时直接用小的，不先加载大的去自测；否则加载大的、量它打一页重负载要多久（进程里只量一次），超过 `budget_ms` 就换小的。按量出来的速度选，不按机型名单选。
    ///
    /// 换下来的那个只从这里放开，还挂着它的会话照常用到结束，之后内存还给系统。
    pub(crate) fn attach(
        &mut self,
        root: &Path,
        budget_ms: f64,
        available: Option<u64>,
    ) -> Option<AttachedOnnxReranker> {
        const LARGE: &str = "keyboard reranker onnx large";
        const SMALL: &str = "keyboard reranker onnx small";
        const SMALL_LOW_MEMORY: &str = "keyboard reranker onnx small (low memory)";
        let large = root.join(RERANK_MODEL_DIRECTORY);
        let small = root.join(SMALL_RERANK_MODEL_DIRECTORY);
        let (large_stamp, small_stamp) = (model_stamp(&large), model_stamp(&small));
        let stamps = [large_stamp, small_stamp];
        if let Some(loaded) = self
            .loaded
            .as_ref()
            .filter(|loaded| loaded.directory.parent() == Some(root) && loaded.stamps == stamps)
        {
            return Some(AttachedOnnxReranker {
                scorer: Arc::clone(&loaded.scorer),
                category: loaded.category,
                detail: format!("{} (shared)", loaded.detail),
            });
        }
        if large_stamp.is_some() && small_stamp.is_some() {
            if let Some(available) =
                available.filter(|&bytes| bytes < LARGE_RERANK_MODEL_MIN_AVAILABLE)
            {
                let note = format!(
                    " (available {} MiB, large needs {} MiB)",
                    available >> 20,
                    LARGE_RERANK_MODEL_MIN_AVAILABLE >> 20
                );
                return self.load(&small, stamps, SMALL_LOW_MEMORY, note);
            }
            let known = self
                .calibrated
                .as_ref()
                .filter(|(calibrated, stamp, _)| calibrated == root && *stamp == large_stamp)
                .map(|&(_, _, measured)| measured);
            let note =
                |measured: f64| format!(" (calibrated {measured:.1} ms, budget {budget_ms:.1} ms)");
            if let Some(measured) = known {
                let note = note(measured);
                if measured > budget_ms {
                    if let Some(attached) = self.load(&small, stamps, SMALL, note.clone()) {
                        return Some(attached);
                    }
                }
                // 新加载的这一份带上之前量出的结论：过了预算的同时也排上屏后联想。
                return self
                    .load_calibrated(&large, stamps, LARGE, note.clone(), (measured, budget_ms))
                    .or_else(|| self.load(&small, stamps, SMALL, note));
            }
            self.loaded = None;
            let Some(mut scorer) = load_onnx_reranker(&large) else {
                return self.load(&small, stamps, SMALL, String::new());
            };
            // 过了预算的大模型同时也排上屏后联想；退到小模型的机器不排，见 `OnnxScorer::fits_keystroke_budget`。
            let measured = scorer.calibrate_for_keystroke_budget(budget_ms);
            self.calibrated = Some((root.to_path_buf(), large_stamp, measured));
            if measured > budget_ms {
                if let Some(attached) = self.load(&small, stamps, SMALL, note(measured)) {
                    return Some(attached);
                }
            }
            return Some(self.keep(&large, stamps, scorer, LARGE, note(measured)));
        }
        self.load(&large, stamps, LARGE, String::new())
            .or_else(|| self.load(&small, stamps, SMALL, String::new()))
    }

    /// 「增强」关掉时放开 `root` 下的重排器：还挂着它的会话照常用到结束，之后模型的内存就还给系统。自测结果留着，再打开时不重测。
    pub(crate) fn release(&mut self, root: &Path) {
        if self
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.directory.parent() == Some(root))
        {
            self.loaded = None;
        }
    }

    fn load(
        &mut self,
        directory: &Path,
        stamps: [ModelStamp; 2],
        category: &'static str,
        note: String,
    ) -> Option<AttachedOnnxReranker> {
        // 没有这个模型就什么都不动，已加载的那个留着。
        model_stamp(directory)?;
        // 先放开换下来的那个，加载新模型的那一刻不必两个都由这里占着。
        self.loaded = None;
        let scorer = load_onnx_reranker(directory)?;
        Some(self.keep(directory, stamps, scorer, category, note))
    }

    /// 同 [`Self::load`]，再把这个进程里之前对同一个模型文件量出的 `(毫秒数, 预算)` 交给它（`OnnxScorer::accept_calibration`），不重测。
    fn load_calibrated(
        &mut self,
        directory: &Path,
        stamps: [ModelStamp; 2],
        category: &'static str,
        note: String,
        (measured_ms, budget_ms): (f64, f64),
    ) -> Option<AttachedOnnxReranker> {
        model_stamp(directory)?;
        self.loaded = None;
        let mut scorer = load_onnx_reranker(directory)?;
        scorer.accept_calibration(measured_ms, budget_ms);
        Some(self.keep(directory, stamps, scorer, category, note))
    }

    fn keep(
        &mut self,
        directory: &Path,
        stamps: [ModelStamp; 2],
        scorer: msime_onnx_rerank::OnnxScorer,
        category: &'static str,
        note: String,
    ) -> AttachedOnnxReranker {
        let scorer = Arc::new(Mutex::new(scorer));
        let detail = format!("{}{note}", directory.display());
        self.loaded = Some(LoadedOnnxReranker {
            directory: directory.to_path_buf(),
            stamps,
            scorer: Arc::clone(&scorer),
            category,
            detail: detail.clone(),
        });
        AttachedOnnxReranker {
            scorer,
            category,
            detail,
        }
    }
}

#[cfg(feature = "onnx-rerank")]
fn load_onnx_reranker(directory: &Path) -> Option<msime_onnx_rerank::OnnxScorer> {
    let info = std::fs::read_to_string(directory.join("model.json")).ok()?;
    let vocabulary = std::fs::read_to_string(directory.join("vocab.json")).ok()?;
    msime_onnx_rerank::OnnxScorer::load_file(
        &directory.join("lm_step.ort"),
        &info,
        &vocabulary,
        RERANK_MODEL_THREADS,
        RERANK_MEMORY_ARENA,
    )
    .ok()
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
