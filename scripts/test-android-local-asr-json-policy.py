#!/usr/bin/env python3
"""本地语音识别直接复用共享 JSON 类型策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/voice/LocalAsrPolicy.java"
RECOGNIZER = ROOT / "platforms/android/java/app/msime/android/voice/LocalAsrRecognizer.java"
SMOKE = ROOT / "platforms/android/tests/voice/LocalAsrPolicySmoke.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    recognizer = RECOGNIZER.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    if "strictBoolean(Object value)" in source:
        print(f"{SOURCE}: 不应保留 strictBoolean 转发方法", file=sys.stderr)
        return 1
    if "strictText(Object value)" in source:
        print(f"{SOURCE}: 不应保留 strictText 转发方法", file=sys.stderr)
        return 1
    if "LocalAsrPolicy.strictText" in recognizer:
        print(f"{RECOGNIZER}: 不应继续通过策略类调用 strictText", file=sys.stderr)
        return 1
    if "getDeclaredMethod(\"strictBoolean\"" in smoke:
        print(f"{SMOKE}: 不应反射检查已移除的转发方法", file=sys.stderr)
        return 1
    if "getDeclaredMethod(\"strictText\"" in smoke:
        print(f"{SMOKE}: 不应反射检查已移除的字符串转发方法", file=sys.stderr)
        return 1
    if "JsonPolicy.strictBoolean" not in smoke:
        print(f"{SMOKE}: 缺少 JsonPolicy.strictBoolean 合同检查", file=sys.stderr)
        return 1
    if any("JsonPolicy.strictString" not in text for text in (recognizer, smoke)):
        print(f"{SMOKE}: 缺少 JsonPolicy.strictString 合同检查", file=sys.stderr)
        return 1
    # 转写文本可以直接用 JsonPolicy.strictString，也可以交给共享转写边界 AiProviderResponse.boundedText（它内部就是 JsonPolicy.strictStringOrEmpty）。
    source_shared = ("JsonPolicy.strictString", "AiProviderResponse.boundedText(value, MAX_TRANSCRIPT)")
    if not any(call in source for call in source_shared):
        print(f"{SOURCE}: 转写文本没有经过共享 JSON 字符串策略", file=sys.stderr)
        return 1
    print("Android 本地语音识别已复用共享 JSON 类型策略")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
