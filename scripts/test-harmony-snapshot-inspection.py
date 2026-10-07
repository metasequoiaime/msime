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
    start = account.index("  private async downloadSnapshot(")
    download = account[start : account.index("\n  private async enqueue(", start)]
    export_start = account.index("  private async exportSnapshot(")
    export_snapshot = account[export_start : account.index("\n  private async preview(", export_start)]

    keyboard = (ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets").read_text(
        encoding="utf-8"
    )
    required = {
        "C ABI declaration": "msime_client_snapshot_inspect" in header,
        "queue C ABI declaration": "msime_client_snapshot_queue" in header,
        "restore C ABI declaration": "msime_client_snapshot_restore" in header,
        "NAPI call": "TEXT_ENTRY(SnapshotInspect, msime_client_snapshot_inspect)" in native,
        "NAPI export": 'ENTRY("snapshotInspect", SnapshotInspect)' in native,
        "queue NAPI call": "TEXT_ENTRY(SnapshotQueue, msime_client_snapshot_queue)" in native,
        "queue NAPI export": 'ENTRY("snapshotQueue", SnapshotQueue)' in native,
        "restore async NAPI export": 'ENTRY("snapshotRestore", SnapshotRestore)' in native
        and "napi_create_async_work" in native,
        "ArkTS declaration": "export const snapshotInspect:" in types,
        "queue ArkTS declaration": "export const snapshotQueue:" in types,
        "restore ArkTS declaration": "export const snapshotRestore:" in types,
        "download inspection": "this.inspectSnapshot(this.snapshotFile)" in download,
        "streamed private download": "this.bridge.downloadAuthenticated(" in download
        and "MAX_SNAPSHOT_DOWNLOAD_BYTES" in download,
        "stream completion wait": "request.on('dataEnd'" in account and "await ended;" in account,
        "user-visible snapshot export": "DocumentViewPicker(this.context).save(options)"
        in export_snapshot
        and "await fs.copyFile(this.snapshotFile, destination)" in export_snapshot,
        "UUID preview token": "util.generateRandomUUID(false)" in account,
        "file identity replay guard": "inspected.fileSha256 !== this.snapshotMetadata.fileSha256"
        in account,
        "native restore picker": "DocumentViewPicker(this.context).select(options)" in account
        and "snapshot_restore_preview" in account,
        "private restore copy": "await fs.copyFile(source, this.restoreFile)" in account,
        "native restore upload": "restoreSnapshotAuthenticated(file, revision, sha256)" in account
        and "snapshot_restore_native" in account,
        "restore cleanup": "this.clearRestorePreview();" in account,
        "shared native-picker capability": "chooseSnapshotRestore: async ()" in app
        and 'operation: "snapshot_restore_preview"' in app,
        "files page restore enabled": "snapshot: true" in app
        and "snapshotNative: true" in app,
        "durable enqueue": "operation: 'enqueue'" in account
        and "client.snapshotQueue(JSON.stringify(action))" in account,
        "cloud revision replay guard": "/dictionary/changes?after=${this.snapshotMetadata.cloudRevision}&limit=1"
        in account,
        "idle queue worker": "this.processDictionarySnapshot(requested, stateRoot)" in keyboard
        and "operation: 'process'" in keyboard,
        "live queue poll": "this.scheduleDictionarySnapshotPoll()" in keyboard
        and "operation: 'state'" in keyboard
        and "acknowledge: false" in keyboard,
        "idle session rebuild": "this.dictionarySnapshotNeedsWork()" in keyboard
        and "this.restartIdleSession('snapshot queue')" in keyboard,
    }
    problems = [name for name, present in required.items() if not present]
    if "response.body.split(" in download:
        problems.append("ArkTS line parser removed")
    if "response.body" in download or "fs.writeSync" in download:
        problems.append("buffered snapshot download removed")
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
