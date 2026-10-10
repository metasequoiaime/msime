#!/usr/bin/env python3
"""Keep the Windows state-directory names in step between C++ and Rust.

The TSF DLL and the Server resolve their state root through `platforms/windows/common/StateDirectory.h`. The Rust host (`crates/host-windows`, `server_state_directory`) keeps its own copy of the same order, and the shell it serves has to land on the directory the Server chose. A renamed environment variable, registry key, value or folder on one side would split state silently, so this compares the two lookups and fails on any drift. It also fails when the TSF, the Server or the native settings window grows its own copy of the lookup again instead of calling the shared header.

环境变量名、注册表键和目录名按版本取：C++ 侧来自 `shared/contracts/msime_edition.h` 的宏，Rust 侧来自版本表 `platforms.windows` 解析出的 `WindowsIdentity`，两边都从 `shared/contracts/editions.json` 生成或读取。所以这里核对的是两边各自读的是版本表里的哪个字段，再核对生成的头文件里每个版本的宏值就是版本表里那个字段的值。
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HEADER = ROOT / "platforms/windows/common/StateDirectory.h"
EDITION_HEADER = ROOT / "shared/contracts/msime_edition.h"
EDITIONS = ROOT / "shared/contracts/editions.json"
RUST = ROOT / "crates/host-windows/src/lib.rs"
CALLERS = [
    ROOT / "platforms/windows/tsf/HostOptionsPaths.cpp",
    ROOT / "platforms/windows/src/entrypoints/server_main.cpp",
    ROOT / "platforms/windows/settings/main.cpp",
]
# StateDirectory.h 的常量、它取值的宏，以及这个宏对应的版本表字段和 Rust 侧 `WindowsIdentity` 的字段（两者同名）。
NAMES = {
    "state_directory_environment_variable": ("MSIME_EDITION_DATA_DIR_ENVIRONMENT_VARIABLE", "data_dir_environment_variable"),
    "state_directory_registry_key": ("MSIME_EDITION_REGISTRY_KEY", "registry_key"),
    "state_directory_folder_name": ("MSIME_EDITION_STATE_DIRECTORY", "state_directory"),
}
REGISTRY_VALUE = "DataDir"


def header_constant(source: str, name: str) -> str:
    match = re.search(rf"inline constexpr wchar_t {name}\[\] = ([^;]*);", source)
    if not match:
        raise SystemExit(f"{HEADER.relative_to(ROOT)}: {name} not found")
    return match.group(1).strip()


def rust_body(source: str, name: str) -> str:
    match = re.search(rf"\bfn {name}\b.*?\n\}}\n", source, re.DOTALL)
    if not match:
        raise SystemExit(f"{RUST.relative_to(ROOT)}: fn {name} not found")
    return match.group(0)


def edition_macros(source: str) -> dict[str, dict[str, str]]:
    """生成的头文件里每个版本选择分支定义的宏：版本 id 到宏名到值。"""
    branches: dict[str, dict[str, str]] = {}
    current: dict[str, str] | None = None
    for line in source.splitlines():
        selector = re.match(r"#(?:el)?if defined\(MSIME_EDITION_([A-Z0-9]+)\)", line)
        if selector:
            current = branches.setdefault(selector.group(1).lower(), {})
            continue
        if line.startswith("#endif"):
            current = None
            continue
        definition = re.match(r"#define (\w+) (.*)", line)
        if definition and current is not None:
            current[definition.group(1)] = definition.group(2)
    return branches


def wide_value(literal: str) -> str | None:
    """`L"..."` 宏值解码成字符串；不是单个宽字符串字面量时为 None。"""
    match = re.fullmatch(r'L"((?:[^"\\]|\\.)*)"', literal)
    if not match:
        return None
    return re.sub(r"\\u([0-9a-f]{4})|\\(.)", lambda m: chr(int(m.group(1), 16)) if m.group(1) else m.group(2), match.group(1))


def main() -> int:
    header = HEADER.read_text(encoding="utf-8")
    rust = RUST.read_text(encoding="utf-8")
    resolver = rust_body(rust, "server_state_directory")
    registry = rust_body(rust, "installed_data_directory")
    failures: list[str] = []

    if '#include "../../../shared/contracts/msime_edition.h"' not in header:
        failures.append(f"{HEADER.relative_to(ROOT)} no longer includes shared/contracts/msime_edition.h")
    for constant, (macro, field) in NAMES.items():
        if header_constant(header, constant) != macro:
            failures.append(f"{HEADER.relative_to(ROOT)}: {constant} must be {macro}, the edition's {field}")
        if f"identity.{field}" not in resolver:
            failures.append(f"server_state_directory does not use the edition's {field}")
    if "Edition::windows_package_identity()" not in resolver:
        failures.append("server_state_directory does not take its names from the package's edition")
    if "installed_data_directory(&identity.registry_key)" not in resolver:
        failures.append("server_state_directory does not consult the installer's DataDir under the edition's key")
    if header_constant(header, "state_directory_registry_value") != f'L"{REGISTRY_VALUE}"':
        failures.append(f"{HEADER.relative_to(ROOT)}: state_directory_registry_value is no longer {REGISTRY_VALUE}")
    if f'"{REGISTRY_VALUE}\\0"' not in registry:
        failures.append(f"installed_data_directory does not read {REGISTRY_VALUE}")
    for flag in ("HKEY_LOCAL_MACHINE", "RRF_RT_REG_SZ", "RRF_SUBKEY_WOW6464KEY"):
        if header.count(flag) < 1:
            failures.append(f"{HEADER.relative_to(ROOT)} no longer uses {flag}")
        if flag not in registry:
            failures.append(f"installed_data_directory no longer uses {flag}")

    # 生成的头文件里，每个版本的宏值等于版本表里那个字段的值。
    table = json.loads(EDITIONS.read_text(encoding="utf-8"))
    branches = edition_macros(EDITION_HEADER.read_text(encoding="utf-8"))
    for entry in table["editions"]:
        windows = entry["platforms"].get("windows")
        if windows is None:
            continue
        macros = branches.get(entry["id"], {})
        for macro, field in NAMES.values():
            if wide_value(macros.get(macro, "")) != windows[field]:
                failures.append(f"{EDITION_HEADER.relative_to(ROOT)}: edition {entry['id']}'s {macro} is not its {field} {windows[field]!r}; run python3 platforms/windows/scripts/edition_windows.py gen")

    for caller in CALLERS:
        text = caller.read_text(encoding="utf-8")
        relative = caller.relative_to(ROOT)
        if "resolve_state_directory()" not in text:
            failures.append(f"{relative} no longer calls msime::windows::resolve_state_directory")
        for entry in table["editions"]:
            windows = entry["platforms"].get("windows")
            if windows is None:
                continue
            for literal in (windows["data_dir_environment_variable"], REGISTRY_VALUE):
                if f'"{literal}"' in text:
                    failures.append(f"{relative} spells {literal} itself instead of using common/StateDirectory.h")

    if failures:
        for failure in failures:
            print(f"state-directory parity: {failure}", file=sys.stderr)
        return 1
    print("Windows state-directory names match between C++ and Rust: both read the edition's data_dir_environment_variable, HKLM registry_key\\DataDir and state_directory")
    return 0


if __name__ == "__main__":
    sys.exit(main())
