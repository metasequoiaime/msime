#!/usr/bin/env python3
"""检查提交的韩文 Hanja 表（crates/engine/src/korean/hanja.tsv）是否符合 msime-dict-build hanja 的生成规则。

表由 msime-dictionary 原样收录的 libhangul hanja.txt（`sources/korean/hanja.txt`）生成并提交，供引擎内嵌；CI 不会重新生成，因此这里在无网络的情况下检查所有不变量，以及锁文件的 `libhangul` 引用。设置了 `MSIME_DICTIONARY=<msime-dictionary checkout>` 时，还会从它的源文件重新生成并逐字节比较。

表按 BSD-3-Clause 授权，所有平台都必须随引擎分发对应许可证文件。
"""
import hashlib
import json
import os
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TABLE = ROOT / "crates/engine/src/korean/hanja.tsv"
LOCK = ROOT / "resources/dictionary-sources.lock.json"
LICENSE = ROOT / "resources/licenses/libhangul-hanja-BSD-3-Clause.txt"
# BSD-3-Clause clause 2: every binary distribution carries the notice. The table is compiled into the engine, which every platform ships, so each platform's notice channel has to name the licence file on a live (non-comment) line.
NOTICE_CHANNELS = {
    "platforms/windows/Collect-Notices.ps1": "Windows: the notice collection the installer ships",
    "platforms/macos/CMakeLists.txt": "macOS: the input method bundle's Resources/Licenses",
    "platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt": "macOS: the notice overview",
    "platforms/linux/CMakeLists.txt": "Linux: the installed notices",
    "platforms/linux/data/THIRD_PARTY_NOTICES.txt": "Linux: the notice overview",
    "platforms/android/build-native.sh": "Android: the native notices both APKs package as assets/native-notices",
    "platforms/ios/project.yml": "iOS: the app's bundled resources",
    "platforms/harmony/stage-resources.sh": "HarmonyOS: the licences staged into the HAP",
}
SOURCE = "sources/korean/hanja.txt"
COMMIT = "717409ce61524bb3d8426060a384822f21354c62"
# `COMMIT` 处 libhangul `data/hanja/hanja.txt` 原样文件的大小与 SHA-256，msime-dictionary 的 `sources/korean/hanja.txt` 必须是这份文件。
SOURCE_SIZE = 6452537
SOURCE_SHA256 = "b1004034589f1357daaea3534a6136f6b5ef825afa8779886b20f0b7908bbe3b"
failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def syllable(character: str) -> bool:
    return len(character) == 1 and 0xAC00 <= ord(character) <= 0xD7A3


# The twelve unified ideographs of the CJK Compatibility Ideographs block, which NFC leaves alone; crates/dict-builder/src/hanja.rs `UNIFIED_IN_COMPATIBILITY_BLOCK`, restated.
UNIFIED_IN_COMPATIBILITY_BLOCK = {0xFA0E, 0xFA0F, 0xFA11, 0xFA13, 0xFA14, 0xFA1F, 0xFA21, 0xFA23, 0xFA24, 0xFA27, 0xFA28, 0xFA29}


def kept_hanja(character: str) -> bool:
    if len(character) != 1:
        return False
    point = ord(character)
    return 0x4E00 <= point <= 0x9FFF or 0x3400 <= point <= 0x4DBF or point in UNIFIED_IN_COMPATIBILITY_BLOCK


def generate(source: str) -> str:
    """crates/dict-builder/src/hanja.rs `build` then `tsv`, restated."""
    groups: dict[str, list[tuple[str, str]]] = {}
    seen = set()
    # str::lines in Rust: only \n and \r\n end a line, unlike str.splitlines.
    for line in source.split("\n"):
        line = line.removesuffix("\r")
        if not line or line.startswith("#"):
            continue
        key, value, comment = line.split(":", 2)
        if not syllable(key) or not kept_hanja(value) or (key, value) in seen:
            continue
        seen.add((key, value))
        groups.setdefault(key, []).append((value, comment.strip()))
    return "".join(f"{key}\t{value}\t{gloss}\n" for key in sorted(groups) for value, gloss in groups[key])


def main() -> int:
    if not TABLE.is_file():
        print(f"FAIL: {TABLE.relative_to(ROOT)} is missing")
        return 1
    table = TABLE.read_text(encoding="utf-8")
    without_decomposition = {point for point in range(0xF900, 0xFB00) if unicodedata.category(chr(point)) == "Lo" and not unicodedata.decomposition(chr(point))}
    check(without_decomposition == UNIFIED_IN_COMPATIBILITY_BLOCK, "the unified ideographs of the compatibility block no longer match the code points NFC leaves alone")
    check(table.endswith("\n"), "the table does not end with a newline")
    order = []
    seen = set()
    for number, line in enumerate(table.splitlines(), 1):
        fields = line.split("\t")
        if len(fields) != 3:
            check(False, f"line {number} has {len(fields)} fields, expected syllable, hanja and gloss")
            continue
        key, value, gloss = fields
        check(syllable(key), f"line {number}: {key!r} is not one precomposed Hangul syllable")
        check(kept_hanja(value), f"line {number}: {value!r} is not one unified ideograph of the Basic Multilingual Plane")
        check(gloss == gloss.strip(), f"line {number}: the gloss has surrounding whitespace")
        check((key, value) not in seen, f"line {number}: {key} {value} is listed twice")
        seen.add((key, value))
        if not order or order[-1] != key:
            check(key not in order, f"line {number}: the readings of {key} are not contiguous")
            order.append(key)
    check(order == sorted(order), "syllables are not in code point order")
    rows = table.splitlines()
    check(rows[:2] == ["가\t可\t옳을 가", "가\t家\t집 가"], "the table no longer starts with the source's first readings of 가")
    han = [row for row in rows if row.startswith("한\t")]
    check(han[:2] == ["한\t韓\t나라 이름 한, 한나라 한", "한\t漢\t한수 한"], "한 no longer starts with 韓 and 漢 in source order")

    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    check(lock["references"].get("libhangul", {}).get("commit") == COMMIT, f"the sources lock's libhangul reference is not {COMMIT}; update COMMIT, SOURCE_SIZE and SOURCE_SHA256 together with the table")
    check(not any(entry["path"] == SOURCE for entry in lock["files"]), f"the sources lock pins {SOURCE}; msime-dictionary data reaches msime only through dict-v release assets")
    check(LICENSE.is_file() and "Choe Hwanjin" in LICENSE.read_text(encoding="utf-8"), "the libhangul BSD-3-Clause text is missing from resources/licenses")
    for channel, description in NOTICE_CHANNELS.items():
        live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
        check(any(LICENSE.name in line for line in live), f"{channel} ({description}) does not ship {LICENSE.name}")
    dictionary = os.environ.get("MSIME_DICTIONARY")
    source = Path(dictionary) / SOURCE if dictionary else None
    if source is None:
        print("skipped: regeneration, set MSIME_DICTIONARY=<msime-dictionary checkout> (msime-dict-build hanja --dictionary reads the same file)")
    elif not source.is_file():
        print(f"skipped: regeneration, {source} does not exist (MSIME_DICTIONARY must be a msime-dictionary checkout)")
    else:
        data = source.read_bytes()
        if len(data) != SOURCE_SIZE or hashlib.sha256(data).hexdigest() != SOURCE_SHA256:
            check(False, f"{source} is not libhangul {COMMIT}; update COMMIT, SOURCE_SIZE and SOURCE_SHA256 with the table")
        else:
            check(generate(data.decode("utf-8")) == table, "the committed table differs from what the libhangul source generates; rerun msime-dict-build hanja")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"korean hanja table: {len(rows)} readings of {len(order)} syllables")
    return 0


if __name__ == "__main__":
    sys.exit(main())
