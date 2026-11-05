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

    #[cfg(feature = "onnx-rerank")]
    #[test]
    fn a_cached_pause_model_must_fit_each_new_hosts_limit() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../onnx-rerank/tests/fixtures");
        let bundle = tempfile::tempdir().unwrap();
        let directory = bundle.path().join(SMALL_RERANK_MODEL_DIRECTORY);
        std::fs::create_dir(&directory).unwrap();
        for name in ["lm_step.ort", "model.json", "vocab.json"] {
            std::fs::copy(fixtures.join(name), directory.join(name)).unwrap();
        }
        // 只加载仓库内的小型合成模型，不量机器速度；20 ms 是前一个停顿宿主留下的自测记录。
        let scorer = Arc::new(Mutex::new(load_onnx_reranker(&directory).unwrap()));
        let mut cache = OnnxRerankers {
            loaded: Some(LoadedOnnxReranker {
                directory: directory.clone(),
                stamps: [None, model_stamp(&directory)],
                scorer: scorer.clone(),
                category: "keyboard reranker onnx small",
                detail: "synthetic calibration (page 20.0 ms, limit 60.0 ms)".into(),
                page_ms: 20.0,
            }),
            ..OnnxRerankers::default()
        };
        let pause = cache
            .attach(bundle.path(), 15.0, PAUSE_RERANK_LIMIT_MS, None)
            .attached()
            .unwrap();
        assert!(Arc::ptr_eq(&pause.scorer, &scorer));
        match cache.attach(bundle.path(), 15.0, KEYSTROKE_RERANK_LIMIT_MS, None) {
            OnnxAttach::OverLimit(detail) => {
                assert!(detail.contains("page 20.0 ms"), "{detail}");
                assert!(detail.contains("limit 15.0 ms"), "{detail}");
            }
            _ => panic!("普通宿主不能复用超过自己逐键上限的缓存模型"),
        }
        // 拒绝较严的宿主不卸载或重选模型；原停顿宿主和恰好达到上限的宿主仍复用同一份。
        for limit in [PAUSE_RERANK_LIMIT_MS, 20.0] {
            let attached = cache
                .attach(bundle.path(), 15.0, limit, None)
                .attached()
                .unwrap();
            assert!(Arc::ptr_eq(&attached.scorer, &scorer));
            assert_eq!(attached.page_ms, 20.0);
        }
    }

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
/// 这个开关默认开（见 `.agents/notes/implemented/feature/2026-11-02-keystroke-reranker-default-on.md`），所以挂不挂由这台设备跑不跑得进逐键预算决定，而不是由开关决定：模型跑在每一次 `character` 调用里、就在宿主打字的线程上，跑不进预算时快打会落后、丢键（2026-10 安卓上的事故）。挂上之前先用 [`KEYSTROKE_RERANK_LIMIT_MS`] 量它打一页重负载，超过就不挂，诊断写 `keyboard reranker over budget`，这台设备上「增强」与「标准」相同。开关关着时模型根本不加载。
///
/// 编译时打开了 `onnx-rerank`、随包资源目录旁边又有 [`RERANK_MODEL_DIRECTORY`] 时用那个模型（ONNX Runtime 后端，大小两档按自测速度选，见 [`OnnxRerankers::attach`]）；ONNX 模型在但连小的也超过上限时不再退到随包的句子模型——它在同一台设备上只会更慢（Pixel 3 XL 上 6x256 一页约 10 ms，随包 `model-v1` 约 60 ms）。ONNX 模型一个都读不了、或者本来就没有时（32 位的安卓与鸿蒙、Win7 这些没有 ONNX Runtime 产物的构建），用随包的句子模型，同样先量（[`bundled_keyboard_reranker`]）。`bundle` 是随包资源目录（`EngineOptions::helpcode_fallback_resources`）：`resources` 换成下载的词库代次时，ONNX 模型仍在安装包里，按它去找；为 `None` 时 `resources` 本身就是随包的那个。`sound_packs` 是会话的内置音效包目录（[`crate::key_sound::PluginRoots::builtin_sounds`]），资源目录旁边没有模型时按它找安装目录，见 [`rerank_models_root`]。`models` 是宿主指明的模型目录（HostOptions 的 `rerank_models`，Windows 的 Server 和鸿蒙的键盘写它），给了就只在那里找，不再按资源目录推。安装包没带模型时（Android）再看 `state_root` 下有没有装好的 `rerank-model-small` 资源包，见 [`rerank_models_root`]。
///
/// 开关打开时写一行诊断，说挂上的是哪一个、或者为什么没挂（见 [`report_keyboard_reranker`]）。
pub(crate) fn keyboard_reranker(
    enabled: bool,
    resources: &str,
    bundle: Option<&str>,
    models: Option<&str>,
    sound_packs: Option<&Path>,
    state_root: Option<&Path>,
    configured: Option<&str>,
) -> Option<Box<dyn msime_input_runtime::CandidateScorer>> {
    let mount = if enabled {
        RerankerMount::Keystroke
    } else {
        RerankerMount::Off
    };
    keyboard_reranker_for(
        mount,
        resources,
        bundle,
        models,
        sound_packs,
        state_root,
        configured,
    )
    .map(MountedReranker::into_scorer)
}

/// 宿主要的逐键重排挂法。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RerankerMount {
    /// 「增强」关着，或者宿主不要（镜像会话）：不挂，已加载的放开。
    Off,
    /// 只挂在按键上，跑不进 [`KEYSTROKE_RERANK_LIMIT_MS`] 就不挂。
    Keystroke,
    /// 宿主停顿时会调 `msime_client_start_settled_rerank`（HostOptions 的 `pause_rerank`）：跑得进逐键预算（一页不超过 [`PAUSE_HOST_KEYSTROKE_PAGE_MS`]）挂在按键上，跑不进就延后到停顿，一页超过 [`PAUSE_RERANK_LIMIT_MS`] 才不挂。
    KeystrokeOrPause,
}

/// 挂上的逐键重排器，挂在哪里：按键上（`Runtime::set_candidate_scorer`），还是延后到宿主停顿时（`Runtime::set_deferred_scorer`）。
pub(crate) enum MountedReranker {
    Keystroke(Box<dyn msime_input_runtime::CandidateScorer>),
    // 只有 ONNX 模型会延后；未编入 ONNX Runtime 的构建不会创建这一项。
    #[cfg_attr(not(feature = "onnx-rerank"), allow(dead_code))]
    Pause(Box<dyn msime_input_runtime::CandidateScorer>),
}

impl MountedReranker {
    /// 挂到 `runtime` 上；`None` 拿掉按键上和延后的两个。
    pub(crate) fn mount(mounted: Option<Self>, runtime: &mut Runtime) {
        match mounted {
            Some(Self::Pause(scorer)) => runtime.set_deferred_scorer(Some(scorer)),
            Some(Self::Keystroke(scorer)) => runtime.set_candidate_scorer(Some(scorer)),
            None => runtime.set_candidate_scorer(None),
        }
    }

    fn into_scorer(self) -> Box<dyn msime_input_runtime::CandidateScorer> {
        match self {
            Self::Keystroke(scorer) | Self::Pause(scorer) => scorer,
        }
    }
}

/// 一页要 `page_ms` 的模型在 `mount` 这样的宿主上是不是延后到停顿：只有停顿时重解的宿主、而且一页超过 [`PAUSE_HOST_KEYSTROKE_PAGE_MS`]。
#[cfg(feature = "onnx-rerank")]
pub(crate) fn defers_to_pause(mount: RerankerMount, page_ms: f64) -> bool {
    mount == RerankerMount::KeystrokeOrPause && page_ms > PAUSE_HOST_KEYSTROKE_PAGE_MS
}

/// 同 [`keyboard_reranker`]，按 `mount` 决定挂在按键上还是延后到停顿。
///
/// 停顿时重解的宿主（[`RerankerMount::KeystrokeOrPause`]）上，ONNX 模型一页超过 [`PAUSE_HOST_KEYSTROKE_PAGE_MS`] 时延后：按键上不挂，引擎照没有模型时那样解码，宿主停顿时 runtime 才带着它重解一遍（`Runtime::settle_deferred_scorer`），诊断写 `keyboard reranker at pause`，细节是原来那一档的类别和细节。随包的句子模型不延后：它只在按键上挂，照旧按 [`KEYSTROKE_RERANK_LIMIT_MS`] 决定挂不挂。
pub(crate) fn keyboard_reranker_for(
    mount: RerankerMount,
    resources: &str,
    bundle: Option<&str>,
    models: Option<&str>,
    sound_packs: Option<&Path>,
    state_root: Option<&Path>,
    configured: Option<&str>,
) -> Option<MountedReranker> {
    #[cfg(feature = "onnx-rerank")]
    let root = onnx_rerank_source(resources, bundle, models, sound_packs, state_root);
    if mount == RerankerMount::Off {
        #[cfg(feature = "onnx-rerank")]
        if let Some(root) = root.as_deref() {
            onnx_rerankers().release(root);
        }
        return None;
    }
    #[cfg(feature = "onnx-rerank")]
    if let Some(root) = root.as_deref() {
        let pause = mount == RerankerMount::KeystrokeOrPause;
        let limit = if pause {
            PAUSE_RERANK_LIMIT_MS
        } else {
            KEYSTROKE_RERANK_LIMIT_MS
        };
        match onnx_reranker(root, limit) {
            OnnxAttach::Attached(attached) => {
                let scorer: Box<dyn msime_input_runtime::CandidateScorer> =
                    Box::new(SharedOnnxScorer::new(attached.scorer));
                if defers_to_pause(mount, attached.page_ms) {
                    report_keyboard_reranker(
                        KEYBOARD_RERANKER_AT_PAUSE,
                        &format!("{}: {}", attached.category, attached.detail),
                    );
                    return Some(MountedReranker::Pause(scorer));
                }
                report_keyboard_reranker(attached.category, &attached.detail);
                return Some(MountedReranker::Keystroke(scorer));
            }
            OnnxAttach::OverLimit(detail) => {
                report_keyboard_reranker(KEYBOARD_RERANKER_OVER_BUDGET, &detail);
                return None;
            }
            OnnxAttach::Unavailable => {}
        }
    }
    #[cfg(not(feature = "onnx-rerank"))]
    let _ = (bundle, models, sound_packs, state_root);
    bundled_keyboard_reranker(resources, configured, KEYSTROKE_RERANK_LIMIT_MS)
        .map(MountedReranker::Keystroke)
}

/// 诊断类别：这台设备上模型打一页重负载超过 [`KEYSTROKE_RERANK_LIMIT_MS`]，没挂逐键重排。
pub(crate) const KEYBOARD_RERANKER_OVER_BUDGET: &str = "keyboard reranker over budget";

/// 诊断类别：模型挂上了，但这台设备跑不进逐键预算，延后到宿主停顿时才用（[`RerankerMount::KeystrokeOrPause`]）。细节是原来那一档的类别和细节。
#[cfg(feature = "onnx-rerank")]
pub(crate) const KEYBOARD_RERANKER_AT_PAUSE: &str = "keyboard reranker at pause";

/// 默认挂上的逐键重排模型打一页重负载（32 字上文、9 个 6 字候选、37 个新前缀，与 `msime_onnx_rerank::OnnxScorer::calibrate` 同一页）最多花这么多毫秒，超过就不挂。
///
/// 逐键预算是整键 p95 ≤ 10 ms。这一页是长句尾部那种按键，比平常重：2026-11-02 在同一台机器上量，它是真实运行时里重排开销 p95 的 1.4 到 1.7 倍——Pixel 3 XL 上随包 `model-v1` 一页 60–68 ms、重排开销 p95 36 ms，6x256（四线程）一页 9.8–10.4 ms、重排开销 p95 6.9 ms（整键 9.3 ms）；Mac（M 系列）上 `model-v1` 一页 6.2–6.4 ms。所以上限取 10 ms 的 1.5 倍：一页 15 ms 以内对应重排开销 p95 约 10 ms 以内。拿 10 ms 直接卡这一页会让 Pixel 3 XL 的 6x256 在线上来回跳，而它的整键 p95 本来就在预算之内。
///
/// 量出来：Mac 和 iPhone 17 的 8x448、iPhone XR 与 Pixel 3 XL 的 6x256、Mac 的 `model-v1` 都挂；Pixel 3 XL 这类老安卓手机上的 `model-v1` 不挂。与 [`LARGE_RERANK_MODEL_BUDGET_MS`]（大小两档之间换档的线，量的是同一页）是两件事：那条线决定用哪一档，这条线决定挂不挂。
pub(crate) const KEYSTROKE_RERANK_LIMIT_MS: f64 = 15.0;

/// 停顿时重解的宿主上（[`RerankerMount::KeystrokeOrPause`]），ONNX 模型一页超过这么多毫秒就不挂在按键上，延后到停顿。
///
/// 整句打字时真实的逐键开销比这一页重：2026-11-04 在华为畅享 90 Plus（Kirin 8000，鸿蒙，两线程）上用触摸屏注入打 typing-chat-v1 的整句（59 条、1698 键），6x256 一页自测 5–11.5 ms，整键 p95 19.5 ms，不挂模型 6.5 ms，模型的开销 p95 约是一页的 1.6 倍，而 [`SENTENCE_POOL_COST`] 的手机系数（Pixel 3 XL、分段打）是 0.72。手机按键预算 10 ms、引擎自己的尾部 3.5 ms，留给模型 6.5 ms，按 1.6 倍算一页要在 4 ms 之内。
#[cfg(feature = "onnx-rerank")]
pub(crate) const PAUSE_HOST_KEYSTROKE_PAGE_MS: f64 = 4.0;

/// 停顿时重解的宿主上，ONNX 模型一页超过这么多毫秒就连停顿也不挂。停顿时的重解在宿主的会话线程上跑，从头解码、带着模型给整句打分，大约是几页的量；同一台畅享 90 Plus 刚装完应用时自测一页量到过 26.5 ms，平常 5–11 ms，这条线让它在忙的那一刻也照样挂上，比它慢四五倍的设备才不挂。
#[cfg(feature = "onnx-rerank")]
pub(crate) const PAUSE_RERANK_LIMIT_MS: f64 = 60.0;

/// 量一页时用的 64 个字：常用汉字，随包 `model-v1` 的字表里都有。前 32 个当上文，其余拼候选。模型不认识的字按未知字编码，算的量一样。
const CALIBRATION_CHARACTERS: &str =
    "的一是在不了有和人这中大为上个国我以要他时来用们生到作地于出就分对成会可主发年动同工也能下过子说产种面而方后多定行学法所民得经十";

/// 随包句子模型打一页重负载的毫秒数：每次换一个新的 [`Reranker`]（不带前缀缓存），先编码上文、只量按键那一步；预热一次再取 3 次的中位数，与 `OnnxScorer::calibrate` 同一页、同一口径。第一次就超过上限两倍时不再量下去：结论不会变，老手机上每多量一次就多卡宿主线程六七十毫秒。
fn calibrate_bundled(model: &Arc<SentenceModel>) -> f64 {
    let ids: Vec<char> = CALIBRATION_CHARACTERS.chars().take(64).collect();
    let context: String = ids[..32].iter().collect();
    let mut times = Vec::new();
    for run in 0..4 {
        let page: Vec<String> = (0..9)
            .map(|c| {
                let head = ids[32 + run];
                let rest = (0..5).map(|t| ids[36 + (c * 3 + t + run * 7) % 28]);
                std::iter::once(head).chain(rest).collect()
            })
            .collect();
        let texts: Vec<&str> = page.iter().map(String::as_str).collect();
        let mut reranker = Reranker::new(Arc::clone(model));
        reranker.log_probabilities(&context, &texts[..1]);
        let started = std::time::Instant::now();
        let scored = reranker.log_probabilities(&context, &texts);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        if scored.len() != texts.len() {
            return f64::INFINITY;
        }
        if run > 0 {
            if elapsed > 2.0 * KEYSTROKE_RERANK_LIMIT_MS {
                return elapsed;
            }
            times.push(elapsed);
        }
    }
    times.sort_by(f64::total_cmp);
    times[times.len() / 2]
}

/// 随包句子模型在这个进程里量过的结果，按模型路径记：同一个路径的模型在进程里只加载一次（[`sentence_model`]），量一次就够，之后每个会话直接按结论挂或不挂，老手机上不再为它卡一次。
fn bundled_calibration(path: &Path, model: &Arc<SentenceModel>) -> f64 {
    static MEASURED: OnceLock<Mutex<std::collections::HashMap<PathBuf, f64>>> = OnceLock::new();
    let mut measured = MEASURED
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *measured
        .entry(path.to_path_buf())
        .or_insert_with(|| calibrate_bundled(model))
}

/// 随包句子模型（`model-v1`，纯 Rust 参考实现）当逐键重排器：先量这台设备上它打一页重负载要多久，不超过 `limit_ms` 才挂。没有 ONNX Runtime 的构建（32 位的安卓与鸿蒙、Win7）和还没带上 ONNX 模型的宿主走这里；Mac 上一页约 6 ms，挂；Pixel 3 XL 上约 60 ms，不挂。
pub(crate) fn bundled_keyboard_reranker(
    resources: &str,
    configured: Option<&str>,
    limit_ms: f64,
) -> Option<Box<dyn msime_input_runtime::CandidateScorer>> {
    let path = sentence_model_path(resources, configured);
    let Some(model) = sentence_model(resources, configured) else {
        report_keyboard_reranker("keyboard reranker missing", &path.display().to_string());
        return None;
    };
    let measured = bundled_calibration(&path, &model);
    let detail = format!(
        "{} (page {measured:.1} ms, limit {limit_ms:.1} ms)",
        path.display()
    );
    if measured > limit_ms {
        report_keyboard_reranker(KEYBOARD_RERANKER_OVER_BUDGET, &detail);
        return None;
    }
    report_keyboard_reranker("keyboard reranker bundled model", &detail);
    Some(Box::new(Reranker::new(model)))
}

/// 「增强」开着时逐键重排挂的是哪一个，写一行诊断。类别本身就说明了档位（`keyboard reranker onnx large`、`keyboard reranker onnx small`、可用内存不够加载大的时的 `keyboard reranker onnx small (low memory)`、`keyboard reranker bundled model`、`keyboard reranker missing`，以及这台设备跑不进逐键预算、没挂的 `keyboard reranker over budget`，和跑不进逐键预算、延后到宿主停顿时才用的 `keyboard reranker at pause`），只留类别的宿主（macOS、iOS）也看得出来；细节是模型目录、大模型的自测毫秒数、挂上的那一档打一页的毫秒数和上限（`page … ms, limit … ms`）、给它的整句池（`sentence pool …`）、可用内存，以及是不是与之前的会话共用（`(shared)`）。只在会话建立和开关打开时写，不在按键路径上写。
fn report_keyboard_reranker(category: &'static str, detail: &str) {
    crate::diagnostics::report(category, detail);
}

/// 放 ONNX 重排模型的目录：随包资源目录的上一级，与 `settled-model/`、`language-dictionaries/` 一样是资源目录的兄弟。不放进资源目录本身，因为那里必须与词库锁逐项相符（`ResourceStore::verify` 只放过 `helpcodes/`），多一个目录整套资源就校验失败。
///
/// 那里没有模型时，再看内置音效包目录（`sound_packs`）的上一级。Linux 的安装包不带词库，`msime-linux-setup` 把它下载到用户自己的 `$XDG_DATA_HOME/msime-client/resources`，所以 `resources` 的上一级是用户目录，模型却装在 `/usr/share/msime-client/rerank-model`；两个宿主都按自己的安装位置把 `/usr/share/msime-client/sound-packs` 写成 `sound_packs`（Host API 已经据此找官方插件包，见 [`crate::key_sound::PluginRoots::new`]），它的上一级就是安装目录。Windows 不这样找：那里的内置音效包在普通用户可写的数据目录里，旁边的目录不能当作安装包的东西。
///
/// 音效包目录的上一级也没有时（Android 发布包不带模型，体积上限放不下），用 `state_root` 下装好的 `rerank-model-small` 资源包：资源包目录 `<state_root>/resource-packs/` 里的子目录名就是资源包 id，与 [`SMALL_RERANK_MODEL_DIRECTORY`] 相同，所以返回的是资源包根目录。只认与锁一致、完整发布的资源包（`installed_file`）；都没有时为 `None`，退回随包的句子模型。
#[cfg(feature = "onnx-rerank")]
pub(crate) fn rerank_models_root(
    resources: &str,
    bundle: Option<&str>,
    sound_packs: Option<&Path>,
    state_root: Option<&Path>,
) -> Option<PathBuf> {
    use msime_client_providers::resource_packs::{self, ResourcePack};
    let bundled = bundle.filter(|path| !path.is_empty()).unwrap_or(resources);
    let holds_models = |root: &Path| {
        root.join(RERANK_MODEL_DIRECTORY).is_dir()
            || root.join(SMALL_RERANK_MODEL_DIRECTORY).is_dir()
    };
    if let Some(beside) = Path::new(bundled)
        .parent()
        .filter(|root| holds_models(root))
    {
        return Some(beside.to_path_buf());
    }
    #[cfg(not(target_os = "windows"))]
    if let Some(installed) = sound_packs
        .and_then(Path::parent)
        .filter(|root| holds_models(root))
    {
        return Some(installed.to_path_buf());
    }
    #[cfg(target_os = "windows")]
    let _ = sound_packs;
    let state_root = state_root?;
    resource_packs::installed_file(state_root, ResourcePack::RerankModelSmall, "lm_step.ort")
        .map(|_| resource_packs::root(state_root))
}

/// 会话此刻会从哪里挂 ONNX 重排模型：宿主指明了模型目录（`models`，HostOptions 的 `rerank_models`）就是那里，否则按 [`rerank_models_root`] 找；没开 `onnx-rerank` 特性时总是 `None`。[`keyboard_reranker`] 按它挂，聚焦时与上次挂的比较，资源包刚装好或被移除时换掉逐键重排。
pub(crate) fn onnx_rerank_source(
    resources: &str,
    bundle: Option<&str>,
    models: Option<&str>,
    sound_packs: Option<&Path>,
    state_root: Option<&Path>,
) -> Option<PathBuf> {
    #[cfg(feature = "onnx-rerank")]
    return match models {
        Some(models) => Some(PathBuf::from(models)),
        None => rerank_models_root(resources, bundle, sound_packs, state_root),
    };
    #[cfg(not(feature = "onnx-rerank"))]
    {
        let _ = (resources, bundle, models, sound_packs, state_root);
        None
    }
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

/// 整句池（[`msime_onnx_rerank::OnnxScorer::set_sentence_pool`]）的候选档位，从大到小试。
///
/// 每档两个系数：模型打一页重负载的自测毫秒数（`OnnxScorer::calibrate`）乘上它，就是这一档在真实运行时里给每次按键加上的尾部耗时，前一个是手机的 p95，后一个是桌面的 p99。用 `rerank_latency --sentence-pool`（sentences-v2 与 typing-chat-v1，8104 次按键，交替跑）在 2026-11-02 量的：
///
/// - 桌面那一列来自 Mac（M 系列，2 线程）：大模型一页 4.0–4.3 ms、小模型 1.1–1.3 ms，两个模型算出的系数相差不到一成，共用一列。
/// - 手机那一列来自 Pixel 3 XL 上的 6x256（4 线程，一页 8.8–8.9 ms）：池 9、12、16 整键 p95 是 9.8、10.9、12.5–12.9 ms，比 Mac 的 p95 系数一致地高约一成；20 条以上没有在手机上量，按同一比例从 Mac 推出来。
#[cfg(feature = "onnx-rerank")]
const SENTENCE_POOL_COST: [(usize, f64, f64); 6] = [
    (32, 1.60, 1.86),
    (24, 1.35, 1.49),
    (20, 1.20, 1.42),
    (16, 1.04, 1.22),
    (12, 0.84, 1.03),
    (9, 0.72, 0.76),
];

/// 引擎自己（不算模型）每次按键的尾部耗时，和逐键预算：手机守整键 p95 ≤ 10 ms，按 Pixel 3 XL 上引擎那一份的 p95（3.5 ms）算；桌面守整键 p99 ≤ 4 ms，按 Mac 上引擎的 p99（0.7 ms）算。
#[cfg(all(feature = "onnx-rerank", any(target_os = "ios", target_os = "android")))]
const ENGINE_TAIL_MS: f64 = 3.5;
#[cfg(all(feature = "onnx-rerank", any(target_os = "ios", target_os = "android")))]
const KEYSTROKE_BUDGET_MS: f64 = 10.0;
#[cfg(all(
    feature = "onnx-rerank",
    not(any(target_os = "ios", target_os = "android"))
))]
const ENGINE_TAIL_MS: f64 = 0.7;
#[cfg(all(
    feature = "onnx-rerank",
    not(any(target_os = "ios", target_os = "android"))
))]
const KEYSTROKE_BUDGET_MS: f64 = 4.0;

/// 模型打一页要 `page_ms` 的这台机器上，按键预算之内能比较多大的整句池；连 9 条都放不下时为 `None`。手机看 p95、桌面看 p99（[`KEYSTROKE_BUDGET_MS`]）。Pixel 3 XL 的 6x256（一页约 8.9 ms）落在 9 条，iPhone 17 的 8x448（一页约 4 ms）落在 24 条，Mac 的 8x448（一页约 4.1 ms）落在 9 条。
#[cfg(feature = "onnx-rerank")]
pub(crate) fn sentence_pool_for(page_ms: f64) -> Option<usize> {
    let phone = cfg!(any(target_os = "ios", target_os = "android"));
    SENTENCE_POOL_COST
        .iter()
        .find(|&&(_, p95, p99)| {
            ENGINE_TAIL_MS + page_ms * if phone { p95 } else { p99 } <= KEYSTROKE_BUDGET_MS
        })
        .map(|&(pool, _, _)| pool)
}

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
    /// 这个模型在这台设备上打一页重负载的毫秒数（`OnnxScorer::calibrate`），宿主按它决定挂在按键上还是延后到停顿，见 [`keyboard_reranker_for`]。
    pub(crate) page_ms: f64,
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
    page_ms: f64,
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
    /// 各个模型打一页重负载的毫秒数：模型目录、当时 `lm_step.ort` 的文件戳和毫秒数。大模型量的是换档的自测（`calibrate_for_keystroke_budget`），小模型和只装了一个的那一个量的是挂不挂的检查（`calibrate`），同一页。挂上的那一个按它定整句池（[`sentence_pool_for`]）。
    calibrated: Vec<(PathBuf, ModelStamp, f64)>,
    /// 按一页的毫秒数定整句池的规则，`None` 是 [`sentence_pool_for`]。测试在这里换成与机器快慢无关的规则：测试用的小模型在并行跑测试时一页可能慢到放不下 32 条，池就跟着机器忙闲变。
    pool_rule: Option<PoolRule>,
}

/// [`OnnxRerankers::attach`] 的结果：挂上了哪一个；装着 ONNX 模型、但挂得上的那一档在这台设备上打一页超过上限（[`KEYSTROKE_RERANK_LIMIT_MS`]），于是什么都不挂（带诊断细节）；或者一个 ONNX 模型都读不了，交给随包的句子模型。
#[cfg(feature = "onnx-rerank")]
pub(crate) enum OnnxAttach {
    Attached(AttachedOnnxReranker),
    OverLimit(String),
    Unavailable,
}

#[cfg(all(test, feature = "onnx-rerank"))]
impl OnnxAttach {
    /// 挂上的那一个；超过上限或读不了时是 `None`。只给测试用，宿主按三种情形分别处理（[`keyboard_reranker`]）。
    pub(crate) fn attached(self) -> Option<AttachedOnnxReranker> {
        match self {
            Self::Attached(attached) => Some(attached),
            Self::OverLimit(_) | Self::Unavailable => None,
        }
    }
}

/// [`OnnxRerankers`] 定整句池的规则：一页的毫秒数 → 放得下的池，`None` 是 9 条也放不下。
#[cfg(feature = "onnx-rerank")]
pub(crate) type PoolRule = Box<dyn FnMut(f64) -> Option<usize> + Send>;

#[cfg(feature = "onnx-rerank")]
fn onnx_rerankers() -> std::sync::MutexGuard<'static, OnnxRerankers> {
    static RERANKERS: OnceLock<Mutex<OnnxRerankers>> = OnceLock::new();
    RERANKERS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 装了两个 ONNX 模型时按自测速度和可用内存在两者之间选，只装了一个就用那一个；挂上的那一档打一页超过 `limit_ms`（[`KEYSTROKE_RERANK_LIMIT_MS`]，停顿时重解的宿主是 [`PAUSE_RERANK_LIMIT_MS`]）时不挂，见 [`OnnxRerankers::attach`]。`root` 是放两个模型目录的地方（[`rerank_models_root`]）。
#[cfg(feature = "onnx-rerank")]
fn onnx_reranker(root: &Path, limit_ms: f64) -> OnnxAttach {
    onnx_rerankers().attach(
        root,
        LARGE_RERANK_MODEL_BUDGET_MS,
        limit_ms,
        available_memory(),
    )
}

/// 共用重排器在一个会话里的那一份，连同交给引擎拼整句的那一份，见 [`msime_onnx_rerank::SharedOnnxScorer`]。
#[cfg(feature = "onnx-rerank")]
pub(crate) use msime_onnx_rerank::SharedOnnxScorer;

#[cfg(feature = "onnx-rerank")]
impl OnnxRerankers {
    /// 用 `rule` 代替 [`sentence_pool_for`] 定整句池，只给测试用。
    #[cfg(test)]
    pub(crate) fn with_pool_rule(rule: PoolRule) -> Self {
        Self {
            pool_rule: Some(rule),
            ..Self::default()
        }
    }

    /// 这一页放得下的整句池，按 `pool_rule`，放不下 9 条时也给 9 条。
    fn pool_for(&mut self, page_ms: f64) -> usize {
        match self.pool_rule.as_mut() {
            Some(rule) => rule(page_ms),
            None => sentence_pool_for(page_ms),
        }
        .unwrap_or(msime_engine::ordering::SENTENCE_POOL)
    }

    /// 给一个新会话挂重排器。已经为 `root` 加载过、两个模型文件都没换时直接共用（细节标 `(shared)`）；否则定档位并加载：
    ///
    /// - 两个都装了：`available`（字节）少于 [`LARGE_RERANK_MODEL_MIN_AVAILABLE`] 时直接用小的，不先加载大的去自测；否则加载大的、量它打一页重负载要多久，超过 `budget_ms` 就换小的。没超过、但它在按键预算里只比较得了九条而小模型比较得了整池时，也换小的（[`Self::small_for_pool`]，桌面的情形）。按量出来的速度选，不按机型名单选。
    /// - 只装了一个就用那一个；大的读不了就用小的。
    /// - 挂上的那一档还要过 `limit_ms`：过了 `budget_ms` 留用的大模型自然在内；小模型和只装了一个的那一个各量一次同一页（`OnnxScorer::calibrate`，不改它排不排上屏后联想），超过就不挂，返回 [`OnnxAttach::OverLimit`]，也不退到随包的句子模型——那个在同一台设备上只会更慢。换下来的大模型超过换档线、却没有小的可换时，同样按 `limit_ms` 决定还挂不挂它。
    ///
    /// 每个模型文件在进程里只量一次，结论留着：放开再挂、换了预算再挂，都按量过的毫秒数定，超过上限的那一档不再加载。换下来的那个只从这里放开，还挂着它的会话照常用到结束，之后内存还给系统。
    pub(crate) fn attach(
        &mut self,
        root: &Path,
        budget_ms: f64,
        limit_ms: f64,
        available: Option<u64>,
    ) -> OnnxAttach {
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
            if loaded.page_ms > limit_ms {
                return OnnxAttach::OverLimit(format!(
                    "{} (page {:.1} ms, limit {limit_ms:.1} ms)",
                    loaded.directory.display(),
                    loaded.page_ms
                ));
            }
            return OnnxAttach::Attached(AttachedOnnxReranker {
                scorer: Arc::clone(&loaded.scorer),
                category: loaded.category,
                detail: format!("{} (shared)", loaded.detail),
                page_ms: loaded.page_ms,
            });
        }
        if large_stamp.is_none() || small_stamp.is_none() {
            return match self.load_within(&large, stamps, LARGE, String::new(), limit_ms) {
                OnnxAttach::Unavailable => {
                    self.load_within(&small, stamps, SMALL, String::new(), limit_ms)
                }
                decided => decided,
            };
        }
        if let Some(available) = available.filter(|&bytes| bytes < LARGE_RERANK_MODEL_MIN_AVAILABLE)
        {
            let note = format!(
                " (available {} MiB, large needs {} MiB)",
                available >> 20,
                LARGE_RERANK_MODEL_MIN_AVAILABLE >> 20
            );
            return self.load_within(&small, stamps, SMALL_LOW_MEMORY, note, limit_ms);
        }
        let note =
            |measured: f64| format!(" (calibrated {measured:.1} ms, budget {budget_ms:.1} ms)");
        // 大模型过了换档线时留用它；没过就换小的，小的读不了时大的只要还在上限之内也挂，否则不挂。
        let over_budget = |this: &mut Self, measured: f64| -> Option<OnnxAttach> {
            if measured <= budget_ms {
                return None;
            }
            match this.load_within(&small, stamps, SMALL, note(measured), limit_ms) {
                OnnxAttach::Unavailable if measured > limit_ms => {
                    Some(OnnxAttach::OverLimit(format!(
                        "{}{} (limit {limit_ms:.1} ms)",
                        large.display(),
                        note(measured)
                    )))
                }
                OnnxAttach::Unavailable => None,
                decided => Some(decided),
            }
        };
        if let Some(measured) = self.measured(&large, large_stamp) {
            if let Some(decided) = over_budget(self, measured) {
                return decided;
            }
            let pool = self.pool_for(measured);
            if let Some(decided) =
                self.small_for_pool((measured, pool), &small, stamps, budget_ms, limit_ms)
            {
                return decided;
            }
            // 新加载的这一份带上之前量出的结论：过了预算的同时也排上屏后联想。
            return match self.load_calibrated(
                &large,
                stamps,
                LARGE,
                note(measured),
                (measured, budget_ms),
            ) {
                Some(attached) => OnnxAttach::Attached(attached),
                None => self.load_within(&small, stamps, SMALL, note(measured), limit_ms),
            };
        }
        self.loaded = None;
        let Some(mut scorer) = load_onnx_reranker(&large) else {
            return self.load_within(&small, stamps, SMALL, String::new(), limit_ms);
        };
        // 过了预算的大模型同时也排上屏后联想；退到小模型的机器不排，见 `OnnxScorer::fits_keystroke_budget`。
        let measured = scorer.calibrate_for_keystroke_budget(budget_ms);
        self.remember(&large, large_stamp, measured);
        if measured > budget_ms {
            // 先放开大模型再去加载小的，加载的那一刻不必两个都占着；小的读不了而大的还在上限之内时再加载一次大的。
            drop(scorer);
            if let Some(decided) = over_budget(self, measured) {
                return decided;
            }
            return self
                .load_calibrated(&large, stamps, LARGE, note(measured), (measured, budget_ms))
                .map_or(OnnxAttach::Unavailable, OnnxAttach::Attached);
        }
        let pool = self.pool_for(measured);
        if pool <= msime_engine::ordering::SENTENCE_POOL {
            // 小模型可能放得下整池；先放开大的，换档时不必两个都占着，换不了再加载一次大的。
            drop(scorer);
            if let Some(decided) =
                self.small_for_pool((measured, pool), &small, stamps, budget_ms, limit_ms)
            {
                return decided;
            }
            return self
                .load_calibrated(&large, stamps, LARGE, note(measured), (measured, budget_ms))
                .map_or(OnnxAttach::Unavailable, OnnxAttach::Attached);
        }
        OnnxAttach::Attached(self.keep(&large, stamps, scorer, LARGE, note(measured), measured))
    }

    /// 大模型在按键预算里只比较得了最少的九条（[`msime_engine::ordering::SENTENCE_POOL`]），而小模型比较得了整池（[`msime_engine::ordering::MAX_SENTENCE_POOL`]）时，挂小模型配整池；否则 `None`，照旧挂大模型。
    ///
    /// 池越大，正确的整句越可能在里面：交给模型比较的九条整句常常前半句都一样，开头一错（`fangan` 选成「反感」）九条全错，模型救不回来。typing-chat-v1 上 8x448 配 9 条有 36/58 个错句段的正确答案根本不在候选里。2026-11-03 在 Mac 上量：首选 dev 62.7% → 68.2%、test 73.6% → 79.5%（6x256 配 32 条）；8x448 配 32 条只有 66.4% / 77.7%，而且桌面整键 p99 远超 4 ms，6x256 配 32 条 p99 约 3.0 ms，比 8x448 配 9 条（3.7–4.1 ms）还低。只在大模型降到九条时才换：iPhone 17 这类手机上大模型能比较 24 条，照旧用它。见 `.agents/notes/implemented/feature/2026-11-03-desktop-small-model-full-pool.md`。
    ///
    /// 小模型的自测结论按大模型同样的预算记下（`accept_calibration`），在预算之内时同样排上屏后联想。小模型读不了、超过上限或放不下整池时返回 `None`。
    ///
    /// `large` 是大模型打一页的毫秒数和它在这台机器上的池。
    fn small_for_pool(
        &mut self,
        (large_ms, large_pool): (f64, usize),
        small: &Path,
        stamps: [ModelStamp; 2],
        budget_ms: f64,
        limit_ms: f64,
    ) -> Option<OnnxAttach> {
        const SMALL: &str = "keyboard reranker onnx small";
        if large_pool > msime_engine::ordering::SENTENCE_POOL {
            return None;
        }
        let stamp = model_stamp(small)?;
        let known = self.measured(small, Some(stamp));
        let fits = |this: &mut Self, measured: f64| {
            measured <= limit_ms
                && this.pool_for(measured) >= msime_engine::ordering::MAX_SENTENCE_POOL
        };
        if known.is_some_and(|measured| !fits(self, measured)) {
            return None;
        }
        self.loaded = None;
        let mut scorer = load_onnx_reranker(small)?;
        let measured = known.unwrap_or_else(|| {
            let measured = scorer.calibrate();
            self.remember(small, Some(stamp), measured);
            measured
        });
        if !fits(self, measured) {
            return None;
        }
        scorer.accept_calibration(measured, budget_ms);
        let note = format!(
            " (large {large_ms:.1} ms compares {large_pool}, small {measured:.1} ms compares {})",
            msime_engine::ordering::MAX_SENTENCE_POOL
        );
        Some(OnnxAttach::Attached(
            self.keep(small, stamps, scorer, SMALL, note, measured),
        ))
    }

    /// 这个进程里之前对 `directory` 这个模型文件量出的毫秒数；文件换了（文件戳不同）就当没量过。
    fn measured(&self, directory: &Path, stamp: ModelStamp) -> Option<f64> {
        self.calibrated
            .iter()
            .find(|(path, known, _)| path == directory && *known == stamp)
            .map(|&(_, _, measured)| measured)
    }

    fn remember(&mut self, directory: &Path, stamp: ModelStamp, measured: f64) {
        self.calibrated.retain(|(path, _, _)| path != directory);
        self.calibrated
            .push((directory.to_path_buf(), stamp, measured));
    }

    /// 加载 `directory` 的模型并检查它打一页重负载在 `limit_ms` 之内（量过的不重量，量过超了的不加载）。没有这个模型或读不了时是 [`OnnxAttach::Unavailable`]，已加载的那个留着；超过上限时什么都不留。量的是 `OnnxScorer::calibrate`，不改模型排不排上屏后联想。
    fn load_within(
        &mut self,
        directory: &Path,
        stamps: [ModelStamp; 2],
        category: &'static str,
        note: String,
        limit_ms: f64,
    ) -> OnnxAttach {
        let Some(stamp) = model_stamp(directory) else {
            return OnnxAttach::Unavailable;
        };
        let over = |measured: f64| {
            OnnxAttach::OverLimit(format!(
                "{}{note} (page {measured:.1} ms, limit {limit_ms:.1} ms)",
                directory.display()
            ))
        };
        let known = self.measured(directory, Some(stamp));
        if let Some(measured) = known.filter(|&measured| measured > limit_ms) {
            self.loaded = None;
            return over(measured);
        }
        // 先放开换下来的那个，加载新模型的那一刻不必两个都由这里占着。
        self.loaded = None;
        let Some(mut scorer) = load_onnx_reranker(directory) else {
            return OnnxAttach::Unavailable;
        };
        let measured = known.unwrap_or_else(|| {
            let measured = scorer.calibrate();
            self.remember(directory, Some(stamp), measured);
            measured
        });
        if measured > limit_ms {
            return over(measured);
        }
        let note = format!("{note} (page {measured:.1} ms, limit {limit_ms:.1} ms)");
        OnnxAttach::Attached(self.keep(directory, stamps, scorer, category, note, measured))
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

    /// 加载 `directory` 的模型，再把这个进程里之前对同一个模型文件量出的 `(毫秒数, 预算)` 交给它（`OnnxScorer::accept_calibration`），不重测。
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
        Some(self.keep(directory, stamps, scorer, category, note, measured_ms))
    }

    /// 留下挂上的这一份，并按它在这台机器上打一页的 `page_ms` 定整句池（[`sentence_pool_for`]）：档位已经定了，池只在这个模型上定（`.agents/notes/implemented/feature/2026-11-02-sentence-pool-by-device-speed.md`）。为池换模型只有一种情形，在定档时就决定了：大模型只比较得了九条、小模型比较得了整池（[`Self::small_for_pool`]）。
    fn keep(
        &mut self,
        directory: &Path,
        stamps: [ModelStamp; 2],
        mut scorer: msime_onnx_rerank::OnnxScorer,
        category: &'static str,
        note: String,
        page_ms: f64,
    ) -> AttachedOnnxReranker {
        let pool = self.pool_for(page_ms);
        scorer.set_sentence_pool(pool);
        let scorer = Arc::new(Mutex::new(scorer));
        let detail = format!("{}{note} (sentence pool {pool})", directory.display());
        self.loaded = Some(LoadedOnnxReranker {
            directory: directory.to_path_buf(),
            stamps,
            scorer: Arc::clone(&scorer),
            category,
            detail: detail.clone(),
            page_ms,
        });
        AttachedOnnxReranker {
            scorer,
            category,
            detail,
            page_ms,
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
