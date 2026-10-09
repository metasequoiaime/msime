#!/usr/bin/env python3
"""遥测请求直接复用共享宿主选项策略。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/core/Telemetry.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    errors = []
    if "private static String preferencesDirectory(" in source:
        errors.append(f"{SOURCE}: 不应保留 preferencesDirectory 转发方法")
    if re.search(r"(?<![.\w])preferencesDirectory\(", source):
        errors.append(f"{SOURCE}: 不应调用未限定的 preferencesDirectory 方法")
    if "HostOptionsPolicy.readOption(app.getFilesDir(), \"preferences_directory\")" not in source:
        errors.append(f"{SOURCE}: 遥测请求应直接复用 HostOptionsPolicy.readOption")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android telemetry requests use the shared host-options policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
