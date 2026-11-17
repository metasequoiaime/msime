#!/usr/bin/env python3
"""在主机上运行 iOS 实际按键批次，检查按下时来源和跨线程快照契约。"""
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    swiftc = shutil.which("swiftc")
    if platform.system() != "Darwin" or not swiftc:
        print("skipped: iOS 按键批次检查需要 macOS 和 swiftc")
        return
    output = ROOT / "target/ios-statistics-key-batch-tests"
    output.mkdir(parents=True, exist_ok=True)
    # 已有键位表与 UIKit/共享存储同文件；只取原始枚举编译，不复制一份测试键位表。
    source = (ROOT / "platforms/ios/SharedUI/core/TypingStatistics.swift").read_text()
    start = source.index("enum TypingKeyID {")
    end = source.index("\n}\n", start) + 3
    with tempfile.TemporaryDirectory(prefix="case-", dir=output) as folder:
        folder = Path(folder)
        key_ids = folder / "TypingKeyID.swift"
        key_ids.write_text("import Foundation\n" + source[start:end])
        executable = folder / "test"
        subprocess.run([
            swiftc, "-swift-version", "5", "-warnings-as-errors", "-parse-as-library",
            str(key_ids), "platforms/ios/SharedUI/core/TypingKeyPressBatch.swift",
            "platforms/ios/tests/tools/statistics-key-batch/TypingKeyBatchTests.swift",
            "-o", str(executable),
        ], cwd=ROOT, check=True, timeout=60)
        subprocess.run([str(executable)], cwd=ROOT, check=True, timeout=15)


if __name__ == "__main__":
    main()
