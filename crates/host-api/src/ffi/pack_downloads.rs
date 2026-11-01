//! 按需下载的资源包：列出、下载安装、取消和收编。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee. 资源包装在 `<state_root>/resource-packs/<id>/`，state_root 就是 HostOptions 的 `preferences_directory`（Android 上是 `files/bootstrap/state`），host-api 会话在获得焦点时就从那里找到新装好的资源包。下载和收编都是阻塞调用，只能放在工作线程上。

use crate::*;
use msime_client_core::is_bounded_text;
use msime_client_providers::voice::local_models::InstallProgress;
use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, Ordering};

/// 请求 JSON 的长度上限：几个路径和镜像前缀，16 KiB 足够。
const RESOURCE_PACK_REQUEST_LIMIT: usize = 16_384;
/// 一次安装最多接受的镜像前缀个数。
const MAX_SOURCES: usize = 8;
/// 不联网安装时一次最多交来的本地文件数，与本地语音模型的导入相同。
const MAX_IMPORT_FILES: usize = 16;

type ResourcePackTasks = Mutex<HashMap<String, Arc<AtomicBool>>>;

/// 本进程里正在安装、校验、收编或卸载的资源包和它们的取消标记。同一个资源包同时只允许一个：两次安装会互相清掉对方的暂存目录，卸载与安装并行则会删掉刚发布的目录。
#[cfg(not(test))]
fn resource_pack_tasks() -> &'static ResourcePackTasks {
    static TASKS: OnceLock<ResourcePackTasks> = OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 测试里每个线程一张登记表：并行跑的测试各占各的资源包、各发各的「全部取消」，不会撞上别的测试正在登记的同一个资源包。每个测试的登记、安装和取消都在它自己的线程上，所以看到的仍是一张完整的表；每个线程泄漏一张空表，只在测试里。
#[cfg(test)]
fn resource_pack_tasks() -> &'static ResourcePackTasks {
    thread_local! {
        static TASKS: &'static ResourcePackTasks = Box::leak(Box::new(Mutex::new(HashMap::new())));
    }
    TASKS.with(|tasks| *tasks)
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

/// 进度回调收到的阶段名。资源包安装只有下载（从本地文件安装时是导入）、校验和完成；从归档取文件也算校验。
fn progress_phase(stage: &str) -> &'static CStr {
    match stage {
        "download" => c"download",
        "import" => c"import",
        "done" => c"done",
        _ => c"verify",
    }
}

/// C 回调：`(context, phase, done, total)`，phase 是 "download"、"import"、"verify" 或 "done"，只在调用期间有效。
pub type ResourcePackProgress =
    unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char, u64, u64);

/// 每个资源包的安装状态。
///
/// 请求为 `{"state_root": "<absolute dir>"}`，响应值为 `[{"id","state":"missing"|"installed"|"outdated","size","schemes":[...],"import_files":[{"name","url","size"}]}]`。
/// 响应数组中每个可用包对应一项，`import_files` 列出该包离线安装（安装请求中的 `files`）所需文件。
/// 包 ID 为 `japanese`、`language-dictionaries`、`handwriting`、`settled-model`、`grammar-model-wanxiang`、`grammar-model-baguwen`、`grammar-model-baguwen-compact`、`offline-glosses`、`voice-runtime`、`wubi`，以及锁文件固定重签运行库后才提供的 `voice-runtime-macos`。
/// `size` 是下载大小（字节）。已安装的包位于 `<state_root>/resource-packs/<id>/`；查询只读取少量文件属性。
///
/// # Safety
/// `request` 必须指向 `length` 个可读字节；拒绝空指针。
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

/// 主动检查语法模型目录，不下载、不修改本地资源。请求为 JSON 对象
/// state_root 和可选 channel（stable、beta 或 nightly），响应值为
/// [{"pack","source_commit","size"}]。输入宿主不调用此接口。
///
/// # Safety
/// request 必须指向 length 个可读字节；拒绝空指针。
#[no_mangle]
pub unsafe extern "C" fn msime_client_resource_pack_check_grammar_updates(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct CheckRequest {
            state_root: String,
            channel: Option<String>,
        }
        let request: CheckRequest = resource_pack_request(request, length)?;
        let state_root = absolute_directory(&request.state_root, "invalid state root")?;
        let channel = request.channel.as_deref().unwrap_or("stable");
        let channel = msime_client_core::preferences::UpdateChannel::parse(channel)
            .ok_or_else(|| "invalid resource pack request".to_owned())?;
        serde_json::to_value(
            resource_packs::check_grammar_updates(state_root, channel)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    })
}

/// 下载、校验并发布一个资源包。阻塞到结束：只能在工作线程上调用。
///
/// 请求为 `{"state_root": "<absolute dir>", "pack": "<id>", "sources": ["https://mirror/", ...]}`。
/// 可选的 `sources` 按用户镜像顺序尝试，跳过空串，其后依次为项目镜像和编译进来的来源地址。
/// 固定资源按锁文件的长度和 SHA-256 校验，下载中断后使用 HTTP Range 续传；会原地替换的
/// 万象 LTS 则检查响应长度和 `.gram` 格式，并在清单中记录实际摘要。响应值为
/// `{"path": "<state_root>/resource-packs/<id>"}`。
///
/// 提供 `"files": ["<absolute path>", ...]`（最多 16 个）时，经 `resource_packs::import`
/// 从本机导入并忽略 `sources`。固定资源按长度和 SHA-256 匹配，万象 LTS 按 `.gram` 格式
/// 匹配并记录实际摘要；下载阶段报告为 `import`，缺失文件报 `local_model_import_missing: <name>`，
/// 无法读取报 `local_model_import_unreadable: <detail>`。
///
/// `progress` 可为 null，在调用线程上收到 `(context, phase, done, total)`，阶段为
/// "download"、"import"、"verify"、"done"，阶段字符串只在回调期间有效。
/// 首次进度在任务登记后、任何文件或网络 I/O 前发送，字节数为 0，总量为资源包大小；
/// 回调可重入 `msime_client_resource_pack_cancel`，取消后返回 "local_model_cancelled"。
/// 每个资源包同时只允许一个安装、校验、收编或卸载，否则返回 "resource_pack_busy"。
/// 其他错误为 "resource_pack_unknown"、"invalid resource pack request"、"invalid state root"
/// 或 "local_model_*"（network、http_status、size_mismatch、checksum_mismatch、format_mismatch、unsafe_archive、
/// missing_file、io、invalid_mirror、invalid_root）。
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
            channel: Option<String>,
            #[serde(default)]
            sources: Vec<String>,
            #[serde(default)]
            files: Option<Vec<String>>,
        }
        let request: InstallRequest = resource_pack_request(request, length)?;
        let state_root = absolute_directory(&request.state_root, "invalid state root")?;
        let pack = known_resource_pack(&request.pack)?;
        let channel = msime_client_core::preferences::UpdateChannel::parse(
            request.channel.as_deref().unwrap_or("stable"),
        )
        .ok_or_else(|| "invalid resource pack request".to_owned())?;
        if request.sources.len() > MAX_SOURCES {
            return Err("invalid resource pack request".into());
        }
        let files = match &request.files {
            Some(files) => {
                if files.len() > MAX_IMPORT_FILES
                    || files
                        .iter()
                        .any(|file| !is_bounded_text(file, 4096) || !Path::new(file).is_absolute())
                {
                    return Err("invalid resource pack request".into());
                }
                Some(
                    files
                        .iter()
                        .map(std::path::PathBuf::from)
                        .collect::<Vec<_>>(),
                )
            }
            None => None,
        };
        let sources: Vec<&str> = request.sources.iter().map(String::as_str).collect();
        let task = Task::register(pack)?;
        let mut report = |event: InstallProgress| {
            if let Some(callback) = progress {
                let phase = progress_phase(event.stage);
                // SAFETY: 调用方保证回调在本次调用期间有效且不展开栈；阶段字符串为静态值。
                unsafe {
                    callback(context, phase.as_ptr(), event.downloaded, event.total);
                }
            }
        };
        // 登记之后、访问文件或网络之前通知宿主，允许回调重入取消而不丢失请求。
        report(InstallProgress {
            stage: if files.is_some() {
                "import"
            } else {
                "download"
            },
            downloaded: 0,
            total: pack.size(),
        });
        if task.cancel.load(Ordering::Relaxed) {
            return Err("local_model_cancelled".into());
        }
        let path = match &files {
            Some(files) => {
                resource_packs::import(state_root, pack, files, &mut report, &task.cancel)
            }
            None => resource_packs::install_for_channel(
                state_root,
                pack,
                channel,
                &sources,
                &mut report,
                &task.cancel,
            ),
        }
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

/// 卸载一个已发布的资源包。阻塞只做一次目录删除。
///
/// Request `{"state_root": "<absolute dir>", "pack": "<id>"}`；response value `{"removed": <bool>}`，`false` 表示这个 state_root 下本来就没装。删除之前释放资源包钉住文件在引擎侧的共享内存映射（目前只有 `.gram` 语法模型会被 `GramDb::shared` 缓存；其他包的文件按路径查缓存是空跳），否则 `.gram` 被 mmap 占着时目录删除在 Windows 上会失败。未发布目录、暂存目录和符号链接都按「没装」处理，不会动。
///
/// 卸载与安装、校验、收编共用同一张登记表：本进程里同一个资源包正在安装、校验、收编或卸载时返回 "resource_pack_busy"，什么也不删；卸载进行中发起的安装同样返回 "resource_pack_busy"。
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_resource_pack_remove(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct RemoveRequest {
            state_root: String,
            pack: String,
        }
        let request: RemoveRequest = resource_pack_request(request, length)?;
        let state_root = absolute_directory(&request.state_root, "invalid state root")?;
        let pack = known_resource_pack(&request.pack)?;
        let _task = Task::register(pack)?;
        for artifact in &pack.set().artifacts {
            crate::evict_grammar_model(
                &resource_packs::root(state_root)
                    .join(pack.id())
                    .join(&*artifact.name),
            );
        }
        let removed =
            resource_packs::remove(state_root, pack).map_err(|error| error.to_string())?;
        Ok(json!({ "removed": removed }))
    })
}

/// 在执行资源包代码前重新校验固定文件的完整内容，返回只读使用的目录。
/// 请求为 `{"state_root":"<absolute dir>","pack":"<id>"}`；不修复或安装文件。
///
/// # Safety
/// `request` 必须指向 `length` 个可读字节；拒绝空指针。
#[no_mangle]
pub unsafe extern "C" fn msime_client_resource_pack_verify(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct VerifyRequest {
            state_root: String,
            pack: String,
        }
        let request: VerifyRequest = resource_pack_request(request, length)?;
        let root = absolute_directory(&request.state_root, "invalid state root")?;
        let pack = known_resource_pack(&request.pack)?;
        let _task = Task::register(pack)?;
        let path =
            resource_packs::verify_installed(root, pack).map_err(|error| error.to_string())?;
        Ok(json!({ "path": path.to_string_lossy() }))
    })
}

/// 把升级前已经解压在本机的文件收编为已安装的资源包，不重新下载。阻塞（要哈希整组文件），只能在工作线程上调用。
///
/// Request `{"state_root": "<absolute dir>", "pack": "<id>", "source": "<absolute dir>"}`. The pack's files are renamed out of `source` into a staging directory on the same filesystem (no copy), verified there against the compiled lock and published as `<state_root>/resource-packs/<id>`; other files in `source` are untouched. On any failure every moved file is renamed back and nothing is published, so the caller can fall back to downloading. If the process dies after the renames and before publishing, the next adoption or install of the same pack first renames the stranded files back into the recorded `source`. A pack already installed with the same bytes is left as it is and `source` is not touched. Response value `{"path": "<state_root>/resource-packs/<id>"}`. Errors as for install, plus "invalid source"; "resource_pack_busy" while the same pack is installing.
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
        // macOS 语音运行库只在锁文件固定了重签的运行库之后才列出。
        let mut expected = vec![
            "japanese",
            "language-dictionaries",
            "handwriting",
            "settled-model",
            "grammar-model-wanxiang",
            "grammar-model-baguwen",
            "grammar-model-baguwen-compact",
            "offline-glosses",
            "pronunciations",
            "target-pronunciations",
            "voice-runtime",
            "wubi",
        ];
        if ResourcePack::MacosVoiceRuntime.pinned() {
            expected.push("voice-runtime-macos");
        }
        if ResourcePack::AndroidOcrModule.pinned() {
            expected.push("ocr-module-android");
        }
        assert_eq!(ids, expected);
        for pack in listed["value"].as_array().unwrap() {
            assert_eq!(pack["state"], "missing");
            assert!(pack["size"].as_u64().unwrap() > 0);
            assert!(pack["schemes"].is_array());
            assert!(
                !pack["import_files"].as_array().unwrap().is_empty(),
                "{pack}"
            );
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
    fn grammar_update_check_rejects_invalid_requests_before_network() {
        let state = tempfile::tempdir().unwrap();
        for request in [
            json!({"state_root": "relative", "channel": "stable"}),
            json!({"state_root": state.path(), "channel": "unknown"}),
            json!({}),
        ] {
            assert_eq!(
                call(msime_client_resource_pack_check_grammar_updates, &request)["ok"],
                false
            );
        }
    }

    #[test]
    fn install_from_files_validates_the_list_and_names_a_missing_file() {
        let state = tempfile::tempdir().unwrap();
        let relative = install(&json!({
            "state_root": state.path(),
            "pack": "voice-runtime",
            "files": ["runtime.aar"],
        }));
        assert_eq!(relative["error"], "invalid resource pack request");
        let many = install(&json!({
            "state_root": state.path(),
            "pack": "voice-runtime",
            "files": vec!["/picked/file"; MAX_IMPORT_FILES + 1],
        }));
        assert_eq!(many["error"], "invalid resource pack request");
        // Nothing of the right size was picked: the error names the upstream file the user has to download.
        let unrelated = state.path().join("unrelated.bin");
        std::fs::write(&unrelated, b"not the archive").unwrap();
        let missing = install(&json!({
            "state_root": state.path(),
            "pack": "voice-runtime",
            "files": [unrelated],
        }));
        let error = missing["error"].as_str().unwrap();
        assert!(
            error.starts_with("local_model_import_missing: "),
            "{missing}"
        );
        assert!(error.ends_with(".aar"), "{missing}");
        // An empty list is still an import, never a download: the Android host sends it when none of the picked documents had a usable size, and the user must see which file is missing rather than a silent network install.
        let empty = install(&json!({
            "state_root": state.path(),
            "pack": "voice-runtime",
            "files": [],
        }));
        let error = empty["error"].as_str().unwrap();
        assert!(error.starts_with("local_model_import_missing: "), "{empty}");
        assert!(error.ends_with(".aar"), "{empty}");
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
        // 登记表在测试里按线程分开（见 resource_pack_tasks），别的测试同时登记 voice-runtime 或发「全部取消」都碰不到这里。
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

    /// 卸载与安装互斥：安装进行中卸载返回忙且不动已发布目录，卸载占住登记时安装同样返回忙。
    #[test]
    fn remove_and_install_exclude_each_other() {
        let state = tempfile::tempdir().unwrap();
        let published = resource_packs::root(state.path()).join(ResourcePack::Handwriting.id());
        std::fs::create_dir_all(&published).unwrap();
        let request = json!({ "state_root": state.path(), "pack": "handwriting" });

        let installing = Task::register(ResourcePack::Handwriting).unwrap();
        let busy = call(msime_client_resource_pack_remove, &request);
        assert_eq!(busy["error"], "resource_pack_busy");
        assert!(published.is_dir());
        drop(installing);

        let removing = Task::register(ResourcePack::Handwriting).unwrap();
        let install_busy = install(&request);
        assert_eq!(install_busy["error"], "resource_pack_busy");
        drop(removing);

        let other = call(
            msime_client_resource_pack_remove,
            &json!({ "state_root": state.path(), "pack": "voice-runtime" }),
        );
        assert_eq!(other["value"]["removed"], false);
        assert!(!resource_pack_tasks()
            .lock()
            .unwrap()
            .contains_key("handwriting"));
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
    fn initial_progress_can_cancel_a_registered_install_before_any_io() {
        unsafe extern "C" fn cancel_on_progress(
            context: *mut std::ffi::c_void,
            phase: *const c_char,
            done: u64,
            total: u64,
        ) {
            // 回调只记录参数并重入取消入口，断言留在调用完成后，避免跨 C 边界 panic。
            let observed = unsafe { &mut *context.cast::<Vec<(String, u64, u64)>>() };
            let phase = unsafe { CStr::from_ptr(phase) }
                .to_string_lossy()
                .into_owned();
            observed.push((phase, done, total));
            let pack = b"japanese";
            unsafe { msime_client_resource_pack_cancel(pack.as_ptr(), pack.len()) };
        }
        for from_files in [false, true] {
            let state = tempfile::tempdir().unwrap();
            let mut request = json!({ "state_root": state.path(), "pack": "japanese" });
            if from_files {
                request["files"] = json!([]);
            } else {
                // 即使初始回调回归，明文镜像也会在联网前被拒绝，测试不会下载真实资源。
                request["sources"] = json!(["http://synthetic.invalid/"]);
            }
            let request = serde_json::to_vec(&request).unwrap();
            let mut observed: Vec<(String, u64, u64)> = Vec::new();
            let result = read(unsafe {
                msime_client_resource_pack_install(
                    request.as_ptr(),
                    request.len(),
                    Some(cancel_on_progress),
                    (&mut observed as *mut Vec<(String, u64, u64)>).cast(),
                )
            });
            assert_eq!(result["error"], "local_model_cancelled", "{result}");
            let phase = if from_files { "import" } else { "download" };
            assert_eq!(
                observed,
                vec![(phase.into(), 0, ResourcePack::Japanese.size())]
            );
            assert!(!resource_packs::root(state.path()).exists());
            assert!(!resource_pack_tasks()
                .lock()
                .unwrap()
                .contains_key("japanese"));
        }
    }

    #[test]
    fn progress_phases_match_the_contract_names() {
        assert_eq!(progress_phase("download"), c"download");
        assert_eq!(progress_phase("import"), c"import");
        assert_eq!(progress_phase("verify"), c"verify");
        assert_eq!(progress_phase("extract"), c"verify");
        assert_eq!(progress_phase("done"), c"done");
    }
}

#[cfg(test)]
mod registry_tests;
