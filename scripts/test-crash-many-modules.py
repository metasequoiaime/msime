#!/usr/bin/env python3
"""实际加载超过旧快照上限的独立模块，再验证末尾库真实故障的身份与偏移。"""

import platform
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    compiler = shutil.which("c++")
    system = platform.system()
    if not compiler or system not in ("Linux", "Darwin"):
        print("skipped: 需要 Linux/macOS 的 C++ 编译器与动态加载器")
        return
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    with tempfile.TemporaryDirectory(prefix="msime-crash-many-modules-") as directory:
        root = Path(directory)
        source = root / "fault.cpp"
        source.write_text('extern "C" __attribute__((noinline)) void synthetic_library_fault(void *page) { *(volatile char *)page = 7; }\n')
        library = root / "source.so"
        shared = ["-dynamiclib"] if system == "Darwin" else ["-shared", "-fPIC", "-Wl,--build-id=sha1"]
        subprocess.run([compiler, *shared, str(source), "-o", str(library)], check=True)
        for index in range(620):
            shutil.copyfile(library, root / f"synthetic-{index}.so")
        shutil.copyfile(library, root / "late.so")
        binary = root / "recorder"
        subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror",
                        str(ROOT / "platforms/common/CrashRecorder.cpp"),
                        str(ROOT / "platforms/common/tests/crash_many_modules.cpp"),
                        *(["-ldl"] if system == "Linux" else []), "-o", str(binary)], check=True)
        subprocess.run([str(binary), str(root)], check=True)
    print("620 个独立动态模块后的真实故障：末尾模块身份、相对偏移和原有致命处置通过；武装后加载的库在刷新模块快照后同样对得上")


if __name__ == "__main__":
    main()
