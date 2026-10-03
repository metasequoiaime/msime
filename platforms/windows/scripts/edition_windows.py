#!/usr/bin/env python3
"""从版本表 `shared/contracts/editions.json` 生成各版本的 Windows 身份。

多个版本可以同时装在一台 Windows 上，彼此完全隔离：每个版本有自己的 TSF CLSID 和 profile、自己的一组 TSF 内部 GUID、命名管道、命名事件、互斥量、窗口类名、HKLM 注册表键、安装目录、状态目录、看门狗计划任务、host DLL 名和 Inno AppId。两个版本的 TIP 还可能被同一个应用同时加载，所以连保留键和 compartment 的 GUID 也各不相同。

这些值只在版本表里写一次，这里把它们变成两个提交进仓库的文件，构建时不需要 Python：

- `shared/contracts/msime_edition.h`：C++ 侧（TSF DLL、Server、看门狗、prepare 工具、WinUI 设置窗口和原生测试）读的宏。构建必须定义且只定义一个 `MSIME_EDITION_<ID>`（CMake 的 `MSIME_EDITION` 缓存变量、设置窗口工程的 `MsimeEdition` 属性），否则头文件以 `#error` 拒绝编译，不会悄悄编成 full；
- `platforms/windows/installer/editions.iss`：Inno Setup 读的 `#define`，按 `ISCC /DEdition=<id>` 选出一组，缺省是 full。文件里只有预处理指令，所以 full 的预处理输出与引入版本之前逐字节相同。

full 是现有产品本身：它的名字后缀是空串，管道、事件、互斥量和窗口类名与引入版本之前相同，GUID 和路径都是今天写死在代码里的值（`scripts/test-editions.py` 检查）。

用法：

    edition_windows.py gen [--check]                 # 重新生成上面两个文件；--check 只比较，有差异时以非零状态退出
    edition_windows.py editions                      # 有 Windows 段的版本 id，逗号分隔，full 在最前
    edition_windows.py field --edition ID KEY        # 打印 Windows 段的字段，或 id、display_name.zh-Hans/en、default_scheme
    edition_windows.py marker --edition ID --output edition.json   # Server 目录里的版本声明；full 不写，已有的删掉
    edition_windows.py host-def --edition ID --dll msime_host_api.dll --output msime_host_api_<id>.def
                                                     # 按 DLL 的导出表写出改名用的模块定义文件，交给 lib.exe /DEF 或 dlltool 生成同名导入库
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
EDITIONS = ROOT / "shared/contracts/editions.json"
HEADER = ROOT / "shared/contracts/msime_edition.h"
INNO = ROOT / "platforms/windows/installer/editions.iss"
FULL = "full"
# 数据目录所有权标记的文件名前缀，后面接版本的名字后缀（full 是空串，所以 full 的标记仍是 `.metasequoiaime-data`）。几个版本的标记文件名各不相同：一个版本的安装器和 Server 看不到别的版本的标记，不会把别的版本的数据目录当成自己的去接管、清理或删除。
DATA_DIR_MARKER_PREFIX = ".metasequoiaime-data"
# 版本表 tsf_guids 的键到 TSF Globals.cpp 里的宏名后缀，顺序就是生成文件里的顺序。
TSF_GUIDS = [
    "preserve_key_ime_mode",
    "preserve_key_ime_mode_02",
    "preserve_key_ime_mode_03",
    "preserve_key_english_input_mode",
    "preserve_key_double_single_byte",
    "preserve_key_punctuation",
    "compartment_double_single_byte",
    "compartment_punctuation",
    "langbar_ime_mode",
    "langbar_double_single_byte",
    "langbar_punctuation",
    "display_attribute_input",
    "display_attribute_converted",
    "candidate_ui_element",
]


def load_table() -> dict:
    return json.loads(EDITIONS.read_text(encoding="utf-8"))


def windows_editions(table: dict | None = None) -> list[dict]:
    """有 Windows 段的版本，顺序与版本表相同（full 在最前）。"""
    table = table or load_table()
    return [entry for entry in table["editions"] if entry["platforms"].get("windows") is not None]


def edition_entry(edition_id: str, table: dict | None = None) -> dict:
    for entry in windows_editions(table):
        if entry["id"] == edition_id:
            return entry
    raise SystemExit(f"edition {edition_id!r} has no Windows identifiers in {EDITIONS.relative_to(ROOT)}")


def guid_initializer(guid: str) -> str:
    """`{E3062E9A-D834-4637-8958-ED8CFA427D01}` 写成 GUID 结构体的初始化列表。"""
    parts = guid.strip("{}").split("-")
    tail = parts[3] + parts[4]
    data4 = ", ".join(f"0x{tail[i:i + 2].lower()}" for i in range(0, 16, 2))
    return f"{{0x{parts[0].lower()}, 0x{parts[1].lower()}, 0x{parts[2].lower()}, {{{data4}}}}}"


def wide(text: str) -> str:
    """宽字符串字面量。非 ASCII 字符写成 `\\uXXXX`，不依赖编译器把源文件当成哪种编码。"""
    out = []
    for character in text:
        if character in '\\"':
            out.append("\\" + character)
        elif 0x20 <= ord(character) < 0x7F:
            out.append(character)
        elif ord(character) <= 0xFFFF:
            out.append(f"\\u{ord(character):04x}")
        else:
            out.append(f"\\U{ord(character):08x}")
    return 'L"' + "".join(out) + '"'


def narrow(text: str) -> str:
    """UTF-8 窄字符串字面量。非 ASCII 字节写成三位八进制转义：八进制转义最多吃三位，不会像 `\\x` 那样把后面的字母也吞进去。"""
    out = []
    for byte in text.encode("utf-8"):
        character = chr(byte)
        if character in '\\"':
            out.append("\\" + character)
        elif 0x20 <= byte < 0x7F:
            out.append(character)
        else:
            out.append(f"\\{byte:03o}")
    return '"' + "".join(out) + '"'


def data_dir_marker(entry: dict) -> str:
    """数据目录所有权标记的文件名。名字后缀在版本之间唯一（`scripts/test-editions.py` 检查），所以标记文件名也唯一。"""
    return DATA_DIR_MARKER_PREFIX + entry["platforms"]["windows"]["name_suffix"]


def macro_suffix(edition_id: str) -> str:
    return edition_id.upper()


def header_text(table: dict) -> str:
    editions = windows_editions(table)
    selectors = [f"MSIME_EDITION_{macro_suffix(entry['id'])}" for entry in editions]
    defined = " + ".join(f"defined({selector})" for selector in selectors)
    lines = [
        "#pragma once",
        "",
        "// 由 platforms/windows/scripts/edition_windows.py 从 shared/contracts/editions.json 生成，不要手改；改了版本表之后运行 `python3 platforms/windows/scripts/edition_windows.py gen` 并提交结果。",
        "//",
        f"// 每个 Windows 构建必须定义且只定义一个版本选择宏（{', '.join(selectors)}）：CMake 按缓存变量 MSIME_EDITION 定义，WinUI 设置窗口工程按 MsimeEdition 属性定义。少了它就停在这里，而不是悄悄编成 full、去用 full 的管道和 CLSID。",
        "//",
        "// full 的名字后缀是空串，管道、事件、互斥量和窗口类名与引入版本之前相同；其他版本的这些名字都带 `.<id>`，GUID 和路径各不相同，所以几个版本可以同时安装，两个版本的 TIP 也可以被同一个应用同时加载。",
        "",
        f"#if ({defined}) != 1",
        f'#error "Define exactly one of {", ".join(selectors)}; platforms/windows/CMakeLists.txt does this from MSIME_EDITION"',
        "#endif",
    ]
    for index, entry in enumerate(editions):
        windows = entry["platforms"]["windows"]
        selector = selectors[index]
        lines.append(("#if" if index == 0 else "#elif") + f" defined({selector})")
        definitions = [
            ("MSIME_EDITION_ID", narrow(entry["id"])),
            ("MSIME_EDITION_IS_FULL", "1" if entry["id"] == FULL else "0"),
            ("MSIME_EDITION_NAME_SUFFIX", wide(windows["name_suffix"])),
            ("MSIME_EDITION_DISPLAY_NAME", wide(entry["display_name"]["zh-Hans"])),
            ("MSIME_EDITION_DISPLAY_NAME_UTF8", narrow(entry["display_name"]["zh-Hans"])),
            ("MSIME_EDITION_TEXT_SERVICE_DESCRIPTION", wide(windows["text_service_description"])),
            ("MSIME_EDITION_LANGID", windows["langid"]),
            ("MSIME_EDITION_LANGID_STRING", wide(windows["langid"])),
            ("MSIME_EDITION_CLSID", guid_initializer(windows["clsid"])),
            ("MSIME_EDITION_CLSID_STRING", wide(windows["clsid"])),
            ("MSIME_EDITION_PROFILE_GUID", guid_initializer(windows["profile_guid"])),
            ("MSIME_EDITION_PROFILE_GUID_STRING", wide(windows["profile_guid"])),
        ]
        definitions += [(f"MSIME_EDITION_GUID_{key.upper()}", guid_initializer(windows["tsf_guids"][key])) for key in TSF_GUIDS]
        definitions += [
            ("MSIME_EDITION_REGISTRY_KEY", wide(windows["registry_key"])),
            ("MSIME_EDITION_STATE_DIRECTORY", wide(windows["state_directory"])),
            ("MSIME_EDITION_USER_DATA_DIRECTORY", wide(windows["user_data_directory"])),
            ("MSIME_EDITION_DATA_DIR_ENVIRONMENT_VARIABLE", wide(windows["data_dir_environment_variable"])),
            ("MSIME_EDITION_WATCHDOG_TASK", wide(windows["watchdog_task"])),
            ("MSIME_EDITION_HOST_DLL", wide(windows["host_dll"])),
            ("MSIME_EDITION_DATA_DIR_MARKER", wide(data_dir_marker(entry))),
            ("MSIME_EDITION_DEFAULT_SCHEME", narrow(entry["default_scheme"])),
            ("MSIME_EDITION_DEFAULT_SCHEME_W", wide(entry["default_scheme"])),
            ("MSIME_EDITION_WUBI_MIXED_PINYIN_DEFAULT", "1" if entry["preference_defaults"].get("wubi_mixed_pinyin", False) else "0"),
            ("MSIME_EDITION_INPUT_SCHEMES", ", ".join(narrow(scheme) for scheme in entry["input_schemes"])),
            ("MSIME_EDITION_TEMPORARY_JAPANESE", "1" if entry["features"]["temporary_japanese"] else "0"),
        ]
        lines += [f"#define {name} {value}" for name, value in definitions]
    lines.append("#endif")
    lines.append("")
    lines.append("// 全部有 Windows 段的版本的名字后缀，与本次构建选的是哪个版本无关：一个版本要知道别的版本的 Server 是否在运行（看它们的单实例互斥量）时用。")
    lines.append("#define MSIME_EDITIONS_NAME_SUFFIXES " + ", ".join(wide(entry["platforms"]["windows"]["name_suffix"]) for entry in editions))
    return "\n".join(lines) + "\n"


def inno_text(table: dict) -> str:
    """Inno Setup 的版本定义。只有预处理指令：ISPP 不把指令行写进预处理结果，所以引入它不改变 full 的预处理输出。"""
    editions = windows_editions(table)
    lines = ["#ifndef Edition", '#define Edition "full"', "#endif"]
    for index, entry in enumerate(editions):
        windows = entry["platforms"]["windows"]
        lines.append(("#if" if index == 0 else "#elif") + f' Edition == "{entry["id"]}"')
        definitions = [
            ("MyEditionIsFull", "1" if entry["id"] == FULL else "0"),
            ("MyEditionAppName", windows["app_name"]),
            # AppId 以 `{` 开头，Inno 要把它写成 `{{` 才不当常量展开。
            ("MyEditionAppId", "{" + windows["inno_app_id"]),
            ("MyEditionClsid", windows["clsid"]),
            ("MyEditionInstallDir", windows["install_dir"]),
            ("MyEditionRegistryKey", windows["registry_key"]),
            ("MyEditionWatchdogTask", windows["watchdog_task"]),
            ("MyEditionInstallerBaseName", windows["installer_base_name"]),
            ("MyEditionDataDirMarker", data_dir_marker(entry)),
        ]
        for name, value in definitions:
            if '"' in value:
                raise SystemExit(f"edition {entry['id']}: {name} {value!r} contains a double quote, which an ISPP string cannot hold")
            lines.append(f'#define {name} "{value}"' if name != "MyEditionIsFull" else f"#define {name} {value}")
    lines.append("#else")
    lines.append(f'#error Unknown edition; use /DEdition= one of {", ".join(entry["id"] for entry in editions)}')
    lines.append("#endif")
    return "\n".join(lines) + "\n"


def generated_files(table: dict) -> dict[pathlib.Path, str]:
    return {HEADER: header_text(table), INNO: inno_text(table)}


def drift(files: dict[pathlib.Path, str]) -> list[str]:
    problems = []
    for path, text in files.items():
        relative = path.relative_to(ROOT)
        if not path.is_file():
            problems.append(f"{relative} is missing")
        elif path.read_text(encoding="utf-8") != text:
            problems.append(f"{relative} differs from the generated content")
    return problems


def gen(check: bool) -> int:
    files = generated_files(load_table())
    if check:
        problems = drift(files)
        for problem in problems:
            print(f"FAIL: {problem}; run python3 platforms/windows/scripts/edition_windows.py gen")
        return 1 if problems else 0
    for path, text in files.items():
        if not path.is_file() or path.read_text(encoding="utf-8") != text:
            path.write_text(text, encoding="utf-8", newline="\n")
            print(f"wrote {path.relative_to(ROOT)}")
    return 0


def field(entry: dict, key: str) -> str:
    if key == "id":
        return entry["id"]
    if key == "default_scheme":
        return entry["default_scheme"]
    if key.startswith("display_name."):
        return entry["display_name"][key.split(".", 1)[1]]
    windows = entry["platforms"]["windows"]
    if key in windows and isinstance(windows[key], str):
        return windows[key]
    raise SystemExit(f"unknown field {key}")


def marker(edition_id: str) -> str | None:
    """Server 目录里 `edition.json` 的内容（`Edition::PACKAGE_MARKER_FILE`）。full 不带这个文件，所以 full 的包与引入版本之前相同。"""
    edition_entry(edition_id)
    return None if edition_id == FULL else json.dumps({"edition": edition_id}) + "\n"


def pe_exports(dll: bytes) -> list[str]:
    """PE 文件导出表里的全部函数名，按导出表的顺序。只读文件头和导出目录，不需要 Windows 上的 dumpbin 或 MinGW 的 objdump。"""
    def u16(offset: int) -> int:
        return int.from_bytes(dll[offset:offset + 2], "little")

    def u32(offset: int) -> int:
        return int.from_bytes(dll[offset:offset + 4], "little")

    pe = u32(0x3C)
    if dll[:2] != b"MZ" or dll[pe:pe + 4] != b"PE\0\0":
        raise SystemExit("not a PE file")
    sections = u16(pe + 6)
    optional = pe + 24
    optional_size = u16(pe + 20)
    magic = u16(optional)
    # PE32（0x10b）的数据目录从可选头的第 96 字节开始，PE32+（0x20b）从第 112 字节开始；第 0 项是导出表。
    directory = optional + (96 if magic == 0x10B else 112)
    export_rva = u32(directory)
    table = optional + optional_size

    def offset(rva: int) -> int:
        for index in range(sections):
            header = table + 40 * index
            virtual_size, virtual_address = u32(header + 8), u32(header + 12)
            raw_size, raw_pointer = u32(header + 16), u32(header + 20)
            if virtual_address <= rva < virtual_address + max(virtual_size, raw_size):
                return rva - virtual_address + raw_pointer
        raise SystemExit(f"RVA {rva:#x} is outside every section")

    if export_rva == 0:
        return []
    exports = offset(export_rva)
    count = u32(exports + 24)
    names = offset(u32(exports + 32))
    result = []
    for index in range(count):
        start = offset(u32(names + 4 * index))
        result.append(dll[start:dll.index(b"\0", start)].decode("ascii"))
    return result


def host_def(edition_id: str, dll: pathlib.Path) -> str:
    """把 msime-host-api 的 DLL 改成本版本的名字（版本表 `host_dll`）时用的模块定义文件。导出的函数与原 DLL 一个不差，只换模块名；拿它生成的导入库让 TSF DLL、Server 和设置窗口按新名字加载。"""
    name = edition_entry(edition_id)["platforms"]["windows"]["host_dll"]
    exports = pe_exports(dll.read_bytes())
    if not exports:
        raise SystemExit(f"{dll} exports nothing")
    return f"LIBRARY {name}\nEXPORTS\n" + "".join(f"    {export}\n" for export in exports)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    generate = commands.add_parser("gen")
    generate.add_argument("--check", action="store_true")
    commands.add_parser("editions")
    for name in ["field", "marker", "host-def"]:
        command = commands.add_parser(name)
        command.add_argument("--edition", required=True)
        if name == "field":
            command.add_argument("key")
        else:
            command.add_argument("--output", type=pathlib.Path, required=True)
        if name == "host-def":
            command.add_argument("--dll", type=pathlib.Path, required=True)
    args = parser.parse_args()

    if args.command == "gen":
        return gen(args.check)
    if args.command == "editions":
        print(",".join(entry["id"] for entry in windows_editions()))
        return 0
    if args.command == "field":
        print(field(edition_entry(args.edition), args.key))
        return 0
    if args.command == "host-def":
        args.output.write_text(host_def(args.edition, args.dll), encoding="utf-8", newline="\n")
        return 0
    if args.command == "marker":
        text = marker(args.edition)
        if text is None:
            args.output.unlink(missing_ok=True)
        else:
            args.output.write_text(text, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
