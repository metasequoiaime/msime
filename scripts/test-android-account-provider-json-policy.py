#!/usr/bin/env python3
"""Android 账号提供商状态直接复用共享 JSON 布尔策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/account/BackendAccount.java"
SMOKE = ROOT / "platforms/android/tests/settings/BackendAccountResponseSmoke.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    if "providerEnabled(Object value)" in source:
        print(f"{SOURCE}: 不应保留 providerEnabled 转发方法", file=sys.stderr)
        return 1
    if "providerEnabled(providers().opt(provider))" in source:
        print(f"{SOURCE}: 不应继续通过领域方法读取提供商状态", file=sys.stderr)
        return 1
    if "BackendAccount.providerEnabled" in smoke:
        print(f"{SMOKE}: 不应测试已移除的转发方法", file=sys.stderr)
        return 1
    if "JsonPolicy.strictTrue(providers().opt(provider))" not in source:
        print(f"{SOURCE}: 提供商状态应直接调用 JsonPolicy.strictTrue", file=sys.stderr)
        return 1
    if "JsonPolicy.strictTrue" not in smoke:
        print(f"{SMOKE}: 缺少 JsonPolicy.strictTrue 合同检查", file=sys.stderr)
        return 1
    print("Android 账号提供商状态已复用共享 JSON 布尔策略")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
