//! 按需下载的资源包：列出、下载安装、取消和收编。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee. 资源包装在 `<state_root>/resource-packs/<id>/`，state_root 就是 HostOptions 的 `preferences_directory`（Android 上是 `files/bootstrap/state`），host-api 会话在获得焦点时就从那里找到新装好的资源包。下载和收编都是阻塞调用，只能放在工作线程上。

use crate::*;
use msime_client_core::is_bounded_text;
use msime_client_core::voice::local_models::InstallProgress;
use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, Ordering};

/// 请求 JSON 的长度上限：几个路径和镜像前缀，16 KiB 足够。
const RESOURCE_PACK_REQUEST_LIMIT: usize = 16_384;
/// 一次安装最多接受的镜像前缀个数。
const MAX_SOURCES: usize = 8;

/// 本进程里正在安装或收编的资源包和它们的取消标记。同一个资源包同时只允许一个：两次安装会互相清掉对方的暂存目录。
fn resource_pack_tasks() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static TASKS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 在登记表里占住 `pack`，结束时（包括出错和 panic）自动释放。
struct Task {
    pack: ResourcePack,
    cancel: Arc<AtomicBool>,
}

impl Task {
    fn register(pack: ResourcePack) -> Result<Task, String> {
        let mut tasks = resource_pack_tasks()
            .lock()
            .map_err(|_| "internal runtime failure")?;
        if tasks.contains_key(pack.id()) {
            return Err("resource_pack_busy".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        tasks.insert(pack.id().to_owned(), cancel.clone());
        Ok(Task { pack, cancel })
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        if let Ok(mut tasks) = resource_pack_tasks().lock() {
            tasks.remove(self.pack.id());
        }
    }
}

fn resource_pack_request<T: serde::de::DeserializeOwned>(
    request: *const u8,
    length: usize,
) -> Result<T, String> {
    if request.is_null() || length == 0 || length > RESOURCE_PACK_REQUEST_LIMIT {
        return Err("invalid resource pack request".into());
    }
    // SAFETY: the caller contract of every entry point using this guarantees `length` readable bytes; null and size are checked above.
    let bytes = unsafe { std::slice::from_raw_parts(request, length) };
    serde_json::from_slice(bytes).map_err(|_| "invalid resource pack request".to_owned())
}

fn absolute_directory<'a>(path: &'a str, invalid: &'static str) -> Result<&'a Path, String> {
    if !is_bounded_text(path, 4096) || !Path::new(path).is_absolute() {
        return Err(invalid.into());
    }
    Ok(Path::new(path))
}

fn known_resource_pack(id: &str) -> Result<ResourcePack, String> {
    ResourcePack::from_id(id).ok_or_else(|| "resource_pack_unknown".to_owned())
}

/// 进度回调收到的阶段名。资源包安装只有下载、校验和完成三段；从归档取文件也算校验。
fn progress_phase(stage: &str) -> &'static CStr {
    match stage {
        "download" => c"download",
        "done" => c"done",
        _ => c"verify",
    }
}

/// C 回调：`(context, phase, done, total)`，phase 是 "download"、"verify" 或 "done"，只在调用期间有效。
pub type ResourcePackProgress =
    unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char, u64, u64);

/// 每个资源包的安装状态。
///
/// Request `{"state_root": "<absolute dir>"}`; response value `[{"id","state":"missing"|"installed"|"outdated","size","schemes":[...]}]`, one entry per pack id (`japanese`, `language-dictionaries`, `handwriting`, `settled-model`, `offline-glosses`, `voice-runtime`). `size` is the download size in bytes. An installed pack lives in `<state_root>/resource-packs/<id>/`. Reads only a few file attributes.
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_resource_packs(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct ListRequest {
            state_root: String,
        }
        let request: ListRequest = resource_pack_request(request, length)?;
        let state_root = absolute_directory(&request.state_root, "invalid state root")?;
        serde_json::to_value(resource_packs::list(state_root)).map_err(|error| error.to_string())
    })
}

/// 下载、校验并发布一个资源包。阻塞到结束：只能在工作线程上调用。
///
/// Request `{"state_root": "<absolute dir>", "pack": "<id>", "sources": ["https://mirror/", ...]}`; `sources` is optional, the user's mirror prefixes in the order to try (empty strings are skipped). After them come the project's own mirrors and finally the URL in the compiled lock; every byte is checked against the lock's size and SHA-256 whichever source served it, and an unfinished file is resumed with an HTTP Range request on the next call. Response value `{"path": "<state_root>/resource-packs/<id>"}`.
///
/// `progress` (may be null) is called on the calling thread with `(context, phase, done, total)`, phase one of "download", "verify", "done"; the phase string is only valid during the call. `msime_client_resource_pack_cancel` stops it from any thread and the call then fails with "local_model_cancelled". One install or adoption per pack at a time; another fails with "resource_pack_busy". Other failures are "resource_pack_unknown", "invalid resource pack request", "invalid state root" or a "local_model_*" code (network, http_status, size_mismatch, checksum_mismatch, unsafe_archive, missing_file, io, invalid_mirror, invalid_root).
///
/// # Safety
/// `request` must point to `length` readable bytes. `progress` must stay valid for the call, must copy the phase string before returning and must not unwind.
#[no_mangle]
pub unsafe extern "C" fn msime_client_resource_pack_install(
    request: *const u8,
    length: usize,
    progress: Option<ResourcePackProgress>,
    context: *mut std::ffi::c_void,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct InstallRequest {
            state_root: String,
            pack: String,
            #[serde(default)]
            sources: Vec<String>,
        }
        let request: InstallRequest = resource_pack_request(request, length)?;
        let state_root = absolute_directory(&request.state_root, "invalid state root")?;
        let pack = known_resource_pack(&request.pack)?;
        if request.sources.len() > MAX_SOURCES {
            return Err("invalid resource pack request".into());
        }
        let sources: Vec<&str> = request.sources.iter().map(String::as_str).collect();
        let task = Task::register(pack)?;
        let mut report = |event: InstallProgress| {
            if let Some(callback) = progress {
                let phase = progress_phase(event.stage);
                // SAFETY: the caller guarantees the callback stays valid for this call and does not unwind; the phase string is static.
                unsafe {
                    callback(context, phase.as_ptr(), event.downloaded, event.total);
                }
            }
        };
        let path = resource_packs::install(state_root, pack, &sources, &mut report, &task.cancel)
            .map_err(|error| error.to_string())?;
        Ok(json!({ "path": path.to_string_lossy() }))
    })
}

/// 让正在运行的安装尽快停下。`pack` 是资源包 id 的 UTF-8 字节（不是 JSON）；为 null 或长度为 0 时停下本进程里所有资源包的安装。安装调用在下一个数据块之间发现后返回 "local_model_cancelled"，已下载的部分留着下次续传。可以在任何线程调用；没有在安装的资源包什么也不发生。
///
/// # Safety
/// `pack` must be null or point to `length` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn msime_client_resource_pack_cancel(pack: *const u8, length: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let id = if pack.is_null() || length == 0 {
            None
        } else if length > 256 {
            return;
        } else {
            // SAFETY: the caller guarantees `length` readable bytes; null and size are checked above.
            match std::str::from_utf8(unsafe { std::slice::from_raw_parts(pack, length) }) {
                Ok(id) => Some(id),
                Err(_) => return,
            }
        };
        let Ok(tasks) = resource_pack_tasks().lock() else {
            return;
        };
        for (running, cancel) in tasks.iter() {
            if id.is_none_or(|id| id == running) {
                cancel.store(true, Ordering::Relaxed);
            }
        }
    }));
}

/// 把升级前已经解压在本机的文件收编为已安装的资源包，不重新下载。阻塞（要哈希整组文件），只能在工作线程上调用。
///
/// Request `{"state_root": "<absolute dir>", "pack": "<id>", "source": "<absolute dir>"}`. The pack's files are renamed out of `source` into a staging directory on the same filesystem (no copy), verified there against the compiled lock and published as `<state_root>/resource-packs/<id>`; other files in `source` are untouched. On any failure every moved file is renamed back and nothing is published, so the caller can fall back to downloading. A pack already installed with the same bytes is left as it is and `source` is not touched. Response value `{"path": "<state_root>/resource-packs/<id>"}`. Errors as for install, plus "invalid source"; "resource_pack_busy" while the same pack is installing.
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_resource_pack_adopt(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct AdoptRequest {
            state_root: String,
            pack: String,
            source: String,
        }
        let request: AdoptRequest = resource_pack_request(request, length)?;
        let state_root = absolute_directory(&request.state_root, "invalid state root")?;
        let source = absolute_directory(&request.source, "invalid source")?;
        let pack = known_resource_pack(&request.pack)?;
        let _task = Task::register(pack)?;
        let path =
            resource_packs::adopt(state_root, pack, source).map_err(|error| error.to_string())?;
        Ok(json!({ "path": path.to_string_lossy() }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::read;

    fn call(
        entry: unsafe extern "C" fn(*const u8, usize) -> *mut c_char,
        request: &Value,
    ) -> Value {
        let bytes = request.to_string();
        read(unsafe { entry(bytes.as_ptr(), bytes.len()) })
    }

    fn install(request: &Value) -> Value {
        let bytes = request.to_string();
        read(unsafe {
            msime_client_resource_pack_install(
                bytes.as_ptr(),
                bytes.len(),
                None,
                std::ptr::null_mut(),
            )
        })
    }

    #[test]
    fn listing_reports_every_pack_and_checks_the_state_root() {
        let state = tempfile::tempdir().unwrap();
        let listed = call(
            msime_client_resource_packs,
            &json!({ "state_root": state.path() }),
        );
        assert_eq!(listed["ok"], true, "{listed}");
        let ids: Vec<&str> = listed["value"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pack| pack["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            [
                "japanese",
                "language-dictionaries",
                "handwriting",
                "settled-model",
                "offline-glosses",
                "voice-runtime"
            ]
        );
        for pack in listed["value"].as_array().unwrap() {
            assert_eq!(pack["state"], "missing");
            assert!(pack["size"].as_u64().unwrap() > 0);
            assert!(pack["schemes"].is_array());
        }
        assert_eq!(
            listed["value"][0]["schemes"],
            json!(["japanese"]),
            "{listed}"
        );

        for request in [json!({ "state_root": "state" }), json!({}), json!([])] {
            assert_eq!(call(msime_client_resource_packs, &request)["ok"], false);
        }
        assert_eq!(
            read(unsafe { msime_client_resource_packs(std::ptr::null(), 4) })["ok"],
            false
        );
    }

    #[test]
    fn install_rejects_bad_requests_before_touching_the_network() {
        let state = tempfile::tempdir().unwrap();
        let unknown = install(&json!({ "state_root": state.path(), "pack": "voice" }));
        assert_eq!(unknown["error"], "resource_pack_unknown");
        let relative = install(&json!({ "state_root": "state", "pack": "japanese" }));
        assert_eq!(relative["error"], "invalid state root");
        let plain = install(&json!({
            "state_root": state.path(),
            "pack": "offline-glosses",
            "sources": ["https://mirror.example.test/", "http://plain.example.test/"],
        }));
        assert_eq!(plain["error"], "local_model_invalid_mirror");
        let many = install(&json!({
            "state_root": state.path(),
            "pack": "offline-glosses",
            "sources": vec!["https://mirror.example.test/"; MAX_SOURCES + 1],
        }));
        assert_eq!(many["error"], "invalid resource pack request");
        // 被拒绝的请求不留下登记，也不建任何目录。
        assert!(!resource_pack_tasks()
            .lock()
            .unwrap()
            .contains_key("offline-glosses"));
        assert!(!resource_packs::root(state.path()).exists());
    }

    /// 同一个资源包同时只能有一个安装或收编；取消只停下点名的那个，空请求停下全部。
    #[test]
    fn one_task_per_pack_and_cancel_reaches_the_running_one() {
        let state = tempfile::tempdir().unwrap();
        // 各个测试并行运行、共用登记表，这里只用别的测试不碰的两个资源包。
        let running = Task::register(ResourcePack::VoiceRuntime).unwrap();
        let other = Task::register(ResourcePack::Handwriting).unwrap();
        let busy = install(&json!({ "state_root": state.path(), "pack": "voice-runtime" }));
        assert_eq!(busy["error"], "resource_pack_busy");
        let adopt = call(
            msime_client_resource_pack_adopt,
            &json!({ "state_root": state.path(), "pack": "voice-runtime", "source": state.path() }),
        );
        assert_eq!(adopt["error"], "resource_pack_busy");

        let id = b"voice-runtime";
        unsafe { msime_client_resource_pack_cancel(id.as_ptr(), id.len()) };
        assert!(running.cancel.load(Ordering::Relaxed));
        assert!(!other.cancel.load(Ordering::Relaxed));
        unsafe { msime_client_resource_pack_cancel(b"\xff".as_ptr(), 1) };
        unsafe { msime_client_resource_pack_cancel(std::ptr::null(), 0) };
        assert!(other.cancel.load(Ordering::Relaxed));

        drop(running);
        drop(other);
        assert!(!resource_pack_tasks()
            .lock()
            .unwrap()
            .contains_key("voice-runtime"));
    }

    #[test]
    fn adopting_bytes_that_do_not_match_puts_them_back() {
        let state = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let pack = ResourcePack::LanguageDictionaries;
        for artifact in &pack.set().artifacts {
            std::fs::write(source.path().join(&artifact.name), b"older bytes").unwrap();
        }
        let adopted = call(
            msime_client_resource_pack_adopt,
            &json!({ "state_root": state.path(), "pack": pack.id(), "source": source.path() }),
        );
        assert_eq!(adopted["ok"], false, "{adopted}");
        assert!(adopted["error"]
            .as_str()
            .unwrap()
            .starts_with("local_model_size_mismatch"));
        for artifact in &pack.set().artifacts {
            assert!(source.path().join(&artifact.name).is_file());
        }
        let relative = call(
            msime_client_resource_pack_adopt,
            &json!({ "state_root": state.path(), "pack": pack.id(), "source": "resources" }),
        );
        assert_eq!(relative["error"], "invalid source");
        let missing_source = call(
            msime_client_resource_pack_adopt,
            &json!({ "state_root": state.path(), "pack": pack.id() }),
        );
        assert_eq!(missing_source["error"], "invalid resource pack request");
    }

    #[test]
    fn progress_phases_are_the_three_the_contract_names() {
        assert_eq!(progress_phase("download"), c"download");
        assert_eq!(progress_phase("verify"), c"verify");
        assert_eq!(progress_phase("extract"), c"verify");
        assert_eq!(progress_phase("done"), c"done");
    }
}
