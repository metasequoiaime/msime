#!/usr/bin/env python3
"""脚本插件只在 4 个桌面入口里打开：Windows TIP 和其余进程从不调用 msime_client_scripts_enable。

TIP 和 Server 共用同一份宿主 DLL，TIP 被注入每个应用进程（包括 System32 部署时的游戏和提权窗口），所以解释器的代码一定在 TIP 加载的 DLL 里；只有"从不调用这个开关"保证它从不执行。设置应用和 MCP 也链接 host-api，只做清单和模块校验，同样不能打开它。这里检查：4 个入口各调用一次 msime_client_scripts_enable(true)，仓库里其余任何源码（host-api 里定义它的 ffi/lifecycle.rs 和测试除外）都不调用它、不按名字取它的地址，也不绕过 C 入口直接调用 host-api 内部的 scripts::enable，platforms/windows/tsf 下一处都没有。host-api 的其余源码同样受检：msime_client_create 这类 TIP 也会走到的入口里多一行调用，和在 TIP 里调用一样危险。
"""

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
CALL = "msime_client_scripts_enable"
# 调用或取地址（dlsym、函数指针）都算；文档里只提名字不算。
REFERENCE = re.compile(r"\bmsime_client_scripts_enable\s*\(|\"msime_client_scripts_enable\"|&msime_client_scripts_enable\b|\bscripts::enable\s*\(")
ENTRY_POINTS = (
    "platforms/macos/src/input/input_method_main.mm",
    "platforms/windows/src/entrypoints/server_main.cpp",
    "platforms/linux/src/entrypoints/ibus_main.cpp",
    "platforms/linux/fcitx5/FcitxEngine.cpp",
)
# 定义、头文件声明和 host-api 自己的测试。只豁免定义它的那个文件，不豁免整个 src/。
DEFINITIONS = (
    "crates/host-api/src/ffi/lifecycle.rs",
    "crates/host-api/src/tests/",
    "crates/host-api/include/msime_client.h",
    "crates/host-api/tests/",
)
SOURCE_SUFFIXES = {
    ".c", ".cc", ".cpp", ".h", ".hpp", ".m", ".mm", ".swift", ".rs", ".kt", ".java",
    ".ets", ".ts", ".tsx", ".js", ".mjs", ".py", ".cs",
}
SKIPPED_DIRECTORIES = {"node_modules", "target", ".git", "build", "dist", "third_party", "oh_modules"}


def sources():
    for top in ("platforms", "apps", "crates", "packages", "shared"):
        stack = [ROOT / top]
        while stack:
            directory = stack.pop()
            for entry in directory.iterdir():
                if entry.is_dir():
                    if entry.name not in SKIPPED_DIRECTORIES and not entry.is_symlink():
                        stack.append(entry)
                elif entry.suffix in SOURCE_SUFFIXES:
                    yield entry


def main() -> int:
    failures = []
    for path in ENTRY_POINTS:
        count = (ROOT / path).read_text(encoding="utf-8").count(f"{CALL}(true);")
        if count != 1:
            failures.append(f"{path}: expected one {CALL}(true); call, found {count}")
    for path in sources():
        relative = path.relative_to(ROOT).as_posix()
        if relative in ENTRY_POINTS or relative.startswith(DEFINITIONS):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        if REFERENCE.search(text):
            where = "the Windows TIP" if relative.startswith("platforms/windows/tsf/") else "a process that must not run scripts"
            failures.append(f"{relative}: {CALL} in {where}")
    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print("script plugins are switched on only by the four desktop entry points, never by the Windows TIP")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
