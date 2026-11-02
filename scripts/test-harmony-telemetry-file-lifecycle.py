#!/usr/bin/env python3
"""Harmony telemetry must leave crash records to the shared Rust store and never write its own files."""
from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    source = (root / "platforms/harmony/entry/src/main/ets/telemetry/Telemetry.ets").read_text(
        encoding="utf-8"
    )
    # value:false from telemetryRecordCrash is Rust refusing the record (wrong consent epoch, a diagnostic from before consent began, or a duplicate); a local fallback file would bypass that refusal and the shared redaction.
    required = ("client.telemetryRecordCrash(JSON.stringify(request))",)
    forbidden = (
        "@ohos.file.fs",
        "telemetry-crashes",
        "TelemetryPolicy.recordText",
        "harmony-keyboard-process.json",
        "atomicWrite(",
    )
    missing = [token for token in required if token not in source]
    present = [token for token in forbidden if token in source]
    if missing or present:
        print("Harmony telemetry must record crashes only through the shared store")
        if missing:
            print("missing: " + ", ".join(missing))
        if present:
            print("forbidden: " + ", ".join(present))
        return 1
    print("harmony telemetry records crashes only through the shared store")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
