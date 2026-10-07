#!/usr/bin/env python3
"""Guard Harmony streamed downloads when the destination cannot be opened."""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    source = (root / "platforms/harmony/entry/src/main/ets/account/HarmonyAccountCloudBridge.ets").read_text()
    start = source.index("  async download(")
    body = source[start : source.index("\n  async uploadSnapshot(", start)]
    export_start = source.index("  private async exportDictionary(")
    export_body = source[export_start : source.index("\n  private cancelQueueForAccount(", export_start)]
    required = {
        "destination open is guarded": "let handle: fs.File | undefined = undefined;" in body
            and "handle = fs.openSync(destination" in body,
        "request is destroyed on open failure": "if (handle !== undefined) fs.closeSync(handle);" in body
            and "request.destroy();" in body,
        "export staging directory rejects symlinks":
            "if (!fs.accessSync(directory)) fs.mkdirSync(directory, true);" in export_body
            and "if (!this.safeDirectory(directory))" in export_body,
    }
    problems = [name for name, present in required.items() if not present]
    if problems:
        for problem in problems:
            print(f"missing Harmony download lifecycle guard: {problem}")
        return 1
    print("harmony download lifecycle: destination-open failures destroy HTTP requests")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
