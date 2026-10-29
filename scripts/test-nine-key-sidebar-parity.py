#!/usr/bin/env python3
"""九键左侧符号栏的两张表在三端用同一套规则（#10223，见 .agents/notes/implemented/feature/2026-10-29-nine-key-sidebar-shared.md）。

共享偏好 `touch_nine_key_sidebar_symbols` 与 `touch_nine_key_digit_sidebar_symbols` 由 client-core 规范化，三端的编辑器各自做整表校验、各自列出可加入的常用符号、各自写标题和说明，自定义的成对符号（`（）`、`()`）上屏后光标停在中间也是各端自己查表。任一端单独改了上限、可加入的符号、标题说明或成对符号表，编辑器就会放行另一端不收的表，或者同一张表在三端看起来、打出来不一样。这里核对：上限与 client-core 常量相同；可加入的符号三端逐项、同顺序相同；两个标题、两条说明三端逐字相同；成对符号表三端相同；字母层出厂表三端相同；HarmonyOS 键盘与共享设置页的出厂表相同（数字层出厂表三端有意不同，不比）。
"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
RUST = ROOT / "crates/client-core/src/preferences.rs"
ANDROID_POLICY = ROOT / "platforms/android/java/app/msime/android/keyboard/NineKeySidebarPolicy.java"
ANDROID_PAGE = ROOT / "platforms/android/java/app/msime/android/home/KeyLayoutStylesPolicy.java"
ANDROID_PAIRS = ROOT / "platforms/android/java/app/msime/android/keyboard/PairedPunctuationPolicy.java"
IOS = ROOT / "platforms/ios/SharedUI/preferences/NineKeySidebarPreference.swift"
SHARED = ROOT / "packages/ui/src/keyboard/touch-key-layout-styles.ts"
HARMONY_PAIRS = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/input/PairedPunctuationPolicy.ts"
HARMONY_POLICY = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/TouchLayoutStylePolicy.ts"

STRING = re.compile(r'"((?:[^"\\]|\\.)*)"')

failures: list[str] = []


def fail(message: str) -> None:
    failures.append(message)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def block(text: str, start: str, end: str, where: str) -> str:
    """`start` 之后到第一个 `end` 为止的一段源码。"""
    at = text.find(start)
    if at < 0:
        fail(f"{where}: 找不到 {start!r}")
        return ""
    stop = text.find(end, at + len(start))
    if stop < 0:
        fail(f"{where}: {start!r} 之后找不到 {end!r}")
        return ""
    return text[at + len(start) : stop]


def strings(text: str, start: str, end: str, where: str) -> list[str]:
    """一段源码里按出现顺序的字符串字面量（三端的这几张表都不含转义）。"""
    return STRING.findall(block(text, start, end, where))


def integer(text: str, pattern: str, where: str) -> int | None:
    match = re.search(pattern, text)
    if match is None:
        fail(f"{where}: 找不到 {pattern}")
        return None
    return int(match.group(1))


def literal(text: str, pattern: str, where: str) -> str | None:
    match = re.search(pattern, text)
    if match is None:
        fail(f"{where}: 找不到 {pattern}")
        return None
    return match.group(1)


def same(name: str, values: dict[str, object]) -> None:
    distinct = {repr(value) for value in values.values()}
    if len(distinct) > 1:
        details = "; ".join(f"{where} = {value!r}" for where, value in values.items())
        fail(f"{name} 三端不同：{details}")


def main() -> int:
    rust = read(RUST)
    android = read(ANDROID_POLICY)
    page = read(ANDROID_PAGE)
    android_pairs = read(ANDROID_PAIRS)
    ios = read(IOS)
    shared = read(SHARED)
    harmony_pairs = read(HARMONY_PAIRS)
    harmony_policy = read(HARMONY_POLICY)

    limits = (
        ("一张表最多几项", "NINE_KEY_SIDEBAR_MAX_SYMBOLS", "MAX_SYMBOLS", "maxSymbols", "nineKeySidebarMaxSymbols"),
        ("一项最多几个码位", "NINE_KEY_SIDEBAR_MAX_SYMBOL_CODE_POINTS", "MAX_SYMBOL_CODE_POINTS", "maxSymbolCodePoints",
         "nineKeySidebarMaxSymbolCodePoints"),
        ("空格连起来最多几个 UTF-16 单元", "NINE_KEY_SIDEBAR_MAX_TEXT_UTF16", "MAX_TEXT_LENGTH", "maxTextUTF16",
         "nineKeySidebarMaxTextUtf16"),
    )
    for name, rust_name, android_name, ios_name, shared_name in limits:
        same(name, {
            "client-core": integer(rust, rf"pub const {rust_name}: usize = (\d+);", "preferences.rs"),
            "Android": integer(android, rf"public static final int {android_name} = (\d+);", "NineKeySidebarPolicy.java"),
            "iOS": integer(ios, rf"static let {ios_name} = (\d+)", "NineKeySidebarPreference.swift"),
            "共享设置页": integer(shared, rf"export const {shared_name} = (\d+);", "touch-key-layout-styles.ts"),
        })

    addable = {
        "Android": strings(android, "ADDABLE_SYMBOLS = List.of(", ");", "NineKeySidebarPolicy.java"),
        "iOS": strings(ios, "static let addableSymbols = [", "]\n", "NineKeySidebarPreference.swift"),
        "共享设置页": strings(shared, "export const nineKeySidebarAddableSymbols: readonly string[] = [", "];",
                         "touch-key-layout-styles.ts"),
    }
    same("可加入的常用符号", addable)
    if not addable["Android"]:
        fail("Android ADDABLE_SYMBOLS 读出来是空的")

    texts = (
        ("字母表标题", r'NINE_KEY_SIDEBAR_TITLE = "([^"]*)";', r'static let letterTitle = "([^"]*)"',
         r'export const nineKeySidebarTitle = "([^"]*)";'),
        ("数字表标题", r'DIGIT_SIDEBAR_TITLE = "([^"]*)";', r'static let digitTitle = "([^"]*)"',
         r'export const numberPadSidebarTitle = "([^"]*)";'),
        ("字母表说明", r'NINE_KEY_SIDEBAR_NOTE = "([^"]*)";', r'static let letterNote = "([^"]*)"',
         r'export const nineKeySidebarNote = "([^"]*)";'),
        ("数字表说明", r'DIGIT_SIDEBAR_NOTE = "([^"]*)";', r'static let digitNote = "([^"]*)"',
         r'export const numberPadSidebarNote = "([^"]*)";'),
    )
    for name, android_pattern, ios_pattern, shared_pattern in texts:
        same(name, {
            "Android": literal(page, android_pattern, "KeyLayoutStylesPolicy.java"),
            "iOS": literal(ios, ios_pattern, "NineKeySidebarPreference.swift"),
            "共享设置页": literal(shared, shared_pattern, "touch-key-layout-styles.ts"),
        })

    pairs = {
        "Android": sorted(re.findall(r'Map\.entry\("([^"]*)", "([^"]*)"\)',
                                     block(android_pairs, "SYMBOL_PAIRS = Map.ofEntries(", ");",
                                           "PairedPunctuationPolicy.java"))),
        "iOS": sorted(re.findall(r'"([^"]*)": "([^"]*)"',
                                 block(ios, "static let pairedSymbols: [String: String] = [", "]\n",
                                       "NineKeySidebarPreference.swift"))),
        "HarmonyOS": sorted(re.findall(r'\{ opening: "([^"]*)", closing: "([^"]*)" \}',
                                       block(harmony_pairs, "const LITERAL_PAIRS: LiteralPair[] = [", "];",
                                             "PairedPunctuationPolicy.ts"))),
    }
    same("成对符号表", pairs)
    if not pairs["Android"]:
        fail("Android SYMBOL_PAIRS 读出来是空的")

    shared_letter = strings(shared, "letter: [", "]", "touch-key-layout-styles.ts")
    shared_digit = strings(shared, "digit: [", "]", "touch-key-layout-styles.ts")
    same("字母层出厂表", {
        "Android": strings(android, "DEFAULT_LETTER_SYMBOLS =\n        List.of(", ");", "NineKeySidebarPolicy.java"),
        "iOS": strings(ios, "static let defaultLetterSymbols = [", "]", "NineKeySidebarPreference.swift"),
        "共享设置页": shared_letter,
    })
    same("HarmonyOS 键盘与共享设置页的字母层出厂表", {
        "键盘": strings(harmony_policy, "NINE_KEY_SIDEBAR_DEFAULTS: string[] = [", "]", "TouchLayoutStylePolicy.ts"),
        "共享设置页": shared_letter,
    })
    same("HarmonyOS 键盘与共享设置页的数字层出厂表", {
        "键盘": strings(harmony_policy, "DIGIT_SIDEBAR_DEFAULTS: string[] = [", "]", "TouchLayoutStylePolicy.ts"),
        "共享设置页": shared_digit,
    })

    if failures:
        for message in failures:
            print(f"FAIL: {message}", file=sys.stderr)
        return 1
    print(f"nine-key sidebar parity: {len(addable['Android'])} addable symbols, {len(pairs['Android'])} pairs, limits, titles and notes match")
    return 0


if __name__ == "__main__":
    sys.exit(main())
