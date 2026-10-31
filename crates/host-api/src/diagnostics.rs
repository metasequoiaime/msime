//! 本库报告自行恢复的失败的地方：音效包或音乐包载入失败、音频设备打不开、Engine 回退了辅助码包、整句模型读不了或不能用。
//!
//! 另有一类不是失败的状态行：开着「桌面神经联想」时，会话建立、开关打开和模型换上时各报告一行落定重排模型接上了没有（`settled model attached` 等，见 `report_settled_model`）。模型缺席在别的会话里是常态、不算失败，但用户开了开关却没有效果时，只有这一行能区分没装、读不了和宿主没给状态目录（#10341）。这类状态行只在低频的会话事件上写，不在按键和评分路径上写。
//!
//! 这些原先只写 stderr。Linux 宿主的 stderr 能进 journal 或终端，但 macOS 输入法由 launchd 启动、stderr 指向 /dev/null，Windows Server 在 Watchdog 下运行、stderr 也没人读，失败的包不留任何痕迹，看上去和一个不起作用的插件一模一样。有自己诊断日志的宿主用 `msime_client_set_diagnostic_sink` 登记出口，收到同样的行（macOS 输入法写 `diagnostic.log`，Windows Server 写 `logs\server.log`）；没登记时仍写 stderr。

use std::ffi::{c_char, CString};
use std::sync::atomic::{AtomicPtr, Ordering};

/// The host's sink: one NUL-terminated UTF-8 line per call, valid only for the call.
pub type DiagnosticSink = unsafe extern "C" fn(*const c_char);

/// The registered sink as a raw pointer, null while none is. An atomic rather than a lock: reporting runs on the player thread, on background loaders and inside session calls, and must never wait on, or be re-entered through, a lock of its own.
static SINK: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());

/// Report one failure as the line `<category>: <detail>`, without the `msime: ` prefix stderr gets. Goes to the registered sink, or to stderr without one. Never blocks on the library's own state and never panics.
///
/// `category` is a fixed English phrase saying what failed, such as `sound pack not loaded`; being `'static` it cannot carry a pack id, a path or an error, and it holds no colon, so the text before the line's first colon is always the category alone. A host whose log must not hold those details (macOS) keeps only that part. `detail` carries everything else: the pack, the file and the cause.
pub(crate) fn report(category: &'static str, detail: &str) {
    debug_assert!(
        !category.is_empty() && !category.contains(':'),
        "a diagnostic category must be non-empty and hold no colon: {category:?}"
    );
    let message = format!("{category}: {detail}");
    let sink = SINK.load(Ordering::Acquire);
    if sink.is_null() {
        eprintln!("msime: {message}");
        return;
    }
    // SAFETY: only `msime_client_set_diagnostic_sink` stores into `SINK`, and it stores nothing but a `DiagnosticSink` cast to a pointer, so a non-null value converts back to the same function pointer.
    let sink = unsafe { std::mem::transmute::<*mut (), DiagnosticSink>(sink) };
    // A NUL inside the message would cut the C string short; none of the messages carries one, but a reason passed through from a file name could.
    let line = CString::new(message.replace('\0', "?")).unwrap_or_default();
    // SAFETY: the host promised a callable sink when it registered it, and `line` outlives the call.
    unsafe { sink(line.as_ptr()) };
}

/// Send the library's diagnostic lines to `sink` instead of stderr; null goes back to stderr. A later call replaces the sink. See `include/msime_client.h` for what the sink receives and must not do.
#[no_mangle]
pub extern "C" fn msime_client_set_diagnostic_sink(sink: Option<DiagnosticSink>) {
    let pointer = sink.map_or(std::ptr::null_mut(), |sink| sink as *mut ());
    SINK.store(pointer, Ordering::Release);
}
