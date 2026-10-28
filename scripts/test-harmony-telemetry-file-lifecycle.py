#!/usr/bin/env python3
"""鸿蒙遥测不在 ArkTS 写 PID 文件，也不在补报时制造原构建头。"""
from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    source = (root / "platforms/harmony/entry/src/main/ets/telemetry/Telemetry.ets").read_text(encoding="utf-8")
    required = (
        "hiAppEvent.setEventParam(parameters, hiAppEvent.domain.OS)",
        "TelemetryPolicy.originalCapture(report, consent, Date.now())",
        "crash_record_header: original.header",
        "request.diagnostic_capture = original.capture",
        "client.telemetryRecordCrash(JSON.stringify(request))",
    )
    forbidden = ("fs.open", "fs.write", "rememberPid", "TelemetryPolicy.recordText(report)")
    missing = [token for token in required if token not in source]
    if missing or any(token in source for token in forbidden):
        print("Harmony telemetry must preserve system capture identity without ArkTS file fallback")
        if missing:
            print("missing: " + ", ".join(missing))
        return 1
    print("harmony telemetry uses original system capture without ArkTS PID files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
