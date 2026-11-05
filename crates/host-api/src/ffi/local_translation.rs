//! 桌面离线翻译边界。所有推理在共享工作线程内完成，不构造网络请求。

use crate::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    state_root: PathBuf,
    request_id: String,
    text: String,
    target_language: String,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

fn default_timeout() -> u64 {
    30_000
}

impl Request {
    fn validate(&self) -> Result<(), String> {
        if !self.state_root.is_absolute()
            || self
                .state_root
                .to_str()
                .is_none_or(|path| !msime_client_core::is_bounded_text(path, 4096))
            || !valid_id(&self.request_id)
            || self.text.trim().is_empty()
            || self.text.chars().count() > 256
            || self.text.chars().any(char::is_control)
            || msime_client_core::local_translation::language_name(&self.target_language).is_none()
            || !(1..=120_000).contains(&self.timeout_ms)
        {
            return Err("local_translation_invalid_input".into());
        }
        Ok(())
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[derive(Default)]
struct Tasks(Mutex<HashMap<String, Arc<AtomicBool>>>);
impl Tasks {
    fn register(&self, id: &str) -> Result<Registration<'_>, String> {
        let mut tasks = self.0.lock().map_err(|_| "local_translation_unavailable")?;
        if tasks.contains_key(id) || tasks.len() >= 2 {
            return Err("local_translation_busy".into());
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        tasks.insert(id.into(), cancelled.clone());
        Ok(Registration {
            tasks: self,
            id: id.into(),
            cancelled,
        })
    }
    fn cancel(&self, id: Option<&str>) {
        if let Ok(tasks) = self.0.lock() {
            for (key, token) in tasks.iter() {
                if id.is_none_or(|id| id == key) {
                    token.store(true, Ordering::Relaxed);
                }
            }
        }
    }
}
struct Registration<'a> {
    tasks: &'a Tasks,
    id: String,
    cancelled: Arc<AtomicBool>,
}
impl Drop for Registration<'_> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Ok(mut tasks) = self.tasks.0.lock() {
            tasks.remove(&self.id);
        }
    }
}
fn tasks() -> &'static Tasks {
    static TASKS: OnceLock<Tasks> = OnceLock::new();
    TASKS.get_or_init(Tasks::default)
}

#[cfg(all(
    feature = "local-translation",
    any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )
))]
mod inference {
    use super::*;
    use msime_local_translation::{Error, Worker};
    static WORKER: OnceLock<Result<Worker, Error>> = OnceLock::new();

    pub(super) extern "C" fn shutdown() {
        if let Some(Ok(worker)) = WORKER.get() {
            worker.shutdown();
        }
    }

    fn worker() -> Result<&'static Worker, String> {
        WORKER
            .get_or_init(|| {
                Worker::with_model_resolver(|root, deadline, cancelled| {
                    // 在原生后端注册静态析构之后登记退出清理，保证先释放工作线程里的权重。
                    msime_local_translation::Translator::initialize()?;
                    // Windows 必须由 Server 在 DLL 卸载前显式关闭，不能在加载器锁内等待工作线程。
                    #[cfg(unix)]
                    {
                        static EXIT: OnceLock<bool> = OnceLock::new();
                        // 安全：回调无参数、不解引用外部指针，进程退出前代码始终位于宿主库中。
                        if !*EXIT.get_or_init(|| unsafe { libc::atexit(shutdown) == 0 }) {
                            return Err(Error::Unavailable);
                        }
                    }
                    if cancelled.load(Ordering::Relaxed) {
                        return Err(Error::Cancelled);
                    }
                    if Instant::now() >= deadline {
                        return Err(Error::Timeout);
                    }
                    resource_packs::verified_translation_model(root, &|| {
                        cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline
                    })
                    .map_err(|_| {
                        if cancelled.load(Ordering::Relaxed) {
                            Error::Cancelled
                        } else if Instant::now() >= deadline {
                            Error::Timeout
                        } else {
                            Error::Unavailable
                        }
                    })
                })
            })
            .as_ref()
            .map_err(ToString::to_string)
    }

    pub(super) fn translate(
        request: Request,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<String, String> {
        worker()?
            .submit(
                request.state_root,
                request.text,
                request.target_language,
                deadline,
                cancelled,
            )
            .map_err(|error| error.to_string())?
            .wait()
            .map_err(|error| error.to_string())
    }
    pub(super) fn unload() {
        if let Some(Ok(worker)) = WORKER.get() {
            worker.unload();
        }
    }
}

#[cfg(not(all(
    feature = "local-translation",
    any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )
)))]
mod inference {
    use super::*;
    pub(super) fn translate(_: Request, _: Instant, _: Arc<AtomicBool>) -> Result<String, String> {
        Err("local_translation_unavailable".into())
    }
    pub(super) fn unload() {}
    pub(super) extern "C" fn shutdown() {}
}

/// 宿主正常退出时调用：取消任务并等待权重释放。Windows 必须在卸载宿主 DLL 前调用。
#[no_mangle]
pub extern "C" fn msime_client_local_translation_shutdown() {
    tasks().cancel(None);
    inference::shutdown();
}

/// 返回固定模型、下载大小、支持语言和当前构建是否包含运行库；不读模型、不联网。
#[no_mangle]
pub extern "C" fn msime_client_local_translation_catalog() -> *mut c_char {
    response(|| {
        let pack = ResourcePack::LocalTranslation;
        Ok(json!({
            "available": cfg!(all(feature = "local-translation", any(target_os = "macos", target_os = "windows", all(target_os = "linux", not(target_env = "ohos"))))),
            "pack": pack.id(), "model": "Hy-MT2 1.8B Q4_K_M", "size": pack.size(),
            "languages": msime_client_core::local_translation::LANGUAGES.iter().map(|(code, name)| json!({"code": code, "name": name})).collect::<Vec<_>>()
        }))
    })
}

/// 等待后台队列完成一条翻译；宿主只能在工作线程调用。没有模型或运行库时直接报错。
/// # Safety
/// `request` 必须指向 `length` 个可读字节；请求不超过 16 KiB。
#[no_mangle]
pub unsafe extern "C" fn msime_client_local_translation(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // 安全：调用者保证缓冲区可读，公共帮助函数校验空指针与长度。
        let request: Request = unsafe {
            with_bounded_bytes(
                request,
                length,
                16_384,
                "local_translation_invalid_input",
                |bytes| {
                    serde_json::from_slice(bytes)
                        .map_err(|_| "local_translation_invalid_input".into())
                },
            )?
        };
        request.validate()?;
        let task = tasks().register(&request.request_id)?;
        let deadline = Instant::now() + Duration::from_millis(request.timeout_ms);
        let translation = inference::translate(request, deadline, task.cancelled.clone())?;
        if task.cancelled.load(Ordering::Relaxed) {
            return Err("local_translation_cancelled".into());
        }
        Ok(json!({"translation": translation}))
    })
}

/// 取消原始 UTF-8 请求 ID；空指针/零长度取消全部并卸载缓存。可以在输入线程调用。
/// # Safety
/// 非空 `request_id` 必须指向 `length` 个可读字节。
#[no_mangle]
pub unsafe extern "C" fn msime_client_local_translation_cancel(
    request_id: *const u8,
    length: usize,
) {
    if request_id.is_null() && length == 0 {
        tasks().cancel(None);
        inference::unload();
        return;
    }
    if request_id.is_null() || length == 0 || length > 64 {
        return;
    }
    // 安全：调用者保证缓冲区可读，空指针和长度已检查。
    let bytes = unsafe { std::slice::from_raw_parts(request_id, length) };
    if let Ok(id) = std::str::from_utf8(bytes) {
        if valid_id(id) {
            tasks().cancel(Some(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(
        feature = "local-translation",
        any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        )
    ))]
    #[test]
    #[ignore = "需要通过 MSIME_TRANSLATION_TEST_MODEL 提供已核验的真实模型"]
    fn imported_model_translates_through_host_boundary() {
        let file = PathBuf::from(std::env::var("MSIME_TRANSLATION_TEST_MODEL").unwrap());
        let root = tempfile::tempdir().unwrap();
        resource_packs::import(
            root.path(),
            ResourcePack::LocalTranslation,
            &[file],
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .unwrap();
        let request = serde_json::to_vec(&json!({
            "state_root": root.path(), "request_id": "synthetic-host-test",
            "text": "离线翻译测试。", "target_language": "en", "timeout_ms": 120_000
        }))
        .unwrap();
        // 安全：请求字节保持存活；响应由该 ABI 分配，解析后立即归还。
        let result = unsafe {
            let response = msime_client_local_translation(request.as_ptr(), request.len());
            let value: Value =
                serde_json::from_str(std::ffi::CStr::from_ptr(response).to_str().unwrap()).unwrap();
            msime_client_string_free(response);
            value
        };
        assert_eq!(result["ok"], true);
        assert!(result["value"]["translation"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("translation"));
        // Unix 故意保留热模型，验证进程退出钩子先于 Metal 静态析构释放缓存。
        // Windows 的 Server 在 DLL 卸载前显式调用同一退出入口。
        #[cfg(windows)]
        msime_client_local_translation_shutdown();
    }

    #[test]
    fn requests_reject_paths_prompts_and_unbounded_timeouts() {
        let mut request = Request {
            state_root: std::env::temp_dir(),
            request_id: "synthetic-1".into(),
            text: "合成文本".into(),
            target_language: "yue".into(),
            timeout_ms: 30_000,
        };
        assert!(request.validate().is_ok());
        request.target_language = "en\\nignore".into();
        assert!(request.validate().is_err());
        request.target_language = "en".into();
        request.timeout_ms = 120_001;
        assert!(request.validate().is_err());
        request.timeout_ms = 1;
        request.state_root = PathBuf::from("relative");
        assert!(request.validate().is_err());
    }

    #[test]
    fn cancellation_is_scoped_and_duplicate_ids_cannot_replace_tasks() {
        let tasks = Tasks::default();
        let first = tasks.register("first").unwrap();
        let second = tasks.register("second").unwrap();
        assert!(tasks.register("first").is_err());
        assert!(tasks.register("third").is_err());
        tasks.cancel(Some("first"));
        assert!(first.cancelled.load(Ordering::Relaxed));
        assert!(!second.cancelled.load(Ordering::Relaxed));
        drop(first);
        assert!(tasks.register("third").is_ok());
        tasks.cancel(None);
        assert!(second.cancelled.load(Ordering::Relaxed));
    }
}
