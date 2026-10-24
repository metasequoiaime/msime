#!/usr/bin/env python3
"""用真实交叉编译产物检查旧系统启动前不能触及的新 DPI 与合成器导入。"""

from pathlib import Path
import re
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parent.parent
COMPILER = shutil.which("x86_64-w64-mingw32-g++")
OBJDUMP = shutil.which("x86_64-w64-mingw32-objdump")
NM = shutil.which("x86_64-w64-mingw32-nm")
MODERN_DPI = {
    "GetDpiForWindow",
    "GetDpiForSystem",
    "SetThreadDpiAwarenessContext",
    "SetProcessDpiAwarenessContext",
    "GetWindowDpiAwarenessContext",
    "GetAwarenessFromDpiAwarenessContext",
}


def main() -> None:
    if not all((COMPILER, OBJDUMP, NM)):
        print("skipped: MinGW 编译器、objdump 或 nm 不可用；Windows CTest 另行执行 DPI 运行测试")
        return

    with tempfile.TemporaryDirectory(prefix="msime-dpi-imports-") as temporary:
        executable = Path(temporary) / "dpi.exe"
        common = [
            COMPILER,
            "-std=c++17",
            "-DUNICODE",
            "-D_UNICODE",
            "-DNOMINMAX",
            "-DWIN32_LEAN_AND_MEAN",
            "-Wall",
            "-Wextra",
            "-Werror",
        ]
        subprocess.run(
            common
            + [
                "-D_WIN32_WINNT=0x0601",
                str(ROOT / "platforms/windows/tests/runtime/windows_dpi.cpp"),
                "-o",
                str(executable),
                "-luser32",
                "-lgdi32",
            ],
            check=True,
        )
        imports = subprocess.check_output([OBJDUMP, "-p", str(executable)], text=True)
        for name in MODERN_DPI:
            if re.search(rf"\b{re.escape(name)}\b", imports):
                raise AssertionError(f"DPI 适配仍静态导入 {name}")

        rendering = Path(temporary) / "device-resources.o"
        subprocess.run(
            common
            + [
                "-D_WIN32_WINNT=0x0A00",
                "-DNTDDI_VERSION=0x0A000003",
                "-I" + str(ROOT / "platforms/windows/msimeui/include"),
                "-c",
                str(ROOT / "platforms/windows/msimeui/src/DeviceResources.cpp"),
                "-o",
                str(rendering),
            ],
            check=True,
        )
        undefined = subprocess.check_output([NM, "--undefined-only", str(rendering)], text=True)
        if "DCompositionCreateDevice" in undefined:
            raise AssertionError("绘制适配仍要求链接 DCompositionCreateDevice")

    print("Windows DPI 与绘制产物没有静态导入现代 DPI 或 DirectComposition 入口")


if __name__ == "__main__":
    main()
