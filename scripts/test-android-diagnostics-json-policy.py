#!/usr/bin/env python3
"""诊断接口直接复用共享 JSON 字符串和整数策略。

上传请求体的编码和崩溃记录的截断已经移到 Rust（#8201 迁移计划 5c，`msime_client_account` 的 diagnostics_upload），Java 只剩读回快照状态这一处解析，这里盯住它不再长出自己的转发方法。
"""
from pathlib import Path
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
    if "requestBody(" in source or "readBundle(" in source:
        print(f"{SOURCE}: 上传请求体由 Rust 从诊断记录重建，Java 不应再自己编码或读诊断包", file=sys.stderr)
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
    print("Android 诊断接口已复用共享 JSON 字符串和整数策略，上传请求体交给 Rust")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
