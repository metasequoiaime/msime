#!/usr/bin/env python3
"""遥测状态与文本处理直接复用共享策略。"""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/core/Telemetry.java"
SMOKE = ROOT / "platforms/android/tests/core/TelemetryHandlerSmoke.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    if "booleanValue(" in source:
        print(f"{SOURCE}: 不应保留或调用 booleanValue 转发方法", file=sys.stderr)
        return 1
    if "Telemetry.booleanValue" in smoke:
        print(f"{SMOKE}: 不应继续通过遥测类检查 JSON 布尔值", file=sys.stderr)
        return 1
    if source.count("JsonPolicy.strictBoolean") < 2:
        print(f"{SOURCE}: 遥测状态应直接调用 JsonPolicy.strictBoolean", file=sys.stderr)
        return 1
    if "import app.msime.android.JsonPolicy;" not in smoke:
        print(f"{SMOKE}: 应直接导入 JsonPolicy", file=sys.stderr)
        return 1
    if smoke.count("JsonPolicy.strictBoolean") < 2:
        print(f"{SMOKE}: 缺少共享 JSON 布尔策略合同检查", file=sys.stderr)
        return 1
    if "static String clipCodePoints(" in source or re.search(r"(?<![.\w])clipCodePoints\(", source):
        print(f"{SOURCE}: 不应保留或调用 clipCodePoints 转发方法", file=sys.stderr)
        return 1
    if source.count("TextPolicy.clipCodePoints(") < 2:
        print(f"{SOURCE}: 崩溃记录应直接调用 TextPolicy.clipCodePoints", file=sys.stderr)
        return 1
    if "import app.msime.android.TextPolicy;" not in smoke:
        print(f"{SMOKE}: 应直接导入 TextPolicy", file=sys.stderr)
        return 1
    if "Telemetry.clipCodePoints" in smoke or smoke.count("TextPolicy.clipCodePoints(") < 2:
        print(f"{SMOKE}: 应直接检查 TextPolicy.clipCodePoints", file=sys.stderr)
        return 1
    print("Android 遥测状态与文本处理已复用共享策略")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
