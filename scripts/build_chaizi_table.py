#!/usr/bin/env python3
"""生成拆字反查（C 模式）编进 msime-engine 的两张表。

拆法来自 kfcd/chaizi（《漢語拆字字典》，© 2015 開放詞典，CC BY 3.0）固定提交的简繁两表，部件和字的读音来自 resources/character-glosses.lock.json 锁定的 Unihan 17.0.0，繁体字形来自仓库里 vendored 的 OpenCC 词典。锁文件是 resources/chaizi.lock.json，许可证与改动说明在 resources/licenses/kfcd-chaizi-CC-BY-3.0.txt。

输出两张 UTF-8 TSV，都提交进仓库，由 crates/engine/src/local/chaizi.rs 用 include_str! 编进引擎：

- crates/engine/src/local/chaizi.tsv：每行 `键<TAB>字串`。键是各部件读音（去调，ü 写 v）以 `'` 连接，字串是这个键能拆出的字，按名次排好，不加分隔。先按去掉 `'` 后的键字节序排序，同键再按原键排序。
- crates/engine/src/local/chaizi-chars.tsv：每行 `字<TAB>名次<TAB>读音<TAB>繁体字形`，只收能被某个键查到的字，按码位排序。名次从 0 连续编号；读音是 Unihan kMandarin 的第一个值（带调），没有时为空；繁体字形按 OpenCC s2t 对这个字单独转换的结果，和字本身相同时为空。

结果只取决于锁定的输入：集合排序后输出，不写时间戳；区块用显式码位范围判断，不用 unicodedata.name()，它随 Python 自带的 Unicode 版本变化。去声调只用到 NFD 和 Mn 类别，对拉丁字母的声调符号是稳定的。

上游数据里有几处需要处理的异常，数目都写成断言，上游或 Unihan 变了就让生成失败，提醒人工复核：两表中以「□」为键的占位行、两行带尾随空格的键、一个私用区字（作为「字」出现）、一个与表里已有字重复的兼容表意字。

--check 重新生成后和仓库里的两张表逐字节比较，不写文件。输入不在缓存里时 --check 打印 skip 并以 0 退出（契约检查不联网），设了 MSIME_FETCH=1 才下载。

usage: build_chaizi_table.py [--cache <directory>] [--check]   （缓存默认 target/chaizi-cache）
"""
import argparse
import itertools
import json
import os
import re
import sys
import unicodedata
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.dont_write_bytecode = True  # 导入 scripts/ 下的脚本时不留下 __pycache__
from fetch_wordbooks import digest, fetch  # noqa: E402  （按锁下载并校验长度和 sha256）
from fetch_opencc import ensure_opencc  # noqa: E402

LOCK = ROOT / "resources/chaizi.lock.json"
GENERATOR = "scripts/build_chaizi_table.py"
DEFAULT_CACHE = ROOT / "target/chaizi-cache"
KEYS_TABLE = ROOT / "crates/engine/src/local/chaizi.tsv"
CHARS_TABLE = ROOT / "crates/engine/src/local/chaizi-chars.tsv"

PLACEHOLDER = "□"
# 上游异常的预期数目，见模块说明。
EXPECTED_PLACEHOLDER_ROWS = {"chaizi-jt.txt": 2909, "chaizi-ft.txt": 5}
EXPECTED_STRIPPED_KEYS = 2
EXPECTED_PRIVATE_USE_KEYS = 1
EXPECTED_MERGED_COMPATIBILITY_KEYS = 1
EXPECTED_UNION = 20463

PRIVATE_USE = (0xE000, 0xF8FF)
COMPATIBILITY = ((0xF900, 0xFAFF), (0x2F800, 0x2FA1F))
URO = (0x4E00, 0x9FFF)
EXT_A = (0x3400, 0x4DBF)
# 《通用规范汉字表》一、二、三级的序号上限。
TGH_LEVELS = (3500, 6500, 8105)

# 部首变体的日常叫法。Unihan 给这些部件的读音是它们作为字的读音（灬 biāo、疒 nè、辶 chuò），和用户说出部首名称时念的不一样，所以在 Unihan 读音之外再加一个本仓库自写的读音。每条只加一个音节：键按部件逐个拼接，多音节的名称（三点水、提手旁）会打乱 `'` 落在部件边界上的约定。
ALIASES = {
    "氵": ("san", "三点水"),
    "扌": ("ti", "提手旁"),
    "忄": ("shu", "竖心旁"),
    "宀": ("bao", "宝盖头"),
    "冖": ("bao", "秃宝盖，宝盖头去掉上面的点"),
    "辶": ("zou", "走之底"),
    "辵": ("zou", "走之底的繁体部件形"),
    "阝": ("er", "耳刀旁"),
    "灬": ("huo", "四点底，火的变形"),
    "疒": ("bing", "病字旁"),
    "攵": ("wen", "反文旁"),
    "攴": ("wen", "反文旁的另一种写法"),
    "罒": ("si", "四字头"),
    "丨": ("shu", "笔画名称：竖"),
    "丶": ("dian", "笔画名称：点"),
    "乛": ("zhe", "笔画名称：横折"),
    "囗": ("kou", "大口框，形同口"),
    "癶": ("deng", "登字头"),
    "廴": ("jian", "建之旁"),
    "彐": ("xue", "雪字底"),
    "冫": ("liang", "两点水"),
    "彳": ("ren", "双人旁"),
    "犭": ("gou", "反犬旁，俗称狗字旁"),
    "冂": ("tong", "同字框"),
    "丬": ("jiang", "将字旁"),
}

READING = re.compile(r"[a-z]+")


def read_lines(path: Path) -> list[str]:
    """按 Rust `str::lines` 的规则分行：只认 `\\n`，去掉行尾的 `\\r`。`splitlines` 还会在 U+2028 等字符处断行，和引擎、OpenCC 解析不一致。"""
    text = path.read_text(encoding="utf-8")
    return [line[:-1] if line.endswith("\r") else line for line in text.split("\n")]


def in_range(codepoint: int, span: tuple[int, int]) -> bool:
    return span[0] <= codepoint <= span[1]


def is_private_use(character: str) -> bool:
    return len(character) == 1 and in_range(ord(character), PRIVATE_USE)


def is_compatibility(character: str) -> bool:
    return len(character) == 1 and any(in_range(ord(character), span) for span in COMPATIBILITY)


def read_opencc(name: str) -> dict[str, str]:
    """OpenCC 词典：键到第一个值，重复键取先出现的一行，与 client-base 的 `Table::parse` 相同。"""
    table: dict[str, str] = {}
    for line in read_lines(ensure_opencc(offline=True) / name):
        if not line or line.startswith("#") or "\t" not in line:
            continue
        key, values = line.split("\t", 1)
        items = values.split()
        if key and items:
            table.setdefault(key, items[0])
    return table


class Conversion:
    """client-base `chinese_conversion::simplified_to_traditional` 对单个字的结果：兼容表意字归一化，再查词组表（`STPhrases` 与地区词组的并集），最后查单字表。单字输入时最长匹配只可能是这一个字。"""

    def __init__(self) -> None:
        self.normalization = read_opencc("CJK_Compatibility_Ideographs.txt")
        phrases = read_opencc("STPhrases.txt")
        for key, value in read_opencc("STPhrases_GeneratedFromRegionalPhrases.txt").items():
            phrases.setdefault(key, value)
        self.phrases = phrases
        self.characters = read_opencc("STCharacters.txt")

    def normalize(self, character: str) -> str:
        return self.normalization.get(character, character)

    def traditional(self, character: str) -> str:
        normalized = self.normalize(character)
        return self.phrases.get(normalized) or self.characters.get(normalized) or normalized


def read_decompositions(cache: Path, conversion: Conversion) -> tuple[dict[str, list[tuple[str, ...]]], dict[str, int]]:
    """两表取并集：字到去重后的拆法列表（保持上游顺序）。"""
    union: dict[str, list[tuple[str, ...]]] = {}
    stats = {"stripped_keys": 0, "private_use_keys": 0, "compatibility_keys": []}
    compatibility_rows: list[tuple[str, list[tuple[str, ...]]]] = []
    for name, expected in EXPECTED_PLACEHOLDER_ROWS.items():
        placeholders = 0
        for line in read_lines(cache / name):
            if not line:
                continue
            fields = line.split("\t")
            key = fields[0].strip()
            if key != fields[0]:
                stats["stripped_keys"] += 1
            if key == PLACEHOLDER:
                placeholders += 1
                continue
            if len(key) != 1:
                raise SystemExit(f"{name}: 键 {key!r} 不是单个字")
            parts = [tuple(component for component in field.split(" ") if component) for field in fields[1:]]
            parts = [part for part in parts if part]
            if is_private_use(key):
                stats["private_use_keys"] += 1
                continue
            if is_compatibility(key):
                compatibility_rows.append((key, parts))
                continue
            known = union.setdefault(key, [])
            for part in parts:
                if part not in known:
                    known.append(part)
        if placeholders != expected:
            raise SystemExit(f"{name}: 「{PLACEHOLDER}」占位行有 {placeholders} 行，不是 {expected}；上游变了，复核后再改预期数目")
    # 兼容表意字归一化后的字已在表里时并入它，否则丢弃：宿主的 OpenCC 转换同样先做这一步，表里留着兼容字只会多出一个上屏后变样的候选。
    for key, parts in compatibility_rows:
        normalized = conversion.normalize(key)
        if normalized != key and normalized in union:
            if key not in stats["compatibility_keys"]:
                stats["compatibility_keys"].append(key)
            known = union[normalized]
            for part in parts:
                if part not in known:
                    known.append(part)
    checks = (
        ("带首尾空白的键", stats["stripped_keys"], EXPECTED_STRIPPED_KEYS),
        ("私用区的键", stats["private_use_keys"], EXPECTED_PRIVATE_USE_KEYS),
        ("并入统一汉字的兼容表意字", len(stats["compatibility_keys"]), EXPECTED_MERGED_COMPATIBILITY_KEYS),
        ("并集的字数", len(union), EXPECTED_UNION),
    )
    for label, found, expected in checks:
        if found != expected:
            raise SystemExit(f"{label}有 {found} 个，不是 {expected}；上游变了，复核后再改预期数目")
    stats["compatibility_keys"] = len(stats["compatibility_keys"])
    return union, stats


def read_unihan(zip_path: Path) -> dict[str, dict[str, object]]:
    fields = {
        "Unihan_Readings.txt": ("kMandarin", "kTGHZ2013"),
        "Unihan_OtherMappings.txt": ("kTGH",),
        "Unihan_Variants.txt": ("kTraditionalVariant",),
    }
    mandarin: dict[str, list[str]] = {}
    tghz: dict[str, list[str]] = {}
    tgh: dict[str, int] = {}
    traditional: dict[str, list[str]] = {}
    with zipfile.ZipFile(zip_path) as archive:
        for member, wanted in fields.items():
            for line in archive.read(member).decode("utf-8").split("\n"):
                if not line or line.startswith("#"):
                    continue
                codepoint, name, value = line.split("\t", 2)
                if name not in wanted:
                    continue
                character = chr(int(codepoint[2:], 16))
                if name == "kMandarin":
                    mandarin[character] = value.split()
                elif name == "kTGHZ2013":
                    # 形如 `482.140:zhòu`，冒号前是页码与位置。
                    tghz[character] = [item.rsplit(":", 1)[1] for item in value.split()]
                elif name == "kTGH":
                    # 形如 `2013:6602`，冒号后是字表序号。
                    tgh[character] = min(int(item.split(":", 1)[1]) for item in value.split())
                else:
                    # 值是 `U+XXXX` 码位，要先解码。
                    traditional[character] = [chr(int(item[2:], 16)) for item in value.split()]
    if len(tgh) != TGH_LEVELS[-1]:
        raise SystemExit(f"kTGH 有 {len(tgh)} 条，不是 {TGH_LEVELS[-1]}；Unihan 的结构变了")
    return {"mandarin": mandarin, "tghz": tghz, "tgh": tgh, "traditional": traditional}


def toneless(reading: str) -> str:
    """去声调：先把 ü 及其带调形式换成 v，再 NFD 去掉组合符号。不先换的话 nǚ 会变成 nu。"""
    reading = re.sub("[üǖǘǚǜ]", "v", reading)
    stripped = "".join(ch for ch in unicodedata.normalize("NFD", reading) if unicodedata.category(ch) != "Mn")
    if not READING.fullmatch(stripped):
        raise SystemExit(f"读音 {reading!r} 去调后是 {stripped!r}，不是小写拼音字母")
    return stripped


def component_readings(component: str, unihan: dict) -> list[str]:
    readings = set()
    for reading in unihan["mandarin"].get(component, []) + unihan["tghz"].get(component, []):
        readings.add(toneless(reading))
    if component in ALIASES:
        readings.add(ALIASES[component][0])
    # nüe、lüe 在拼音输入习惯里常写成 nue、lue，两种写法都出键。
    for reading in list(readings):
        if reading in ("nve", "lve"):
            readings.add(reading.replace("v", "u"))
    return sorted(readings)


def build_keys(union: dict[str, list[tuple[str, ...]]], unihan: dict) -> tuple[dict[str, set[str]], dict[str, int]]:
    keys: dict[str, set[str]] = {}
    stats = {"decompositions": 0, "dropped_placeholder_or_private": 0, "dropped_unread": 0}
    unread: set[str] = set()
    for character, parts in union.items():
        for part in parts:
            stats["decompositions"] += 1
            if any(component == PLACEHOLDER or any(is_private_use(ch) for ch in component) for component in part):
                stats["dropped_placeholder_or_private"] += 1
                continue
            readings = [component_readings(component, unihan) for component in part]
            missing = [component for component, options in zip(part, readings) if not options]
            if missing:
                stats["dropped_unread"] += 1
                unread.update(missing)
                continue
            for combination in itertools.product(*readings):
                keys.setdefault("'".join(combination), set()).add(character)
    stats["unread_components"] = len(unread)
    # 别名表只收真正补上读音的条目：部件不在任何拆法里，或别名和 Unihan 读音相同，都是写错了或已经过时。
    used = {component for parts in union.values() for part in parts for component in part}
    for component, (alias, _) in ALIASES.items():
        if component not in used:
            raise SystemExit(f"别名表里的 {component} 不在任何拆法里")
        native = {toneless(reading) for reading in unihan["mandarin"].get(component, []) + unihan["tghz"].get(component, [])}
        if alias in native:
            raise SystemExit(f"别名表里 {component} 的 {alias} 已是 Unihan 读音")
    return keys, stats


def layer(character: str, unihan: dict, traditional_of_tgh: set[str]) -> tuple[int, int]:
    """名次的层与层内序：《通用规范汉字表》一二三级按字表序号，其余按码位。"""
    serial = unihan["tgh"].get(character)
    if serial is not None:
        level = next(index for index, limit in enumerate(TGH_LEVELS) if serial <= limit)
        return level, serial
    codepoint = ord(character)
    if character in traditional_of_tgh:
        return 3, codepoint
    if in_range(codepoint, URO):
        return 4, codepoint
    if in_range(codepoint, EXT_A):
        return 5, codepoint
    return 6, codepoint


def render(keys: dict[str, set[str]], unihan: dict, conversion: Conversion) -> tuple[str, str, dict[str, int]]:
    findable = sorted({character for characters in keys.values() for character in characters})
    traditional_of_tgh = {variant for character in unihan["tgh"] for variant in unihan["traditional"].get(character, [])} - set(unihan["tgh"])
    ordered = sorted(findable, key=lambda character: (layer(character, unihan, traditional_of_tgh), ord(character)))
    rank = {character: index for index, character in enumerate(ordered)}

    key_lines = []
    for key in sorted(keys, key=lambda key: (key.replace("'", "").encode("utf-8"), key.encode("utf-8"))):
        characters = "".join(sorted(keys[key], key=rank.__getitem__))
        key_lines.append(f"{key}\t{characters}\n")

    char_lines = []
    for character in findable:
        readings = unihan["mandarin"].get(character, [])
        traditional = conversion.traditional(character)
        char_lines.append(f"{character}\t{rank[character]}\t{readings[0] if readings else ''}\t{traditional if traditional != character else ''}\n")

    layers: dict[int, int] = {}
    for character in findable:
        level = layer(character, unihan, traditional_of_tgh)[0]
        layers[level] = layers.get(level, 0) + 1
    stats = {
        "keys": len(keys),
        "pairs": sum(len(characters) for characters in keys.values()),
        "max_per_key": max(len(characters) for characters in keys.values()),
        "findable": len(findable),
        "without_reading": sum(1 for character in findable if character not in unihan["mandarin"]),
        "with_traditional_form": sum(1 for line in char_lines if not line.endswith("\t\n")),
        "tgh_covered": sum(1 for character in unihan["tgh"] if character in rank),
    }
    stats.update({f"layer_{level}": count for level, count in sorted(layers.items())})
    return "".join(key_lines), "".join(char_lines), stats


def inputs(cache: Path, check: bool) -> tuple[list[Path], Path] | None:
    """按锁取拆字表和 Unihan zip：缓存里有且摘要一致就用，否则下载（--check 时要 MSIME_FETCH=1）。"""
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    unihan_lock = json.loads((ROOT / lock["unihan_lock"]).read_text(encoding="utf-8"))
    artifacts = lock["artifacts"] + [unihan_lock["artifact"]]
    missing = [artifact for artifact in artifacts if not ((cache / artifact["name"]).is_file() and digest(cache / artifact["name"]) == artifact["sha256"])]
    if missing and check and os.environ.get("MSIME_FETCH") != "1":
        return None
    paths = [fetch(artifact, cache) for artifact in artifacts]
    return paths[:-1], paths[-1]


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache", default=str(DEFAULT_CACHE), help="放锁定输入的目录，默认 target/chaizi-cache")
    parser.add_argument("--check", action="store_true", help="重新生成并和仓库里的两张表逐字节比较，不写文件")
    arguments = parser.parse_args(argv)
    cache = Path(arguments.cache).resolve()

    found = inputs(cache, arguments.check)
    if found is None:
        print(f"skip: {cache} 里没有锁定的拆字表或 Unihan zip（设 MSIME_FETCH=1 下载后再检查）")
        return 0
    _, unihan_zip = found
    try:
        ensure_opencc(offline=arguments.check and os.environ.get("MSIME_FETCH") != "1")
    except FileNotFoundError:
        if not arguments.check or os.environ.get("MSIME_FETCH") == "1":
            raise
        print("skip: 固定的 OpenCC 输入不在缓存里（运行 scripts/fetch_opencc.py 准备）")
        return 0
    conversion = Conversion()
    union, union_stats = read_decompositions(cache, conversion)
    unihan = read_unihan(unihan_zip)
    keys, key_stats = build_keys(union, unihan)
    keys_text, chars_text, table_stats = render(keys, unihan, conversion)
    stats = {"union": len(union), **union_stats, **key_stats, **table_stats}

    outputs = ((KEYS_TABLE, keys_text.encode("utf-8")), (CHARS_TABLE, chars_text.encode("utf-8")))
    if arguments.check:
        stale = [path.relative_to(ROOT) for path, content in outputs if not path.is_file() or path.read_bytes() != content]
        if stale:
            print(f"FAIL: {', '.join(map(str, stale))} 和生成结果不一致，运行 python3 {GENERATOR} 重新生成")
            return 1
        print(f"ok: 拆字表与生成结果一致（{stats['keys']} 个键，{stats['findable']} 字）")
        return 0
    for path, content in outputs:
        path.write_bytes(content)
        print(f"wrote {path.relative_to(ROOT)}（{len(content)} 字节）")
    print(json.dumps(stats, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
