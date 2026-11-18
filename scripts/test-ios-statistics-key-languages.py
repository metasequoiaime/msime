#!/usr/bin/env python3
"""在 macOS 主机运行 iOS 原生按键语言筛选模型，不启动设备或模拟器。"""
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    swiftc = shutil.which("swiftc")
    if platform.system() != "Darwin" or not swiftc:
        print("skipped: iOS 按键语言检查需要 macOS 和 swiftc")
        return
    output = ROOT / "target/ios-statistics-key-language-tests"
    output.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run([
        swiftc, "-swift-version", "5", "-warnings-as-errors", "-parse-as-library",
        "platforms/ios/SharedUI/core/TypingKeyLanguages.swift",
        "platforms/ios/tests/tools/statistics-key-languages/TypingKeyLanguageTests.swift",
        "-o", str(output),
    ], cwd=ROOT, check=True, timeout=60)
    subprocess.run([str(output)], cwd=ROOT, check=True, timeout=15)


if __name__ == "__main__":
    main()
