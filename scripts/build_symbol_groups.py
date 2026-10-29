#!/usr/bin/env python3
"""生成符号模式（Shift+S）的分组码表。

分组码（`xl` 希腊字母、`sx` 数学符号、`szq` 带圈数字、`jt` 箭头……，沿用此前那张表的编码，来历见 GROUPS 上方的注释）、中文组名和每组的取字规则都写在本文件的 GROUPS 里，组员只按 Unicode 字符数据库（UCD）的事实挑选：区块（Blocks.txt）、通用类别、字符名、分解映射（UnicodeData.txt），偏旁一组再用部首到统一汉字的等价映射（EquivalentUnifiedIdeograph.txt）。组内按码位排序，几处例外的排序键也由数据导出、写在对应规则旁边（上标、下标按分解出的基本字符；带变音的字母跟在它的基本字母后面；数学字母各字体按区里的排法，空位由字母类符号区的同形字补上；拼音按《汉语拼音方案》的字母与四声），不沿用任何输入法符号表的取字或顺序。输入按 resources/symbol-groups.lock.json 固定的 UCD 版本从 unicode.org 下载并校验长度与 sha256，许可是 Unicode License V3（resources/licenses/Unicode-License-V3.txt，拆字反查的 Unihan 数据已经把它接进了各平台的声明渠道）。

输出一张 UTF-8 TSV，提交进仓库，由 crates/engine/src/local/symbol_groups.rs 用 include_str! 编进引擎：

- crates/engine/src/local/symbol_groups.tsv：每行 `编码<TAB>中文组名<TAB>符号[<空格>符号...]`，组的先后即 GROUPS 的先后。符号之间用一个空格分隔，所以符号本身不含空白；规则选出空白、控制字符或变体选择符时生成失败。

结果只取决于锁定的输入：不用 Python 自带的 unicodedata（它随解释器的 Unicode 版本变化），字符名、类别、分解和区块全部从锁定的文件里读。

--check 重新生成后和仓库里的表逐字节比较，不写文件。输入不在缓存里时 --check 打印 skip 并以 0 退出（契约检查不联网），设了 MSIME_FETCH=1 才下载。

usage: build_symbol_groups.py [--cache <directory>] [--check]   （缓存默认 target/symbol-groups-cache）
"""
import argparse
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.dont_write_bytecode = True  # 导入 scripts/ 下的脚本时不留下 __pycache__
from fetch_wordbooks import digest, fetch  # noqa: E402  （按锁下载并校验长度和 sha256）

LOCK = ROOT / "resources/symbol-groups.lock.json"
OUTPUT = ROOT / "crates/engine/src/local/symbol_groups.tsv"
DEFAULT_CACHE = ROOT / "target/symbol-groups-cache"

# 与 crates/engine/src/local/symbol_groups.rs 的 SYMBOL_MODE_LIMIT 相同：整串分组码一次最多给出这么多行，超出的符号打不出来。康熙部首是 214 个部首的完整集合，按规则不拆，是唯一允许超出的组。
SYMBOL_MODE_LIMIT = 200
OVERSIZED = {"kx"}


class Ucd:
    """锁定版本的 UnicodeData.txt、Blocks.txt 与 EquivalentUnifiedIdeograph.txt。"""

    def __init__(self, unicode_data: str, blocks: str, equivalents: str) -> None:
        self.name: dict[int, str] = {}
        self.category: dict[int, str] = {}
        self.decomposition: dict[int, str] = {}
        self.upper: dict[int, int] = {}
        # `<CJK Ideograph, First>` 到 `<…, Last>` 这类区间没有逐字的名字，规则不按名字挑它们，只在检查输出时要知道它们的类别。
        self.ranges: list[tuple[int, int, str]] = []
        first = None
        for line in unicode_data.splitlines():
            fields = line.split(";")
            point, name = int(fields[0], 16), fields[1]
            if name.endswith(", First>"):
                first = point
                continue
            if name.endswith(", Last>"):
                self.ranges.append((first, point, fields[2]))
                continue
            if name.startswith("<"):
                continue
            self.name[point] = name
            self.category[point] = fields[2]
            self.decomposition[point] = fields[5]
            if fields[12]:
                self.upper[point] = int(fields[12], 16)
        self.blocks: list[tuple[int, int, str]] = []
        for line in blocks.splitlines():
            match = re.match(r"([0-9A-F]+)\.\.([0-9A-F]+); (.+)$", line)
            if match:
                self.blocks.append((int(match[1], 16), int(match[2], 16), match[3]))
        self.equivalent: dict[int, int] = {}
        for line in equivalents.splitlines():
            line = line.split("#", 1)[0].strip()
            if not line:
                continue
            source, target = (part.strip() for part in line.split(";"))
            # 一行可以是一段码位（`2EBE..2EC0 ; 8279`，艹的三种写法都等价于 U+8279），段里每个码位都映射到同一个字。
            first, _, last = source.partition("..")
            for point in range(int(first, 16), int(last or first, 16) + 1):
                self.equivalent[point] = int(target, 16)
        # 规范组合：两个码位的规范分解 → 组合后的字。拼音一组用它把基本字母与声调符号拼成预组合字。
        self.compose: dict[tuple[int, int], int] = {}
        for point, mapping in self.decomposition.items():
            parts = mapping.split()
            if len(parts) == 2 and not parts[0].startswith("<"):
                self.compose[(int(parts[0], 16), int(parts[1], 16))] = point

    def category_of(self, point: int) -> str:
        """通用类别，区间里的字（统一汉字等）也算；未分配的码位是 Cn。"""
        if point in self.category:
            return self.category[point]
        for first, last, category in self.ranges:
            if first <= point <= last:
                return category
        return "Cn"

    def block(self, name: str) -> range:
        for first, last, block in self.blocks:
            if block == name:
                return range(first, last + 1)
        raise ValueError(f"no block named {name!r} in the pinned Blocks.txt")

    def assigned(self, *blocks: str) -> list[int]:
        """几个区块里已分配（UnicodeData.txt 有名字）的码位，按码位排序。"""
        return sorted(point for name in blocks for point in self.block(name) if point in self.name)

    def named(self, pattern: str, points=None) -> list[int]:
        """名字整串匹配正则的码位，按码位排序；不给 points 时查整个 UCD。"""
        regex = re.compile(pattern)
        pool = sorted(self.name) if points is None else points
        return [point for point in pool if regex.fullmatch(self.name[point])]

    def font_target(self, point: int) -> int | None:
        """`<font>`、`<super>`、`<sub>` 这类兼容分解只有一个目标字时返回它。"""
        parts = self.decomposition.get(point, "").split()
        if len(parts) == 2 and parts[0].startswith("<"):
            return int(parts[1], 16)
        return None

    def canonical_singleton(self, point: int) -> int | None:
        """规范分解只有一个码位时返回它：OHM SIGN → Ω、ANGSTROM SIGN → Å。这类字只为兼容旧编码而存在，NFC 会把它们改写成目标字。"""
        parts = self.decomposition.get(point, "").split()
        if len(parts) == 1 and not parts[0].startswith("<"):
            return int(parts[0], 16)
        return None

    def base_letter(self, point: int) -> int:
        """规范分解的第一个码位，递归到底：Ё → Е、ǖ → u；没有规范分解时是它自己。"""
        parts = self.decomposition.get(point, "").split()
        if parts and not parts[0].startswith("<"):
            return self.base_letter(int(parts[0], 16))
        return point

    def composed(self, base: str, mark: int) -> str:
        """基本字母加一个组合符号：有预组合字时给预组合字，没有时给两个码位的序列（m̄、ê̄）。"""
        if len(base) == 1 and (ord(base), mark) in self.compose:
            return chr(self.compose[(ord(base), mark)])
        return base + chr(mark)


# ---- 各组的取字规则 ----
# 每条规则收一个 Ucd、返回有序的符号列表（一般是 chr(码位)，拼音一组有两个码位的序列）。规则按区块、名字、类别和分解挑字；要点名个别字时写的是 Unicode 字符名（单位、声调、音标几组），拼音一组按《汉语拼音方案》列出字母与声调，原因都写在规则旁边。


def chars(points) -> list[str]:
    return [chr(point) for point in points]


def by_name(pattern: str, *blocks: str):
    def rule(ucd: Ucd) -> list[str]:
        return chars(ucd.named(pattern, ucd.assigned(*blocks) if blocks else None))

    return rule


def whole_blocks(*blocks: str, categories: str = ""):
    """区块里已分配的字；给了 categories 时只取通用类别以其中某个前缀开头的字（`"L"` 是全部字母、`"Lo"` 只要其他字母）。"""

    def rule(ucd: Ucd) -> list[str]:
        return chars(
            point
            for point in ucd.assigned(*blocks)
            if not categories or any(ucd.category[point].startswith(category) for category in categories.split())
        )

    return rule


def union(*rules):
    """几条规则的并集，按码位排序、去重。"""

    def rule(ucd: Ucd) -> list[str]:
        merged = {symbol for part in rules for symbol in part(ucd)}
        return sorted(merged, key=lambda symbol: [ord(character) for character in symbol])

    return rule


def math_operators(ucd: Ucd) -> list[str]:
    # 拉丁-1 的数学符号（¬ ± × ÷，ASCII 里的 + < = > | ~ 键盘上就有）加上数学运算符区，去掉几类变体把一组控制在 200 行内：名字带 WITH 的修饰形（带点、带横、带圈的变体），名字以 SMALL 开头的小号形（∊ ∍），有兼容分解的连写形（∬ ∭ ∯ ∰），以及有规范分解的否定形（∉ ≢ ⊄）——否定形只留下否定 ASCII 关系符的 ≠ ≮ ≯。省略号（⋮ ⋯）归在非夹注符号一组。
    points = [point for point in ucd.assigned("Latin-1 Supplement") if ucd.category[point] == "Sm"]
    for point in ucd.assigned("Mathematical Operators"):
        name, mapping = ucd.name[point], ucd.decomposition[point]
        if " WITH " in name or name.startswith("SMALL ") or "ELLIPSIS" in name:
            continue
        if mapping and (mapping.startswith("<") or int(mapping.split()[0], 16) >= 0x80):
            continue
        points.append(point)
    return chars(points)


def relation(word: str):
    # 名字含 GREATER-THAN / LESS-THAN 的数学符号，只取 ASCII、数学运算符区和全角、小号形式，补充数学运算符区里那几十个罕用变体不收。
    return by_name(
        rf".*\b{word}\b.*",
        "Basic Latin",
        "Mathematical Operators",
        "Small Form Variants",
        "Halfwidth and Fullwidth Forms",
    )


def scripted(kind: str):
    """上标（`<super>`）或下标（`<sub>`）：基本多文种平面里分解成单个字的字，且那个字是拉丁、希腊、西里尔字母或数字、符号、标点（不收分解成汉字的汉文训读符号 ㆒ 一类，也不收格鲁吉亚、提非纳文的修饰字母）；按对应的基本字符排序，同一基本字符按码位。"""

    def rule(ucd: Ucd) -> list[str]:
        picked = []
        for point in sorted(ucd.name):
            if point > 0xFFFF or not ucd.decomposition[point].startswith(f"<{kind}> "):
                continue
            target = ucd.font_target(point)
            if target is None or target not in ucd.name:
                continue
            if not (ucd.name[target].startswith(("LATIN ", "GREEK ", "CYRILLIC ", "MODIFIER LETTER ")) or ucd.category[target][0] in "NSP"):
                continue
            picked.append((target, point))
        return chars(point for _, point in sorted(picked))

    return rule


def letters(first: int, last: int, extra: tuple[int, ...] = ()):
    """一段连续码位里的字母，再加上规范分解到这一段里的字母（俄文的 ё 分解为 е + ̈）；带变音的字母排在它的基本字母后面。"""

    def rule(ucd: Ucd) -> list[str]:
        points = [point for point in range(first, last + 1) if point in ucd.name and ucd.category[point].startswith("L")]
        points += [point for point in extra if point in ucd.name]
        return chars(sorted(points, key=lambda point: (ucd.base_letter(point), point != ucd.base_letter(point), point)))

    return rule


# 汉语拼音方案：韵母表的单韵母 a o e ê i u ü 加上 m、n 两个可以自成音节的鼻音，声调符号依次是阴平（U+0304）、阳平（U+0301）、上声（U+030C）、去声（U+0300）；ê、ü 本身也是拼音字母。zh ch sh 可以省作 ẑ ĉ ŝ（U+0302），ng 可以省作 ŋ。
PINYIN_VOWELS = ("a", "o", "e", "ê", "i", "u", "ü", "m", "n")
PINYIN_TONES = (0x0304, 0x0301, 0x030C, 0x0300)
PINYIN_BARE = ("ê", "ü")
PINYIN_ABBREVIATIONS = (("z", 0x0302), ("c", 0x0302), ("s", 0x0302))
PINYIN_ENG = "ŋ"


def pinyin(upper: bool):
    def rule(ucd: Ucd) -> list[str]:
        def case(text: str) -> str:
            return "".join(chr(ucd.upper.get(ord(character), ord(character))) if upper else character for character in text)

        symbols = []
        for vowel in PINYIN_VOWELS:
            if vowel in PINYIN_BARE:
                symbols.append(case(vowel))
            symbols += [ucd.composed(case(vowel), tone) for tone in PINYIN_TONES]
        symbols += [ucd.composed(case(letter), mark) for letter, mark in PINYIN_ABBREVIATIONS]
        symbols.append(case(PINYIN_ENG))
        return symbols

    return rule


def tone_marks(ucd: Ucd) -> list[str]:
    # 声调：名字里有 TONE BAR、CHINESE TONE 或以 TONE MARK 结尾的字（五度标调的调号、汉语四声的 ꜀ ꜁ 一类、汉字的 〪 〫 〬 〭），加上注音与拼音标调用的间隔修饰字母 ˉ ˊ ˇ ˋ ˙。不收带规范分解的字（U+0340、U+0341 是 U+0300、U+0301 的别名）和韩文的旁点（〮 〯）。
    spacing = {"MODIFIER LETTER MACRON", "MODIFIER LETTER ACUTE ACCENT", "CARON", "MODIFIER LETTER GRAVE ACCENT", "DOT ABOVE"}
    points = []
    for point in sorted(ucd.name):
        name = ucd.name[point]
        if ucd.decomposition[point] and not ucd.decomposition[point].startswith("<"):
            continue
        if name.startswith("HANGUL"):
            continue
        if name in spacing or re.search(r"TONE BAR|CHINESE TONE|TONE MARK$", name):
            points.append(point)
    return chars(points)


def phonetic(ucd: Ucd) -> list[str]:
    # IPA 扩展区的全部字母，加上英语音标常用而不在这个区里的 æ ð ŋ θ 和长音符 ː（按名字取）。
    names = {"LATIN SMALL LETTER AE", "LATIN SMALL LETTER ETH", "LATIN SMALL LETTER ENG", "GREEK SMALL LETTER THETA", "MODIFIER LETTER TRIANGULAR COLON"}
    points = ucd.assigned("IPA Extensions") + [point for point, name in ucd.name.items() if name in names]
    return chars(sorted(points))


# 数学字母数字符号区的字体：名字是 `MATHEMATICAL <字体> CAPITAL A` 这种形式。拉丁字母的几种字体在这个区里留了空位，空位上的字早已收在字母类符号区（ℎ ℬ ℭ ℂ……），它们的 `<font>` 分解指向同一个基本字母、名字里有同一种字体（BLACK-LETTER 就是 FRAKTUR）。
MATH_NAME = re.compile(r"MATHEMATICAL (?P<style>[A-Z -]+?) (?P<rest>(?:CAPITAL|SMALL) .+|DIGIT \w+|NABLA|PARTIAL DIFFERENTIAL|\w+ SYMBOL)")
STYLE_WORDS = {"SCRIPT": "SCRIPT", "FRAKTUR": "BLACK-LETTER", "DOUBLE-STRUCK": "DOUBLE-STRUCK"}


def math_style(style: str, greek: bool):
    """一种字体的拉丁字母与数字（greek=False）或希腊字母（greek=True）。拉丁字母按区里的排法：大写 A–Z、小写 a–z、数字 0–9，空位由字母类符号区的同形字补上；希腊字母按码位。"""

    def rule(ucd: Ucd) -> list[str]:
        found: dict[int, int] = {}
        for point in ucd.assigned("Mathematical Alphanumeric Symbols"):
            match = MATH_NAME.fullmatch(ucd.name[point])
            if not match or match["style"] != style:
                continue
            target = ucd.font_target(point)
            if target is None:
                continue
            is_greek = target not in ucd.block("Basic Latin")
            # 斜体的无点 ı ȷ（𝚤 𝚥）不是这两组要的基本拉丁字母，也不是希腊字母。
            if is_greek and ucd.name[target].startswith("LATIN"):
                continue
            if is_greek == greek:
                found[target] = point
        if greek:
            return chars(sorted(found.values()))
        wanted = [*range(ord("A"), ord("Z") + 1), *range(ord("a"), ord("z") + 1), *range(ord("0"), ord("9") + 1)]
        has_digits = any(chr(target).isdigit() for target in found)
        symbols = []
        for target in wanted:
            if chr(target).isdigit() and not has_digits:
                continue
            if target in found:
                symbols.append(chr(found[target]))
                continue
            fillers = [point for point in ucd.assigned("Letterlike Symbols") if ucd.font_target(point) == target and ucd.decomposition[point].startswith("<font>")]
            word = STYLE_WORDS.get(style)
            styled = [point for point in fillers if word and word in ucd.name[point]]
            pick = styled or (fillers if len(fillers) == 1 and not word else [])
            if len(pick) != 1:
                raise ValueError(f"MATHEMATICAL {style}: no single letterlike symbol fills the hole for {chr(target)!r}: {[ucd.name[p] for p in fillers]}")
            symbols.append(chr(pick[0]))
        return symbols

    return rule


def units(ucd: Ucd) -> list[str]:
    # 单位：CJK 兼容区里 `<square>` 分解全是拉丁、希腊字母和数字的方块单位（㎏ ㎡ ㏄……，不收片假名和年号的方块），加上名字点明是单位或比例的几个字。OHM SIGN、ANGSTROM SIGN 有单码位的规范分解，NFC 会改写它们，Unicode 也建议用目标字，所以收的是 Ω（U+03A9）和 Å（U+00C5）；KELVIN SIGN 的目标字是键盘上就有的 K，不收。
    names = {"DEGREE SIGN", "MICRO SIGN", "PER MILLE SIGN", "PER TEN THOUSAND SIGN", "DEGREE CELSIUS", "DEGREE FAHRENHEIT", "OHM SIGN", "ANGSTROM SIGN", "FULLWIDTH PERCENT SIGN"}
    points = [ucd.canonical_singleton(point) or point for point, name in ucd.name.items() if name in names]
    for point in ucd.assigned("CJK Compatibility"):
        parts = ucd.decomposition[point].split()
        if parts[:1] == ["<square>"] and all(int(part, 16) < 0x3000 for part in parts[1:]):
            points.append(point)
    return chars(sorted(points))


def currency(ucd: Ucd) -> list[str]:
    # 货币：基本多文种平面里通用类别为 Sc（货币符号）的全部字。
    return chars(point for point in sorted(ucd.name) if point <= 0xFFFF and ucd.category[point] == "Sc")


def radicals_as_ideographs(block: str):
    """部首区里的字换成等价的统一汉字：康熙部首区用 UnicodeData.txt 的兼容分解（⼀ → 一），部首补充区用 EquivalentUnifiedIdeograph.txt（⺅ → 亻），没有等价字的部首保留原字；去重后按部首的码位排。"""

    def rule(ucd: Ucd) -> list[str]:
        symbols: list[str] = []
        for point in ucd.assigned(block):
            target = ucd.font_target(point) or ucd.equivalent.get(point) or point
            if chr(target) not in symbols:
                symbols.append(chr(target))
        return symbols

    return rule


# 箭头区里印在键帽上的 ISO/IEC 9995-7 键盘符号：⇧ Shift、⇪ Caps Lock、⇥ ⇤ Tab、↩ ↵ 回车、⇞ ⇟ 翻页、⇱ ⇲ Home 与 End。
KEYCAP_ARROWS = {
    "UPWARDS WHITE ARROW",
    "UPWARDS WHITE ARROW FROM BAR",
    "RIGHTWARDS ARROW TO BAR",
    "LEFTWARDS ARROW TO BAR",
    "LEFTWARDS ARROW WITH HOOK",
    "DOWNWARDS ARROW WITH CORNER LEFTWARDS",
    "UPWARDS ARROW WITH DOUBLE STROKE",
    "DOWNWARDS ARROW WITH DOUBLE STROKE",
    "NORTH WEST ARROW TO CORNER",
    "SOUTH EAST ARROW TO CORNER",
}


def keyboard(ucd: Ucd) -> list[str]:
    # 电脑按键：杂项技术符号区里名字点明按键的字（⌘ PLACE OF INTEREST SIGN、⌥ OPTION KEY、⌫ ERASE TO THE LEFT、⌧ X IN A RECTANGLE BOX 即清除键、⏏ EJECT SYMBOL……）、媒体键（⏩ ⏪ ⏭ ⏮ ⏯ ⏸ ⏹ ⏺）和 ISO/IEC 9995-7 键盘符号所在的 U+2380–U+2395 一段，加上箭头区里同属 ISO/IEC 9995-7 的键帽箭头（KEYCAP_ARROWS），按码位排；最后是控制图形区（␛ ␡ ␣ 等控制字符与空格的图形）。
    named = ucd.named(
        r".*\b(KEY|KEYBOARD|ERASE|EJECT|RETURN|OPTION|PLACE OF INTEREST|ARROWHEAD)\b.*|X IN A RECTANGLE BOX|BLACK (RIGHT|LEFT|UP|DOWN)-POINTING (DOUBLE )?TRIANGLE.*|DOUBLE VERTICAL BAR|BLACK (SQUARE|CIRCLE) FOR \w+",
        ucd.assigned("Miscellaneous Technical"),
    )
    iso = [point for point in range(0x2380, 0x2396) if point in ucd.name]
    arrows = [point for point in ucd.assigned("Arrows") if ucd.name[point] in KEYCAP_ARROWS]
    if len(arrows) != len(KEYCAP_ARROWS):
        raise ValueError(f"the pinned UCD names only {len(arrows)} of the {len(KEYCAP_ARROWS)} keycap arrows")
    return chars(sorted(set(named + iso + arrows))) + chars(ucd.assigned("Control Pictures"))


def ungrouped_misc_symbols(ucd: Ucd) -> list[str]:
    # 常用符号：拉丁-1 与字母类符号区里通用类别为 So 的字（© ® ™ ℃ ℡……），加上杂项符号区里没有被其他组收走的字（星座、棋牌、音乐、性别、天体、骰子、五角星、叉号、太玄经的单双爻都另有分组）。
    taken = {symbol for code, _, rule in GROUPS if code != "fh" for symbol in rule(ucd)}
    points = [point for point in ucd.assigned("Latin-1 Supplement", "Letterlike Symbols") if ucd.category[point] == "So"]
    points += [point for point in ucd.assigned("Miscellaneous Symbols") if chr(point) not in taken]
    return chars(points)


PUNCTUATION_BLOCKS = ("Basic Latin", "Latin-1 Supplement", "General Punctuation", "Supplemental Punctuation", "CJK Symbols and Punctuation", "Vertical Forms", "CJK Compatibility Forms", "Small Form Variants", "Halfwidth and Fullwidth Forms")


# GB/T 15834—2011《标点符号用法》规定中文的破折号与省略号各占两个字的位置，写作两个 EM DASH、两个 HORIZONTAL ELLIPSIS；Unicode 没有对应的单个字符，这里按标准写成两个码位的序列，排在本组最前。
GB_T_15834_DOUBLED = ("\u2014\u2014", "\u2026\u2026")


def separators(ucd: Ucd) -> list[str]:
    # 非夹注符号（连接号、破折号、省略号、间隔号、分隔号）：先是 GB/T 15834 的 —— 与 ……，然后按码位排标点区块里通用类别为 Pd 的字，名字含 ELLIPSIS 或是 TWO DOT LEADER 的字（数学运算符区的 ⋮ ⋯ 也算），名字以 MIDDLE DOT 结尾的字（含片假名区的 ・），名字含 SOLIDUS 的斜线，以及 GB/T 15834 列为浪纹式连接号的 ～（FULLWIDTH TILDE）。
    points = []
    for point in ucd.assigned(*PUNCTUATION_BLOCKS, "Mathematical Operators", "Katakana"):
        name, category = ucd.name[point], ucd.category[point]
        if category[0] not in "PS":
            continue
        if category == "Pd" or "ELLIPSIS" in name or name == "TWO DOT LEADER" or name.endswith("MIDDLE DOT") or "SOLIDUS" in name or name == "FULLWIDTH TILDE":
            points.append(point)
    return [*GB_T_15834_DOUBLED, *chars(points)]


# ---- 分组表 ----
# 编码、中文组名、规则。编码与有哪些组沿用此前那张表的界面，好让符号模式的用法不变（Windows 正式版出厂配置点名了 xl、sx、szq、jt）：87 个编码里 54 个与 rime-prelude 的 symbols.yaml 相同，其余 33 个是 rime-wanxiang 加的，都是拼音首字母缩写，保留它们要维护者确认，见 .agents/notes/implemented/feature/2026-10-29-symbol-groups-from-unicode.md。中文组名和组的先后是本项目写的，组员与组内顺序只由下面的规则决定。
GROUPS = [
    # 数学
    ("sx", "数学符号", math_operators),
    ("dy", "大于", relation("GREATER-THAN")),
    ("xy", "小于", relation("LESS-THAN")),
    ("yw", "因为", by_name("BECAUSE")),
    ("sy", "所以", by_name("THEREFORE")),
    ("sb", "上标", scripted("super")),
    ("xb", "下标", scripted("sub")),
    # 序号
    ("szq", "带圈数字", by_name(r"(NEGATIVE |DOUBLE |DINGBAT (NEGATIVE )?)?CIRCLED (SANS-SERIF )?(DIGIT|NUMBER) .*")),
    ("szh", "带括号数字", by_name(r"PARENTHESIZED (DIGIT|NUMBER) .*")),
    ("szd", "带点数字", by_name(r"(DIGIT|NUMBER) \w+ FULL STOP")),
    ("lmd", "罗马数字大写", by_name(r"ROMAN NUMERAL .*", "Number Forms")),
    ("lm", "罗马数字小写", by_name(r"SMALL ROMAN NUMERAL .*", "Number Forms")),
    ("zmq", "带圈字母", by_name(r"CIRCLED LATIN (CAPITAL|SMALL) LETTER [A-Z]")),
    ("zmh", "带括号字母", by_name(r"PARENTHESIZED LATIN (CAPITAL|SMALL) LETTER [A-Z]")),
    ("hzq", "带圈汉字", by_name(r"CIRCLED IDEOGRAPH .*")),
    ("hzh", "带括号汉字", by_name(r"PARENTHESIZED IDEOGRAPH .*")),
    ("szm", "苏州码", by_name(r"HANGZHOU NUMERAL .*")),
    ("num", "编号", by_name(r"NUMERO SIGN")),
    # 字母
    ("xl", "希腊字母小写", letters(0x03B1, 0x03C9)),
    ("xld", "希腊字母大写", letters(0x0391, 0x03A9)),
    ("ey", "俄文字母小写", letters(0x0430, 0x044F, (0x0451,))),
    ("eyd", "俄文字母大写", letters(0x0410, 0x042F, (0x0401,))),
    ("py", "拼音字母", pinyin(upper=False)),
    ("pyd", "拼音字母大写", pinyin(upper=True)),
    ("sd", "声调", tone_marks),
    ("zy", "注音符号", whole_blocks("Bopomofo", "Bopomofo Extended", categories="Lo")),
    ("yb", "英文音标", phonetic),
    # 数学字母
    ("sxc", "数学粗体", math_style("BOLD", greek=False)),
    ("sxx", "数学斜体", math_style("ITALIC", greek=False)),
    ("sxz", "数学粗斜体", math_style("BOLD ITALIC", greek=False)),
    ("sxh", "数学花体", math_style("SCRIPT", greek=False)),
    ("sxhc", "数学花体粗体", math_style("BOLD SCRIPT", greek=False)),
    ("sxg", "数学哥特体", math_style("FRAKTUR", greek=False)),
    ("sxgc", "数学哥特粗体", math_style("BOLD FRAKTUR", greek=False)),
    ("sxs", "数学双线体", math_style("DOUBLE-STRUCK", greek=False)),
    ("sxw", "数学无衬线体", math_style("SANS-SERIF", greek=False)),
    ("sxwc", "数学无衬线粗体", math_style("SANS-SERIF BOLD", greek=False)),
    ("sxwx", "数学无衬线斜体", math_style("SANS-SERIF ITALIC", greek=False)),
    ("sxwz", "数学无衬线粗斜体", math_style("SANS-SERIF BOLD ITALIC", greek=False)),
    ("sxd", "数学等宽体", math_style("MONOSPACE", greek=False)),
    ("xlc", "希腊字母粗体", math_style("BOLD", greek=True)),
    ("xlx", "希腊字母斜体", math_style("ITALIC", greek=True)),
    ("xlz", "希腊字母粗斜体", math_style("BOLD ITALIC", greek=True)),
    ("xlwc", "希腊字母无衬线粗体", math_style("SANS-SERIF BOLD", greek=True)),
    ("xlwz", "希腊字母无衬线粗斜体", math_style("SANS-SERIF BOLD ITALIC", greek=True)),
    # 单位
    ("dw", "单位", units),
    ("hb", "货币", currency),
    ("yf", "月份", by_name(r"IDEOGRAPHIC TELEGRAPH SYMBOL FOR (JANUARY|FEBRUARY|MARCH|APRIL|MAY|JUNE|JULY|AUGUST|SEPTEMBER|OCTOBER|NOVEMBER|DECEMBER)")),
    ("rf", "日期", by_name(r"IDEOGRAPHIC TELEGRAPH SYMBOL FOR DAY .*")),
    ("sj", "时间", by_name(r"IDEOGRAPHIC TELEGRAPH SYMBOL FOR HOUR .*")),
    # 箭头与图形
    ("jt", "箭头", union(whole_blocks("Arrows"), by_name(r".*\bARROW(HEAD)?\b.*", "Dingbats"))),
    ("jh", "几何图形", whole_blocks("Geometric Shapes")),
    ("fk", "方块", whole_blocks("Block Elements")),
    ("wjx", "五角星", by_name(r".*PENTAGRAM|(BLACK|WHITE)( SMALL| MEDIUM)? STAR")),
    ("xh", "星号", union(by_name(r".*\bASTERIS[KM]\b.*", *PUNCTUATION_BLOCKS), by_name(r".*\b(ASTERISK|STAR|FLORETTE|SPARKLE|SNOWFLAKE)\b.*", "Dingbats"))),
    ("yd", "圆点", by_name(r".*MIDDLE DOT|(WHITE |TRIANGULAR |HYPHEN )?BULLET|BULLET OPERATOR|DOT OPERATOR", *PUNCTUATION_BLOCKS, "Mathematical Operators", "Geometric Shapes", "Katakana")),
    ("ih", "叉号", by_name(r".*\b(MULTIPLICATION X|CROSS MARK|BALLOT X|BALLOT BOX WITH X)\b.*")),
    # 标点
    ("dd", "点号", by_name(r".*(FULL STOP|COMMA|COLON|SEMICOLON|EXCLAMATION MARK|QUESTION MARK)", "CJK Symbols and Punctuation", "Halfwidth and Fullwidth Forms")),
    ("fj", "非夹注符号", separators),
    ("jz", "夹注符号", whole_blocks("Latin-1 Supplement", "General Punctuation", "CJK Symbols and Punctuation", "Halfwidth and Fullwidth Forms", categories="Ps Pe Pi Pf")),
    ("hj", "行间符号", by_name(r".*(LOW LINE|OVERLINE|SESAME DOT)", *PUNCTUATION_BLOCKS)),
    # 常用标点：拉丁-1、CJK 符号和标点、半角与全角形式三个区里的标点（¡ ¿ § ¶、、 。 【 】……），加上 CJK 符号和标点区里不算标点的字：修饰字母和其他字母（々 〆 〱 〻 〼）、其他符号（〄 〒 〓 〠 〾）和中文日期里的 〇（IDEOGRAPHIC NUMBER ZERO）；同区的苏州码归在苏州码一组，〪–〭 归在声调一组。
    ("bd", "常用标点", union(whole_blocks("Latin-1 Supplement", "CJK Symbols and Punctuation", "Halfwidth and Fullwidth Forms", categories="P"), whole_blocks("CJK Symbols and Punctuation", categories="Lm Lo So"), by_name("IDEOGRAPHIC NUMBER ZERO"))),
    ("bdz", "竖排标点", by_name(r"PRESENTATION FORM FOR VERTICAL .*")),
    ("bdzy", "中英标点", whole_blocks("General Punctuation", categories="P S")),
    # 杂项
    ("fh", "常用符号", ungrouped_misc_symbols),
    ("dn", "电脑按键", keyboard),
    ("xq", "象棋", union(by_name(r"(WHITE|BLACK) CHESS .*", "Miscellaneous Symbols"), by_name(r"XIANGQI .*"))),
    ("mj", "麻将", whole_blocks("Mahjong Tiles")),
    ("sz", "骰子", by_name(r"DIE FACE-\d")),
    ("pk", "扑克", by_name(r".* SUIT", "Miscellaneous Symbols")),
    ("yy", "音乐", union(by_name(r".*\b(NOTE|NOTES|MUSIC)\b.*", "Miscellaneous Symbols"), by_name(r"MUSICAL SYMBOL ([GFC] CLEF|(WHOLE|HALF|QUARTER|EIGHTH|SIXTEENTH|THIRTY-SECOND|SIXTY-FOURTH|ONE HUNDRED TWENTY-EIGHTH) (NOTE|REST)|MULTI REST)", "Musical Symbols"))),
    ("lx", "性别", by_name(r".*\b(MALE|FEMALE|NEUTER|MARRIAGE|DIVORCE|PARTNERSHIP)\b.*", "Miscellaneous Symbols")),
    ("tt", "天体", by_name(r"(BLACK |WHITE )?SUN( WITH RAYS)?|COMET|(FIRST|LAST) QUARTER MOON|MERCURY|EARTH|JUPITER|SATURN|URANUS|NEPTUNE|PLUTO|CERES|PALLAS|JUNO|VESTA|CHIRON|ASTRONOMICAL SYMBOL FOR URANUS", "Miscellaneous Symbols")),
    ("xz", "星座", by_name(r"ARIES|TAURUS|GEMINI|CANCER|LEO|VIRGO|LIBRA|SCORPIUS|SAGITTARIUS|CAPRICORN|AQUARIUS|PISCES|OPHIUCHUS")),
    ("lssg", "六十四卦", whole_blocks("Yijing Hexagram Symbols")),
    ("txj", "太玄经", union(by_name(r"(MONOGRAM|DIGRAM) FOR .*", "Miscellaneous Symbols"), whole_blocks("Tai Xuan Jing Symbols"))),
    # 汉字部件
    ("jg", "汉字结构", by_name(r"IDEOGRAPHIC DESCRIPTION CHARACTER .*")),
    ("pp", "偏旁", radicals_as_ideographs("CJK Radicals Supplement")),
    ("kx", "康熙部首", radicals_as_ideographs("Kangxi Radicals")),
    ("bh", "笔画", by_name(r"CJK STROKE .*")),
    # 假名与韩文
    ("jm", "平假名", whole_blocks("Hiragana", categories="Lo Lm")),
    ("pjm", "片假名", whole_blocks("Katakana", "Katakana Phonetic Extensions", categories="Lo Lm")),
    ("jmq", "带圈假名", by_name(r"CIRCLED KATAKANA .*")),
    ("jmbj", "半角假名", by_name(r"HALFWIDTH KATAKANA.*")),
    ("hw", "韩文字母", lambda ucd: chars(point for point in ucd.assigned("Hangul Compatibility Jamo") if ucd.name[point] != "HANGUL FILLER")),
    ("hwq", "带圈韩文", by_name(r"CIRCLED (HANGUL|KOREAN CHARACTER) .*|KOREAN STANDARD SYMBOL")),
    ("hwh", "带括号韩文", by_name(r"PARENTHESIZED (HANGUL|KOREAN CHARACTER) .*")),
]


def rows(ucd: Ucd) -> list[tuple[str, str, list[str]]]:
    result = []
    for code, name, rule in GROUPS:
        symbols = rule(ucd)
        if not symbols:
            raise ValueError(f"group {code} is empty under the pinned UCD")
        if len(set(symbols)) != len(symbols):
            raise ValueError(f"group {code} lists a symbol twice")
        if len(symbols) > SYMBOL_MODE_LIMIT and code not in OVERSIZED:
            raise ValueError(f"group {code} has {len(symbols)} symbols, more than the {SYMBOL_MODE_LIMIT} the symbol mode shows")
        for symbol in symbols:
            if any(character.isspace() or ucd.category_of(ord(character))[0] in "CZ" or 0xFE00 <= ord(character) <= 0xFE0F for character in symbol):
                raise ValueError(f"group {code} has {symbol!r}, which contains whitespace, a control, format or unassigned code point, or a variation selector")
        result.append((code, name, symbols))
    return result


def render(lock: dict, table: list) -> str:
    # 头注释只写格式、来源和写法规则；组名与码位由下面的行自己说明。
    head = [
        "# 符号模式（Shift+S）的分组码：编码、中文组名、符号。",
        f"# 组员按 Unicode 字符数据库 {lock['unicode_version']} 的区块、类别、字符名和分解映射挑选（Unicode License V3，固定版本见 resources/symbol-groups.lock.json），取字规则和组名写在生成器里。",
        "# 格式：编码<TAB>中文组名<TAB>符号[<空格>符号...]；符号之间用一个空格分隔，所以符号本身不含空格。",
        "# 由 scripts/build_symbol_groups.py 生成，不要手工编辑；写法规则由 symbol_groups.rs 的单元测试检查。",
        "",
    ]
    return "\n".join(head) + "".join(f"{code}\t{name}\t{' '.join(symbols)}\n" for code, name, symbols in table)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    artifacts = {artifact["name"]: artifact for artifact in lock["artifacts"]}
    if arguments.check and not all((arguments.cache / name).is_file() for name in artifacts):
        if not os.environ.get("MSIME_FETCH"):
            print("skip: the pinned Unicode Character Database files are not cached (set MSIME_FETCH=1 to download)")
            return 0
    try:
        paths = {name: fetch(artifact, arguments.cache) for name, artifact in artifacts.items()}
    except (OSError, RuntimeError) as error:
        # 契约检查不联网时下载失败只算跳过；设了 MSIME_FETCH=1（CI 的逐字节核对）就必须拿到输入，与拆字表的生成器相同。
        if arguments.check and os.environ.get("MSIME_FETCH") != "1":
            print(f"skip: could not fetch the pinned Unicode Character Database files: {error}")
            return 0
        raise
    ucd = Ucd(*(paths[name].read_text(encoding="utf-8") for name in ("UnicodeData.txt", "Blocks.txt", "EquivalentUnifiedIdeograph.txt")))
    table = rows(ucd)
    rendered = render(lock, table)
    if arguments.check:
        current = OUTPUT.read_text(encoding="utf-8")
        if current != rendered:
            print(f"FAIL: {OUTPUT.relative_to(ROOT)} differs from the pinned generator output; run scripts/build_symbol_groups.py")
            return 1
        sources = ", ".join(f"{name} {digest(path)[:12]}" for name, path in paths.items())
        print(f"symbol groups: {OUTPUT.relative_to(ROOT)} matches the generator and the pinned UCD ({sources})")
        return 0
    OUTPUT.write_text(rendered, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}: {len(table)} groups, {sum(len(symbols) for _, _, symbols in table)} symbols")
    return 0


if __name__ == "__main__":
    sys.exit(main())
