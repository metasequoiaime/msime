#!/usr/bin/env python3
"""云 API 与账号入口不重复定义共享 JSON 布尔策略：登录方式的开关（Rust 的 providers，#8201 迁移计划 5b 起由 BackendAccount 读）直接调用 JsonPolicy.strictTrue。"""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
CLOUD_API = ROOT / "platforms/android/java/app/msime/android/account/CloudApi.java"
ACCOUNT = ROOT / "platforms/android/java/app/msime/android/account/BackendAccount.java"


def main() -> int:
    for path in (CLOUD_API, ACCOUNT):
        source = path.read_text(encoding="utf-8")
        if "strictTrue(Object value)" in source:
            print(f"{path}: 不应保留 strictTrue 转发方法", file=sys.stderr)
            return 1
        if re.search(r"(?<![.\w])strictTrue\(\w+\.opt\(", source):
            print(f"{path}: 不应继续通过本地 strictTrue 读取提供商标志", file=sys.stderr)
            return 1
    if ACCOUNT.read_text(encoding="utf-8").count("JsonPolicy.strictTrue(providers.opt(") < 4:
        print(f"{ACCOUNT}: 提供商标志应直接调用 JsonPolicy.strictTrue", file=sys.stderr)
        return 1
    print("Android 云 API 与账号入口未重复定义共享 JSON 布尔策略")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
