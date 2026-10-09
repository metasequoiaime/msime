#!/usr/bin/env python3
"""诊断接口直接复用共享 JSON 字符串、整数、引用和截断策略。"""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/account/DiagnosticsApi.java"
SMOKE = ROOT / "platforms/android/tests/core/DiagnosticsApiSmoke.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    if "strictString(Object value)" in source:
        print(f"{SOURCE}: 不应保留 strictString 转发方法", file=sys.stderr)
        return 1
    if "strictInteger(Object value)" in source:
        print(f"{SOURCE}: 不应保留 strictInteger 转发方法", file=sys.stderr)
        return 1
    if "static void quote(StringBuilder out, String value)" in source:
        print(f"{SOURCE}: 不应保留 quote 转发方法", file=sys.stderr)
        return 1
    if "quote(out," in source:
        print(f"{SOURCE}: 诊断请求仍通过 quote 转发", file=sys.stderr)
        return 1
    if source.count("out.append(JsonPolicy.quote(") < 6:
        print(f"{SOURCE}: 诊断请求应直接复用共享 JSON 引用策略", file=sys.stderr)
        return 1
    if "static String clipUtf8(String value, int maxBytes)" in source:
        print(f"{SOURCE}: 不应保留 clipUtf8 转发方法", file=sys.stderr)
        return 1
    if re.search(r"(?<![.\w])clipUtf8\(", source):
        print(f"{SOURCE}: 崩溃记录仍通过 clipUtf8 转发", file=sys.stderr)
        return 1
    if source.count("TextPolicy.clipUtf8(") < 2:
        print(f"{SOURCE}: 崩溃记录应直接复用共享 UTF-8 截断策略", file=sys.stderr)
        return 1
    if "DiagnosticsApi.class.getDeclaredMethod(\"strictString\"" in smoke:
        print(f"{SMOKE}: 不应反射检查已删除的字符串转发方法", file=sys.stderr)
        return 1
    if "DiagnosticsApi.strictInteger" in smoke:
        print(f"{SMOKE}: 不应继续通过诊断类调用 strictInteger", file=sys.stderr)
        return 1
    if "JsonPolicy.strictString" not in smoke:
        print(f"{SMOKE}: 缺少 JsonPolicy.strictString 合同检查", file=sys.stderr)
        return 1
    if "JsonPolicy.strictLong" not in source or "JsonPolicy.strictLong" not in smoke:
        print(f"{SMOKE}: 应直接检查 JsonPolicy.strictLong", file=sys.stderr)
        return 1
    print("Android 诊断接口已复用共享 JSON 字符串、整数、引用和截断策略")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
