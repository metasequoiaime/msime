#!/usr/bin/env python3
"""AI 模型目录直接复用共享 JSON 策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/voice/AiPolishModelCatalog.java"
SMOKE = ROOT / "platforms/android/tests/voice/AiPolishModelCatalogSmoke.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    for method in ("strictString", "strictBoolean"):
        if f"{method}(Object value)" in source:
            print(f"{SOURCE}: 不应保留 {method} 转发方法", file=sys.stderr)
            return 1
        if f"AiPolishModelCatalog.{method}" in smoke:
            print(f"{SMOKE}: 不应继续通过目录类调用 {method}", file=sys.stderr)
            return 1
    if "getDeclaredMethod" in smoke:
        print(f"{SMOKE}: 不应通过反射检查已移除的转发方法", file=sys.stderr)
        return 1
    for expression in ("JsonPolicy.strictString", "JsonPolicy.strictBoolean"):
        if expression not in smoke:
            print(f"{SMOKE}: 缺少 {expression} 合同检查", file=sys.stderr)
            return 1
    print("Android AI model catalog uses the shared JSON policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
