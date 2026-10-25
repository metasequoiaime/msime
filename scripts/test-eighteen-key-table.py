#!/usr/bin/env python3
"""校验 Android、iOS 十八键键面、引擎编码表和四端统计键位的契约。"""

from pathlib import Path
import re
import string

ROOT = Path(__file__).resolve().parent.parent


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def main() -> None:
    engine = read("crates/engine/src/nine_key.rs")
    block = re.search(r"const EIGHTEEN_KEY_GROUPS:.*?= \[(.*?)\];", engine, re.S)
    assert block, "引擎十八键分组声明缺失"
    groups = re.findall(r'"([a-z]+)"', block[1])
    assert len(groups) == 18 and sorted("".join(groups)) == list(string.ascii_lowercase), \
        "十八键必须恰好覆盖二十六个字母"
    codes = {letter: group[0] for group in groups for letter in group}
    keypad = re.search(r'const EIGHTEEN_KEYPAD:.*?= b"([a-z]+)";', engine)
    assert keypad and keypad[1] == "".join(codes[c] for c in string.ascii_lowercase), \
        "引擎编码表与十八组字母不符"

    android = read("platforms/android/java/app/msime/android/keyboard/EighteenKeyLayout.java")
    block = re.search(r"ROWS = List\.of\((.*?)\);", android, re.S)
    assert block, "Android 十八键键表缺失"
    rows = [re.findall(r'key\("([a-z]+)"\)', line) for line in block[1].splitlines()]
    rows = [row for row in rows if row]
    assert list(map(len, rows)) == [7, 6, 5], "Android 十八键行数或键数变化"
    assert [group for row in rows for group in row] == groups, "Android 键面与引擎分组不符"

    ios_layout = read("platforms/ios/KeyboardExtension/Sources/input/EighteenKeyLayout.swift")
    block = re.search(r"static let rows: \[\[FourteenKeyLayout\.Key\]\] = \[(.*?)\]\.map", ios_layout, re.S)
    assert block, "iOS 十八键键表缺失"
    rows = [re.findall(r'"([a-z]+)"', row) for row in re.findall(r"\[([^\[\]]*)\]", block[1])]
    rows = [row for row in rows if row]
    assert list(map(len, rows)) == [7, 6, 5], "iOS 十八键行数或键数变化"
    assert [group for row in rows for group in row] == groups, "iOS 键面与引擎分组不符"

    expected = ["Eighteen" + group.upper() for group in groups]
    for path in [
        "crates/client-services/src/typing_statistics.rs",
        "platforms/android/java/app/msime/android/statistics/KeyPressIds.java",
        "platforms/harmony/entry/src/main/ets/keyboard/KeyIdPolicy.ts",
    ]:
        actual = re.findall(r'"(Eighteen[A-Z]+)"', read(path))
        assert actual == expected, f"{path} 的十八键统计白名单不符"
    ios = read("platforms/ios/SharedUI/core/TypingStatistics.swift")
    block = re.search(r"let eighteenKey: \[String\] = \[(.*?)\]\.map", ios, re.S)
    assert block and ["Eighteen" + face for face in re.findall(r'"([A-Z]+)"', block[1])] \
        == expected, "iOS 十八键统计白名单不符"
    print("eighteen-key table and statistics contracts: passed")


if __name__ == "__main__":
    main()
