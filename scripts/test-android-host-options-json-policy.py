#!/usr/bin/env python3
"""验证宿主选项字符串读取复用共享 JSON 策略。"""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/policy/HostOptionsPolicy.java"


def main() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    errors = []
    if "optString(key, \"\")" in source:
        errors.append(f"{SOURCE}: 不应直接使用 optString 读取宿主选项")
    if "JsonPolicy.strictStringOrEmpty" not in source:
        errors.append(f"{SOURCE}: 应直接复用 JsonPolicy.strictStringOrEmpty")
    if errors:
        for error in errors:
            print(error)
        raise SystemExit(1)
    print("Android host options use the shared JSON string policy")


if __name__ == "__main__":
    main()
