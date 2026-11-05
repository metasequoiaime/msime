#!/usr/bin/env python3
"""从 `quanpin-words-v1.tsv` 派生 `quanpin-words-v2.tsv`：去掉三类杂质，给同码用例打标签。v1 不动。

v1 的金标准来自微软的示例词表，按一份固定的参照词库核对后有三类用例量的不是排序：

- 截断（truncated）：源格式在 12 个字符处截断，截出来的串碰巧也能切成金标准那么多个音节（`aizengfenmin` 爱憎分明），v1 的截断过滤因此漏掉了它们。判据：参照词库里有这个词，但没有一个读音等于输入，而输入是某个读音的真前缀。
- 读音不符（reading-mismatch）：参照词库里有这个词，但任何读音都既不等于输入、也不以输入开头（`aiban` 呆板、`geiyu` 给予）。
- 缺词（missing）：参照词库里没有这个词（`bihua` 比划、`chexiao` 撤消）。

比较读音时把 `lue`、`nue` 当作 `lve`、`nve`：引擎把用户打的 `lue` 规整成词库里的 `lve`（`lattice_span_key_plan_tests.rs`），`celue` 打得出 策略，不算读音不符。截断落在 `lue` 中间时（`fazhanzhanlu` 发展战略），输入是 `…lue` 写法的前缀，也算截断。方案文档按字面比较数出的 83 条读音不符里有 37 条是这种写法，这里留下了。

同码标签：在留下的用例里，同一串码对应不止一个金标准时，这几条都打上标签。全拼按输入的字母串（`same-code-full`），九键按键上印的数字（`same-code-nine`），14 键按组码（`same-code-fourteen`）；码表在 `scripts/eval_keys.py`，照抄 `crates/engine/src/nine_key.rs`。同码用例里至多一条能排第一，top-1 的上限因此不到 1，`scripts/eval_compare.py` 把它们单独分桶。

参照词库固定为 dict-v2.0.41 的 `msime-pinyin.db`（`scripts/eval_keys.py` 的 `REFERENCE_RELEASE`、`REFERENCE_PINYIN_SHA256`），脚本先核对它的 SHA-256，再把发布名和 SHA 写进文件头。不读当前的资源锁：锁升级以后，同一份参照词库照样逐字节重新生成 v2。换参照词库要发 v3，不改 v2。

用法（在仓库根目录）：

    python3 scripts/build-quanpin-words-v2.py --pinyin-db <dict-v2.0.41 的 msime-pinyin.db> \\
        [--v1 resources/eval/quanpin-words-v1.tsv] [--out resources/eval/quanpin-words-v2.tsv] [--removed <removed.tsv>]

`--removed` 另写一份被去掉的用例清单（id、输入、金标准、类别、参照词库里的读音），供审阅，不入库。
"""
from __future__ import annotations

import argparse
import collections
import sqlite3
import sys
from pathlib import Path

from eval_keys import REFERENCE_PINYIN_SHA256, REFERENCE_RELEASE, canonical_reading as canonical, check_reference_pinyin, encode

ROOT = Path(__file__).resolve().parents[1]
KEYBOARDS = ("full", "nine", "fourteen")
CLASSES = ("truncated", "reading-mismatch", "missing")


def load_readings(database: Path) -> dict:
    """参照词库里每个词的全部读音（规整后的字母串）。"""
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    tables = [name for (name,) in connection.execute("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'tbl\\_%' ESCAPE '\\'")]
    readings = collections.defaultdict(set)
    for table in tables:
        for key, value in connection.execute(f'SELECT key, value FROM "{table}"'):
            readings[value].add(canonical(key))
    connection.close()
    return readings


def load_v1(path: Path) -> tuple:
    """v1 的文件头（`#` 行）和用例行（按列拆开）。"""
    header, rows = [], []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            header.append(line)
        elif line and not line.startswith("id\t"):
            fields = line.split("\t")
            while len(fields) < 5:
                fields.append("")
            rows.append(fields)
    return header, rows


def classify(case_input: str, gold: str, readings: dict) -> str | None:
    """用例属于哪一类杂质；干净的返回 None。"""
    known = readings.get(gold)
    if not known:
        return "missing"
    typed = canonical(case_input)
    if typed in known:
        return None
    # 截断可能落在 `lue` 中间：`…lu` 不是 `…lve` 的前缀，却是 `…lue` 的前缀。只把 lve/nve 换回去，`lv`（驴）不动，`lu` 打 驴 仍算读音不符。
    typed_ue = typed.replace("lve", "lue").replace("nve", "nue")
    if any(reading.startswith(typed) or reading.replace("lve", "lue").replace("nve", "nue").startswith(typed_ue) for reading in known):
        return "truncated"
    return "reading-mismatch"


def same_code_tags(rows: list) -> dict:
    """每条用例在三种键盘上是不是同码用例：同一串码对应不止一个金标准。返回 {id: [标签...]}。"""
    tags = collections.defaultdict(list)
    for keyboard in KEYBOARDS:
        golds = collections.defaultdict(set)
        for row in rows:
            golds[encode(keyboard, row[1])].add(row[2])
        for row in rows:
            if len(golds[encode(keyboard, row[1])]) > 1:
                tags[row[0]].append(f"same-code-{keyboard}")
    return tags


def upper_bound(rows: list, keyboard: str) -> float:
    """每串码只能有一个金标准排第一时，top-1 最多能到多少：每串码取用例最多的那个金标准，加起来 ÷ 用例数（同一串码同一个金标准的重复用例都能对）。"""
    best = collections.defaultdict(collections.Counter)
    for row in rows:
        best[encode(keyboard, row[1])][row[2]] += 1
    return sum(counter.most_common(1)[0][1] for counter in best.values()) / len(rows)


def build(v1_rows: list, readings: dict) -> tuple:
    """返回（留下的行，去掉的行及类别）。留下的行带上同码标签，保留 v1 的 id、输入、金标准和音节数。"""
    kept, removed = [], []
    for row in v1_rows:
        kind = classify(row[1], row[2], readings)
        if kind is None:
            kept.append(row)
        else:
            removed.append((row, kind))
    tags = same_code_tags(kept)
    tagged = []
    for row in kept:
        existing = [tag for tag in row[4].split(",") if tag]
        tagged.append(row[:4] + [",".join(existing + tags.get(row[0], []))])
    return tagged, removed


def main(argv: list | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--pinyin-db", type=Path, required=True)
    parser.add_argument("--v1", type=Path, default=ROOT / "resources/eval/quanpin-words-v1.tsv")
    parser.add_argument("--out", type=Path, default=ROOT / "resources/eval/quanpin-words-v2.tsv")
    parser.add_argument("--removed", type=Path)
    args = parser.parse_args(argv)

    check_reference_pinyin(args.pinyin_db)
    readings = load_readings(args.pinyin_db)
    v1_header, v1_rows = load_v1(args.v1)
    kept, removed = build(v1_rows, readings)
    counts = collections.Counter(kind for _, kind in removed)
    tag_counts = collections.Counter(tag for row in kept for tag in row[4].split(",") if tag)
    bounds = {keyboard: upper_bound(kept, keyboard) for keyboard in KEYBOARDS}
    lines = [
        "# 由 quanpin-words-v1.tsv 派生，用例的来源与授权同 v1。以下 [v1] 开头的几行是 v1 的文件头：",
        *(f"# [v1] {line[1:].strip()}" for line in v1_header),
        f"# reference-dictionary: {REFERENCE_RELEASE} msime-pinyin.db sha256={REFERENCE_PINYIN_SHA256}",
        f"# derived-by: python3 scripts/build-quanpin-words-v2.py --pinyin-db <msime-pinyin.db of {REFERENCE_RELEASE}>",
        f"# v1={len(v1_rows)} kept={len(kept)} " + " ".join(f"{kind}={counts[kind]}" for kind in CLASSES),
        "# same-code: " + " ".join(f"{tag}={tag_counts[f'same-code-{tag}']}" for tag in ("full", "nine", "fourteen")),
        "# top1-upper-bound: " + " ".join(f"{name}={value:.3f}" for name, value in bounds.items()),
        "id\tinput\tgold\tsyllables\ttags",
        *("\t".join(row) for row in kept),
    ]
    args.out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    if args.removed:
        args.removed.write_text(
            "id\tinput\tgold\tclass\treference_readings\n"
            + "".join(f"{row[0]}\t{row[1]}\t{row[2]}\t{kind}\t{','.join(sorted(readings.get(row[2], ())))}\n" for row, kind in removed),
            encoding="utf-8",
        )
    print(f"{args.out}: kept {len(kept)} of {len(v1_rows)}; " + ", ".join(f"{kind} {counts[kind]}" for kind in CLASSES), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
