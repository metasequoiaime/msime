#!/usr/bin/env python3
"""编译实际删除按压状态，回归快速点按、滑出滑回、取消和交错按键。"""
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    swiftc = shutil.which("swiftc")
    if platform.system() != "Darwin" or not swiftc:
        print("skipped: iOS 删除按压回归需要 macOS 和 swiftc")
        return
    output = ROOT / "target/ios-backspace-press-tests"
    output.parent.mkdir(parents=True, exist_ok=True)
    sources = [
        "platforms/ios/KeyboardExtension/Sources/keyboard/BackspacePressState.swift",
        "platforms/ios/tests/backspace/BackspacePressTests.swift",
    ]
    subprocess.run([swiftc, "-swift-version", "5", "-parse-as-library", *sources,
                    "-o", str(output)], cwd=ROOT, check=True, timeout=60)
    subprocess.run([str(output)], cwd=ROOT, check=True, timeout=15)


if __name__ == "__main__":
    main()
