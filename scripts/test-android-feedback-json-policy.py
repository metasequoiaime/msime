#!/usr/bin/env python3
"""反馈接口直接复用共享 JSON 字符串策略。"""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/account/FeedbackApi.java"
SMOKE = ROOT / "platforms/android/tests/core/FeedbackApiSmoke.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    if "strictString(Object value)" in source:
        print(f"{SOURCE}: 不应保留 strictString 转发方法", file=sys.stderr)
        return 1
    if "static String clip(String value, int maxBytes)" in source:
        print(f"{SOURCE}: 不应保留 clip 转发方法", file=sys.stderr)
        return 1
    if re.search(r"(?<![.\w])clip\(", source):
        print(f"{SOURCE}: 不应调用未限定的 clip 方法", file=sys.stderr)
        return 1
    if source.count("TextPolicy.clipUtf8(") < 2:
        print(f"{SOURCE}: 应直接复用 TextPolicy.clipUtf8", file=sys.stderr)
        return 1
    if "import app.msime.android.JsonPolicy;" not in smoke:
        print(f"{SMOKE}: 应直接导入 JsonPolicy", file=sys.stderr)
        return 1
    if "FeedbackApi.class.getDeclaredMethod" in smoke:
        print(f"{SMOKE}: 不应反射检查已删除的转发方法", file=sys.stderr)
        return 1
    if "JsonPolicy.strictString" not in smoke:
        print(f"{SMOKE}: 缺少 JsonPolicy.strictString 合同检查", file=sys.stderr)
        return 1
    print("Android feedback JSON reads use the shared string policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
