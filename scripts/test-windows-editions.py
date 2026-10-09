#!/usr/bin/env python3
"""Windows 各版本的身份没有漂移，安装脚本按版本展开后互不越界。

版本表 `shared/contracts/editions.json` 的 `platforms.windows` 段经 `platforms/windows/scripts/edition_windows.py gen` 变成两个提交进仓库的文件：C++ 读的 `shared/contracts/msime_edition.h` 和 Inno Setup 读的 `platforms/windows/installer/editions.iss`。这里检查：

- 两个生成文件与版本表一致（等同于 `edition_windows.py gen --check`）；
- `installer/msime_setup.iss` 用一个只覆盖本脚本用到的那部分 ISPP 语法的预处理器按每个版本展开：每个版本的结果里出现本版本的 AppId、CLSID、安装目录、注册表键、看门狗任务名和安装包名，不出现任何别的版本的，也不出现 msime-windows（`edition_windows.py` 的 `MSIME_WINDOWS`）的，唯一的例外是 OtherEditionDataDirs 里别的全部版本和 msime-windows 的注册表键和安装目录名（安装器拿它们找出别人的数据目录，本版本的数据目录不能和它们重叠或互相包含）；所有版本都只结束可执行文件在本安装 server 目录里的进程，不按映像名结束（`taskkill /IM` 会停掉同时安装的其他版本）；所有版本的数据目录所有权都只认本版本的标记，目录里有别的版本或 msime-windows 的标记就不归它管，即使那是它的默认数据目录；所有版本（包括 full）的所有权标记都带着自己的版本 id；
- full 和其他版本走同一套写法，没有只属于 full 的分支：引入版本之前的那组标识属于 msime-windows，full 现在有自己的一组；
- Windows 原生代码和 Rust 侧不自己写 full 或 msime-windows 的 CLSID、profile 和注册表键：这些值只能出现在生成的头文件、版本表、生成器和本检查允许的地方，否则某个版本会悄悄用上 full 的名字，或者碰到 msime-windows 的注册。
- 每个版本注册在它的默认方案所属的语言下（版本表 `langid`）：中文版本是简体中文 0x0804，日文版 0x0411，越南文版 0x042A，藏文版 0x0451。TSF 按它把文本服务列在 Windows 设置的对应语言下；
- `release-windows.yml` 的发布矩阵恰好是有 Windows 段的全部版本：少了一个，那个版本就不出安装包，共存检查（按 `edition_windows.py editions` 找安装包）也会因找不到它而失败。

ISPP 的真实编译只能在装了 Inno Setup 的 Windows 上做（`release-windows.yml`）；这里的预处理器只认 `#include`、`#define`、`#if`/`#ifdef`/`#ifndef`/`#elif`/`#else`/`#endif`、`#error` 和行内 `{#名字}`，遇到别的指令就失败，免得在它看不懂的地方给出错误的结论。
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EDITIONS = ROOT / "shared/contracts/editions.json"
INSTALLER = ROOT / "platforms/windows/installer"
SETUP = INSTALLER / "msime_setup.iss"
RELEASE_WORKFLOW = ROOT / ".github/workflows/release-windows.yml"
# 默认方案到它所属语言的 LANGID。中文方案都是简体中文；一个版本的默认方案不在这里时，先想清楚它该注册在哪个语言下再补上。
SCHEME_LANGID = {
    "quanpin": "0x0804",
    "shuangpin": "0x0804",
    "wubi": "0x0804",
    "japanese": "0x0411",
    "vietnamese": "0x042A",
    "tibetan": "0x0451",
}
GENERATOR = ROOT / "platforms/windows/scripts/edition_windows.py"
FULL = "full"
# 允许出现 full 的 Windows 标识的源文件：生成器的输入和输出，以及描述、检查它们的地方。
IDENTIFIER_HOMES = {
    "shared/contracts/editions.json",
    "shared/contracts/editions.frozen.json",
    "shared/contracts/editions.schema.json",
    "shared/contracts/msime_edition.h",
    "platforms/windows/installer/editions.iss",
    "scripts/test-editions.py",
    "scripts/test-windows-editions.py",
    # 它的单测核对 full 解析出来的值就是今天的值。
    "crates/client-core/src/edition.rs",
}
# 不该自己写出来的 full 和 msime-windows 标识：TSF 的 CLSID 和 profile，以及 HKLM 键。管道、事件和互斥量的基名不在其中，它们本来就要和版本后缀拼在一起写。
SCANNED = ["platforms/windows", "crates/host-windows", "crates/client-core/src", "crates/mcp-server/src", "apps/desktop/src-tauri/src"]
SCANNED_SUFFIXES = {".h", ".cpp", ".rs", ".ps1", ".iss", ".rc", ".vcxproj", ".cmake", ".txt"}


class Ispp:
    """ISPP 的一个小子集，只够展开本仓库的安装脚本。字符串是 Pascal 风格（反斜杠不转义），整数和字符串可以用 + 拼接；版本用预定义的 Ver 和 EncodeVer(主, 次, 修订[, 构建]) 比较。"""

    TOKEN = re.compile(r'\s*(?:(?P<string>"[^"]*")|(?P<number>\d+)|(?P<name>[A-Za-z_]\w*)|(?P<op>==|!=|>=|!|\+|\(|\)|,))')

    def __init__(self, defines: dict[str, object]):
        self.defines = dict(defines)

    def evaluate(self, expression: str) -> object:
        tokens = []
        position = 0
        expression = expression.strip()
        while position < len(expression):
            match = self.TOKEN.match(expression, position)
            if not match or match.end() == position:
                raise SystemExit(f"ISPP subset: cannot read expression {expression!r}")
            position = match.end()
            kind = match.lastgroup
            tokens.append((kind, match.group(kind)))
        value, rest = self.parse_comparison(tokens)
        if rest:
            raise SystemExit(f"ISPP subset: trailing tokens in {expression!r}")
        return value

    def parse_comparison(self, tokens):
        left, tokens = self.parse_sum(tokens)
        if tokens and tokens[0] == ("op", "=="):
            right, tokens = self.parse_sum(tokens[1:])
            return int(left == right), tokens
        if tokens and tokens[0] == ("op", "!="):
            right, tokens = self.parse_sum(tokens[1:])
            return int(left != right), tokens
        if tokens and tokens[0] == ("op", ">="):
            right, tokens = self.parse_sum(tokens[1:])
            return int(left >= right), tokens
        return left, tokens

    def parse_sum(self, tokens):
        value, tokens = self.parse_unary(tokens)
        while tokens and tokens[0] == ("op", "+"):
            right, tokens = self.parse_unary(tokens[1:])
            value = value + right
        return value, tokens

    def parse_unary(self, tokens):
        if not tokens:
            raise SystemExit("ISPP subset: empty expression")
        kind, text = tokens[0]
        if (kind, text) == ("op", "!"):
            value, rest = self.parse_unary(tokens[1:])
            return int(not value), rest
        if (kind, text) == ("op", "("):
            value, rest = self.parse_comparison(tokens[1:])
            if not rest or rest[0] != ("op", ")"):
                raise SystemExit("ISPP subset: unbalanced parenthesis")
            return value, rest[1:]
        if kind == "string":
            return text[1:-1], tokens[1:]
        if kind == "number":
            return int(text), tokens[1:]
        if kind == "name" and text == "EncodeVer" and tokens[1:2] == [("op", "(")]:
            parts, rest = [], tokens[2:]
            while True:
                part, rest = self.parse_sum(rest)
                parts.append(part)
                if rest and rest[0] == ("op", ","):
                    rest = rest[1:]
                    continue
                if not rest or rest[0] != ("op", ")") or not 3 <= len(parts) <= 4:
                    raise SystemExit("ISPP subset: EncodeVer takes three or four numbers")
                return encode_ver(*parts), rest[1:]
        if kind == "name":
            if text not in self.defines:
                raise SystemExit(f"ISPP subset: {text} is not defined")
            return self.defines[text], tokens[1:]
        raise SystemExit(f"ISPP subset: unexpected {text!r}")

    def expand_inline(self, line: str) -> str:
        def value(match: re.Match) -> str:
            return str(self.evaluate(match.group(1)))
        return re.sub(r"\{#([^}]*)\}", value, line)

    def run(self, path: pathlib.Path) -> list[str]:
        output: list[str] = []
        # 每层条件：(这一层当前是否生效, 这一层是否已经有分支生效过, 外层是否生效)
        stack: list[tuple[bool, bool, bool]] = []

        def active() -> bool:
            return all(entry[0] for entry in stack)

        for line in path.read_text(encoding="utf-8").splitlines(keepends=True):
            stripped = line.strip()
            if stripped.startswith("#"):
                directive, _, argument = stripped[1:].partition(" ")
                argument = argument.strip()
                if directive in ("if", "ifdef", "ifndef"):
                    outer = active()
                    if directive == "if":
                        condition = bool(self.evaluate(argument)) if outer else False
                    elif directive == "ifdef":
                        condition = argument in self.defines
                    else:
                        condition = argument not in self.defines
                    stack.append((condition, condition, outer))
                elif directive == "elif":
                    current, taken, outer = stack.pop()
                    condition = (not taken) and outer and bool(self.evaluate(argument))
                    stack.append((condition, taken or condition, outer))
                elif directive == "else":
                    current, taken, outer = stack.pop()
                    stack.append((not taken, True, outer))
                elif directive == "endif":
                    stack.pop()
                elif not active():
                    continue
                elif directive == "include":
                    output += self.run(path.parent / argument.strip('"'))
                elif directive == "define":
                    name, _, value = argument.partition(" ")
                    self.defines[name] = self.evaluate(value) if value.strip() else ""
                elif directive == "error":
                    raise SystemExit(f"ISPP subset: #error {argument}")
                else:
                    raise SystemExit(f"ISPP subset: unsupported directive #{directive} in {path.name}")
                continue
            if active():
                output.append(self.expand_inline(line))
        if stack:
            raise SystemExit(f"ISPP subset: unterminated conditional in {path.name}")
        return output


def encode_ver(major: int, minor: int, revision: int, build: int = 0) -> int:
    return (major << 24) | (minor << 16) | (revision << 8) | build


# CI 用 Inno Setup 6.7.1 编安装包（release-windows.yml），本机构建优先用 7（Compile-Installer.ps1），两个版本走 msime_setup.iss 里不同的分支。
CI_ISPP_VERSION = encode_ver(6, 7, 1)


def preprocess(edition_id: str, light: bool = False, ispp_version: int = CI_ISPP_VERSION) -> str:
    defines: dict[str, object] = {"Edition": edition_id, "Ver": ispp_version}
    if light:
        defines["LightPackage"] = 1
    return "".join(Ispp(defines).run(SETUP))


def load_generator():
    spec = importlib.util.spec_from_file_location("edition_windows", GENERATOR)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def identifiers(windows: dict) -> dict[str, str]:
    """一个版本（或 msime-windows）在安装脚本里留下的标识：展开后必须出现自己的，不能出现别人的。msime-windows 的记录没有 app_name：它的显示名与 full 相同，显示名不是安装器用来区分产品的东西。"""
    found = {
        "inno_app_id": "AppId={" + windows["inno_app_id"],
        "clsid": "SOFTWARE\\Microsoft\\CTF\\TIP\\" + windows["clsid"],
        "install_dir": "\\" + windows["install_dir"] + "\\",
        "registry_key": windows["registry_key"],
        "watchdog_task": '"' + windows["watchdog_task"] + '"',
        "installer_base_name": "OutputBaseFilename=" + windows["installer_base_name"] + "_v",
    }
    if "app_name" in windows:
        found["app_name"] = "AppName=" + windows["app_name"] + "\n"
    return found


def check_installer(errors: list[str], editions: list[dict], msime_windows: dict) -> None:
    outputs = {entry["id"]: preprocess(entry["id"]) for entry in editions}
    for entry in editions:
        edition_id = entry["id"]
        output = outputs[edition_id]
        own = identifiers(entry["platforms"]["windows"])
        # 别的版本和 msime-windows 的注册表键和安装目录名只能出现在 OtherEditionDataDirs 的名单里：安装器拿它找出别人的数据目录，拒绝和它们重叠或互相包含的数据目录，也不在卸载、换目录时删掉嵌在本版本目录里的它们。名单必须恰好是别的全部版本（按版本表的顺序）再加上最后的 msime-windows。
        others = [other["platforms"]["windows"] for other in editions if other["id"] != edition_id] + [msime_windows]
        sibling_lines = [
            "    RegistryKeys := '" + "|".join(other["registry_key"] for other in others) + "';\n",
            "    InstallDirs := '" + "|".join(other["install_dir"] for other in others) + "';\n",
        ]
        for line in sibling_lines:
            if output.count(line) != 1:
                errors.append(f"msime_setup.iss for edition {edition_id}: OtherEditionDataDirs does not list exactly the other editions and msime-windows ({line.strip()!r})")
            output = output.replace(line, "")
        for key, text in own.items():
            if text not in output:
                errors.append(f"msime_setup.iss for edition {edition_id}: {key} {text.strip()!r} does not appear")
        strangers = [(f"edition {other['id']}", other["platforms"]["windows"]) for other in editions if other["id"] != edition_id] + [("msime-windows", msime_windows)]
        for name, section in strangers:
            for key, text in identifiers(section).items():
                # 一个产品的名字可能恰好是另一个的前缀（msime-windows 的 MetasequoiaIME 和 MetasequoiaIME-Wubi）；只看完整出现、又不属于本版本那一处的情况。
                if text in output and text not in own.values() and not any(text in mine for mine in own.values()):
                    errors.append(f"msime_setup.iss for edition {edition_id}: contains {name}'s {key} {text.strip()!r}")
        # 所有权标记的文件名接版本的名字后缀，每个版本各不相同，也不是 msime-windows 的 .metasequoiaime-data：两个产品用同一个文件名，一个的安装器就会把另一个的数据目录当成自己的去接管、清理或删除。
        marker = f"DataDirMarkerName = '.metasequoiaime-data{entry['platforms']['windows']['name_suffix']}';"
        if marker not in output:
            errors.append(f"msime_setup.iss for edition {edition_id}: the data-directory ownership marker is not named {marker!r}")
        # 每个版本（包括 full）都只停本安装 server 目录里的进程：几个版本的进程同名，按映像名结束会停掉同时安装的其他版本。
        if "taskkill.exe" in output:
            errors.append(f"msime_setup.iss for edition {edition_id}: stops processes by image name, which also stops the other installed editions")
        server_dir = "ServerDir := ExpandConstant('{commonpf64}\\" + entry["platforms"]["windows"]["install_dir"] + "\\server');"
        if server_dir not in output or "StopProcessesUnder(ServerDir, '');" not in output:
            errors.append(f"msime_setup.iss for edition {edition_id}: does not stop exactly the processes under its own server directory ({server_dir!r})")
        if marker == f"DataDirMarkerName = '{msime_windows['data_dir_marker']}';":
            errors.append(f"msime_setup.iss for edition {edition_id}: the data-directory ownership marker is msime-windows' {msime_windows['data_dir_marker']!r}")
        # 每个版本（包括 full）都不认带着别的版本或 msime-windows 标记的目录，即使那是它的默认数据目录。msime-windows 的标记就是不带后缀的前缀本身，所以前缀必须是它。
        owns = output[output.find("function OwnsDataDir"):output.find("procedure WriteDataDirMarker")]
        if f"DataDirMarkerPrefix = '{msime_windows['data_dir_marker']}';" not in output or "(not HasOtherEditionDataDirMarker(Directory)) and" not in owns:
            errors.append(f"msime_setup.iss for edition {edition_id}: OwnsDataDir may claim a directory that carries another edition's or msime-windows' marker")
        if f"(edition {edition_id})" not in output:
            errors.append(f"msime_setup.iss for edition {edition_id}: the data-directory ownership marker does not name the edition")
    # 轻量包也要能展开（它走另一组 #ifdef 分支）。
    for entry in editions:
        preprocess(entry["id"], light=True)
    # 系统目录里的工具要绕开 WOW64 重定向才看得到真正的 System32：Inno 6 在 64 位安装模式下 Exec 本来就绕开，Inno 7 的 32 位 Setup 只有 ExecWithNativeSysDir 绕开。两个版本各展开一次，确认走到了对的那个函数。
    for version, expected in ((CI_ISPP_VERSION, "Exec("), (encode_ver(7, 0, 0), "ExecWithNativeSysDir(")):
        expanded = preprocess(editions[0]["id"], ispp_version=version)
        for tool in ("{sys}\\cmd.exe", "{sys}\\WindowsPowerShell\\v1.0\\powershell.exe"):
            launches = re.findall(r"(\w+)\(\s*ExpandConstant\('" + re.escape(tool) + "'", expanded)
            if not launches or any(name + "(" != expected for name in launches):
                errors.append(f"msime_setup.iss under ISPP {version:#x}: {tool} is launched through {launches}, expected {expected[:-1]}")


def check_generated(errors: list[str]) -> None:
    generator = load_generator()
    for problem in generator.drift(generator.generated_files(generator.load_table())):
        errors.append(f"{problem}; run python3 platforms/windows/scripts/edition_windows.py gen")


def check_hardcoded(errors: list[str], full: dict, msime_windows: dict) -> None:
    """full 和 msime-windows 的 CLSID、profile 和注册表键不能写在生成文件之外。"""
    needles = {}
    for owner, section in (("full", full), ("msime-windows", msime_windows)):
        needles.update({
            section["clsid"].strip("{}").upper(): f"the {owner} CLSID",
            section["profile_guid"].strip("{}").upper(): f"the {owner} profile GUID",
            "0x" + section["clsid"].strip("{}").split("-")[0].lower(): f"the {owner} CLSID",
            "0x" + section["profile_guid"].strip("{}").split("-")[0].lower(): f"the {owner} profile GUID",
        })
    needles[full["registry_key"].replace("\\", "\\\\")] = "the full registry key"
    # msime-windows 的键是本仓库所有版本的键的前缀（MetasequoiaIME-Full 等），只找后面不再接名字的完整写法。
    msime_windows_key = re.compile(re.escape(msime_windows["registry_key"].replace("\\", "\\\\")) + r"(?![-\w])")
    listed = subprocess.run(["git", "ls-files", *SCANNED], cwd=ROOT, capture_output=True, text=True, check=True)
    for relative in listed.stdout.splitlines():
        path = ROOT / relative
        if relative in IDENTIFIER_HOMES or path.suffix not in SCANNED_SUFFIXES or not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for needle, what in needles.items():
            if needle in text or needle in text.upper():
                errors.append(f"{relative} spells {what} itself; take it from shared/contracts/msime_edition.h (C++) or the edition's WindowsIdentity (Rust)")
        if msime_windows_key.search(text):
            errors.append(f"{relative} spells the msime-windows registry key itself; no edition may touch msime-windows' key")


def check_languages(errors: list[str], editions: list[dict]) -> None:
    for entry in editions:
        expected = SCHEME_LANGID.get(entry["default_scheme"])
        actual = entry["platforms"]["windows"]["langid"]
        if expected is None:
            errors.append(f"edition {entry['id']}: no Windows language is known for its default scheme {entry['default_scheme']!r}; add it to SCHEME_LANGID")
        elif actual != expected:
            errors.append(f"edition {entry['id']}: platforms.windows.langid is {actual}, but its default scheme {entry['default_scheme']} belongs under {expected}")


def check_release_matrix(errors: list[str], editions: list[dict]) -> None:
    # 构建（release）、编译安装包（package）和 Windows on Arm 冒烟（arm64）各有一份版本矩阵，每一份都要齐：package 少一个版本，那个版本就没有安装包，签名时 SignPath 也收不到它。
    matches = list(re.finditer(r"^\s+edition: \[([^\]]*)\]\s*$", RELEASE_WORKFLOW.read_text(encoding="utf-8"), re.MULTILINE))
    if not matches:
        errors.append(f"{RELEASE_WORKFLOW.relative_to(ROOT)} has no edition matrix")
        return
    expected = [entry["id"] for entry in editions]
    for match in matches:
        matrix = [item.strip() for item in match.group(1).split(",") if item.strip()]
        if sorted(matrix) != sorted(expected) or len(set(matrix)) != len(matrix):
            errors.append(f"{RELEASE_WORKFLOW.relative_to(ROOT)} has an edition matrix {matrix}, but the editions with Windows identifiers are {expected}")


def main() -> int:
    table = json.loads(EDITIONS.read_text(encoding="utf-8"))
    editions = [entry for entry in table["editions"] if entry["platforms"].get("windows") is not None]
    errors: list[str] = []
    check_generated(errors)
    msime_windows = load_generator().MSIME_WINDOWS
    check_installer(errors, editions, msime_windows)
    check_languages(errors, editions)
    check_release_matrix(errors, editions)
    full = next(entry for entry in editions if entry["id"] == FULL)["platforms"]["windows"]
    check_hardcoded(errors, full, msime_windows)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"windows editions: generated identity files are current, msime_setup.iss expands to {len(editions)} disjoint installers, every edition registers under its default scheme's language and is in the release matrix, and no Windows source spells the full or msime-windows identifiers itself")
    return 0


if __name__ == "__main__":
    sys.exit(main())
