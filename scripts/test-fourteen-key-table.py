#!/usr/bin/env python3
"""全拼 14 键的键表三端一致：Android、iOS、HarmonyOS 的 `FourteenKeyLayout` 与引擎 `KeyGrid::FourteenKey` 是同一张表。

14 键的键面由三端各自画，解码只在引擎里：宿主把一组的首字母经 `msime_client_grid_key` 交给引擎，引擎按 `FOURTEEN_KEYPAD` 把 `a`–`z` 归成组码。某一端把一个字母挪到相邻的键上（比如画成 `QW E RT`），照样能编译、能打字，候选也照样出来，只是那一端按下的组和引擎认的组不再是同一组，用户打出的字悄悄变了，三端也不再一致。打字统计的 `Fourteen*` 键位 id 同理：client-core 的 `KEY_IDS` 是严格契约，宿主上报一个它不认识的 id 会让整批统计被拒。

这里离线比对：
- 引擎的十四组按键面顺序正好覆盖 `a`–`z` 各一次，`FOURTEEN_KEYPAD` 就是由这十四组逐字母取首字母生成的；
- 三端的三行键与引擎的十四组逐键相同，每行键数也相同（5、5、4）；
- Android 的 `GROUP_CODES` 与引擎的编码表逐字相同；
- client-core、Android、iOS、HarmonyOS 的 `Fourteen*` 键位 id 都是按键面顺序的 `Fourteen` 加这一组的大写字母。
"""

from __future__ import annotations

import pathlib
import re
import string
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ENGINE = ROOT / "crates/engine/src/nine_key.rs"
ANDROID = ROOT / "platforms/android/java/app/msime/android/keyboard/FourteenKeyLayout.java"
IOS = ROOT / "platforms/ios/KeyboardExtension/Sources/input/FourteenKeyLayout.swift"
HARMONY = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/input/FourteenKeyLayout.ts"
KEY_ID_SOURCES = {
    "client-core": ROOT / "crates/client-core/src/typing_statistics.rs",
    "android": ROOT / "platforms/android/java/app/msime/android/statistics/KeyPressIds.java",
    "harmony": ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyIdPolicy.ts",
}
IOS_KEY_IDS = ROOT / "platforms/ios/SharedUI/core/TypingStatistics.swift"

ROW_SIZES = [5, 5, 4]


def relative(path: pathlib.Path) -> str:
    return str(path.relative_to(ROOT))


def engine_table(findings: list[str]) -> tuple[list[str], str] | None:
    text = ENGINE.read_text(encoding="utf-8")
    keypad = re.search(r'const FOURTEEN_KEYPAD: &\[u8; 26\] = b"([a-z]{26})";', text)
    groups = re.search(r"const FOURTEEN_KEY_GROUPS: \[&str; 14\] = \[(.*?)\];", text, re.S)
    if not keypad or not groups:
        findings.append(f"{relative(ENGINE)} 不再以字面量声明 FOURTEEN_KEYPAD 和 FOURTEEN_KEY_GROUPS")
        return None
    return re.findall(r'"([a-z]+)"', groups.group(1)), keypad.group(1)


def codes_from_groups(groups: list[str]) -> str:
    code = {letter: group[0] for group in groups for letter in group}
    return "".join(code.get(letter, "?") for letter in string.ascii_lowercase)


def rows_of(path: pathlib.Path, row_pattern: str, key_pattern: str, findings: list[str]) -> list[list[str]] | None:
    text = path.read_text(encoding="utf-8")
    rows = [re.findall(key_pattern, row) for row in re.findall(row_pattern, text)]
    rows = [row for row in rows if row]
    if not rows:
        findings.append(f"{relative(path)} 里找不到 14 键的三行键表")
        return None
    return rows


def host_rows(findings: list[str]) -> dict[str, list[list[str]] | None]:
    android_text = ANDROID.read_text(encoding="utf-8")
    block = re.search(r"ROWS = List\.of\((.*?)\);", android_text, re.S)
    android = None
    if block:
        # 一行键写在一行源码里：`List.of(new Key("qw"), ...)`，逐行取。
        android = [re.findall(r'new Key\("([a-z]+)"\)', line) for line in block.group(1).splitlines()]
        android = [row for row in android if row] or None
    if android is None:
        findings.append(f"{relative(ANDROID)} 里找不到 14 键的三行键表（ROWS = List.of(...)）")

    ios_text = IOS.read_text(encoding="utf-8")
    block = re.search(r"static let rows: \[\[Key\]\] = \[(.*?)\]\.map", ios_text, re.S)
    ios = None
    if block:
        ios = [re.findall(r'"([a-z]+)"', row) for row in re.findall(r"\[([^\[\]]*)\]", block.group(1))]
        ios = [row for row in ios if row] or None
    if ios is None:
        findings.append(f"{relative(IOS)} 里找不到 14 键的三行键表（static let rows）")

    harmony_text = HARMONY.read_text(encoding="utf-8")
    block = re.search(r"const ROWS: FourteenKey\[\]\[\] = \[(.*?)\];", harmony_text, re.S)
    harmony = None
    if block:
        harmony = [re.findall(r'key\("([a-z]+)"\)', row) for row in re.findall(r"\[([^\[\]]*)\]", block.group(1))]
        harmony = [row for row in harmony if row] or None
    if harmony is None:
        findings.append(f"{relative(HARMONY)} 里找不到 14 键的三行键表（const ROWS）")
    return {"android": android, "ios": ios, "harmony": harmony}


def key_ids(findings: list[str]) -> dict[str, list[str] | None]:
    found: dict[str, list[str] | None] = {}
    for name, path in KEY_ID_SOURCES.items():
        ids = re.findall(r'"(Fourteen[A-Z]+)"', path.read_text(encoding="utf-8"))
        if not ids:
            findings.append(f"{relative(path)} 里没有 Fourteen* 键位 id")
        found[name] = ids or None
    # iOS 把键面字母列成一行再拼上前缀：`["QW", ...].map { "Fourteen\($0)" }`。
    match = re.search(r"let fourteenKey: \[String\] = \[([^\]]*)\]\.map \{ \"Fourteen\\\(\$0\)\" \}", IOS_KEY_IDS.read_text(encoding="utf-8"))
    if match:
        found["ios"] = ["Fourteen" + face for face in re.findall(r'"([A-Z]+)"', match.group(1))]
    else:
        findings.append(f"{relative(IOS_KEY_IDS)} 里找不到 14 键键位 id 的列表（let fourteenKey）")
        found["ios"] = None
    return found


def main() -> int:
    findings: list[str] = []
    engine = engine_table(findings)
    if engine is None:
        for finding in findings:
            print(f"FAIL {finding}", file=sys.stderr)
        return 1
    groups, keypad = engine

    letters = "".join(groups)
    if len(groups) != 14 or sorted(letters) != list(string.ascii_lowercase):
        findings.append(f"引擎的 FOURTEEN_KEY_GROUPS 不是十四组、各含 a–z 一次：{groups}")
    derived = codes_from_groups(groups)
    if derived != keypad:
        findings.append(f"引擎的 FOURTEEN_KEYPAD {keypad} 与十四组逐字母取首字母得到的 {derived} 不同")

    for host, rows in host_rows(findings).items():
        if rows is None:
            continue
        sizes = [len(row) for row in rows]
        if sizes != ROW_SIZES:
            findings.append(f"{host} 的 14 键每行键数是 {sizes}，应为 {ROW_SIZES}")
        flat = [key for row in rows for key in row]
        if flat != groups:
            findings.append(f"{host} 的 14 键键表 {flat} 与引擎的 {groups} 不同")

    android_codes = re.search(r'GROUP_CODES = "([a-z]+)";', ANDROID.read_text(encoding="utf-8"))
    if not android_codes:
        findings.append(f"{relative(ANDROID)} 不再声明 GROUP_CODES")
    elif android_codes.group(1) != keypad:
        findings.append(f"Android 的 GROUP_CODES {android_codes.group(1)} 与引擎的 {keypad} 不同")

    expected_ids = ["Fourteen" + group.upper() for group in groups]
    for source, ids in key_ids(findings).items():
        if ids is not None and ids != expected_ids:
            findings.append(f"{source} 的 14 键键位 id {ids} 与按键表推出的 {expected_ids} 不同")

    if findings:
        for finding in findings:
            print(f"FAIL {finding}", file=sys.stderr)
        return 1
    print(f"14 键键表：引擎、Android、iOS、HarmonyOS 一致（{len(groups)} 键，编码表 {keypad}），四处键位 id 一致")
    return 0


if __name__ == "__main__":
    sys.exit(main())
