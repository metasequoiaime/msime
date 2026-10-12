#!/usr/bin/env python3
"""生成两个生僻字插件（rare-chars-last、hide-rare-chars）共用的常用字集。

常用字集 = 《通用规范汉字表》的 8105 字（Unihan 的 kTGH 字段）
         ∪ 这些字在 Unihan 里的 kTraditionalVariant
         ∪ 这些字作为 OpenCC STCharacters.txt 的键时的全部单字值。

并入繁体形是因为注音方案按词库直接输出繁体，粤语方案的词库也以繁体为主；不并入的话，們、這、發、乾 都会被当成生僻字。Unihan 来自 resources/character-glosses.lock.json 锁定的 Unihan-17.0.0.zip（校验 sha256），OpenCC 来自仓库里 vendored 的 crates/client-core/data/opencc/STCharacters.txt。

输出两个文件，都提交进仓库：

- plugins/script/data/common-chars.bin：小端二进制，格式见下面的 encode()，插件用 include_bytes! 编进模块。
- plugins/script/data/common-chars-NOTICE.txt：来源、版本、摘要和条目数，插件包的 NOTICE.txt 按它写。

结果只取决于锁定的 zip 和仓库里的 OpenCC 文件：集合排序后编码，没有时间戳，同样的输入总是逐字节相同的输出。

--check 重新生成后和仓库里的两个文件逐字节比较，不写文件。zip 不在缓存里时 --check 打印 skip 并以 0 退出（契约检查和 CI 不联网），设了 MSIME_FETCH=1 才下载。

usage: gen_common_chars.py [--zip <Unihan-17.0.0.zip>] [--check]
"""
import argparse
import json
import os
import struct
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
sys.dont_write_bytecode = True  # 导入 scripts/ 下的脚本时不留下 __pycache__
from fetch_wordbooks import digest, fetch  # noqa: E402  （按锁下载并校验）

LOCK = ROOT / "resources/character-glosses.lock.json"
OPENCC = ROOT / "crates/client-core/data/opencc/STCharacters.txt"
OPENCC_COMMIT = "26753884f1984add422f3b0249ccee8613deaff6"
CACHE = ROOT / "target/script-plugins/cache"
DATA = ROOT / "plugins/script/data"
BINARY = DATA / "common-chars.bin"
NOTICE = DATA / "common-chars-NOTICE.txt"
GENERATOR = "plugins/script/tools/gen_common_chars.py"

MAGIC = b"MSCC"
VERSION = 1
# 两段用位图表示的区段，闭区间。其余常用字（实测只有辅助平面的汉字）放进升序列表。
URO = (0x4E00, 0x9FFF)
EXT_A = (0x3400, 0x4DBF)
HEADER_BYTES = 16
# 《通用规范汉字表》的字数；Unihan 改了 kTGH 的覆盖面时让生成直接失败，而不是悄悄换掉集合。
TGH_COUNT = 8105


def read_unihan(zip_path: Path) -> tuple[set[int], dict[int, list[int]]]:
    tgh: set[int] = set()
    traditional: dict[int, list[int]] = {}
    with zipfile.ZipFile(zip_path) as archive:
        for member, field in (("Unihan_OtherMappings.txt", "kTGH"), ("Unihan_Variants.txt", "kTraditionalVariant")):
            for line in archive.read(member).decode("utf-8").splitlines():
                if not line or line.startswith("#"):
                    continue
                codepoint, name, value = line.split("\t", 2)
                if name != field:
                    continue
                cp = int(codepoint[2:], 16)
                if field == "kTGH":
                    tgh.add(cp)
                else:
                    traditional[cp] = [int(item[2:], 16) for item in value.split()]
    return tgh, traditional


def read_opencc() -> dict[int, list[int]]:
    table: dict[int, list[int]] = {}
    for line in OPENCC.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        key, values = line.split("\t", 1)
        if len(key) != 1:
            raise SystemExit(f"{OPENCC.name}: 键 {key!r} 不是单字")
        items = values.split()
        if any(len(item) != 1 for item in items):
            raise SystemExit(f"{OPENCC.name}: {key} 的值 {values!r} 含多字")
        table[ord(key)] = [ord(item) for item in items]
    return table


def collect(tgh: set[int], traditional: dict[int, list[int]], opencc: dict[int, list[int]]) -> tuple[set[int], dict[str, int]]:
    from_unihan = {variant for cp in tgh for variant in traditional.get(cp, [])} - tgh
    from_opencc = {value for cp in tgh for value in opencc.get(cp, [])} - tgh
    common = tgh | from_unihan | from_opencc
    stats = {
        "tgh": len(tgh),
        "traditional_unihan": len(from_unihan),
        "traditional_opencc_only": len(from_opencc - from_unihan),
        "total": len(common),
    }
    return common, stats


def bitmap(common: set[int], span: tuple[int, int]) -> bytes:
    start, end = span
    bits = bytearray((end - start + 1 + 7) // 8)
    for cp in common:
        if start <= cp <= end:
            index = cp - start
            bits[index >> 3] |= 1 << (index & 7)
    return bytes(bits)


def encode(common: set[int]) -> bytes:
    """文件格式（全部小端）：

    偏移  长度  内容
    0     4     魔数 "MSCC"
    4     2     版本 1
    6     2     保留，0
    8     4     常用字总数
    12    4     列表条目数 n
    16    2624  U+4E00..U+9FFF 的位图：第 cp-0x4E00 位，字节内低位在前
    2640  824   U+3400..U+4DBF 的位图，同上
    3464  4n    其余常用字的码位，u32 升序
    """
    others = sorted(cp for cp in common if not (URO[0] <= cp <= URO[1] or EXT_A[0] <= cp <= EXT_A[1]))
    header = MAGIC + struct.pack("<HHII", VERSION, 0, len(common), len(others))
    assert len(header) == HEADER_BYTES
    return header + bitmap(common, URO) + bitmap(common, EXT_A) + b"".join(struct.pack("<I", cp) for cp in others)


def notice(lock: dict, stats: dict[str, int], common: set[int]) -> str:
    planes = {
        "基本区（U+4E00..U+9FFF）": sum(URO[0] <= cp <= URO[1] for cp in common),
        "扩展 A（U+3400..U+4DBF）": sum(EXT_A[0] <= cp <= EXT_A[1] for cp in common),
        "其余（辅助平面等）": sum(not (URO[0] <= cp <= URO[1] or EXT_A[0] <= cp <= EXT_A[1]) for cp in common),
    }
    artifact = lock["artifact"]
    lines = [
        "常用字集（common-chars.bin）的来源说明",
        "",
        f"这份常用字集由 {GENERATOR} 从下面两份数据变换生成，只保留「这个字在不在集合里」这一个事实，编码成位图和码位列表。",
        "",
        f"1. Unicode Han Database（Unihan）{lock['unicode_version']}：{artifact['url']}",
        f"   文件 {artifact['name']}，{artifact['size']} 字节，SHA-256 {artifact['sha256']}。",
        "   使用其中 Unihan_OtherMappings.txt 的 kTGH 字段（《通用规范汉字表》2013 年版）和 Unihan_Variants.txt 的 kTraditionalVariant 字段。",
        "   许可证：Unicode License v3（Unicode-3.0），全文见同一个包里的 Unicode-License-V3.txt。",
        "",
        f"2. OpenCC（https://github.com/BYVoid/OpenCC）提交 {OPENCC_COMMIT} 的 data/dictionary/STCharacters.txt。",
        "   使用其中《通用规范汉字表》各字的繁体单字值。",
        "   许可证：Apache License 2.0（Apache-2.0），全文见同一个包里的 OpenCC-LICENSE.txt。本文件即 Apache-2.0 第 4 条要求的修改说明：原始词典已被变换为上述二进制集合。",
        "",
        "集合的组成：",
        f"- 《通用规范汉字表》（kTGH）：{stats['tgh']} 字",
        f"- 这些字的 kTraditionalVariant（不在表内的）：{stats['traditional_unihan']} 字",
        f"- 只由 OpenCC 补充的繁体形：{stats['traditional_opencc_only']} 字",
        f"- 合计：{stats['total']} 字，其中" + "，".join(f"{name}{count} 字" for name, count in planes.items()),
        "",
    ]
    return "\n".join(lines)


def source_zip(arguments, lock: dict) -> Path | None:
    artifact = lock["artifact"]
    if arguments.zip:
        path = Path(arguments.zip)
        if digest(path) != artifact["sha256"]:
            raise SystemExit(f"{path} 和 {LOCK.relative_to(ROOT)} 记录的 sha256 不一致")
        return path
    cached = CACHE / artifact["name"]
    if cached.is_file() and digest(cached) == artifact["sha256"]:
        return cached
    if arguments.check and os.environ.get("MSIME_FETCH") != "1":
        return None
    return fetch(artifact, CACHE)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--zip", help="本地的 Unihan-17.0.0.zip，仍按锁校验 sha256")
    parser.add_argument("--check", action="store_true", help="重新生成并和仓库里的文件逐字节比较，不写文件")
    arguments = parser.parse_args(argv)

    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    license_path = ROOT / lock["license_file"]["path"]
    if digest(license_path) != lock["license_file"]["sha256"]:
        raise SystemExit(f"{license_path} 和锁不一致")
    source = source_zip(arguments, lock)
    if source is None:
        print(f"skip: {CACHE.relative_to(ROOT)}/{lock['artifact']['name']} 不在缓存里（设 MSIME_FETCH=1 下载后再检查）")
        return 0

    tgh, traditional = read_unihan(source)
    if len(tgh) != TGH_COUNT:
        raise SystemExit(f"kTGH 有 {len(tgh)} 条，不是 {TGH_COUNT}；Unihan 的结构变了")
    common, stats = collect(tgh, traditional, read_opencc())
    binary = encode(common)
    text = notice(lock, stats, common)

    if arguments.check:
        stale = [path.relative_to(ROOT) for path, content in ((BINARY, binary), (NOTICE, text.encode("utf-8"))) if not path.is_file() or path.read_bytes() != content]
        if stale:
            print(f"FAIL: {', '.join(map(str, stale))} 和生成结果不一致，运行 python3 {GENERATOR} 重新生成")
            return 1
        print(f"ok: common-chars.bin 与生成结果一致（{stats['total']} 字，{len(binary)} 字节）")
        return 0

    DATA.mkdir(parents=True, exist_ok=True)
    BINARY.write_bytes(binary)
    NOTICE.write_text(text, encoding="utf-8")
    print(f"wrote {BINARY.relative_to(ROOT)}（{len(binary)} 字节）和 {NOTICE.relative_to(ROOT)}")
    print(json.dumps(stats, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
