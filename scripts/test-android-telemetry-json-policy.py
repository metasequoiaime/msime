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
    # 授权由 Rust 偏好锁下产生；Java 严格解析响应，不再重复读取偏好里的布尔缓存。
    if ('JsonPolicy.strictBoolean(value.opt("enabled"))' not in source
            or 'if (allowed == null) throw' not in source
            or "NativeClient.telemetryConsent(" not in source):
        print(f"{SOURCE}: 遥测状态和共享同意应直接调用 JsonPolicy 的严格布尔策略", file=sys.stderr)
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
