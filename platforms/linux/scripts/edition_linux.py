#!/usr/bin/env python3
"""从版本表 `shared/contracts/editions.json` 生成各版本的 Linux 身份。

多个版本可以同时装在一台 Linux 上，彼此完全隔离。full 就是今天的 `msime-linux` 包，装在 `/usr` 下，包结构、文件和名字都不变；其他版本是各自独立的包（`msime-linux-<id>`），整个装在自己的前缀 `/opt/msime-linux-<id>` 下，只有系统按名字查找的那几样装到系统目录，名字都带版本：IBus 组件和引擎、Fcitx5 插件与输入法条目、systemd 用户单元、桌面入口、XDG 自启动项、图标，以及 `/usr/bin` 下指向前缀的 `<package>-setup` 和 `<package>-settings`。每个用户的状态目录、运行时目录（socket 和状态文件）、下载的词库和缓存都在 `<client_directory>` 下，宿主生成的 Fcitx5 主题和 Omarchy 钩子按 Fcitx5 插件名命名。

这些值只在版本表里写一次，这里把它们交给三处：

- `platforms/linux/src/core/LinuxEdition.h`：C++ 侧（IBus 宿主、Fcitx5 插件、prepare 工具和各 provider 入口）读的宏，提交进仓库，构建时不需要 Python。构建必须定义且只定义一个 `MSIME_EDITION_<ID>`（CMake 的 `MSIME_EDITION` 缓存变量），否则头文件以 `#error` 拒绝编译，不会悄悄编成 full；
- 脚本和数据文件：full 原样安装仓库里的文件，所以 full 的包与引入版本之前逐字节相同；其他版本在配置阶段由 `render` 按下面的规则改写一份再安装。每条规则必须至少命中一次，改写后不能再留下任何一个 full 的名字，否则配置失败——源文件改了写法而规则没跟上时，不会悄悄装出一个去读写 full 状态目录的版本；
- 版本声明：其他版本在前缀的 `bin` 目录写 `edition.json`（`Edition::PACKAGE_MARKER_FILE`），设置应用和 `msime-mcp` 从它知道自己属于哪个版本；full 不写。

用法：

    edition_linux.py gen [--check]                              # 重新生成 LinuxEdition.h；--check 只比较，有差异时以非零状态退出
    edition_linux.py editions                                   # 有 Linux 段的版本 id，逗号分隔，full 在最前
    edition_linux.py field --edition ID KEY                     # 打印 Linux 段的字段，或 id、display_name.zh-Hans/en、default_scheme
    edition_linux.py marker --edition ID --output edition.json  # 前缀 bin 目录里的版本声明；full 不写，已有的删掉
    edition_linux.py render --edition ID --input FILE --output FILE  # 按版本改写一个脚本或数据文件；full 原样复制
    edition_linux.py check                                      # 按每个版本改写全部文件，确认规则全部命中且没有留下 full 的名字
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from typing import Callable

ROOT = pathlib.Path(__file__).resolve().parents[3]
LINUX = ROOT / "platforms/linux"
EDITIONS = ROOT / "shared/contracts/editions.json"
HEADER = LINUX / "src/core/LinuxEdition.h"
FULL = "full"

Identity = dict[str, str]
Replacement = Callable[[Identity], str]


def load_table() -> dict:
    return json.loads(EDITIONS.read_text(encoding="utf-8"))


def linux_editions(table: dict | None = None) -> list[dict]:
    """有 Linux 段的版本，顺序与版本表相同（full 在最前）。"""
    table = table or load_table()
    return [entry for entry in table["editions"] if entry["platforms"].get("linux") is not None]


def edition_entry(edition_id: str, table: dict | None = None) -> dict:
    for entry in linux_editions(table):
        if entry["id"] == edition_id:
            return entry
    raise SystemExit(f"edition {edition_id!r} has no Linux identifiers in {EDITIONS.relative_to(ROOT)}")


def identity(entry: dict) -> Identity:
    """一个版本改写文件和生成头文件要用到的全部名字。IBus 引擎名、图标名和包名是同一个字符串（`scripts/test-editions.py` 检查它们按同一规则推出），所以改写规则用一个名字替换它们。"""
    linux = entry["platforms"]["linux"]
    if linux["ibus_engine"] != linux["package"]:
        raise SystemExit(f"edition {entry['id']}: platforms.linux.ibus_engine must equal platforms.linux.package")
    return {
        "id": entry["id"],
        "zh": entry["display_name"]["zh-Hans"],
        "en": entry["display_name"]["en"],
        **linux,
    }


# ---- 改写规则 ----

# 每条规则是（说明，正则，替换）。替换拿到版本的名字，返回替换成的文本；正则按多行模式匹配。
Rule = tuple[str, str, Replacement]

# 用户单元：<package>-online.socket 等。
UNITS: Rule = ("systemd user units", r"msime-linux-(online|voice|clipboard)\.(socket|service)", lambda n: n["package"] + r"-\1.\2")
# /usr/bin 下的两个命令，前缀 bin 里也用同一个名字安装，脚本之间按这个名字互相找到。
COMMANDS: Rule = ("setup and settings commands", r"msime-linux-(setup|settings)(?![-\w])", lambda n: n["package"] + r"-\1")
# 每个用户的状态、运行时、下载词库和缓存目录，以及前缀下 share、lib、doc 和 /etc 里的子目录。
CLIENT_DIRECTORY: Rule = ("client directory", r"msime-client(?![-\w])", lambda n: n["client_directory"])
# 包名、IBus 组件与引擎名和图标名。
PACKAGE: Rule = ("package, IBus engine and icon", r"msime-linux(?![-\w])", lambda n: n["package"])
DISPLAY_NAME: Rule = ("display name", r"水杉输入法", lambda n: n["zh"])
# 桌面入口和 Fcitx5 条目的英文名。
ENGLISH_NAME: Rule = ("English name", r"^Name=MSIME\b", lambda n: "Name=" + n["en"])
FCITX5_ADDON_PYTHON: Rule = ("Fcitx5 addon and theme", r'^(FCITX5_INPUT_METHOD|FCITX5_THEME) = "msime"$', lambda n: rf'\1 = "{n["fcitx5_addon"]}"')

RULES: dict[str, list[Rule]] = {
    "scripts/msime-linux-setup": [
        UNITS, COMMANDS, CLIENT_DIRECTORY, PACKAGE, DISPLAY_NAME, FCITX5_ADDON_PYTHON,
        ("Omarchy hook", r'theme-set\.d/msime"', lambda n: f'theme-set.d/{n["fcitx5_addon"]}"'),
        ("Omarchy plugin", r'"metasequoia\.msime"', lambda n: f'"metasequoia.{n["fcitx5_addon"]}"'),
        ("English name in the Fcitx5 hint", r"「MSIME」", lambda n: f"「{n['en']}」"),
    ],
    "scripts/msime-linux-ibus-launcher": [CLIENT_DIRECTORY],
    "scripts/msime-linux-first-run-guide": [COMMANDS, CLIENT_DIRECTORY, PACKAGE, DISPLAY_NAME],
    "scripts/msime-linux-provider-session": [CLIENT_DIRECTORY],
    "scripts/msime-linux-online-provider": [CLIENT_DIRECTORY],
    "data/msime-linux.xml.in": [
        # full 的组件写的图标名是 msime-client，没有这个名字的图标；其他版本直接用装上的图标名。
        ("engine icon", r"<icon>msime-client</icon>", lambda n: f"<icon>{n['package']}</icon>"),
        CLIENT_DIRECTORY, PACKAGE, DISPLAY_NAME,
    ],
    "data/msime-linux-clipboard.service.in": [CLIENT_DIRECTORY],
    "data/msime-linux-online.socket": [CLIENT_DIRECTORY],
    "data/msime-linux-voice.socket": [CLIENT_DIRECTORY],
    "data/msime-linux-clipboard.desktop": [UNITS, DISPLAY_NAME, ENGLISH_NAME],
    "data/msime-linux-settings.in": [COMMANDS, CLIENT_DIRECTORY],
    "data/msime-linux.desktop.in": [
        COMMANDS, PACKAGE, DISPLAY_NAME, ENGLISH_NAME,
        ("English name in About", r"About MSIME\b", lambda n: f"About {n['en']}"),
    ],
    "data/omarchy/menu.in": [
        COMMANDS, DISPLAY_NAME,
        ("Fcitx5 addon to reload", r"\"'msime'\"", lambda n: f"\"'{n['fcitx5_addon']}'\""),
    ],
    "data/omarchy/theme-set.in": [
        COMMANDS,
        ("Omarchy hook name", r"as `msime`", lambda n: f"as `{n['fcitx5_addon']}`"),
    ],
    "data/omarchy/plugin/Widget.qml.in": [
        COMMANDS, CLIENT_DIRECTORY, DISPLAY_NAME,
        ("Omarchy plugin", r'"metasequoia\.msime"', lambda n: f'"metasequoia.{n["fcitx5_addon"]}"'),
    ],
    "data/omarchy/plugin/manifest.json": [
        DISPLAY_NAME,
        ("Omarchy plugin", r'"metasequoia\.msime"', lambda n: f'"metasequoia.{n["fcitx5_addon"]}"'),
    ],
    "fcitx5/msime.conf": [
        DISPLAY_NAME, ENGLISH_NAME,
        ("addon library", r"^Library=libmsime-fcitx5$", lambda n: f"Library=lib{n['fcitx5_addon']}-fcitx5"),
    ],
    "fcitx5/msime-inputmethod.conf": [
        PACKAGE, DISPLAY_NAME, ENGLISH_NAME,
        ("addon", r"^Addon=msime$", lambda n: f"Addon={n['fcitx5_addon']}"),
    ],
    "cmake/deb-prerm.in": [
        COMMANDS, CLIENT_DIRECTORY, DISPLAY_NAME,
        ("English name in the removal hint", r"MSIME from the current group", lambda n: f"{n['en']} from the current group"),
    ],
    "cmake/deb-postinst.in": [CLIENT_DIRECTORY],
    "cmake/uninstall.cmake.in": [
        COMMANDS, CLIENT_DIRECTORY, DISPLAY_NAME,
        ("English name in the removal hint", r"MSIME from the current group", lambda n: f"{n['en']} from the current group"),
    ],
}

# 改写之后不能再出现的 full 的名字。
LEFTOVERS = [
    r"msime-client(?![-\w])",
    r"msime-linux(?![-\w])",
    r"msime-linux-(online|voice|clipboard)\.(socket|service)",
    r"msime-linux-(setup|settings)(?![-\w])",
    r"水杉输入法",
    r"""["'`]msime["'`]""",
    r"theme-set\.d/msime\b(?!-)",
    r"metasequoia\.msime(?![-\w])",
    r"^(Name|Addon)=(MSIME|msime)$",
    r"libmsime-fcitx5",
]


def render_text(relative: str, text: str, names: Identity) -> str:
    """按版本改写一个文件的内容。full 原样返回。"""
    if names["id"] == FULL:
        return text
    rules = RULES.get(relative)
    if rules is None:
        raise SystemExit(f"{relative} has no rendering rules in {pathlib.Path(__file__).relative_to(ROOT)}")
    for description, pattern, replacement in rules:
        text, count = re.subn(pattern, replacement(names), text, flags=re.MULTILINE)
        if count == 0:
            raise SystemExit(f"{relative}: the {description} rule no longer matches; update {pathlib.Path(__file__).relative_to(ROOT)}")
    for pattern in LEFTOVERS:
        match = re.search(pattern, text, flags=re.MULTILINE)
        if match:
            line = text.count("\n", 0, match.start()) + 1
            raise SystemExit(f"{relative}:{line}: {match.group(0)!r} still names full after rendering for edition {names['id']}; add a rule to {pathlib.Path(__file__).relative_to(ROOT)}")
    return text


def relative_source(path: pathlib.Path) -> str:
    return path.resolve().relative_to(LINUX.resolve()).as_posix()


# ---- 头文件 ----


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


def header_text(table: dict) -> str:
    editions = linux_editions(table)
    macros = [f"MSIME_EDITION_{entry['id'].upper()}" for entry in editions]
    lines = [
        "#pragma once",
        "",
        "// 由 platforms/linux/scripts/edition_linux.py 从 shared/contracts/editions.json 生成，不要手改；改了版本表之后运行 `python3 platforms/linux/scripts/edition_linux.py gen` 并提交结果。",
        "//",
        f"// 每个 Linux 构建必须定义且只定义一个版本选择宏（{', '.join(macros)}），CMake 按缓存变量 MSIME_EDITION 定义。少了它就停在这里，而不是悄悄编成 full、去读写 full 的状态目录和 socket。",
        "//",
        "// full 的名字与引入版本之前相同；其他版本的每用户目录、IBus 引擎、Fcitx5 插件与动作名、图标和命令名都带版本，所以几个版本可以同时安装，两个版本的 Fcitx5 插件也可以被同一个 fcitx5 进程同时加载。",
        "",
        "#if (" + " + ".join(f"defined({macro})" for macro in macros) + ") != 1",
        f'#error "Define exactly one of {", ".join(macros)}; platforms/linux/cmake/Edition.cmake does this from MSIME_EDITION"',
        "#endif",
    ]
    for index, entry in enumerate(editions):
        names = identity(entry)
        lines.append(f"#{'if' if index == 0 else 'elif'} defined({macros[index]})")
        values = [
            ("ID", narrow(entry["id"])),
            ("IS_FULL", "1" if entry["id"] == FULL else "0"),
            ("DISPLAY_NAME", narrow(names["zh"])),
            ("DISPLAY_NAME_EN", narrow(names["en"])),
            ("PACKAGE", narrow(names["package"])),
            ("CLIENT_DIRECTORY", narrow(names["client_directory"])),
            ("IBUS_ENGINE", narrow(names["ibus_engine"])),
            ("IBUS_LONGNAME", narrow("Metasequoia " + names["zh"])),
            ("FCITX5_ADDON", narrow(names["fcitx5_addon"])),
            ("ICON", narrow(names["package"])),
            ("SETUP_PROGRAM", narrow(names["package"] + "-setup")),
            ("SETTINGS_PROGRAM", narrow(names["package"] + "-settings")),
            ("TAURI_IDENTIFIER", narrow(names["tauri_identifier"])),
            # 使用统计目录（$XDG_STATE_HOME 下）：full 仍是 msime，其他版本是同级的 msime-<id>，互不嵌套。
            ("TELEMETRY_DIRECTORY", narrow("msime" if entry["id"] == FULL else "msime-" + entry["id"])),
            ("DEFAULT_SCHEME", narrow(entry["default_scheme"])),
            ("INPUT_SCHEMES", ", ".join(narrow(scheme) for scheme in entry["input_schemes"])),
            ("TEMPORARY_JAPANESE", "1" if entry["features"]["temporary_japanese"] else "0"),
        ]
        lines += [f"#define MSIME_EDITION_{name} {value}" for name, value in values]
    lines += ["#endif", ""]
    return "\n".join(lines)


def drift(table: dict) -> list[str]:
    relative = HEADER.relative_to(ROOT)
    if not HEADER.is_file():
        return [f"{relative} is missing"]
    if HEADER.read_text(encoding="utf-8") != header_text(table):
        return [f"{relative} differs from the generated content"]
    return []


def gen(check: bool) -> int:
    table = load_table()
    if check:
        problems = drift(table)
        for problem in problems:
            print(f"FAIL: {problem}; run python3 platforms/linux/scripts/edition_linux.py gen")
        return 1 if problems else 0
    text = header_text(table)
    if not HEADER.is_file() or HEADER.read_text(encoding="utf-8") != text:
        HEADER.write_text(text, encoding="utf-8", newline="\n")
        print(f"wrote {HEADER.relative_to(ROOT)}")
    return 0


def check_all() -> int:
    """按每个版本改写全部有规则的文件。full 的改写是原样复制，其他版本的规则必须全部命中、不留下 full 的名字。"""
    table = load_table()
    for entry in linux_editions(table):
        names = identity(entry)
        for relative in RULES:
            render_text(relative, (LINUX / relative).read_text(encoding="utf-8"), names)
    print(f"linux editions: {len(RULES)} files render for {len(linux_editions(table))} editions without leaving a full identifier")
    return 0


def field(entry: dict, key: str) -> str:
    if key in ("id", "default_scheme"):
        return entry[key]
    if key.startswith("display_name."):
        return entry["display_name"][key.split(".", 1)[1]]
    linux = entry["platforms"]["linux"]
    if key in linux:
        return linux[key]
    raise SystemExit(f"unknown field {key}")


def marker(edition_id: str) -> str | None:
    """前缀 bin 目录里 `edition.json` 的内容（`Edition::PACKAGE_MARKER_FILE`）。full 不带这个文件，所以 full 的包与引入版本之前相同。"""
    edition_entry(edition_id)
    return None if edition_id == FULL else json.dumps({"edition": edition_id}) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    generate = commands.add_parser("gen")
    generate.add_argument("--check", action="store_true")
    commands.add_parser("editions")
    commands.add_parser("check")
    for name in ["field", "marker", "render"]:
        command = commands.add_parser(name)
        command.add_argument("--edition", required=True)
        if name == "field":
            command.add_argument("key")
        else:
            command.add_argument("--output", type=pathlib.Path, required=True)
        if name == "render":
            command.add_argument("--input", type=pathlib.Path, required=True)
    args = parser.parse_args()

    if args.command == "gen":
        return gen(args.check)
    if args.command == "check":
        return check_all()
    if args.command == "editions":
        print(",".join(entry["id"] for entry in linux_editions()))
        return 0
    entry = edition_entry(args.edition)
    if args.command == "field":
        print(field(entry, args.key))
        return 0
    if args.command == "render":
        text = render_text(relative_source(args.input), args.input.read_text(encoding="utf-8"), identity(entry))
        args.output.parent.mkdir(parents=True, exist_ok=True)
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != text:
            args.output.write_text(text, encoding="utf-8", newline="\n")
        args.output.chmod(args.input.stat().st_mode & 0o777)
        return 0
    text = marker(args.edition)
    if text is None:
        args.output.unlink(missing_ok=True)
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
