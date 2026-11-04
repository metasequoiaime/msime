#!/usr/bin/env python3
"""粗俗词首屏曝光和九键英文第 2 位曝光报告：高频词的每一个部分输入都打一遍，数首屏里有多少粗俗词。

为什么逐键打：粗俗词往往不是在用户打它自己的读音时冒出来，而是在用户打别的词打到一半时（九键 243542 是 别叫 的前缀，首屏里有 别鸡巴）、或者九键一串数字对应好几种读法时出现。现有评测集每条只打全一次读音，一条都量不到。

输入集由 `inputs` 子命令从参照词库生成并提交（`resources/eval/exposure-quanpin-v1.tsv`、`exposure-nine-key-v1.tsv`），换词库前后打同一批输入，数字才可比：取参照词库里权重最高的 N 个 2 到 4 字词（每个音节一个汉字，按全部读音），每个读音生成打全的输入、每一个字母前缀和前 k 个音节的简拼（首字母），按键盘去重（九键按数字串），再加上方案点名要清零的目标码。

判定：
- 粗俗词：候选含 `crates/engine/src/lattice/offensive_words.txt` 里的词（按子串，豁免写法盖住的不算，与引擎 `lattice::offensive::is_offensive` 同一规则）。`--extra-words` 可以再给一份同格式的表（以后词库仓库 curation/ 里的粗俗词表、牛逼 卧槽 这类只在九键歧义码上避开的流行口语）。
- 英文冒犯词：英文候选（不分大小写）在 `crates/dict-builder/src/english_supplement.rs` 的 `OFFENSIVE`（55 个蔑称）或 `--extra-words` 的 ASCII 词里。同时读引擎的 `offensive_english_words.txt`，覆盖脏话和蔑称。九键另读 `mild_slang_words.txt`，全键盘不把轻度粗口计入。
- 精确读音：粗俗词本身的读音恰好就是这串输入（全键盘逐字母、九键按数字串）时记为 exact。方案的政策是这些词不当首选，也不出现在前缀、简拼和九键、14 键歧义码的首屏；只有全键盘打它自己读音时可以在首屏（但不当首选）。九键的一串数字几乎都对应好几种读法，exact 在九键上不豁免，所以 `vulgar_first_page_inputs_violating`（违反政策的首屏输入数）在全键盘上去掉只有 exact 命中的输入，在九键、14 键上等于全部首屏命中；exact 仍逐行标出供审阅。

`run` 用 `convert_eval --dump --mixed-english` 在全键盘和九键上各打一遍（混输英文是产品出厂设置），再出报告；`report` 只读已有的 dump。给了 `--baseline`（上一次的报告 JSON）时逐项比较：粗俗首屏输入数、粗俗首选输入数、英文冒犯词首屏输入数任何一项比基线多，退出码 1。

用法（在仓库根目录）：

    python3 scripts/vulgar_exposure.py inputs --pinyin-db <dict-v2.0.41 的 msime-pinyin.db> [--top 5000]
    python3 scripts/vulgar_exposure.py run --resources <已校验的资源目录> [--resource-lock <锁>] [--binary target/release/examples/convert_eval] \\
        [--out target/vulgar-exposure] [--json <报告.json>] [--baseline <上次的报告.json>] [--extra-words <表>]
    python3 scripts/vulgar_exposure.py report --dump full=<dump> --dump nine=<dump> [--pinyin-db <词库>] [--json ...] [--baseline ...]
"""
from __future__ import annotations

import argparse
import collections
import json
import sqlite3
import sys
from pathlib import Path

import eval_keys

ROOT = eval_keys.ROOT
OFFENSIVE_WORDS = ROOT / "crates/engine/src/lattice/offensive_words.txt"
ENGLISH_OFFENSIVE_SOURCE = ROOT / "crates/dict-builder/src/english_supplement.rs"
ENGLISH_WORDS = ROOT / "crates/engine/src/lattice/offensive_english_words.txt"
MILD_WORDS = ROOT / "crates/engine/src/lattice/mild_slang_words.txt"
SETS = {"full": ROOT / "resources/eval/exposure-quanpin-v1.tsv", "nine": ROOT / "resources/eval/exposure-nine-key-v1.tsv"}
DEFAULT_TOP = 5000
FIRST_PAGE_ROWS = 6
# 方案 4.3 点名的九键目标码：别鸡巴、个鸡巴、傻逼 的码，九键 63476（内容 后面第 2 位是 Negro），以及 内容、尴尬、试图 打全的码。
TARGETS = [
    ("nine", "2435422", "别鸡巴"),
    ("nine", "435422", "个鸡巴"),
    ("nine", "74224", "傻逼"),
    ("nine", "63476", "内容"),
    ("letters", "neirong", "内容"),
    ("letters", "ganga", "尴尬"),
    ("letters", "shitu", "试图"),
]


class WordList:
    """粗俗词表：词和豁免写法，格式同 `offensive_words.txt`（`#` 注释、`!` 开头是豁免写法）。ASCII 的词归英文表，按小写整词比较。"""

    def __init__(self) -> None:
        self.listed: set = set()
        self.exempt: list = []
        self.english: set = set()
        self.longest = 0

    def add_file(self, path: Path) -> None:
        for line in path.read_text(encoding="utf-8").splitlines():
            entry = line.strip()
            if not entry or entry.startswith("#"):
                continue
            if entry.startswith("!"):
                self.exempt.append(entry[1:])
            elif entry.isascii():
                self.english.add(entry.casefold())
            else:
                self.listed.add(entry)
                self.longest = max(self.longest, len(entry))

    def add_english(self, words) -> None:
        self.english.update(word.casefold() for word in words)

    def is_offensive(self, word: str) -> bool:
        """与引擎 `is_offensive` 同一规则：两字以上的任何一段在表里，而且这一段没有被一条豁免写法整个盖住。单字不查。"""
        if len(word) < 2 or word.isascii():
            return False
        for start in range(len(word)):
            for length in range(2, min(self.longest, len(word) - start) + 1):
                if word[start:start + length] in self.listed and not self.exempted(word, start, start + length):
                    return True
        return False

    def exempted(self, word: str, start: int, end: int) -> bool:
        for pattern in self.exempt:
            at = word.find(pattern)
            while at != -1:
                if at <= start and end <= at + len(pattern):
                    return True
                at = word.find(pattern, at + 1)
        return False

    def is_offensive_english(self, word: str) -> bool:
        return word.isascii() and word.casefold() in self.english


def english_offensive(source: Path) -> list:
    """`english_supplement.rs` 里 `OFFENSIVE` 数组的词：从 `pub const OFFENSIVE` 那一行起，到 `];` 为止，取每行引号里的词。"""
    words, inside = [], False
    for line in source.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped.startswith("pub const OFFENSIVE:"):
            inside = True
            continue
        if inside:
            if stripped.startswith("];"):
                return words
            if stripped.startswith('"') and stripped.endswith('",'):
                words.append(stripped[1:-2])
    raise SystemExit(f"{source}: could not find the OFFENSIVE array")


def load_word_list(extra: list, keyboard: str = "full") -> WordList:
    words = WordList()
    words.add_file(OFFENSIVE_WORDS)
    words.add_english(english_offensive(ENGLISH_OFFENSIVE_SOURCE))
    words.add_file(ENGLISH_WORDS)
    if keyboard != "full":
        words.add_file(MILD_WORDS)
    for path in extra:
        words.add_file(path)
    return words


def is_han(text: str) -> bool:
    return all("一" <= ch <= "鿿" or "㐀" <= ch <= "䶿" for ch in text)


def top_words(database: Path, top: int) -> list:
    """参照词库里权重最高的 `top` 个 2 到 4 字词（每个音节一个汉字），连同这个词的全部读音，按权重从高到低。"""
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    best: dict = {}
    for syllables in (2, 3, 4):
        tables = [name for (name,) in connection.execute(f"SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'tbl\\_{syllables}\\_%' ESCAPE '\\'")]
        for table in tables:
            for key, value, weight in connection.execute(f'SELECT key, value, weight FROM "{table}"'):
                if len(value) == syllables and is_han(value) and key.count("'") == syllables - 1:
                    entry = best.setdefault(value, [weight, set()])
                    entry[0] = max(entry[0], weight)
                    entry[1].add(key)
    connection.close()
    ranked = sorted(best.items(), key=lambda item: (-item[1][0], item[0]))[:top]
    return [(word, weight, sorted(keys)) for word, (weight, keys) in ranked]


def partial_inputs(reading: str) -> list:
    """一个读音的全部输入和它们的类别：打全（full）、每一个更短的字母前缀（prefix）、前 k 个音节的首字母（initials，k 从 2 起；k=1 就是一个字母，已经在前缀里）。"""
    syllables = reading.split("'")
    letters = "".join(syllables)
    inputs = [(letters, "full")]
    inputs += [(letters[:end], "prefix") for end in range(1, len(letters))]
    inputs += [("".join(s[0] for s in syllables[:count]), "initials") for count in range(2, len(syllables) + 1)]
    return inputs


def build_inputs(words: list) -> dict:
    """按键盘去重的输入集：键是这种键盘上的码，值是 [第一次生成它的字母输入、词、类别集合]。词按权重从高到低，所以码对应的是最常用的那个词。目标码排在最前面。"""
    sets = {"full": {}, "nine": {}}
    for kind, text, word in TARGETS:
        letters = eval_keys.nine_digits_to_letters(text) if kind == "nine" else text
        for keyboard, table in sets.items():
            if kind == "nine" and keyboard != "nine":
                continue
            entry = table.setdefault(eval_keys.encode(keyboard, letters), [letters, word, set()])
            entry[2].add("target")
    for word, _, readings in words:
        for reading in readings:
            for letters, kind in partial_inputs(reading):
                for keyboard, table in sets.items():
                    entry = table.setdefault(eval_keys.encode(keyboard, letters), [letters, word, set()])
                    entry[2].add(kind)
    return sets


def write_inputs(sets: dict, words: list, release: str, digest: str, top: int) -> None:
    for keyboard, table in sets.items():
        lines = [
            f"# 粗俗词与英文首屏曝光报告的输入集（scripts/vulgar_exposure.py），{'全键盘全拼' if keyboard == 'full' else '九键，输入写字母，convert_eval --grid nine 换成数字'}。",
            f"# reference-dictionary: {release} msime-pinyin.db sha256={digest}",
            f"# derived-by: python3 scripts/vulgar_exposure.py inputs --pinyin-db <msime-pinyin.db of {release}> --top {top}",
            f"# words={len(words)} inputs={len(table)} " + " ".join(f"{kind}={sum(kind in entry[2] for entry in table.values())}" for kind in ("full", "prefix", "initials", "target")),
            "# 第 5 列是这串码由哪几类输入生成（同一串码可以既是某个词打全的码，又是另一个词的前缀）；第 3 列是生成它的权重最高的词，只给报告做参照，不是金标准。",
            "id\tinput\tgold\tsyllables\ttags",
        ]
        for index, (code, (letters, word, kinds)) in enumerate(table.items(), 1):
            lines.append(f"x-{index:05d}\t{letters}\t{word}\t{len(word)}\t{','.join(sorted(kinds))}")
        SETS[keyboard].write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"{SETS[keyboard]}: {len(table)} inputs", file=sys.stderr)


def load_set(path: Path) -> dict:
    cases = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#") or line.startswith("id\t"):
            continue
        fields = line.split("\t")
        cases[fields[0]] = {"input": fields[1], "word": fields[2], "kinds": fields[4].split(",") if len(fields) > 4 and fields[4] else []}
    return cases


def readings_of(database: Path | None, words: set) -> dict:
    """`words` 里每个词在词库里的全部读音（规整后的字母串）；没给词库时为空。"""
    found = collections.defaultdict(set)
    if database is None or not words:
        return found
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    tables = [name for (name,) in connection.execute("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'tbl\\_%' ESCAPE '\\'")]
    for table in tables:
        for key, value in connection.execute(f'SELECT key, value FROM "{table}"'):
            if value in words:
                found[value].add(eval_keys.canonical_reading(key))
    connection.close()
    return found


def analyze(keyboard: str, cases: dict, dump: dict, words: WordList, first_page_rows: int) -> dict:
    """一种键盘上的曝光：每串输入首屏里的粗俗词和英文冒犯词，以及第 2 位的英文词。"""
    missing = sorted(cases.keys() - dump.keys())
    if missing:
        raise SystemExit(f"{len(missing)} {keyboard} input(s) are not in the dump (e.g. {missing[:3]}); it was run on another set")
    for case_id, case in cases.items():
        row = dump[case_id]
        if (row.get("input"), row.get("gold")) != (case["input"], case["word"]):
            raise SystemExit(f"{keyboard} {case_id}: input or gold differs from the exposure set")
    hits, english_hits, second_slot, targets = [], [], [], []
    for case_id, case in cases.items():
        candidates = dump[case_id].get("candidates", [])
        page = candidates[:first_page_rows]
        code = eval_keys.encode(keyboard, case["input"])
        for rank, candidate in enumerate(page, 1):
            text = candidate["text"]
            if words.is_offensive(text):
                hits.append({"id": case_id, "input": case["input"], "code": code, "word": text, "rank": rank, "kinds": case["kinds"], "from": case["word"]})
            elif words.is_offensive_english(text):
                english_hits.append({"id": case_id, "input": case["input"], "code": code, "word": text, "rank": rank, "kinds": case["kinds"], "from": case["word"]})
        if len(candidates) > 1 and candidates[1].get("source") == eval_keys.ENGLISH_SOURCE:
            second_slot.append({"id": case_id, "input": case["input"], "code": code, "word": candidates[1]["text"], "kinds": case["kinds"], "from": case["word"], "first": candidates[0]["text"]})
        if "target" in case["kinds"]:
            targets.append({"input": case["input"], "code": code, "for": case["word"], "first_page": [c["text"] for c in page]})
    return {"inputs": len(cases), "hits": hits, "english_hits": english_hits, "second_slot": second_slot, "targets": targets}


def mark_exact(keyboard: str, hits: list, readings: dict) -> None:
    """粗俗词本身的读音恰好就是这串输入时记 exact。"""
    for hit in hits:
        typed = eval_keys.encode(keyboard, eval_keys.canonical_reading(hit["input"]))
        hit["exact"] = any(eval_keys.encode(keyboard, reading) == typed for reading in readings.get(hit["word"], ()))


def summarize(keyboard: str, analysis: dict, cases: dict) -> dict:
    """报告的计数。门槛只看 `GATED` 里的三项，其余供审阅。exact 只在全键盘上豁免：九键、14 键的码几乎都有歧义，政策不许粗俗词上这些码的首屏。"""
    hits = analysis["hits"]
    by_input = collections.defaultdict(list)
    for hit in hits:
        by_input[hit["id"]].append(hit)
    kinds = collections.Counter()
    for case_hits in by_input.values():
        for kind in set(case_hits[0]["kinds"]):
            kinds[kind] += 1
    second = analysis["second_slot"]
    summary = {
        "inputs": analysis["inputs"],
        "vulgar_first_page_inputs": len(by_input),
        "vulgar_first_page_rows": len(hits),
        "vulgar_top1_inputs": sum(any(hit["rank"] == 1 for hit in case_hits) for case_hits in by_input.values()),
        "vulgar_first_page_inputs_violating": sum(keyboard != "full" or any(not hit.get("exact", False) for hit in case_hits) for case_hits in by_input.values()),
        "vulgar_first_page_inputs_by_kind": dict(sorted(kinds.items())),
        "english_offensive_first_page_inputs": len({hit["id"] for hit in analysis["english_hits"]}),
        "english_second_slot_inputs": len(second),
        "english_second_slot_full_codes": sum("full" in item["kinds"] for item in second),
        "full_code_inputs": sum("full" in case["kinds"] for case in cases.values()),
    }
    return summary


def report(dumps: dict, database: Path | None, extra: list, first_page_rows: int) -> dict:
    result = {"first_page_rows": first_page_rows, "word_lists": [str(OFFENSIVE_WORDS.relative_to(ROOT)), str(ENGLISH_OFFENSIVE_SOURCE.relative_to(ROOT)) + " OFFENSIVE", str(ENGLISH_WORDS.relative_to(ROOT)), str(MILD_WORDS.relative_to(ROOT)) + " (ambiguous keyboards only)", *map(str, extra)], "keyboards": {}}
    for keyboard, dump in dumps.items():
        words = load_word_list(extra, keyboard)
        cases = load_set(SETS[keyboard])
        analysis = analyze(keyboard, cases, dump, words, first_page_rows)
        mark_exact(keyboard, analysis["hits"], readings_of(database, {hit["word"] for hit in analysis["hits"]}))
        summary = summarize(keyboard, analysis, cases)
        summary["set_sha256"] = eval_keys.sha256(SETS[keyboard])
        top_english = collections.Counter(item["word"] for item in analysis["second_slot"])
        result["keyboards"][keyboard] = {
            "summary": summary,
            "vulgar": sorted(analysis["hits"], key=lambda hit: (hit["rank"], hit["code"], hit["word"])),
            "english_offensive": analysis["english_hits"],
            "english_second_slot_top": top_english.most_common(100),
            "english_second_slot": analysis["second_slot"],
            "targets": analysis["targets"],
        }
    return result


GATED = ("vulgar_first_page_inputs", "vulgar_top1_inputs", "english_offensive_first_page_inputs")


def compare_baseline(result: dict, baseline: dict) -> list:
    """逐项比较：基线里有的键盘，粗俗首屏、粗俗首选、英文冒犯词首屏三项都不得增加。输入集不同（set_sha256 不同）时不可比，报错。"""
    if result.get("first_page_rows") != baseline.get("first_page_rows"):
        return ["first_page_rows differs from the baseline; measure both arms with the same page size"]
    problems = []
    for keyboard, previous in baseline["keyboards"].items():
        current = result["keyboards"].get(keyboard)
        if current is None:
            problems.append(f"{keyboard}: not measured this time")
            continue
        if current["summary"]["set_sha256"] != previous["summary"]["set_sha256"]:
            problems.append(f"{keyboard}: the input set changed since the baseline; regenerate the baseline on the new set first")
            continue
        for name in GATED:
            if current["summary"][name] > previous["summary"][name]:
                problems.append(f"{keyboard}: {name} {previous['summary'][name]} -> {current['summary'][name]}")
    return problems


def print_report(result: dict, listed: int) -> None:
    for keyboard, values in result["keyboards"].items():
        summary = values["summary"]
        exemption = "typed at the word's own reading exempt" if keyboard == "full" else "no exemption on ambiguous codes"
        print(f"[{keyboard}] {summary['inputs']} inputs: vulgar on first page {summary['vulgar_first_page_inputs']} inputs ({summary['vulgar_first_page_rows']} rows, {summary['vulgar_first_page_inputs_violating']} against the policy, {exemption}), vulgar first {summary['vulgar_top1_inputs']}; by kind {summary['vulgar_first_page_inputs_by_kind']}")
        print(f"  english: offensive on first page {summary['english_offensive_first_page_inputs']} inputs; second slot {summary['english_second_slot_inputs']} inputs, {summary['english_second_slot_full_codes']} of {summary['full_code_inputs']} full codes")
        for hit in values["vulgar"][:listed]:
            print(f"  vulgar {hit['rank']} {hit['word']} at {hit['code']} ({hit['input']}, {'/'.join(hit['kinds'])} of {hit['from']}{', exact' if hit.get('exact') else ''})")
        for hit in values["english_offensive"][:listed]:
            print(f"  english-offensive {hit['rank']} {hit['word']} at {hit['code']} ({hit['input']} of {hit['from']})")
        if values["english_second_slot_top"]:
            print("  second-slot english, most frequent: " + ", ".join(f"{word}×{count}" for word, count in values["english_second_slot_top"][:20]))
        for target in values["targets"]:
            print(f"  target {target['code']} ({target['for']}): {' '.join(target['first_page'])}")


def main(argv: list | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    inputs = commands.add_parser("inputs", help="regenerate the committed input sets from a reference dictionary")
    inputs.add_argument("--pinyin-db", type=Path, required=True)
    inputs.add_argument("--top", type=int, default=DEFAULT_TOP)
    for name in ("run", "report"):
        command = commands.add_parser(name)
        command.add_argument("--json", type=Path)
        command.add_argument("--baseline", type=Path)
        command.add_argument("--extra-words", type=Path, action="append", default=[])
        command.add_argument("--list", type=int, default=30)
        command.add_argument("--first-page-rows", type=int, default=FIRST_PAGE_ROWS)
        if name == "run":
            command.add_argument("--resources", type=Path, required=True)
            command.add_argument("--resource-lock", type=Path)
            command.add_argument("--binary", type=Path, default=eval_keys.DEFAULT_BINARY)
            command.add_argument("--out", type=Path, default=ROOT / "target/vulgar-exposure")
            command.add_argument("--keyboards", default="full,nine")
        else:
            command.add_argument("--dump", action="append", default=[], metavar="KEYBOARD=PATH", required=True)
            command.add_argument("--pinyin-db", type=Path, help="mark hits typed at the word's own reading; without it no hit is exact")
    args = parser.parse_args(argv)

    if args.command == "inputs":
        eval_keys.check_reference_pinyin(args.pinyin_db)
        words = top_words(args.pinyin_db, args.top)
        write_inputs(build_inputs(words), words, eval_keys.REFERENCE_RELEASE, eval_keys.REFERENCE_PINYIN_SHA256, args.top)
        return 0

    dumps = {}
    if args.command == "run":
        args.out.mkdir(parents=True, exist_ok=True)
        database = args.resources.resolve() / "msime-pinyin.db"
        for keyboard in args.keyboards.split(","):
            if keyboard not in SETS:
                raise SystemExit(f"--keyboards takes {', '.join(SETS)}")
            dump = args.out / f"{keyboard}.jsonl"
            eval_keys.run_convert_eval(args.binary, args.resources.resolve(), args.resource_lock, SETS[keyboard], dump, args.out / f"{keyboard}.report.json", keyboard,
                                       ["--mixed-english", "--first-page-rows", str(args.first_page_rows)])
            dumps[keyboard] = eval_keys.load_dump(dump)
    else:
        database = args.pinyin_db
        for item in args.dump:
            keyboard, _, path = item.partition("=")
            if keyboard not in SETS or not path:
                raise SystemExit(f"--dump takes KEYBOARD=PATH with KEYBOARD one of {', '.join(SETS)}")
            dumps[keyboard] = eval_keys.load_dump(Path(path))

    result = report(dumps, database, args.extra_words, args.first_page_rows)
    print_report(result, args.list)
    if args.json:
        args.json.write_text(json.dumps(result, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    if args.baseline:
        problems = compare_baseline(result, json.loads(args.baseline.read_text(encoding="utf-8")))
        for problem in problems:
            print(f"baseline: {problem}", file=sys.stderr)
        return 1 if problems else 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
