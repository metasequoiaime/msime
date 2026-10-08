#!/usr/bin/env python3
"""Harmony telemetry markers must publish through private atomic siblings."""
from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    source = (root / "platforms/harmony/entry/src/main/ets/telemetry/Telemetry.ets").read_text(
        encoding="utf-8"
    )
    required = (
        "private static atomicWrite(path: string, text: string, replaceExisting: boolean): boolean",
        "util.generateRandomUUID(false)",
        "fs.renameSync(staging, path)",
        "HarmonyTelemetry.atomicWrite(path, TelemetryPolicy.recordText(report), false)",
        "HarmonyTelemetry.atomicWrite(path, JSON.stringify(remembered), true)",
    )
    missing = [token for token in required if token not in source]
    direct_writes = (
        "fs.openSync(path, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY)",
        "fs.openSync(path, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC)",
    )
    if missing or any(token in source for token in direct_writes):
        print("Harmony telemetry markers are not published atomically")
        if missing:
            print("missing: " + ", ".join(missing))
        return 1
    print("harmony telemetry markers use atomic private publication")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
