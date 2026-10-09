#!/usr/bin/env python3
"""Android 打字统计解析统一复用严格计数策略。"""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/statistics/TypingStatisticsSummary.java"
SMOKE = ROOT / "platforms/android/tests/settings/TypingStatisticsSummarySmoke.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    if "private static long count(Object value)" in source:
        print(f"{SOURCE}: 不应保留 count 转发方法", file=sys.stderr)
        return 1
    if re.search(r"(?<![.\w])count\(", source):
        print(f"{SOURCE}: 统计字段不应继续通过 count 转发", file=sys.stderr)
        return 1
    if source.count("strictCount(") < 14:
        print(f"{SOURCE}: 统计字段没有统一复用 strictCount", file=sys.stderr)
        return 1
    if smoke.count("TypingStatisticsSummary.strictCount") < 3:
        print(f"{SMOKE}: 缺少 strictCount 合同检查", file=sys.stderr)
        return 1
    print("Android 打字统计解析已统一复用严格计数策略")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
