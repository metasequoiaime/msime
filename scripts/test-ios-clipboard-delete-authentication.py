#!/usr/bin/env python3
"""运行 iOS「剪贴板删除数据验证」（#10408）的切换规则、删除前判断、本机存储和认证器，覆盖没有锁屏密码、重复请求、失败和面板收起后迟到的通过。"""
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    swiftc = shutil.which("swiftc")
    if platform.system() != "Darwin" or not swiftc:
        print("skipped: iOS 剪贴板删除验证检查需要 macOS 和 swiftc")
        return
    output = ROOT / "target/ios-clipboard-delete-auth-tests"
    output.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run([
        swiftc, "-swift-version", "5", "-parse-as-library",
        "platforms/ios/SharedUI/clipboard/ClipboardDeleteAuthentication.swift",
        "platforms/ios/tests/tools/clipboard-delete-authentication/ClipboardDeleteAuthenticationTests.swift",
        "-o", str(output),
    ], cwd=ROOT, check=True, timeout=120)
    subprocess.run([str(output)], cwd=ROOT, check=True, timeout=15)


if __name__ == "__main__":
    main()
