//! 原生语音面板复用现有 Server 控制协议；只适配会话所有权与 C ABI。

use super::with_bounded_bytes;
use crate::response;
use msime_client_providers::voice::controller::{Error, Update};
use serde_json::json;
use std::collections::BTreeMap;
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

type Progress = unsafe extern "C" fn(*const u8, usize, *mut c_void);
const CAPACITY: usize = 32;

#[derive(Default)]
struct Session {
    stopped: AtomicBool,
    cancelled: AtomicBool,
    // 0 尚未消费，1 执行中，2 已结束；同一句柄不能重复发起录音。
    phase: AtomicU8,
}

#[derive(Default)]
struct Registry {
    next: u64,
    sessions: BTreeMap<u64, Arc<Session>>,
}

impl Registry {
    fn create(&mut self) -> Result<u64, String> {
        if self.sessions.len() >= CAPACITY {
            return Err("voice_session_capacity".into());
        }
        let handle = self.next.checked_add(1).ok_or("voice_session_capacity")?;
        self.next = handle;
        self.sessions.insert(handle, Arc::new(Session::default()));
        Ok(handle)
    }

    fn get(&self, handle: u64) -> Result<Arc<Session>, String> {
        self.sessions
            .get(&handle)
            .cloned()
            .ok_or_else(|| "voice_session_invalid".into())
    }

    fn control(&mut self, handle: u64, operation: u32) -> Result<(), String> {
        if operation > 2 {
            return Err("voice_session_invalid".into());
        }
        let session = if operation == 2 {
            self.sessions
                .remove(&handle)
                .ok_or("voice_session_invalid")?
        } else {
            self.get(handle)?
        };
        if operation == 0 {
            session.stopped.store(true, Ordering::Release);
        } else {
            session.cancelled.store(true, Ordering::Release);
        }
        Ok(())
    }
}

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Running(Arc<Session>);
impl Running {
    fn claim(session: Arc<Session>) -> Result<Self, String> {
        session
            .phase
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "voice_session_consumed".to_owned())?;
        Ok(Self(session))
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.0.phase.store(2, Ordering::Release);
    }
}

fn error_code(error: Error) -> &'static str {
    match error {
        Error::Invalid => "voice_invalid",
        Error::Unavailable => "voice_unavailable",
        Error::Busy => "voice_busy",
        Error::Stale => "voice_stale",
        Error::Denied => "voice_denied",
        Error::Cancelled => "voice_cancelled",
    }
}

fn report(progress: Option<Progress>, context: *mut c_void, value: serde_json::Value) {
    if let Some(progress) = progress {
        let bytes = value.to_string();
        // SAFETY: 调用方按同步调用契约保留回调和上下文；缓冲区只借用到本次回调返回。
        unsafe { progress(bytes.as_ptr(), bytes.len(), context) };
    }
}

/// 创建一次性语音审阅句柄；尚不访问麦克风、文件或管道。
#[no_mangle]
pub extern "C" fn msime_client_windows_voice_session_create() -> *mut c_char {
    response(|| Ok(json!({"handle": registry().create()?})))
}

/// 停止、取消或关闭指定会话；可从任意线程调用，执行中的识别继续持有自己的信号。
#[no_mangle]
pub extern "C" fn msime_client_windows_voice_session_control(
    handle: u64,
    operation: u32,
) -> *mut c_char {
    response(|| {
        registry().control(handle, operation)?;
        Ok(json!({"accepted": true}))
    })
}

/// 在调用线程通过现有专用协议录音与识别，结果只用于审阅，不自动上屏。
///
/// # Safety
/// `language` 指向调用期间可读的 `length` 字节；回调和上下文保持有效且不能展开异常。
/// 回调可重入控制接口，不能对同一句柄重入识别。返回缓冲区由调用方释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_windows_voice_session_recognize(
    handle: u64,
    language: *const u8,
    length: usize,
    progress: Option<Progress>,
    context: *mut c_void,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(language, length, 64, "voice_invalid", |bytes| {
            if bytes.is_empty()
                || !bytes
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
            {
                return Err("voice_invalid".into());
            }
            let language = std::str::from_utf8(bytes).map_err(|_| "voice_invalid")?;
            let running = Running::claim(registry().get(handle)?)?;
            if running.0.cancelled.load(Ordering::Acquire) {
                return Err("voice_cancelled".into());
            }
            report(progress, context, json!({"phase":"connecting","level":0}));
            let text = msime_host_windows::voice_controller::recognize(
                language,
                handle,
                &running.0.stopped,
                &running.0.cancelled,
                |update: &Update| {
                    report(
                        progress,
                        context,
                        json!({"phase":update.phase.name(),"level":update.level,"text":update.text}),
                    );
                },
            )
            .map_err(error_code)?;
            if running.0.cancelled.load(Ordering::Acquire) {
                return Err("voice_cancelled".into());
            }
            Ok(json!({"text":text}))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(raw: *mut c_char) -> serde_json::Value {
        assert!(!raw.is_null());
        // SAFETY: ABI 返回以 NUL 结尾的自有字符串，解析后由配套释放函数回收。
        let value =
            serde_json::from_slice(unsafe { std::ffi::CStr::from_ptr(raw) }.to_bytes()).unwrap();
        unsafe { crate::msime_client_string_free(raw) };
        value
    }

    #[test]
    fn capacity_close_and_handle_reuse_are_bounded() {
        let mut registry = Registry::default();
        let first = registry.create().unwrap();
        for _ in 1..CAPACITY {
            registry.create().unwrap();
        }
        assert_eq!(registry.create().unwrap_err(), "voice_session_capacity");
        let retained = registry.get(first).unwrap();
        registry.control(first, 2).unwrap();
        assert!(retained.cancelled.load(Ordering::Acquire));
        assert!(registry.get(first).is_err());
        let next = registry.create().unwrap();
        assert!(next > first);
        assert!(!registry
            .get(next)
            .unwrap()
            .cancelled
            .load(Ordering::Acquire));
        assert!(registry.control(first, 1).is_err());
    }

    #[test]
    fn one_shot_claim_and_control_preserve_other_sessions() {
        let mut registry = Registry::default();
        let first = registry.create().unwrap();
        let second = registry.create().unwrap();
        let retained = registry.get(first).unwrap();
        let running = Running::claim(retained.clone()).unwrap();
        assert!(Running::claim(retained.clone()).is_err());
        registry.control(first, 0).unwrap();
        assert!(retained.stopped.load(Ordering::Acquire));
        assert!(!retained.cancelled.load(Ordering::Acquire));
        assert!(!registry
            .get(second)
            .unwrap()
            .stopped
            .load(Ordering::Acquire));
        registry.control(first, 2).unwrap();
        assert!(retained.cancelled.load(Ordering::Acquire));
        drop(running);
        assert_eq!(retained.phase.load(Ordering::Acquire), 2);
        assert!(Running::claim(retained).is_err());
        assert!(registry.control(second, 3).is_err());
        assert!(!registry
            .get(second)
            .unwrap()
            .cancelled
            .load(Ordering::Acquire));
    }

    #[test]
    fn cancellation_before_recognition_does_not_open_a_pipe() {
        let created = decode(msime_client_windows_voice_session_create())["value"].clone();
        let handle = created["handle"].as_u64().unwrap();
        assert_eq!(
            decode(msime_client_windows_voice_session_control(handle, 1))["ok"],
            true
        );
        let language = b"en-US";
        // SAFETY: 合成字节在同步调用中有效；预先取消保证不会访问实际 Server 或麦克风。
        let raw = unsafe {
            msime_client_windows_voice_session_recognize(
                handle,
                language.as_ptr(),
                language.len(),
                None,
                std::ptr::null_mut(),
            )
        };
        let owned = decode(raw);
        assert_eq!(owned["error"], "voice_cancelled");
        assert_eq!(
            decode(msime_client_windows_voice_session_control(handle, 2))["ok"],
            true
        );
    }

    #[test]
    fn invalid_language_does_not_consume_the_session() {
        let created = decode(msime_client_windows_voice_session_create());
        let handle = created["value"]["handle"].as_u64().unwrap();
        for language in [&b""[..], &b"en_US"[..], &b"a\0b"[..], &[b'a'; 65][..]] {
            // SAFETY: 合成缓冲区在调用期间有效；无效语言在访问管道前被拒绝。
            let answer = unsafe {
                msime_client_windows_voice_session_recognize(
                    handle,
                    language.as_ptr(),
                    language.len(),
                    None,
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(decode(answer)["error"], "voice_invalid");
        }
        assert_eq!(
            registry()
                .get(handle)
                .unwrap()
                .phase
                .load(Ordering::Acquire),
            0
        );
        assert_eq!(
            decode(msime_client_windows_voice_session_control(handle, 2))["ok"],
            true
        );
    }

    #[test]
    fn connecting_callback_can_close_without_deadlock_or_opening_a_pipe() {
        unsafe extern "C" fn close(_: *const u8, _: usize, context: *mut c_void) {
            // SAFETY: 同步调用持有测试栈上的句柄，回调只读取它。
            let handle = unsafe { *context.cast::<u64>() };
            assert_eq!(
                decode(msime_client_windows_voice_session_control(handle, 2))["ok"],
                true
            );
        }
        let created = decode(msime_client_windows_voice_session_create());
        let mut handle = created["value"]["handle"].as_u64().unwrap();
        // SAFETY: 语言与上下文在同步调用中有效，回调关闭会话会在管道连接前触发取消。
        let answer = unsafe {
            msime_client_windows_voice_session_recognize(
                handle,
                b"en-US".as_ptr(),
                5,
                Some(close),
                std::ptr::from_mut(&mut handle).cast(),
            )
        };
        assert_eq!(decode(answer)["error"], "voice_cancelled");
        assert!(registry().get(handle).is_err());
    }
}
