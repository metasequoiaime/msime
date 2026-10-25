#!/usr/bin/env python3
"""检查符号模式（Shift+S）的分组码表、它的锁和 CC BY 4.0 署名。

crates/engine/src/local/symbol_groups.tsv 由 scripts/build_symbol_groups.py 从 rime-wanxiang 的
wanxiang_symbols.yaml（CC BY 4.0）生成并提交，引擎用 include_str! 编进每个平台的二进制。手工改动、
改了生成器却没重新生成，都会原样发布，所以这里离线检查表的不变量：三列、编码只含小写字母且不重复、
组名是汉字、符号非空且不含空白、几个样例分组码查得到、点名的组名与符号数。

CC BY 4.0 第 3(a) 条要求随改编数据署名、给出许可证地址并说明改动，所以许可证文件必须写明固定提交、
署名和改动说明，锁里的提交必须与它一致；NOTICE_CHANNELS 里的每个声明渠道都要在非注释行写出许可证
文件名，总览还要写出固定提交。上游那张表是从 Rime 的 symbols.yaml 抄来的，许可证文件里也写了这一层。

缓存里有锁定的输入或设了 MSIME_FETCH=1 时，再调用生成器的 --check 逐字节比较；都没有时只做离线
检查，不联网也以 0 退出。
"""
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCK = ROOT / "resources/symbol-groups.lock.json"
TABLE = ROOT / "crates/engine/src/local/symbol_groups.tsv"
GENERATOR = ROOT / "scripts/build_symbol_groups.py"
LICENCE = ROOT / "resources/licenses/rime-wanxiang-CC-BY-4.0.txt"
# 许可证文件必须保留的段落：署名、作品名、许可证与它的地址、改动说明、不背书声明、法律文本的首尾。
LICENCE_PHRASES = (
    "rime-wanxiang (https://github.com/amzxyz/rime-wanxiang)",
    "Copyright amzxyz and contributors",
    "wanxiang_symbols.yaml",
    "Creative Commons Attribution 4.0 International",
    "https://creativecommons.org/licenses/by/4.0/",
    "The changes made to the original work",
    "do not endorse",
    "Attribution 4.0 International",
    "Creative Commons may be contacted at creativecommons.org.",
)
# 每个渠道都要在非注释行写出许可证的文件名。
NOTICE_CHANNELS = {
    "platforms/macos/CMakeLists.txt": "macOS: the input method bundle's Resources/Licenses",
    "platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt": "macOS: the notice overview",
    "platforms/macos/tests/settings/bundle_contents.py": "macOS: the built bundle check",
    "platforms/linux/CMakeLists.txt": "Linux: the installed notices",
    "platforms/linux/data/THIRD_PARTY_NOTICES.txt": "Linux: the notice overview",
    "platforms/windows/Collect-Notices.ps1": "Windows: the notice collection the installer ships",
    "platforms/windows/Notices.md": "Windows: the notice collection's documentation",
    "platforms/android/build-native.sh": "Android: the native notices both APKs package as assets/native-notices",
    "platforms/harmony/stage-resources.sh": "HarmonyOS: the licences staged into the HAP",
    "platforms/ios/project.yml": "iOS: the app's bundled resources",
    "platforms/ios/tests/settings/ProjectConfigurationTests.py": "iOS: the project configuration test",
    "docs/third-party.md": "the repository's third-party index",
}
# 总览写明每份文本覆盖什么，所以还要写出固定提交。
OVERVIEWS = (
    "platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt",
    "platforms/linux/data/THIRD_PARTY_NOTICES.txt",
    "platforms/windows/Collect-Notices.ps1",
    "docs/third-party.md",
)
# 样例：分组码 → (组名, 头几个符号, 符号数)。数目变了说明上游分组动过，要人工复核。
SAMPLES = {
    "xl": ("希腊字母小写", ["α", "β", "γ"], 24),
    "xld": ("希腊字母大写", ["Α", "Β", "Γ"], 24),
    "sx": ("数学符号", ["＋", "－", "×"], 45),
    "szq": ("带圈数字", ["⓪", "①", "②"], 72),
    "jt": ("箭头", ["→", "↑", "↓"], 148),
}
CODE = re.compile(r"[a-z]+\Z")
failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def table_lines(path: Path) -> list[str]:
    data = path.read_bytes()
    check(b"\r" not in data, f"{path.relative_to(ROOT)} 含 \\r；.gitattributes 要让它按 LF 检出")
    text = data.decode("utf-8")
    check(text.endswith("\n"), f"{path.relative_to(ROOT)} 不以换行结尾")
    return [line for line in text.split("\n")[:-1] if line and not line.startswith("#")]


def check_table() -> None:
    groups: dict[str, tuple[str, list[str]]] = {}
    for number, line in enumerate(table_lines(TABLE), 1):
        fields = line.split("\t")
        if len(fields) != 3:
            failures.append(f"symbol_groups.tsv 第 {number} 行不是 3 列")
            continue
        code, name, symbols = fields
        check(bool(CODE.fullmatch(code)) and code == code.lower(), f"symbol_groups.tsv 第 {number} 行的编码 {code!r} 不是小写字母")
        check(code not in groups, f"symbol_groups.tsv 第 {number} 行的编码 {code!r} 重复")
        check(bool(name) and all("\u4e00" <= character <= "\u9fff" for character in name), f"symbol_groups.tsv 第 {number} 行的组名 {name!r} 不是汉字")
        items = symbols.split(" ")
        check(bool(items) and all(items), f"symbol_groups.tsv 第 {number} 行有空的符号")
        for symbol in items:
            check(symbol == symbol.strip() and not any(character.isspace() for character in symbol), f"symbol_groups.tsv 的 {code} 里符号 {symbol!r} 含空白")
            check("\ufe0f" not in symbol, f"symbol_groups.tsv 的 {code} 里符号 {symbol!r} 带变体选择符")
        groups[code] = (name, items)
    check(len(groups) == 87, f"分组码个数是 {len(groups)}，应为 87")
    check(sum(len(items) for _, items in groups.values()) == 3929, "符号总数不是 3929")
    for code, (name, head, count) in SAMPLES.items():
        actual = groups.get(code)
        check(actual is not None, f"表里没有样例分组码 {code}")
        if actual is None:
            continue
        check(actual[0] == name, f"{code} 的组名是 {actual[0]!r}，应为 {name!r}")
        check(actual[1][: len(head)] == head, f"{code} 的头几个符号是 {actual[1][: len(head)]!r}，应为 {head!r}")
        check(len(actual[1]) == count, f"{code} 有 {len(actual[1])} 个符号，应为 {count}")


def check_licence(lock: dict) -> None:
    check(LICENCE.is_file(), f"{LICENCE.relative_to(ROOT)} 不存在")
    if not LICENCE.is_file():
        return
    text = LICENCE.read_text(encoding="utf-8")
    commit = lock["source_commit"]
    check(text.splitlines()[0] == f"rime-wanxiang ({lock['source']}), release {lock['source_release']}, commit {commit}", f"{LICENCE.relative_to(ROOT)} 第一行没有写出锁里的提交 {commit}")
    check(lock["license_file"] == LICENCE.relative_to(ROOT).as_posix(), "锁的 license_file 不是许可证文件的路径")
    check(lock["license"] == "CC-BY-4.0", "锁的 license 不是 CC-BY-4.0")
    for phrase in LICENCE_PHRASES:
        check(phrase in text, f"{LICENCE.relative_to(ROOT)} 不再含有 {phrase!r}")
    # 输入取自词库仓 msime-dictionary 的 sources/symbols/（与上游 source_commit 逐字节一致，见词库仓 NOTICE.md），不直连上游。
    dictionary = lock.get("dictionary_commit", "")
    check(lock.get("dictionary_repository") == "https://github.com/metasequoiaime/msime-dictionary", "锁的 dictionary_repository 不是 msime-dictionary")
    check(re.fullmatch(r"[0-9a-f]{40}", dictionary) is not None, "锁的 dictionary_commit 不是完整的 Git 提交")
    for artifact in lock["artifacts"]:
        check(
            artifact["url"] == f"https://raw.githubusercontent.com/metasequoiaime/msime-dictionary/{dictionary}/sources/symbols/{artifact['name']}",
            f"锁里 {artifact['name']} 的地址不是词库仓固定提交 {dictionary} 的 sources/symbols/{artifact['name']}",
        )
    check(lock["outputs"] == [TABLE.relative_to(ROOT).as_posix()], "锁的 outputs 不是这张表")
    for channel, description in NOTICE_CHANNELS.items():
        path = ROOT / channel
        if not path.is_file():
            check(False, f"{channel}（{description}）不存在")
            continue
        live = [line for line in path.read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
        check(any(LICENCE.name in line for line in live), f"{channel}（{description}）没有列出 {LICENCE.name}")
    for overview in OVERVIEWS:
        path = ROOT / overview
        if path.is_file():
            check(commit in path.read_text(encoding="utf-8"), f"{overview} 没有写出 rime-wanxiang 的固定提交 {commit}")


def check_generated() -> None:
    """有缓存或 MSIME_FETCH=1 时重新生成并逐字节比较；生成器自己负责没有输入时的 skip。"""
    result = subprocess.run([sys.executable, str(GENERATOR), "--check"], cwd=ROOT, capture_output=True, text=True, timeout=600, env=os.environ.copy())
    output = (result.stdout + result.stderr).strip()
    print(output)
    check(result.returncode == 0, f"build_symbol_groups.py --check 失败：{output}")


def main() -> int:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    check_table()
    check_licence(lock)
    if not failures:
        check_generated()
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"symbol groups: {len(table_lines(TABLE))} groups, licence in {len(NOTICE_CHANNELS)} channels")
    return 0


if __name__ == "__main__":
    sys.exit(main())
