#!/usr/bin/env python3
"""真实故障首帧校验；宿主运行不替代 iOS 设备上的故障生命周期验收。"""

import platform
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    if platform.system() != "Darwin" or not shutil.which("xcrun"):
        print("skipped: 需要 macOS 与 Xcode 命令行工具")
        return
    with tempfile.TemporaryDirectory(prefix="msime-apple-crash-pc-") as directory:
        root = Path(directory)
        common = root / "common-recorder"
        subprocess.run(["xcrun", "clang++", "-std=c++17", "-Wall", "-Wextra", "-Werror",
                        str(ROOT / "platforms/common/CrashRecorder.cpp"),
                        str(ROOT / "platforms/common/tests/crash_recorder.cpp"), "-o", str(common)], check=True)
        subprocess.run([str(common)], check=True)
        ios = root / "ios-recorder"
        subprocess.run(["xcrun", "clang", "-std=c11", "-Wall", "-Wextra", "-Werror",
                        str(ROOT / "platforms/ios/SharedUI/core/CrashSignalRecorder.c"),
                        str(ROOT / "platforms/ios/tests/CrashSignalRecorderTest.c"), "-o", str(ios)], check=True)
        subprocess.run([str(ios), str(root / "synthetic.crash")], check=True)
    print("Apple 共同记录器与 iOS C 记录器：真实内存故障首帧通过")


if __name__ == "__main__":
    main()
