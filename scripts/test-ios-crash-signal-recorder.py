#!/usr/bin/env python3
"""在宿主 macOS 上编译并运行 iOS 信号记录器的 fork 故障夹具：故障首帧取自 ucontext 的 PC、模块 UUID 与相对偏移、之后加载的映像补登记、前任处理函数恢复执行时撤回记录。"""

import platform
import resource
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RECORDER = ROOT / "platforms/ios/SharedUI/core/CrashSignalRecorder.c"
FIXTURE = ROOT / "platforms/ios/tests/CrashSignalRecorderTest.c"


def main():
    compiler = shutil.which("cc")
    # 夹具用 Darwin 的 ucontext 布局与 Mach-O 映像接口，只能在 macOS 上编译运行。
    if platform.system() != "Darwin" or not compiler:
        print("skipped: 需要 macOS 的 C 编译器（夹具依赖 Darwin ucontext 与 Mach-O 接口）")
        return
    # 夹具会故意让子进程崩溃，不留 core 文件。
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    with tempfile.TemporaryDirectory(prefix="msime-ios-crash-recorder-") as directory:
        root = Path(directory)
        source = root / "fault.c"
        source.write_text("__attribute__((noinline)) void synthetic_library_fault(void *page) { *(volatile char *)page = 7; }\n")
        library = root / "fault.dylib"
        subprocess.run([compiler, "-dynamiclib", str(source), "-o", str(library)], check=True)
        binary = root / "ios-recorder"
        # 夹具靠 assert 判定，不能带 -DNDEBUG。
        subprocess.run([compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", "-UNDEBUG",
                        str(RECORDER), str(FIXTURE), "-o", str(binary)], check=True)
        subprocess.run([str(binary), str(root / "ios-record")], check=True)
        subprocess.run([str(binary), str(root / "ios-record-lib"), str(library)], check=True)
    print("iOS 信号记录器：故障首帧、模块身份与相对偏移、补登记映像、恢复执行撤回记录通过")


if __name__ == "__main__":
    main()
