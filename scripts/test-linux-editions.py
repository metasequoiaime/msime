#!/usr/bin/env python3
"""Linux 各版本的身份没有漂移，几个版本同时装、同时跑时互不越界。

版本表的 Linux 段（`scripts/test-editions.py` 检查它本身）由 `platforms/linux/scripts/edition_linux.py` 交给三处：提交进仓库的 `platforms/linux/src/core/LinuxEdition.h`、按版本改写的脚本和数据文件，以及 CMake（`platforms/linux/cmake/Edition.cmake`）。这里检查：

- `LinuxEdition.h` 与版本表一致（`edition_linux.py gen --check`）；
- 每个版本都能改写全部有规则的文件：规则全部命中，改写后不留下 full 的目录名、包名、单元名、命令名、显示名、Fcitx5 插件名或主题名（`edition_linux.py check`）；
- 两个版本的 Fcitx5 插件被同一个 fcitx5 进程加载时，动作名（`MSIME_EDITION_FCITX5_ADDON "-<名字>"` 展开后）两两不撞：fcitx5 的动作名在整个进程里唯一，撞名的那个注册不上，菜单里就少了它；
- C++ 源码不再自己写 full 的每用户目录、IBus 引擎名和设置命令，CMake 不再自己写 `msime-client` 子目录，否则那一处在其他版本里仍指向 full。
"""

from __future__ import annotations

import importlib.util
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LINUX = ROOT / "platforms/linux"
GENERATOR = LINUX / "scripts/edition_linux.py"
FCITX_ENGINE = LINUX / "fcitx5/FcitxEngine.cpp"
# C++ 里不能再出现的 full 的字面量：每用户目录、IBus 引擎名、设置命令和主题名都要从 LinuxEdition.h 取。
CPP_FORBIDDEN = [
    (re.compile(r'"msime-client[/"]'), "the per-user directory"),
    (re.compile(r'"/msime-client/'), "the per-user directory"),
    (re.compile(r'"msime-linux"'), "the IBus engine name"),
    (re.compile(r'"msime-linux-(setup|settings)"'), "a user command"),
    (re.compile(r'registerAction\("msime-'), "a Fcitx5 action name"),
    (re.compile(r'kFcitxCandidateTheme = "msime"'), "the Fcitx5 theme name"),
]
CMAKE_FILES = [LINUX / "CMakeLists.txt", LINUX / "fcitx5/CMakeLists.txt", LINUX / "cmake/packaging.cmake"]


def load_generator():
    spec = importlib.util.spec_from_file_location("edition_linux", GENERATOR)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def code_lines(path: pathlib.Path):
    """源码里不是整行注释的行。"""
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.lstrip()
        if not stripped.startswith(("//", "#")) or stripped.startswith(("#define", "#include")):
            yield number, line


def main() -> int:
    errors: list[str] = []
    for command in (["gen", "--check"], ["check"]):
        result = subprocess.run([sys.executable, str(GENERATOR), *command], capture_output=True, text=True)
        if result.returncode != 0:
            errors.append(f"edition_linux.py {' '.join(command)}: {(result.stdout + result.stderr).strip()}")

    generator = load_generator()
    editions = generator.linux_editions()
    suffixes = set(re.findall(r'MSIME_EDITION_FCITX5_ADDON "-([a-z0-9-]+)"', FCITX_ENGINE.read_text(encoding="utf-8")))
    if not suffixes:
        errors.append(f"{FCITX_ENGINE.relative_to(ROOT)} names no action through MSIME_EDITION_FCITX5_ADDON; update this check")
    seen: dict[str, str] = {}
    for entry in editions:
        addon = entry["platforms"]["linux"]["fcitx5_addon"]
        for suffix in sorted(suffixes):
            name = f"{addon}-{suffix}"
            if name in seen:
                errors.append(f"Fcitx5 action {name!r} of edition {entry['id']} is also edition {seen[name]}'s; two editions loaded into one fcitx5 would collide")
            seen[name] = entry["id"]

    sources = sorted((LINUX / "src").rglob("*.cpp")) + sorted((LINUX / "src").rglob("*.h")) + [FCITX_ENGINE]
    for path in sources:
        if path.name == "LinuxEdition.h":
            continue
        for number, line in code_lines(path):
            for pattern, what in CPP_FORBIDDEN:
                if pattern.search(line):
                    errors.append(f"{path.relative_to(ROOT)}:{number}: spells {what} of full; take it from LinuxEdition.h")
    for path in CMAKE_FILES:
        for number, line in code_lines(path):
            if re.search(r"/msime-client[/\"]", line):
                errors.append(f"{path.relative_to(ROOT)}:{number}: spells the msime-client directory; use ${{MSIME_CLIENT_DIRECTORY}}")

    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"linux editions: LinuxEdition.h is current, {len(editions)} editions render every script and data file, {len(suffixes)} Fcitx5 actions stay distinct across editions, and no host source spells the full identifiers itself")
    return 0


if __name__ == "__main__":
    sys.exit(main())
