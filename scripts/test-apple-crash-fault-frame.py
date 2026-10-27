#!/usr/bin/env python3
"""真实故障首帧校验；宿主运行不替代 iOS 设备上的故障生命周期验收。"""

import platform
import importlib.util
import re
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
        subprocess.run(["xcrun", "clang", "-std=c11", "-gline-tables-only", "-Wall", "-Wextra", "-Werror",
                        str(ROOT / "platforms/ios/SharedUI/core/CrashSignalRecorder.c"),
                        str(ROOT / "platforms/ios/tests/CrashSignalRecorderTest.c"), "-o", str(ios)], check=True)
        subprocess.run([str(ios), str(root / "synthetic.crash")], check=True)
        # 用实际故障帧的模块偏移与 UUID 选择符号，不能拿编译时的函数地址冒充还原。
        spec = importlib.util.spec_from_file_location("apple_crash_symbols", ROOT / "platforms/macos/scripts/crash_symbols.py")
        symbols = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(symbols)
        symbols.collect(ios, root / "archive", Path(str(ios) + ".dSYM"), "1.2.3")
        frame = (root / "synthetic.crash").read_text().splitlines()[2]
        match = re.fullmatch(r"ios-recorder\+(0x[0-9a-f]+) \[id 0x([0-9a-f]{32})\]", frame)
        assert match, frame
        result = symbols.symbolize(root / "archive", "ios-recorder", match[2], platform.machine(), int(match[1], 16))
        assert "synthetic_fault" in result and "CrashSignalRecorderTest.c:" in result, result
        source = root / "synthetic-library.c"
        source.write_text("__attribute__((noinline)) void synthetic_library_fault(void *page) { *(volatile char *)page = 7; }\n")
        library = root / "synthetic-library.dylib"
        subprocess.run(["xcrun", "clang", "-dynamiclib", "-gline-tables-only", str(source), "-o", str(library)], check=True)
        record = root / "library.crash"
        subprocess.run([str(ios), str(record), str(library)], check=True)
        symbols.collect(library, root / "library-archive", Path(str(library) + ".dSYM"), "1.2.3")
        match = re.fullmatch(r"synthetic-library\.dylib\+(0x[0-9a-f]+) \[id 0x([0-9a-f]{32})\]", record.read_text().splitlines()[2])
        assert match, record.read_text()
        result = symbols.symbolize(root / "library-archive", library.name, match[2], platform.machine(), int(match[1], 16))
        assert "synthetic_library_fault" in result and "synthetic-library.c:1" in result, result
    print("Apple 共同记录器与 iOS C 记录器：真实内存故障首帧、模块 UUID 与源码还原通过")


if __name__ == "__main__":
    main()
