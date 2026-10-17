//! AI 键盘皮肤：`msime_client_ai_skin_generate` 走完整条生成（模型目录、对话出三套设计、三张插画），`msime_client_ai_skin_cancel` 取消进行中的生成，`msime_client_ai_skin_job` 给自己轮询的宿主新建、读取和删除单个插画任务。规则、校验和清理都在 client-services 的 [`BackendAiSkinService`]，与 Tauri 移动端的 `ai_skin_generate` 相同；登录账号优先、没有登录时用本机匿名账号（#8201 的决定 16），Tauri 那边仍只用登录账号。
//!
//! 会话按 `directory` 或 `session_store` 选，与 `msime_client_account` 相同：按目录时每个目录一个服务对象，经同一个账号会话对象和同一目录的匿名会话对象认证；注入时用注册时建好的那一个。取消登记表是进程级的一张，按宿主给的请求 id 登记。
//!
//! 属于 C ABI 的一部分，这些薄封装保证什么见父模块。

use super::account::{account_failure_response, anonymous_session_for, session_for, Failure};
use super::session_store::{session_source, SessionStorage, Source};
use super::with_bounded_bytes;
use crate::*;
use msime_client_services::account::{AccountError, BackendAccountClient};
use msime_client_services::skin::ai::{AiSkinFailure, AiSkinRequests, BackendAiSkinService};
use std::ffi::c_void;

pub(super) type AiSkinService = BackendAiSkinService<BackendAccountClient, SessionStorage>;

/// 生成请求的上限：提示词最多 500 个字符，64 KiB 绰绰有余。
const MAX_REQUEST_BYTES: usize = 64 * 1024;
/// 取消请求只有一个 id。
const MAX_CANCEL_BYTES: usize = 4 * 1024;
const MAX_DIRECTORY_BYTES: usize = 16 * 1024;
/// 请求 id 是不超过 96 字节的 ASCII 标识（字母、数字、`-`、`_`），与 Tauri 的 `ai_skin_generate` 相同。
const MAX_REQUEST_ID_BYTES: usize = 96;
const INVALID: &str = "ai_skin_invalid";

/// 头文件的 `msime_client_ai_skin_progress_callback`。
pub type AiSkinProgressCallback = unsafe extern "C" fn(*mut c_void, u32);

static REQUESTS: OnceLock<AiSkinRequests> = OnceLock::new();
/// 每个账号目录一个服务对象。只在取对象时持锁，生成本身在锁外进行。
static SERVICES: OnceLock<Mutex<HashMap<PathBuf, Arc<AiSkinService>>>> = OnceLock::new();

fn requests() -> &'static AiSkinRequests {
    REQUESTS.get_or_init(AiSkinRequests::default)
}

fn absolute_directory(value: &str) -> Result<&Path, String> {
    let path = Path::new(value);
    if value.len() > MAX_DIRECTORY_BYTES || !path.is_absolute() {
        return Err(INVALID.into());
    }
    Ok(path)
}

/// 请求选的会话上的服务。`directory` 和 `session_store` 恰好给一个，`legacy_directory` 只能与 `directory` 一起给，否则是 `ai_skin_invalid`。
fn service(
    directory: Option<&str>,
    legacy: Option<&str>,
    session_store: Option<u64>,
) -> Result<Arc<AiSkinService>, String> {
    match session_source(directory, legacy.is_some(), session_store).ok_or(INVALID)? {
        Source::Store(entry) => Ok(Arc::clone(&entry.ai_skin)),
        Source::Directory(directory) => {
            let directory = absolute_directory(directory)?;
            let legacy = legacy.map(absolute_directory).transpose()?;
            let account = |error: AccountError| error.code().to_owned();
            let services = SERVICES.get_or_init(|| Mutex::new(HashMap::new()));
            let mut services = services
                .lock()
                .map_err(|_| AccountError::Unavailable.code().to_owned())?;
            if let Some(existing) = services.get(directory) {
                return Ok(Arc::clone(existing));
            }
            let session = session_for(directory, legacy).map_err(account)?;
            let anonymous = anonymous_session_for(directory).map_err(account)?;
            let created = Arc::new(BackendAiSkinService::with_anonymous(
                BackendAccountClient::new().map_err(account)?,
                session,
                anonymous,
            ));
            services.insert(directory.to_path_buf(), Arc::clone(&created));
            Ok(created)
        }
    }
}

/// 生成和任务请求的失败：AI 皮肤自己的码，或者带 `retry_after_seconds` 的账号错误码（[`AccountFailure::code`](msime_client_services::account::AccountFailure::code)）。
fn ai_skin_failure(failure: AiSkinFailure) -> Failure {
    match failure {
        AiSkinFailure::Cancelled => Failure::from("ai_skin_cancelled".to_owned()),
        AiSkinFailure::InvalidResponse => Failure::from(INVALID.to_owned()),
        AiSkinFailure::Account(failure) => failure.into(),
    }
}

fn valid_request_id(id: &str) -> bool {
    msime_client_core::is_bounded_ascii_identifier(id, MAX_REQUEST_ID_BYTES)
}

/// 生成结束时注销请求 id，出错或 panic 时也一样。
struct Registered<'a>(&'a str);

impl Drop for Registered<'_> {
    fn drop(&mut self) {
        requests().finish(self.0);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerateRequest {
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    legacy_directory: Option<String>,
    #[serde(default)]
    session_store: Option<u64>,
    request_id: String,
    prompt: String,
}

/// 生成三套 AI 键盘皮肤。请求、进度回调的约定、返回值和错误码见头文件。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节，NULL 被拒绝。`progress` 可为 NULL；否则按头文件的约定在本线程上带着 `context` 调用。
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_skin_generate(
    request: *const u8,
    length: usize,
    progress: Option<AiSkinProgressCallback>,
    context: *mut c_void,
) -> *mut c_char {
    account_failure_response(|| {
        // SAFETY: 由调用方契约保证；长度先经过上限检查。
        let request: GenerateRequest = unsafe {
            with_bounded_bytes(request, length, MAX_REQUEST_BYTES, INVALID, |bytes| {
                serde_json::from_slice(bytes).map_err(|_| INVALID.to_owned())
            })
        }?;
        if !valid_request_id(&request.request_id) {
            return Err(INVALID.to_owned().into());
        }
        let service = service(
            request.directory.as_deref(),
            request.legacy_directory.as_deref(),
            request.session_store,
        )?;
        let cancelled = requests()
            .begin(&request.request_id)
            .map_err(|code| Failure::from(code.to_owned()))?;
        let _registered = Registered(&request.request_id);
        let proposals = service
            .generate_detailed(&request.prompt, &cancelled, |completed| {
                if let Some(progress) = progress {
                    let completed = u32::try_from(completed).unwrap_or(u32::MAX);
                    // SAFETY: 宿主给的回调和 context 在本次调用期间有效；只在调用线程上依次调用。
                    unsafe { progress(context, completed) }
                }
            })
            .map_err(ai_skin_failure)?;
        serde_json::to_value(proposals)
            .map_err(|_| Failure::from(AccountError::Unavailable.code().to_owned()))
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelRequest {
    request_id: String,
}

/// 取消一次进行中的生成，Value 是当时是否有一个在跑。任意线程上调用，立即返回；也可以在进度回调里调用。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节，NULL 被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_skin_cancel(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由调用方契约保证；长度先经过上限检查。
        let request: CancelRequest = unsafe {
            with_bounded_bytes(request, length, MAX_CANCEL_BYTES, INVALID, |bytes| {
                serde_json::from_slice(bytes).map_err(|_| INVALID.to_owned())
            })
        }?;
        if !valid_request_id(&request.request_id) {
            return Err(INVALID.into());
        }
        Ok(Value::Bool(requests().cancel(&request.request_id)?))
    })
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum JobAction {
    Create { prompt: String },
    Get { id: String },
    Delete { id: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JobRequest {
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    legacy_directory: Option<String>,
    #[serde(default)]
    session_store: Option<u64>,
    action: JobAction,
}

/// 单个插画任务：新建、读取或删除。请求、返回值和错误码见头文件。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节，NULL 被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_skin_job(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    account_failure_response(|| {
        // SAFETY: 由调用方契约保证；长度先经过上限检查。
        let request: JobRequest = unsafe {
            with_bounded_bytes(request, length, MAX_REQUEST_BYTES, INVALID, |bytes| {
                serde_json::from_slice(bytes).map_err(|_| INVALID.to_owned())
            })
        }?;
        let service = service(
            request.directory.as_deref(),
            request.legacy_directory.as_deref(),
            request.session_store,
        )?;
        let job = match request.action {
            JobAction::Create { prompt } => service.create_artwork_job(&prompt),
            JobAction::Get { id } => service.artwork_job(&id),
            JobAction::Delete { id } => {
                service.delete_artwork_job(&id).map_err(ai_skin_failure)?;
                return Ok(Value::Null);
            }
        }
        .map_err(ai_skin_failure)?;
        serde_json::to_value(job)
            .map_err(|_| Failure::from(AccountError::Unavailable.code().to_owned()))
    })
}

#[cfg(test)]
mod failure_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    unsafe fn text(raw: *mut c_char) -> Value {
        // SAFETY: 调用方给的是本库返回、尚未释放的字符串。
        let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_owned();
        // SAFETY: 同上，只释放一次。
        unsafe { msime_client_string_free(raw) };
        serde_json::from_str(&text).unwrap()
    }

    fn generate(request: Value) -> Value {
        let bytes = request.to_string();
        // SAFETY: 缓冲区在调用期间有效，返回的字符串在 `text` 里释放。
        unsafe {
            text(msime_client_ai_skin_generate(
                bytes.as_ptr(),
                bytes.len(),
                None,
                std::ptr::null_mut(),
            ))
        }
    }

    fn cancel(request: Value) -> Value {
        let bytes = request.to_string();
        // SAFETY: 同上。
        unsafe { text(msime_client_ai_skin_cancel(bytes.as_ptr(), bytes.len())) }
    }

    fn job(request: Value) -> Value {
        let bytes = request.to_string();
        // SAFETY: 同上。
        unsafe { text(msime_client_ai_skin_job(bytes.as_ptr(), bytes.len())) }
    }

    #[test]
    fn bad_requests_fail_before_the_network() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        let invalid = json!({ "ok": false, "error": "ai_skin_invalid" });
        for request in [
            json!({ "directory": directory, "request_id": "bad id", "prompt": "合成" }),
            json!({ "directory": directory, "request_id": "a".repeat(97), "prompt": "合成" }),
            json!({ "request_id": "synthetic", "prompt": "合成" }),
            json!({ "directory": "relative", "request_id": "synthetic", "prompt": "合成" }),
            json!({ "directory": directory, "session_store": u64::MAX, "request_id": "synthetic", "prompt": "合成" }),
            json!({ "directory": directory, "request_id": "synthetic", "prompt": "合成", "extra": 1 }),
        ] {
            assert_eq!(generate(request.clone()), invalid, "{request}");
        }
        // 提示词为空或超过 500 个字符时在取令牌之前就结束。
        for prompt in [String::new(), "字".repeat(501)] {
            assert_eq!(
                generate(
                    json!({ "directory": directory, "request_id": "synthetic", "prompt": prompt })
                ),
                invalid
            );
        }
        assert_eq!(cancel(json!({ "request_id": "bad id" })), invalid);
        for action in [
            json!({ "operation": "create", "prompt": "  " }),
            json!({ "operation": "get", "id": "not-a-job" }),
            json!({ "operation": "delete", "id": "A".repeat(48) }),
            json!({ "operation": "rename", "id": "a".repeat(48) }),
        ] {
            assert_eq!(
                job(json!({ "directory": directory, "action": action })),
                invalid
            );
        }
        assert!(!directory.exists());
    }

    #[test]
    fn signed_out_requests_are_unauthorized_and_release_their_id() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        let request = json!({ "directory": directory, "request_id": "synthetic-signed-out", "prompt": "合成的提示词" });
        let unauthorized = json!({ "ok": false, "error": "account_unauthorized" });
        assert_eq!(generate(request.clone()), unauthorized);
        // 第一次结束时 id 已经注销，同一个 id 可以再用，也没有在跑的可以取消。
        assert_eq!(generate(request), unauthorized);
        assert_eq!(
            cancel(json!({ "request_id": "synthetic-signed-out" })),
            json!({ "ok": true, "value": false })
        );
        assert_eq!(
            job(
                json!({ "directory": directory, "action": { "operation": "create", "prompt": "合成插画" } })
            ),
            unauthorized
        );
    }

    #[test]
    fn signed_out_requests_read_the_directorys_anonymous_session() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        // 只有一份解不开的匿名会话：要是只看登录会话，这里会是 account_unauthorized；读了同一目录的匿名会话，在发请求之前就以 account_storage 结束。
        std::fs::create_dir_all(&directory).unwrap();
        let session = directory.join("anonymous-session.json");
        std::fs::write(&session, b"not a session").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
            std::fs::set_permissions(&session, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let storage = json!({ "ok": false, "error": "account_storage" });
        assert_eq!(
            generate(
                json!({ "directory": directory, "request_id": "synthetic-anonymous", "prompt": "合成的提示词" })
            ),
            storage
        );
        assert_eq!(
            job(
                json!({ "directory": directory, "action": { "operation": "create", "prompt": "合成插画" } })
            ),
            storage
        );
    }

    #[test]
    fn a_running_generation_is_busy_and_cancellable_by_id() {
        let cancelled = requests().begin("synthetic-running").unwrap();
        let _registered = Registered("synthetic-running");
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            generate(
                json!({ "directory": root.path(), "request_id": "synthetic-running", "prompt": "合成的提示词" })
            ),
            json!({ "ok": false, "error": "ai_skin_busy" })
        );
        assert_eq!(
            cancel(json!({ "request_id": "synthetic-running" })),
            json!({ "ok": true, "value": true })
        );
        assert!(cancelled.load(std::sync::atomic::Ordering::Acquire));
    }
}
