#!/usr/bin/env python3
"""生成繁体字形注释插件（traditional-glyph）的简繁对照表。

数据只来自仓库里 vendored 的 OpenCC（crates/client-core/data/opencc/，固定在提交 26753884，见 docs/third-party.md），不联网：

- STCharacters.txt：单字的全部繁体值（发 → 發 髮）。
- STPhrases.txt 和 STPhrases_GeneratedFromRegionalPhrases.txt：词组，两者合并，前者优先，取每条的第一个值；和 crates/client-core/src/chinese_conversion.rs 的读法相同。

插件对多字候选的转换是「词组表上的正向最长匹配（2 到 4 字），匹配不到的字取单字表的第一个值」。完整的 OpenCC 词组表有 1 MB，放不进 256 KiB 的模块，所以这里只保留会让结果和「逐字取第一个值」不同的 2 到 4 字词组：先收所有转换结果和逐字结果不同的词组，再反复补上「在已有条目下会被转错」的词组（比如短词组错误地套进了长词组），直到每一条 2 到 4 字的词组在插件的算法下都和 OpenCC 一致。每条词组只存每个字在单字表里取第几个值（2 位一字，也可以表示保留原字），所以单字表给不出的结果和含辅助平面字的词组不收，数量会打印出来。插件只在附近有「在词组里会变的字」（IN_PHRASE）的位置找词组，查表前先看过滤位图。

和 OpenCC 的完整算法相比，差别只来自没收的 5 字以上的词组，以及长词组把后面的短词组挡住的情形；--evaluate 打印这两类的统计。

还附带两个每字一位的标记，插件用它判断窗口是不是本来就是繁体（注音、粤语方案）：

- 繁体专用字：在单字表的值里、本身不是键、也不在 GB2312 里（們、這、發）。
- 简体专用字：是单字表的键、它的值里不含它自己、也不是任何键的值（发、这、们）。

输出（都提交进仓库）：

- plugins/script/traditional-glyph/data/s2t.bin：小端二进制，格式见 encode()，插件用 include_bytes! 编进模块。
- plugins/script/traditional-glyph/pack/NOTICE.txt：数据来源和 Apache-2.0 第 4 条要求的修改说明，随插件包分发。

同样的输入总是逐字节相同的输出：所有集合排序后编码，没有时间戳。GB2312 由 Python 自带的 gb2312 编解码器给出。

--check 重新生成后和仓库里的两个文件逐字节比较，不写文件。--evaluate 另外用和插件相同的算法读生成的二进制，对 OpenCC 的结果做一致性统计（长词组和随机拼接的词组），只打印，不影响输出。

usage: gen_s2t_table.py [--check] [--evaluate]
"""
import argparse
import random
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OPENCC = ROOT / "crates/client-core/data/opencc"
OPENCC_COMMIT = "26753884f1984add422f3b0249ccee8613deaff6"
CHARACTERS = "STCharacters.txt"
PHRASES = ("STPhrases.txt", "STPhrases_GeneratedFromRegionalPhrases.txt")
PLUGIN = ROOT / "plugins/script/traditional-glyph"
BINARY = PLUGIN / "data/s2t.bin"
NOTICE = PLUGIN / "pack/NOTICE.txt"
GENERATOR = "plugins/script/tools/gen_s2t_table.py"

MAGIC = b"MSTG"
# 码位块的目录：基本平面 1024 块，第二、第三平面 2048 块，每块 64 个码位。
DIRECTORY_SLOTS = 1024 + 2048
VERSION = 1
HEADER_BYTES = 40
# 词组表收录的长度。插件的正向最长匹配从 MAX_PHRASE 字试到 2 字。
MIN_PHRASE = 2
MAX_PHRASE = 4
# 词组的过滤位图：每个长度一张，位 h(key) 置 1。插件查表前先看这一位，绝大多数不在表里的窗口不用二分查找。h 是乘法散列取高位，和插件的 table::maybe 一致。
BLOOM_LOG2 = {2: 15, 3: 14, 4: 14}
BLOOM_MULTIPLIER = 0x9E3779B97F4A7C15

# 条目（u32）的位布局，和插件 src/table.rs 的常量一一对应。
POOL_BITS = 14  # 0..13：值池里的字节偏移
JOINED_SHIFT = 14  # 14..18：全部值用「/」连起来的字节数
JOINED_BITS = 5
COUNT_SHIFT = 19  # 19..21：值的个数，0 表示不是单字表的键
COUNT_BITS = 3
TRAD = 1 << 22
SIMP = 1 << 23
START_SHIFT = 24  # 24、25、26：是 2、3、4 字词组的首字
SELF_SHIFT = 27  # 27、28：这个字自己是第几个值（有 HAS_SELF 时才有意义）
HAS_SELF = 1 << 29  # 这个字自己也在它的值里（台 → 臺 檯 颱 台）
IN_PHRASE = 1 << 30  # 在某条词组里不取第一个值（取别的值，或者保留原字）


def read_table(names) -> dict[str, list[str]]:
    table: dict[str, list[str]] = {}
    for name in names:
        for line in (OPENCC / name).read_text(encoding="utf-8").splitlines():
            if not line or line.startswith("#"):
                continue
            key, values = line.split("\t", 1)
            items = values.split()
            if key and items:
                table.setdefault(key, items)
    return table


def gb2312() -> set[int]:
    chars: set[int] = set()
    for high in range(0xA1, 0xF8):
        for low in range(0xA1, 0xFF):
            try:
                text = bytes([high, low]).decode("gb2312")
            except UnicodeDecodeError:
                continue
            chars.add(ord(text))
    return chars


def first_value(characters: dict[str, list[str]], char: str) -> str:
    values = characters.get(char)
    return values[0] if values else char


def convert(text: str, characters: dict[str, list[str]], phrases: dict[str, str]) -> str:
    """插件的算法：2 到 4 字词组的正向最长匹配，其余逐字取第一个值。"""
    out: list[str] = []
    i = 0
    while i < len(text):
        for length in range(MAX_PHRASE, MIN_PHRASE - 1, -1):
            match = phrases.get(text[i : i + length]) if i + length <= len(text) else None
            if match is not None:
                out.append(match)
                i += length
                break
        else:
            out.append(first_value(characters, text[i]))
            i += 1
    return "".join(out)


def opencc(text: str, characters: dict[str, list[str]], phrases: dict[str, str], longest: int) -> str:
    """OpenCC s2t 的转换段（不含兼容汉字的规范化）：全部词组上的正向最长匹配。"""
    out: list[str] = []
    i = 0
    while i < len(text):
        for length in range(min(longest, len(text) - i), 1, -1):
            match = phrases.get(text[i : i + length])
            if match is not None:
                out.append(match)
                i += length
                break
        else:
            out.append(first_value(characters, text[i]))
            i += 1
    return "".join(out)


def selection(key: str, value: str, characters: dict[str, list[str]]) -> int | None:
    """每个字取单字表第几个值，2 位一字；表里给不出时是 None。

    词组保留原字、而原字不在它自己的值里时（倒霉、恒生：霉 → 黴，恒 → 恆，词组里不转），选择写成值的个数，表示「保留原字」。值有 4 个的字没有这个余地，这样的词组不收。
    """
    if len(key) != len(value):
        return None
    bits = 0
    for position, (source, target) in enumerate(zip(key, value)):
        values = characters.get(source, [source])
        if target in values:
            index = values.index(target)
        elif target == source and len(values) < 4:
            index = len(values)
        else:
            return None
        bits |= index << (2 * position)
    return bits


def build_phrases(characters, phrases) -> tuple[dict[str, str], dict[str, int]]:
    candidates = sorted(key for key in phrases if MIN_PHRASE <= len(key) <= MAX_PHRASE)
    dropped = {"sip": 0, "inexpressible": 0}
    usable: list[str] = []
    for key in candidates:
        if any(ord(char) > 0xFFFF for char in key):
            if convert(key, characters, {}) != phrases[key]:
                dropped["sip"] += 1
            continue
        if selection(key, phrases[key], characters) is None:
            dropped["inexpressible"] += 1
            continue
        usable.append(key)
    table: dict[str, str] = {}
    while True:
        added = 0
        for key in usable:
            if key not in table and convert(key, characters, table) != phrases[key]:
                table[key] = phrases[key]
                added += 1
        if not added:
            break
    return table, dropped


def entries(characters, table) -> tuple[dict[int, int], bytes, dict[str, int]]:
    """每个要查的字一个 u32 条目；值池是全部值用「/」连起来的 UTF-8。"""
    gb = gb2312()
    values_of_any = {value for values in characters.values() for value in values}
    trad = {ord(value) for value in values_of_any if value not in characters and ord(value) not in gb}
    simp = {ord(key) for key, values in characters.items() if key not in values and key not in values_of_any}
    starts: dict[int, int] = {}
    for key in table:
        starts[ord(key[0])] = starts.get(ord(key[0]), 0) | 1 << (START_SHIFT + len(key) - MIN_PHRASE)
    # 插件只在附近有 IN_PHRASE 字的位置找词组，所以每一条词组都必须含有这样的字。逐字结果就不对的词组自然含有；为了挡住更短的词组而收的那些，含有被挡住的那条，也就含有。这里核对一遍。
    in_phrase: set[int] = set()
    for key in table:
        bits = selection(key, table[key], characters)
        in_phrase |= {ord(char) for position, char in enumerate(key) if bits >> (2 * position) & 3}
    for key in table:
        if not any(ord(char) in in_phrase for char in key):
            raise SystemExit(f"词组 {key} 里没有 IN_PHRASE 的字，插件会跳过它")
        for position, char in enumerate(key):
            values = characters.get(char, [char])
            if len(values[0].encode("utf-8")) != len(char.encode("utf-8")) and table[key][position] == char:
                raise SystemExit(f"词组 {key} 保留 {char}，但它和它的值不一样长，插件没法就地替换")

    pool = bytearray()
    result: dict[int, int] = {}
    for char in sorted(set(map(ord, characters)) | trad | set(starts)):
        entry = starts.get(char, 0)
        if char in trad:
            entry |= TRAD
        if char in simp:
            entry |= SIMP
        if char in in_phrase:
            entry |= IN_PHRASE
        values = characters.get(chr(char))
        if values:
            if len(values) >= 1 << COUNT_BITS or any(len(value) != 1 or ord(value) < 0x800 for value in values):
                raise SystemExit(f"{chr(char)} 的值 {values} 超出编码范围")
            # 插件先逐字写出第一个值，命中词组时再就地换成别的值，所以同一个字的各个值必须一样长。
            if len({len(value.encode("utf-8")) for value in values}) != 1:
                raise SystemExit(f"{chr(char)} 的值 {values} 的 UTF-8 长度不一，插件没法就地替换")
            joined = "/".join(values).encode("utf-8")
            if len(joined) >= 1 << JOINED_BITS:
                raise SystemExit(f"{chr(char)} 的值连起来 {len(joined)} 字节，超出编码范围")
            entry |= len(pool) | len(joined) << JOINED_SHIFT | len(values) << COUNT_SHIFT
            if chr(char) in values:
                entry |= HAS_SELF | values.index(chr(char)) << SELF_SHIFT
            pool += joined
        result[char] = entry
    # 插件只对 U+3400..U+FDFF 和第二、第三平面的字查表（更小的码位里是假名和要另外判断的字符），条目必须都落在这里。
    outside = [f"U+{char:04X}" for char in result if not (0x3400 <= char < 0xFE00 or 0x20000 <= char < 0x40000)]
    if outside:
        raise SystemExit(f"这些字在插件查表的范围之外：{', '.join(outside)}")
    if len(pool) >= 1 << POOL_BITS:
        raise SystemExit(f"值池 {len(pool)} 字节，超出 {POOL_BITS} 位偏移")
    stats = {"trad": len(trad), "simp": len(simp), "starts": len(starts), "in_phrase": len(in_phrase)}
    return result, bytes(pool), stats


def block_of(char: int) -> int:
    if char <= 0xFFFF:
        return char >> 6
    assert 0x20000 <= char < 0x40000
    return 1024 + ((char - 0x20000) >> 6)


def encode(characters, table) -> tuple[bytes, dict[str, int]]:
    """文件格式（全部小端）：

    头（40 字节）：魔数 "MSTG"；u16 版本 1；u16 保留 0；然后 u32 依次是 非空块数 B、条目总数 E、目录项数 D（DIRECTORY_SLOTS）、值池字节数 P、2/3/4 字词组条数 N2/N3/N4，最后 4 字节保留。

    码位每 64 个一块。块的编号：基本平面是 cp >> 6（0..1023），第二、第三平面是 1024 + ((cp - 0x20000) >> 6)（1024..3071）。其他平面没有条目。

    之后各段紧接着排列：
    - 目录：D 个 u16，按块的编号，值是非空块的序号加 1，0 表示这一块没有条目。
    - 块的位图：B 个 u64，第 k 位表示块里第 k 个码位有条目。
    - 块的起点：B 个 u16，这一块第一个条目在条目表里的下标；块内第 k 位的条目下标是起点加上低于 k 的置位数。
    - 条目：E 个 u32，按块的编号、块内按码位排列，位布局见本文件开头的常量。
    - 值池：P 字节 UTF-8。
    - 词组的键：N2 个 u32（两个 UTF-16 码元，首字在高位），N3、N4 个 u64（同样首字在高位），各自升序。
    - 词组的选择：N2、N3、N4 个 u8，第 j 字取单字表第 (sel >> 2j) & 3 个值；不是键的字是 0，取它自己；等于值的个数时也取它自己（见 selection()）。
    - 词组的过滤位图：2、3、4 字各 2^BLOOM_LOG2[L] 位，字节内低位在前。键 k 对应第 bloom_bit(k, L) 位，表里每个键的那一位都是 1。
    """
    char_entries, pool, stats = entries(characters, table)
    ordered = sorted(char_entries)
    blocks: list[int] = sorted({block_of(char) for char in ordered})
    directory = [0] * DIRECTORY_SLOTS
    masks: list[int] = []
    bases: list[int] = []
    index = 0
    for number, block in enumerate(blocks):
        directory[block] = number + 1
        members = [char for char in ordered if block_of(char) == block]
        masks.append(sum(1 << (char & 63) for char in members))
        bases.append(index)
        index += len(members)
    sip = [char for char in ordered if char > 0xFFFF]

    by_length = {length: sorted(key for key in table if len(key) == length) for length in range(MIN_PHRASE, MAX_PHRASE + 1)}

    def packed(key: str) -> int:
        value = 0
        for char in key:
            value = value << 16 | ord(char)
        return value

    sections = [
        struct.pack(f"<{DIRECTORY_SLOTS}H", *directory),
        b"".join(struct.pack("<Q", mask) for mask in masks),
        b"".join(struct.pack("<H", base) for base in bases),
        b"".join(struct.pack("<I", char_entries[char]) for char in ordered),
        pool,
        b"".join(struct.pack("<I", packed(key)) for key in by_length[2]),
        b"".join(struct.pack("<Q", packed(key)) for key in by_length[3]),
        b"".join(struct.pack("<Q", packed(key)) for key in by_length[4]),
    ]
    for length in range(MIN_PHRASE, MAX_PHRASE + 1):
        sections.append(bytes(selection(key, table[key], characters) for key in by_length[length]))
    for length in range(MIN_PHRASE, MAX_PHRASE + 1):
        sections.append(bloom([packed(key) for key in by_length[length]], length))
    header = MAGIC + struct.pack(
        "<HH8I",
        VERSION,
        0,
        len(blocks),
        len(ordered),
        DIRECTORY_SLOTS,
        len(pool),
        len(by_length[2]),
        len(by_length[3]),
        len(by_length[4]),
        0,
    )
    assert len(header) == HEADER_BYTES
    stats.update(
        {
            "blocks": len(blocks),
            "entries": len(ordered),
            "sip_entries": len(sip),
            "pool": len(pool),
            "phrases2": len(by_length[2]),
            "phrases3": len(by_length[3]),
            "phrases4": len(by_length[4]),
        }
    )
    return header + b"".join(sections), stats


def bloom_bit(key: int, length: int) -> int:
    """64 位乘法散列取高 BLOOM_LOG2[length] 位。"""
    return (key * BLOOM_MULTIPLIER & (1 << 64) - 1) >> (64 - BLOOM_LOG2[length])


def bloom(keys: list[int], length: int) -> bytes:
    bits = bytearray(1 << BLOOM_LOG2[length] >> 3)
    for key in keys:
        bit = bloom_bit(key, length)
        bits[bit >> 3] |= 1 << (bit & 7)
    return bytes(bits)


class Reader:
    """按插件的读法读 s2t.bin，供 --evaluate 核对编码。"""

    def __init__(self, blob: bytes):
        (self.blocks, self.entries, slots, self.pool_len, n2, n3, n4, _) = struct.unpack_from("<8I", blob, 8)
        assert slots == DIRECTORY_SLOTS
        offset = HEADER_BYTES
        self.directory = struct.unpack_from(f"<{slots}H", blob, offset)
        offset += 2 * slots
        self.masks = struct.unpack_from(f"<{self.blocks}Q", blob, offset)
        offset += 8 * self.blocks
        self.bases = struct.unpack_from(f"<{self.blocks}H", blob, offset)
        offset += 2 * self.blocks
        self.table = struct.unpack_from(f"<{self.entries}I", blob, offset)
        offset += 4 * self.entries
        self.pool = blob[offset : offset + self.pool_len]
        offset += self.pool_len
        keys = {}
        for length, count, width, code in ((2, n2, 4, "I"), (3, n3, 8, "Q"), (4, n4, 8, "Q")):
            keys[length] = struct.unpack_from(f"<{count}{code}", blob, offset)
            offset += width * count
        self.phrases = {}
        for length, count in ((2, n2), (3, n3), (4, n4)):
            self.phrases[length] = dict(zip(keys[length], blob[offset : offset + count]))
            offset += count
        self.blooms = {}
        for length in (2, 3, 4):
            size = 1 << BLOOM_LOG2[length] >> 3
            self.blooms[length] = blob[offset : offset + size]
            offset += size
        assert offset == len(blob)

    def entry(self, char: int) -> int:
        if not (char <= 0xFFFF or 0x20000 <= char < 0x40000):
            return 0
        block = self.directory[block_of(char)]
        if not block:
            return 0
        mask = self.masks[block - 1]
        bit = char & 63
        if not mask >> bit & 1:
            return 0
        return self.table[self.bases[block - 1] + bin(mask & ((1 << bit) - 1)).count("1")]

    def values(self, char: int) -> list[str]:
        entry = self.entry(char)
        if not entry >> COUNT_SHIFT & ((1 << COUNT_BITS) - 1):
            return [chr(char)]
        start = entry & ((1 << POOL_BITS) - 1)
        joined = self.pool[start : start + (entry >> JOINED_SHIFT & ((1 << JOINED_BITS) - 1))]
        return joined.decode("utf-8").split("/")

    def convert(self, text: str) -> str:
        out: list[str] = []
        i = 0
        while i < len(text):
            for length in range(MAX_PHRASE, MIN_PHRASE - 1, -1):
                window = text[i : i + length]
                if len(window) < length or any(ord(char) > 0xFFFF for char in window):
                    continue
                if not self.entry(ord(window[0])) >> (START_SHIFT + length - MIN_PHRASE) & 1:
                    continue
                key = 0
                for char in window:
                    key = key << 16 | ord(char)
                bit = bloom_bit(key, length)
                if not self.blooms[length][bit >> 3] >> (bit & 7) & 1:
                    assert key not in self.phrases[length]
                    continue
                sel = self.phrases[length].get(key)
                if sel is not None:
                    for j, char in enumerate(window):
                        values = self.values(ord(char))
                        index = sel >> (2 * j) & 3
                        out.append(values[index] if index < len(values) else char)
                    i += length
                    break
            else:
                out.append(self.values(ord(text[i]))[0])
                i += 1
        return "".join(out)


def evaluate(blob: bytes, characters, phrases, table) -> None:
    reader = Reader(blob)
    longest = max(map(len, phrases))
    short = [key for key in phrases if MIN_PHRASE <= len(key) <= MAX_PHRASE]
    encoded_wrong = sum(reader.convert(key) != convert(key, characters, table) for key in short)
    short_wrong = sum(reader.convert(key) != phrases[key] for key in short)
    long_keys = [key for key in phrases if len(key) > MAX_PHRASE]
    long_wrong = sum(reader.convert(key) != phrases[key] for key in long_keys)
    single_wrong = sum(reader.values(ord(key)) != values for key, values in characters.items())
    generator = random.Random(20261012)
    keys = sorted(phrases)
    trials = 20000
    pair_wrong = 0
    for _ in range(trials):
        text = generator.choice(keys) + generator.choice(keys)
        if reader.convert(text) != opencc(text, characters, phrases, longest):
            pair_wrong += 1
    print(f"单字表读回不一致：{single_wrong}")
    print(f"二进制和 Python 算法不一致：{encoded_wrong}")
    print(f"2 到 4 字词组和 OpenCC 不一致：{short_wrong}/{len(short)}")
    print(f"5 字以上词组和 OpenCC 不一致：{long_wrong}/{len(long_keys)}")
    print(f"两个词组随机拼接和 OpenCC 不一致：{pair_wrong}/{trials}")


def notice(stats: dict[str, int], dropped: dict[str, int]) -> str:
    left_out = "".join(
        text for count, text in ((dropped["sip"], f"含辅助平面字的 {dropped['sip']} 条没有收录，"), (dropped["inexpressible"], f"单字表表达不了的 {dropped['inexpressible']} 条没有收录，")) if count
    )
    lines = [
        "traditional-glyph 的第三方数据说明",
        "",
        f"插件模块里的简繁对照表（源码树 plugins/script/traditional-glyph/data/s2t.bin）由 {GENERATOR} 从 OpenCC 的词典变换生成。",
        "",
        f"来源：OpenCC（Open Chinese Convert，https://github.com/BYVoid/OpenCC）提交 {OPENCC_COMMIT}，data/dictionary/ 下的 STCharacters.txt、STPhrases.txt 和 STPhrases_GeneratedFromRegionalPhrases.txt。",
        "许可证：Apache License 2.0（Apache-2.0），全文见同一个包里的 OpenCC-LICENSE.txt。",
        "",
        "修改说明（Apache-2.0 第 4 条第 2 款）：原始词典已被变换，没有原样包含。",
        "- STCharacters.txt：全部单字条目，按码位重新编码为二进制条目和值池。",
        f"- STPhrases.txt 与 STPhrases_GeneratedFromRegionalPhrases.txt：只保留 2 到 4 字、且逐字转换会出错的词组 {stats['phrases2'] + stats['phrases3'] + stats['phrases4']} 条（2 字 {stats['phrases2']}、3 字 {stats['phrases3']}、4 字 {stats['phrases4']}），每条只存每个字取单字表的第几个值（或者保留原字）；{left_out}5 字以上的词组没有收录。",
        f"- 另由单字表派生两个标记：繁体专用字 {stats['trad']} 个（单字表的值里、本身不是键、不在 GB2312 里的字，GB2312 取自 Python 的 gb2312 编解码器），简体专用字 {stats['simp']} 个。",
        "",
        "插件代码本身以 MIT 许可证发布，见 LICENSE.txt。",
        "",
    ]
    return "\n".join(lines)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="重新生成并和仓库里的文件逐字节比较，不写文件")
    parser.add_argument("--evaluate", action="store_true", help="另外统计和 OpenCC 完整算法的一致程度")
    arguments = parser.parse_args(argv)

    characters = read_table([CHARACTERS])
    phrases = {key: values[0] for key, values in read_table(PHRASES).items()}
    for key in characters:
        if len(key) != 1:
            raise SystemExit(f"{CHARACTERS}: 键 {key!r} 不是单字")
    table, dropped = build_phrases(characters, phrases)
    binary, stats = encode(characters, table)
    text = notice(stats, dropped)

    if arguments.evaluate:
        evaluate(binary, characters, phrases, table)

    if arguments.check:
        stale = [path.relative_to(ROOT) for path, content in ((BINARY, binary), (NOTICE, text.encode("utf-8"))) if not path.is_file() or path.read_bytes() != content]
        if stale:
            print(f"FAIL: {', '.join(map(str, stale))} 和生成结果不一致，运行 python3 {GENERATOR} 重新生成")
            return 1
        print(f"ok: s2t.bin 与生成结果一致（{stats['entries']} 个字，{stats['phrases2'] + stats['phrases3'] + stats['phrases4']} 条词组，{len(binary)} 字节）")
        return 0

    BINARY.parent.mkdir(parents=True, exist_ok=True)
    NOTICE.parent.mkdir(parents=True, exist_ok=True)
    BINARY.write_bytes(binary)
    NOTICE.write_text(text, encoding="utf-8")
    print(f"wrote {BINARY.relative_to(ROOT)}（{len(binary)} 字节）和 {NOTICE.relative_to(ROOT)}")
    print(f"stats: {stats}, dropped: {dropped}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
