#!/usr/bin/env python3
"""检查拆字反查（C 模式）编进引擎的两张表、它们的锁和署名。

crates/engine/src/local/chaizi.tsv 和 chaizi-chars.tsv 由 scripts/build_chaizi_table.py 从 kfcd/chaizi（CC BY 3.0）、Unihan 和 OpenCC 生成并提交，引擎用 include_str! 编进每个平台的二进制。手工改动、改了生成器却没重新生成，都会原样发布，所以这里离线检查表的不变量：格式与排序、键只含拼音字母和 `'`、字是单个非私用区非兼容区的标量、名次连续、读音只有拼音字母和声调符号、几个样例字查得到。

CC BY 3.0 第 4(b) 条要求随改编作品署名并说明改动，所以许可证文件必须写明固定提交、署名和改动说明，锁里的提交必须与它一致；NOTICE_CHANNELS 里的每个声明渠道都要在非注释行写出许可证文件名，总览还要写出固定提交。表里的读音、名次和繁体字形取自 Unihan，Unicode License V3 要求随数据附上它的版权与许可声明，所以每个渠道同样要写出 Unicode-License-V3.txt。各平台宿主把这份许可证接进自己的声明渠道时，同时把渠道加进 NOTICE_CHANNELS（渠道清单照 scripts/test-language-data-notices.py）。

缓存里有锁定的输入或设了 MSIME_FETCH=1 时，再调用生成器的 --check 逐字节比较；都没有时只做离线检查，不联网也以 0 退出。
"""
import json
import os
import re
import subprocess
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/chaizi.lock.json"
KEYS_TABLE = ROOT / "crates/engine/src/local/chaizi.tsv"
CHARS_TABLE = ROOT / "crates/engine/src/local/chaizi-chars.tsv"
GENERATOR = ROOT / "scripts/build_chaizi_table.py"
LICENCE = ROOT / "resources/licenses/kfcd-chaizi-CC-BY-3.0.txt"
UNICODE_LICENCE = ROOT / "resources/licenses/Unicode-License-V3.txt"
# 许可证文件必须保留的段落：署名、作品名、许可证与它的地址、改动说明、不背书声明、法律文本的首尾。
LICENCE_PHRASES = (
    "© 2015 開放詞典 (http://www.kaifangcidian.com)",
    "漢語拆字字典",
    "Creative Commons Attribution 3.0 Unported",
    "https://creativecommons.org/licenses/by/3.0/",
    "The changes made to the original work",
    "does not endorse",
    "Attribution 3.0 Unported",
    "Creative Commons may be contacted at",
)
# 每个渠道都要在非注释行写出两份许可证的文件名。
NOTICE_CHANNELS = {
    "platforms/windows/Collect-Notices.ps1": "Windows: the notice collection the installer ships",
    "platforms/windows/tests/tools/collect_notices.ps1": "Windows: the collection test's synthetic notice list",
    "platforms/macos/CMakeLists.txt": "macOS: the input method bundle's Resources/Licenses",
    "platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt": "macOS: the notice overview",
    "platforms/macos/tests/settings/bundle_contents.py": "macOS: the built bundle check",
    "platforms/linux/CMakeLists.txt": "Linux: the installed notices",
    "platforms/linux/data/THIRD_PARTY_NOTICES.txt": "Linux: the notice overview",
    "platforms/ios/project.yml": "iOS: the app's bundled resources",
    "platforms/ios/tests/settings/ProjectConfigurationTests.py": "iOS: the project configuration test",
    "platforms/android/build-native.sh": "Android: the native notices both APKs package as assets/native-notices",
    "platforms/harmony/stage-resources.sh": "HarmonyOS: the licences staged into the HAP",
    "docs/third-party.md": "the repository's third-party index",
}
# 总览写明每份文本覆盖什么，所以还要写出固定提交。
OVERVIEWS = (
    "platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt",
    "platforms/linux/data/THIRD_PARTY_NOTICES.txt",
    "platforms/windows/Collect-Notices.ps1",
    "docs/third-party.md",
)
# 每个样例键的第一行（按名次）必须是这个字。
SAMPLES = {
    "huo'huo'huo": "焱",
    "nv'zi": "好",
    "mu'zi": "李",
    "shou'chi": "拆",
}
KEY = re.compile(r"[a-z]+(?:'[a-z]+)*")
# kMandarin 用到的组合符号：抑音、扬音、长音、抑扬、分音、短音（ê̄ 等）。
TONE_MARKS = {"\u0300", "\u0301", "\u0304", "\u030c", "\u0308", "\u0302"}
PRIVATE_USE = (0xE000, 0xF8FF)
COMPATIBILITY = ((0xF900, 0xFAFF), (0x2F800, 0x2FA1F))
failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def table_lines(path: Path) -> list[str]:
    data = path.read_bytes()
    check(b"\r" not in data, f"{path.relative_to(ROOT)} 含 \\r；.gitattributes 要让它按 LF 检出")
    text = data.decode("utf-8")
    check(text.endswith("\n"), f"{path.relative_to(ROOT)} 不以换行结尾")
    return text.split("\n")[:-1]


def usable_character(character: str) -> bool:
    if len(character) != 1:
        return False
    codepoint = ord(character)
    return not (PRIVATE_USE[0] <= codepoint <= PRIVATE_USE[1] or any(start <= codepoint <= end for start, end in COMPATIBILITY))


def valid_reading(reading: str) -> bool:
    decomposed = unicodedata.normalize("NFD", reading)
    letters = "".join(ch for ch in decomposed if ch not in TONE_MARKS)
    return bool(re.fullmatch(r"[a-z]+", letters))


def check_tables() -> None:
    chars: dict[str, tuple[int, str, str]] = {}
    previous = None
    for number, line in enumerate(table_lines(CHARS_TABLE), 1):
        fields = line.split("\t")
        if len(fields) != 4:
            failures.append(f"chaizi-chars.tsv 第 {number} 行不是 4 列")
            continue
        character, rank, reading, traditional = fields
        check(usable_character(character), f"chaizi-chars.tsv 第 {number} 行的字 {character!r} 不是单个可用的汉字")
        check(previous is None or character > previous, f"chaizi-chars.tsv 第 {number} 行没有按码位升序")
        previous = character
        check(rank.isdigit(), f"chaizi-chars.tsv 第 {number} 行的名次 {rank!r} 不是数字")
        check(reading == "" or valid_reading(reading), f"chaizi-chars.tsv 第 {number} 行的读音 {reading!r} 不是带调拼音")
        check(traditional == "" or (usable_character(traditional) and traditional != character), f"chaizi-chars.tsv 第 {number} 行的繁体字形 {traditional!r} 不对")
        chars[character] = (int(rank) if rank.isdigit() else -1, reading, traditional)
    check(sorted(rank for rank, _, _ in chars.values()) == list(range(len(chars))), "chaizi-chars.tsv 的名次不是从 0 连续编号")

    listed: set[str] = set()
    keys: dict[str, str] = {}
    previous_key = None
    for number, line in enumerate(table_lines(KEYS_TABLE), 1):
        fields = line.split("\t")
        if len(fields) != 2:
            failures.append(f"chaizi.tsv 第 {number} 行不是 2 列")
            continue
        key, characters = fields
        check(bool(KEY.fullmatch(key)), f"chaizi.tsv 第 {number} 行的键 {key!r} 不是以 ' 连接的小写拼音")
        order = (key.replace("'", "").encode("utf-8"), key.encode("utf-8"))
        check(previous_key is None or order > previous_key, f"chaizi.tsv 第 {number} 行没有按去掉 ' 的键排序")
        previous_key = order
        check(bool(characters) and len(set(characters)) == len(characters), f"chaizi.tsv 第 {number} 行的字串为空或有重复")
        ranks = []
        for character in characters:
            check(character in chars, f"chaizi.tsv 第 {number} 行的 {character} 不在 chaizi-chars.tsv 里")
            ranks.append(chars.get(character, (-1, "", ""))[0])
        check(ranks == sorted(ranks), f"chaizi.tsv 第 {number} 行的字没有按名次排序")
        listed.update(characters)
        keys[key] = characters
    check(listed == set(chars), "chaizi-chars.tsv 收了查不到的字，或漏了查得到的字")
    for key, first in SAMPLES.items():
        check(keys.get(key, "")[:1] == first, f"{key} 的第一行应是 {first}，表里是 {keys.get(key)!r}")


def check_licence(lock: dict) -> None:
    check(LICENCE.is_file(), f"{LICENCE.relative_to(ROOT)} 不存在")
    if not LICENCE.is_file():
        return
    text = LICENCE.read_text(encoding="utf-8")
    commit = lock["source_commit"]
    check(text.splitlines()[0] == f"漢語拆字字典 ({lock['source']}), commit {commit}", f"{LICENCE.relative_to(ROOT)} 第一行没有写出锁里的 {lock['source']} 提交 {commit}")
    check(lock["license_file"] == LICENCE.relative_to(ROOT).as_posix(), "锁的 license_file 不是许可证文件的路径")
    for phrase in LICENCE_PHRASES:
        check(phrase in text, f"{LICENCE.relative_to(ROOT)} 不再含有 {phrase!r}")
    for artifact in lock["artifacts"]:
        check(f"/{commit}/" in artifact["url"], f"锁里 {artifact['name']} 的地址不在固定提交 {commit} 上")
    check(UNICODE_LICENCE.is_file() and "UNICODE LICENSE V3" in UNICODE_LICENCE.read_text(encoding="utf-8"), f"{UNICODE_LICENCE.relative_to(ROOT)} 不存在或不是 Unicode License V3")
    for channel, description in NOTICE_CHANNELS.items():
        live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
        for licence in (LICENCE, UNICODE_LICENCE):
            check(any(licence.name in line for line in live), f"{channel}（{description}）没有列出 {licence.name}")
    for overview in OVERVIEWS:
        check(commit in (ROOT / overview).read_text(encoding="utf-8"), f"{overview} 没有写出 kfcd/chaizi 的固定提交 {commit}")


def check_generated() -> None:
    """有缓存或 MSIME_FETCH=1 时重新生成并逐字节比较；生成器自己负责没有输入时的 skip。"""
    result = subprocess.run([sys.executable, str(GENERATOR), "--check"], cwd=ROOT, capture_output=True, text=True, timeout=600, env=os.environ.copy())
    output = (result.stdout + result.stderr).strip()
    print(output)
    check(result.returncode == 0, f"build_chaizi_table.py --check 失败：{output}")


def main() -> int:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    check_tables()
    check_licence(lock)
    if not failures:
        check_generated()
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"chaizi table: {len(table_lines(KEYS_TABLE))} keys, {len(table_lines(CHARS_TABLE))} characters, licence in {len(NOTICE_CHANNELS)} channels")
    return 0


if __name__ == "__main__":
    sys.exit(main())
