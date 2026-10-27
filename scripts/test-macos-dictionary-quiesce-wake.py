#!/usr/bin/env python3
"""macOS 设置应用请输入法让出词库时立刻叫醒 IMK：设置页改词（`dictionary_request`）和词库同步（`dictionary_sync`）都经 `msime_host_macos::quiesce_input_sessions` 发分布式通知，不让输入法等每秒一次的偏好轮询。

两条路的租约都由 `QuiescedHosts` 挂上，叫醒是它的 `announce`；漏传时什么也不报错，只是 IMK 要晚一秒才让出、手动同步更容易在 2500 毫秒预算里答「忙」，所以这里守着两处调用点。
"""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LIB = (ROOT / "apps/desktop/src-tauri/src/lib.rs").read_text(encoding="utf-8")
SYNC = (ROOT / "apps/desktop/src-tauri/src/platform/dictionary_sync.rs").read_text(encoding="utf-8")
HOST = (ROOT / "crates/host-api/src/dictionary_webdav.rs").read_text(encoding="utf-8")

edit = LIB[LIB.index("async fn dictionary_request(") :]
edit = edit[: edit.index("\n}\n")]
wake = SYNC[SYNC.index("fn wake_input_method()") :]
wake = wake[: wake.index("\n}\n")]

checks = {
    "settings edits wake the input method": "msime_host_macos::quiesce_input_sessions();" in edit,
    "dictionary sync wakes it the same way": '#[cfg(target_os = "macos")]\n    msime_host_macos::quiesce_input_sessions();' in wake,
    "dictionary sync hands the wake to Rust": "request_json_announcing(&body, wake_input_method)" in SYNC,
    "Rust passes it to the lease": "EngineDictionary::announcing(&engine, interactive, announce)" in HOST
    and "                    announce,\n" in HOST,
}
problems = [name for name, ok in checks.items() if not ok]
if problems:
    for problem in problems:
        print(f"macos dictionary quiesce wake: {problem}")
    raise SystemExit(1)

print("macos dictionary quiesce wake: settings edits and dictionary sync both wake the input method at once")
