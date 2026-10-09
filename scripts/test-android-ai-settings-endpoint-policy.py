#!/usr/bin/env python3
"""AI 设置页直接复用共享端点策略。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/AiSettingsPage.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    errors = []
    if "private static boolean validEndpoint(" in source:
        errors.append(f"{SOURCE}: 不应保留 validEndpoint 转发方法")
    if re.search(r"(?<![.\w])validEndpoint\(", source):
        errors.append(f"{SOURCE}: 不应调用未限定的 validEndpoint 方法")
    if source.count("AiEndpointPolicy.allowed(") < 3:
        errors.append(f"{SOURCE}: AI 设置应直接复用 AiEndpointPolicy.allowed")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android AI settings use the shared endpoint policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
