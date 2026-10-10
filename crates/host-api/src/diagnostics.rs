//! Where the library reports a failure it recovers from by itself: a sound or music pack that does not load, an audio device that does not open, a helpcode pack the Engine falls back from.
//!
//! These used to go to stderr only. That is enough for Linux and Windows, where the host's stderr reaches a journal or a console, but the macOS input method is started by launchd with stderr on /dev/null, so a pack that failed left no trace anywhere and looked exactly like a plugin that does nothing. A host that keeps its own diagnostic log registers a sink with `msime_client_set_diagnostic_sink` and gets the same lines; without one they still go to stderr.

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
