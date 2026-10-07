#!/usr/bin/env python3
"""从版本表 `shared/contracts/editions.json` 生成各版本的 Windows 身份。

多个版本可以同时装在一台 Windows 上，彼此完全隔离：每个版本有自己的 TSF CLSID 和 profile、自己的一组 TSF 内部 GUID、命名管道、命名事件、互斥量、窗口类名、HKLM 注册表键、安装目录、状态目录、看门狗计划任务、host DLL 名和 Inno AppId。两个版本的 TIP 还可能被同一个应用同时加载，所以连保留键和 compartment 的 GUID 也各不相同。

这些值只在版本表里写一次，这里把它们变成两个提交进仓库的文件，构建时不需要 Python：

- `shared/contracts/msime_edition.h`：C++ 侧（TSF DLL、Server、看门狗、prepare 工具、WinUI 设置窗口和原生测试）读的宏。构建必须定义且只定义一个 `MSIME_EDITION_<ID>`（CMake 的 `MSIME_EDITION` 缓存变量、设置窗口工程的 `MsimeEdition` 属性），否则头文件以 `#error` 拒绝编译，不会悄悄编成 full；
- `platforms/windows/installer/editions.iss`：Inno Setup 读的 `#define`，按 `ISCC /DEdition=<id>` 选出一组，缺省是 full。

full 和其他版本一样有自己的一组 Windows 身份：名字后缀是 `.full`，GUID、安装目录、注册表键、环境变量、看门狗任务和安装包名都是 full 自己的。引入版本之前的那组值（空后缀、`{E3062E9A-...}` 等）是另一个产品 msime-windows（https://github.com/metasequoiaime/msime-windows）的身份，记在下面的 `MSIME_WINDOWS` 里：本仓库的任何版本都不能用它们，否则装上或卸掉本仓库的某个版本就会覆盖或删掉 msime-windows（`scripts/test-editions.py` 检查）；每个版本的安装器还把 msime-windows 的注册表键和安装目录当作别的产品的数据目录来避让（见 `inno_text`）。

用法：

    edition_windows.py gen [--check]                 # 重新生成上面两个文件；--check 只比较，有差异时以非零状态退出
    edition_windows.py editions                      # 有 Windows 段的版本 id，逗号分隔，full 在最前
    edition_windows.py field --edition ID KEY        # 打印 Windows 段的字段，或 id、display_name.zh-Hans/en、default_scheme、arm64_host_dll
    edition_windows.py marker --edition ID --output edition.json   # Server 目录里的版本声明，每个版本（包括 full）都写
    edition_windows.py host-def --edition ID --dll msime_host_api.dll --output msime_host_api_<id>.def [--arm64]
                                                     # 按 DLL 的导出表写出改名用的模块定义文件，交给 lib.exe /DEF 或 dlltool 生成同名导入库；--arm64 用 ARM64 宿主的名字（见 arm64_host_dll）
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
# 数据目录所有权标记的文件名前缀，后面接版本的名字后缀（full 的标记是 `.metasequoiaime-data.full`）。几个版本的标记文件名各不相同：一个版本的安装器和 Server 看不到别的版本的标记，不会把别的版本的数据目录当成自己的去接管、清理或删除。不带后缀的 `.metasequoiaime-data` 是 msime-windows 的标记，同样以这个前缀开头，所以每个版本的安装器都把它当作别人的标记。
DATA_DIR_MARKER_PREFIX = ".metasequoiaime-data"
# msime-windows（另一个维护者的独立产品，https://github.com/metasequoiaime/msime-windows）在 Windows 上的身份，取自它的 `installer/msime_setup.iss`、TSF 的 Globals.cpp 和 Server。本仓库的 full 在引入版本时沿用了这组值，装上或卸掉 full 就会覆盖或删掉 msime-windows；现在 full 有自己的一组，这组值只用来避让：`scripts/test-editions.py` 检查没有任何版本的 Windows 标识等于其中的值，`inno_text` 把它的注册表键和安装目录加进每个版本安装器的「别的产品的数据目录」名单。它不写 state_directory、user_data_directory、host_dll 和 tauri_identifier：msime-windows 不用本仓库的这几个名字。
MSIME_WINDOWS = {
    "clsid": "{E3062E9A-D834-4637-8958-ED8CFA427D01}",
    "profile_guid": "{4D59B1B4-D503-44AE-9259-BAD9BB2778AB}",
    "tsf_guids": {
        "preserve_key_ime_mode": "{34764E82-AE6D-4F71-BB3A-96799AECE466}",
        "preserve_key_ime_mode_02": "{748C1D81-246B-4849-921F-143BA2BED3F5}",
        "preserve_key_ime_mode_03": "{B7E4F2A1-9C3D-4E8F-A1B2-C3D4E5F60718}",
        "preserve_key_english_input_mode": "{D625C0B1-5A8F-4CC4-9C65-6C536BFF2D91}",
        "preserve_key_double_single_byte": "{4393748A-89DC-485C-A7F7-5FA232CEC70B}",
        "preserve_key_punctuation": "{628DDA3B-38D8-4521-BDD4-85CA38F475B8}",
        "compartment_double_single_byte": "{851BC7CB-8395-4FA6-9C95-DB6EFC2E648E}",
        "compartment_punctuation": "{58DA9E0F-88B2-426F-91C8-802C9B4D9115}",
        "langbar_ime_mode": "{94B8FD94-E918-4667-93BE-57A49D35B02D}",
        "langbar_double_single_byte": "{3E044725-9617-402E-B113-9865AD9B4F8E}",
        "langbar_punctuation": "{596E7EE3-B629-4895-A5B0-C60A82B47A04}",
        "display_attribute_input": "{688746FF-BAF2-4153-93ED-96943436422F}",
        "display_attribute_converted": "{1E2209EA-13CD-4550-8A8F-B352E9744DF2}",
        "candidate_ui_element": "{9FFF12AA-B5EE-4477-A1AA-A4BF5F7B2447}",
    },
    "inno_app_id": "{A7C3E91F-4B2D-4E8A-9F1C-6D5E8B0A2C4D}",
    # 开始菜单文件夹和快捷方式按 app_name 命名，系统键盘列表显示 text_service_description；同名时两边共用开始菜单文件夹，卸载一个就带走另一个的快捷方式。
    "app_name": "Metasequoia IME 水杉输入法",
    "text_service_description": "Metasequoia 水杉输入法",
    "install_dir": "metasequoiaime",
    "registry_key": "Software\\Metasequoia\\MetasequoiaIME",
    "data_dir_environment_variable": "METASEQUOIA_IME_DATA_DIR",
    # 管道、事件、互斥量和窗口类名不带后缀（`FanyImeNamedPipe`、`Local\MetasequoiaImeServer_SingleInstance` 等）。
    "name_suffix": "",
    "watchdog_task": "Metasequoia IME Watchdog",
    "installer_base_name": "MetasequoiaIME_Setup",
    "data_dir_marker": DATA_DIR_MARKER_PREFIX,
}
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
        "// 每个版本（包括 full）的管道、事件、互斥量和窗口类名都带 `.<id>` 后缀，GUID 和路径各不相同，所以几个版本可以同时安装，两个版本的 TIP 也可以被同一个应用同时加载。不带后缀的名字和引入版本之前的那组 GUID 属于 msime-windows（edition_windows.py 的 MSIME_WINDOWS），本仓库的版本都不用，所以也能和 msime-windows 同时安装。",
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
            # 手写模型只认汉字：为 0 的版本（日文、越南文和藏文版）托盘菜单、悬浮工具栏和原生设置窗口都不提供手写。
            ("MSIME_EDITION_HANDWRITING", "1" if entry["features"]["handwriting"] else "0"),
        ]
        lines += [f"#define {name} {value}" for name, value in definitions]
    lines.append("#endif")
    lines.append("")
    lines.append("// 全部有 Windows 段的版本的名字后缀，与本次构建选的是哪个版本无关：一个版本要知道别的版本的 Server 是否在运行（看它们的单实例互斥量）时用。")
    lines.append("#define MSIME_EDITIONS_NAME_SUFFIXES " + ", ".join(wide(entry["platforms"]["windows"]["name_suffix"]) for entry in editions))
    return "\n".join(lines) + "\n"


def inno_text(table: dict) -> str:
    """Inno Setup 的版本定义。只有预处理指令，ISPP 不把指令行写进预处理结果。"""
    editions = windows_editions(table)
    lines = ["#ifndef Edition", '#define Edition "full"', "#endif"]
    # 所有版本的标记文件名都以它开头，msime-windows 的标记就是它本身。安装器拿它找目录里别的版本和 msime-windows 的标记：带着别人标记的目录，哪怕是本版本的默认数据目录，也不归本版本管。
    lines.append(f'#define MyDataDirMarkerPrefix "{DATA_DIR_MARKER_PREFIX}"')
    for name, section in [(entry["id"], entry["platforms"]["windows"]) for entry in editions] + [("msime-windows", MSIME_WINDOWS)]:
        for key in ("registry_key", "install_dir"):
            if "|" in section[key] or "'" in section[key]:
                raise SystemExit(f"{name}: {key} contains | or a single quote, which the installer's list of other editions cannot hold")
    for index, entry in enumerate(editions):
        windows = entry["platforms"]["windows"]
        lines.append(("#if" if index == 0 else "#elif") + f' Edition == "{entry["id"]}"')
        others = [other["platforms"]["windows"] for other in editions if other is not entry] + [MSIME_WINDOWS]
        definitions = [
            ("MyEditionAppName", windows["app_name"]),
            # AppId 以 `{` 开头，Inno 要把它写成 `{{` 才不当常量展开。
            ("MyEditionAppId", "{" + windows["inno_app_id"]),
            ("MyEditionClsid", windows["clsid"]),
            ("MyEditionInstallDir", windows["install_dir"]),
            ("MyEditionRegistryKey", windows["registry_key"]),
            ("MyEditionWatchdogTask", windows["watchdog_task"]),
            ("MyEditionInstallerBaseName", windows["installer_base_name"]),
            ("MyEditionDataDirMarker", data_dir_marker(entry)),
            # 别的版本和 msime-windows（排在最后）的 HKLM 键和安装目录名（默认数据目录是 %LOCALAPPDATA% 下的同名目录），两个列表按同一顺序以 | 分隔。几个版本可以和 msime-windows 同时安装，安装器拿它们找出别人已登记或将来会用的数据目录：本版本的数据目录不能和它们互相嵌套，否则外层的产品卸载或更换数据目录时会递归删掉里层产品还在用的目录。
            ("MyOtherEditionRegistryKeys", "|".join(other["registry_key"] for other in others)),
            ("MyOtherEditionInstallDirs", "|".join(other["install_dir"] for other in others)),
        ]
        for name, value in definitions:
            if '"' in value:
                raise SystemExit(f"edition {entry['id']}: {name} {value!r} contains a double quote, which an ISPP string cannot hold")
            lines.append(f'#define {name} "{value}"')
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
    if key == "arm64_host_dll":
        return arm64_host_dll(windows["host_dll"])
    if key in windows and isinstance(windows[key], str):
        return windows[key]
    raise SystemExit(f"unknown field {key}")


def marker(edition_id: str) -> str:
    """Server 目录里 `edition.json` 的内容（`Edition::PACKAGE_MARKER_FILE`）。每个版本（包括 full）都写：没有这个文件的进程虽然也按 full 运行，但写明了更不会被误认。"""
    edition_entry(edition_id)
    return json.dumps({"edition": edition_id}) + "\n"


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


def arm64_host_dll(host_dll: str) -> str:
    """Windows on Arm 上 Arm64X TIP 的原生 ARM64 那一半导入的宿主 DLL 名。它与 x64 宿主装在同一个版本目录里，ARM64EC 那一半在模拟的 x64 进程里导入 x64 宿主（`host_dll`），所以 ARM64 宿主换一个名字：`msime_host_api.dll` 对应 `msime_host_api_arm64.dll`。"""
    stem, dot, extension = host_dll.rpartition(".")
    if not dot or extension.lower() != "dll":
        raise SystemExit(f"host DLL name {host_dll} does not end in .dll")
    return f"{stem}_arm64.{extension}"


def host_def(edition_id: str, dll: pathlib.Path, arm64: bool = False) -> str:
    """把 msime-host-api 的 DLL 改成本版本的名字（版本表 `host_dll`，`arm64` 时是 `arm64_host_dll` 的名字）时用的模块定义文件。导出的函数与原 DLL 一个不差，只换模块名；拿它生成的导入库让 TSF DLL、Server 和设置窗口按新名字加载。"""
    name = edition_entry(edition_id)["platforms"]["windows"]["host_dll"]
    if arm64:
        name = arm64_host_dll(name)
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
            command.add_argument("--arm64", action="store_true")
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
        args.output.write_text(host_def(args.edition, args.dll, args.arm64), encoding="utf-8", newline="\n")
        return 0
    if args.command == "marker":
        args.output.write_text(marker(args.edition), encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
