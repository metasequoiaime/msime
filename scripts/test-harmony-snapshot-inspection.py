#!/usr/bin/env python3
"""Keep Harmony snapshot preview on the native, complete-document validator.

Line-counting the downloaded NDJSON in ArkTS once shipped truncated snapshots as validated
previews: the Engine reader intentionally consumes only stageable records and does not authenticate
the cloud envelope. This check keeps the host wired to the C ABI that verifies the footer, body
digest, record identities and full-file identity before preview or enqueue.
"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ACCOUNT = ROOT / "platforms/harmony/entry/src/main/ets/account/HarmonyAccountCloudBridge.ets"
NATIVE = ROOT / "platforms/harmony/native/client_napi.cpp"
TYPES = ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts"
HEADER = ROOT / "crates/host-api/include/msime_client.h"
APP = ROOT / "apps/harmony/src/main.tsx"


def main() -> int:
    account = ACCOUNT.read_text(encoding="utf-8")
    native = NATIVE.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    header = HEADER.read_text(encoding="utf-8")
    app = APP.read_text(encoding="utf-8")
    keyboard = (ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets").read_text(
        encoding="utf-8"
    )
    export_start = account.index("  private async exportSnapshot(")
    export_snapshot = account[export_start : account.index("\n  /** 单个云词库导出", export_start)]
    # 预览、入列和恢复的校验（尾部、正文摘要、记录身份、整份文件的身份）都在 Rust 的 msime_client_cloud_dictionary 里；这里守住宿主把整份快照交给它、不自己解析。
    required = {
        "C ABI declaration": "msime_client_snapshot_inspect" in header,
        "queue C ABI declaration": "msime_client_snapshot_queue" in header,
        "cloud dictionary file operations": "snapshot_export_file" in header
        and "snapshot_restore_preview_file" in header
        and "snapshot_restore_file" in header,
        "NAPI call": "TEXT_ENTRY(SnapshotInspect, msime_client_snapshot_inspect)" in native,
        "NAPI export": 'ENTRY("snapshotInspect", SnapshotInspect)' in native,
        "queue NAPI call": "TEXT_ENTRY(SnapshotQueue, msime_client_snapshot_queue)" in native,
        "queue NAPI export": 'ENTRY("snapshotQueue", SnapshotQueue)' in native,
        "snapshot thread NAPI export": 'ENTRY("cloudDictionarySnapshot", CloudDictionarySnapshot)' in native
        and "std::thread(runThreadRequest, work).detach();" in native,
        "ArkTS declaration": "export const snapshotInspect:" in types,
        "queue ArkTS declaration": "export const snapshotQueue:" in types,
        "snapshot ArkTS declaration": "export const cloudDictionarySnapshot:" in types,
        "preview, enqueue and status go to Rust": "case 'snapshot_preview':" in account
        and "case 'snapshot_enqueue':" in account
        and "client.cloudDictionarySnapshot(request)" in account,
        "user-visible snapshot export": "operation: 'snapshot_export_file'" in export_snapshot
        and "this.copyOut(staging, filename, 'NDJSON|.ndjson')" in export_snapshot,
        "UUID preview token": "util.generateRandomUUID(false)" in account,
        "native restore picker": "DocumentViewPicker(this.context).select(options)" in account
        and "operation: 'snapshot_restore_preview_file'" in account,
        "native restore upload": "operation: 'snapshot_restore_file'" in account
        and "expected_sha256: this.restoreSha256" in account,
        "restore stays with the previewing account": "this.currentUserId() !== this.restoreAccountId" in account,
        "restore cleanup": "this.clearRestorePreview();" in account,
        "shared native-picker capability": "chooseSnapshotRestore: async ()" in app
        and 'operation: "snapshot_restore_preview"' in app,
        "files page restore enabled": "snapshot: true" in app
        and "snapshotNative: true" in app,
        "idle queue worker": "this.processDictionarySnapshot(requested, stateRoot)" in keyboard
        and "operation: 'process'" in keyboard,
        "live queue poll": "this.scheduleDictionarySnapshotPoll()" in keyboard
        and "operation: 'state'" in keyboard
        and "acknowledge: false" in keyboard,
        "idle session rebuild": "this.dictionarySnapshotNeedsWork()" in keyboard
        and "this.restartIdleSession('snapshot queue')" in keyboard,
    }
    problems = [name for name, present in required.items() if not present]
    if "access_token" in account or "client.snapshotRestore(" in account:
        problems.append("snapshot restore with a host-held access token removed")
    if "client.snapshotPrepare(" in account or "snapshotHandle" in account:
        problems.append("process-local prepared handle removed from account bridge")
    if keyboard.find("this.processDictionarySnapshot(requested, stateRoot)") > keyboard.find(
        "client.create(options)"
    ):
        problems.append("durable queue worker runs before session creation")
    if problems:
        print(
            "harmony snapshot inspection: missing " + ", ".join(problems),
            file=sys.stderr,
        )
        return 1
    print("harmony snapshot inspection: native validation and durable idle queue are wired")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
