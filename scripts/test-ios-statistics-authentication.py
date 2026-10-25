#!/usr/bin/env python3
"""运行实际 iOS 统计认证状态机，覆盖无密码、失败重试和后台过期回调。"""
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    swiftc = shutil.which("swiftc")
    if platform.system() != "Darwin" or not swiftc:
        print("skipped: iOS 统计认证检查需要 macOS 和 swiftc")
        return
    output = ROOT / "target/ios-statistics-auth-tests"
    output.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run([
        swiftc, "-swift-version", "5", "-parse-as-library",
        "platforms/ios/App/Sources/settings/StatisticsAuthentication.swift",
        "platforms/ios/tests/tools/statistics-authentication/StatisticsAuthenticationTests.swift",
        "-o", str(output),
    ], cwd=ROOT, check=True, timeout=60)
    subprocess.run([str(output)], cwd=ROOT, check=True, timeout=15)


if __name__ == "__main__":
    main()
