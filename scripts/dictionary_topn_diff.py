#!/usr/bin/env python3
"""五笔和英文两份词库的 top-N 差异清单，给词库 PR 做审阅附件。只报告，不设通过线。

为什么只报告：五笔和英文都没有独立的金标准。五笔的码表本身就是答案，没有一份外部的「这个码该出哪个字」可以拿来打分；英文的补全和释义也没有评测集。能做的是把换词库前后用户第一眼看到的东西并排列出来，由人判断变好还是变坏。

为什么是数据层：五笔和英文的候选顺序几乎就是 SQL 的排序（五笔 `weight DESC` 再按码和字，英文补全照 `crates/engine/src/dictionary/english.rs` 的 `PREFIX_SQL`：完全相同的词在前，再按权重、长度），所以这里直接读两份数据库按同样的排序取前 N 个，不经引擎。量不到的有：个人学习和调频、五笔四码唯一自动上屏、九键英文的数字前缀（`KEYPAD_PREFIX_SQL`），这些要在真实宿主上看。

清单（写在 `--out` 下，制表符分隔，只列有变化的行）：

- `wubi86-exact.tsv`、`wubi98-exact.tsv`：每个码打全时的前 N 个字词。按码长、再按换之前首选的权重从高到低排，最常打的码在前。
- `wubi86-completion.tsv`、`wubi98-completion.tsv`：一码、二码打下去时补全的前 N 行（`字词(码)`），方案 1.1 说的 jidian 尾段占住补全前 5 就在这里。
- `english-prefix.tsv`：每个 1 到 3 个字母的前缀的补全前 N 个词。
- `english-en-zh.tsv`：换之前权重最高的 `--english-words` 个英文词的英译中释义。
- `english-zh-en.tsv`：中译英释义变了的中文词；目录里有 `msime-pinyin.db` 时按它的权重排，常用词在前。

用法（在仓库根目录；两个目录是已校验的资源目录或任何放着这几个数据库的目录）：

    python3 scripts/dictionary_topn_diff.py --before <A 目录> --after <B 目录> --out <清单目录> [--top 5] [--english-words 5000]

`summary.json` 和标准输出给出每份清单比较了多少行、首选变了多少、前 N 个变了多少。
"""
from __future__ import annotations

import argparse
import json
import sqlite3
import string
import sys
from pathlib import Path

WUBI_TABLES = ("wubi86", "wubi98")
# crates/engine/src/dictionary/english.rs 的 `PREFIX_SQL`，LIMIT 由调用方补上。
ENGLISH_PREFIX_SQL = "SELECT word,display,weight FROM english_words WHERE word >= ?1 AND word < ?2 ORDER BY CASE WHEN word = ?1 THEN 0 ELSE 1 END, weight DESC, length(word), word, display LIMIT ?3"


def connect(path: Path) -> sqlite3.Connection:
    if not path.is_file():
        raise SystemExit(f"{path} does not exist")
    return sqlite3.connect(f"file:{path}?mode=ro", uri=True)


def table_names(connection: sqlite3.Connection) -> set:
    return {name for (name,) in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}


def wubi_exact(connection: sqlite3.Connection, table: str, top: int) -> dict:
    """每个码打全时的前 `top` 个字词，以及首选的权重（排序用）。"""
    rows: dict = {}
    for key, value, weight in connection.execute(f'SELECT key, value, weight FROM "{table}" ORDER BY key, weight DESC, value'):
        entry = rows.setdefault(key, [weight, []])
        if len(entry[1]) < top:
            entry[1].append(value)
    return rows


def wubi_completion(connection: sqlite3.Connection, table: str, top: int, prefixes: list) -> dict:
    """一码、二码前缀补全的前 `top` 行，写成 `字词(码)`。"""
    result = {}
    for prefix in prefixes:
        rows = connection.execute(
            f'SELECT key, value FROM "{table}" WHERE key >= ? AND key < ? ORDER BY weight DESC, key, value LIMIT ?',
            (prefix, prefix + "\x7f", top),
        ).fetchall()
        if rows:
            result[prefix] = [f"{value}({key})" for key, value in rows]
    return result


def english_prefix(connection: sqlite3.Connection, top: int, prefixes: list) -> dict:
    result = {}
    for prefix in prefixes:
        rows = connection.execute(ENGLISH_PREFIX_SQL, (prefix, prefix + "\x7f", top)).fetchall()
        if rows:
            result[prefix] = [display for _, display, _ in rows]
    return result


def glosses(connection: sqlite3.Connection, table: str, key: str, gloss: str) -> dict:
    return dict(connection.execute(f'SELECT {key}, {gloss} FROM "{table}"'))


def top_english(connection: sqlite3.Connection, count: int) -> list:
    return [word for (word,) in connection.execute("SELECT word FROM english_words GROUP BY word ORDER BY max(weight) DESC, word LIMIT ?", (count,))]


def pinyin_weights(path: Path) -> dict:
    """msime-pinyin.db 里每个词的最大权重；没有这个文件时为空。"""
    if not path.is_file():
        return {}
    connection = connect(path)
    weights: dict = {}
    for name in sorted(table_names(connection)):
        if name.startswith("tbl_"):
            for value, weight in connection.execute(f'SELECT value, max(weight) FROM "{name}" GROUP BY value'):
                if weight > weights.get(value, -1):
                    weights[value] = weight
    connection.close()
    return weights


def diff_lists(before: dict, after: dict) -> tuple:
    """两张「键 → 前 N 个」的表：返回（变了的键，比较了几个键，首选变了几个）。"""
    keys = sorted(set(before) | set(after))
    changed, top1 = [], 0
    for key in keys:
        a, b = before.get(key, []), after.get(key, [])
        if a != b:
            changed.append(key)
            if a[:1] != b[:1]:
                top1 += 1
    return changed, len(keys), top1


def write_tsv(path: Path, header: list, rows: list) -> None:
    path.write_text("\t".join(header) + "\n" + "".join("\t".join(map(str, row)) + "\n" for row in rows), encoding="utf-8")


def run(before: Path, after: Path, out: Path, top: int, english_words: int) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    summary = {"top": top, "before": str(before), "after": str(after), "lists": {}}
    letters = string.ascii_lowercase
    one_two = list(letters) + [a + b for a in letters for b in letters]

    wubi_a, wubi_b = connect(before / "msime-wubi.db"), connect(after / "msime-wubi.db")
    for table in WUBI_TABLES:
        if table not in table_names(wubi_a) or table not in table_names(wubi_b):
            continue
        exact_a, exact_b = wubi_exact(wubi_a, table, top), wubi_exact(wubi_b, table, top)
        changed, compared, top1 = diff_lists({k: v[1] for k, v in exact_a.items()}, {k: v[1] for k, v in exact_b.items()})
        changed.sort(key=lambda key: (len(key), -(exact_a.get(key) or exact_b[key])[0], key))
        write_tsv(out / f"{table}-exact.tsv", ["code", "before", "after"], [(key, " ".join(exact_a.get(key, [None, []])[1]), " ".join(exact_b.get(key, [None, []])[1])) for key in changed])
        summary["lists"][f"{table}-exact"] = {"compared": compared, "changed": len(changed), "top1_changed": top1}
        comp_a, comp_b = wubi_completion(wubi_a, table, top, one_two), wubi_completion(wubi_b, table, top, one_two)
        changed, compared, top1 = diff_lists(comp_a, comp_b)
        changed.sort(key=lambda key: (len(key), key))
        write_tsv(out / f"{table}-completion.tsv", ["prefix", "before", "after"], [(key, " ".join(comp_a.get(key, [])), " ".join(comp_b.get(key, []))) for key in changed])
        summary["lists"][f"{table}-completion"] = {"compared": compared, "changed": len(changed), "top1_changed": top1}
    wubi_a.close()
    wubi_b.close()

    english_a, english_b = connect(before / "msime-english.db"), connect(after / "msime-english.db")
    prefixes = one_two + [a + b + c for a in letters for b in letters for c in letters]
    prefix_a, prefix_b = english_prefix(english_a, top, prefixes), english_prefix(english_b, top, prefixes)
    changed, compared, top1 = diff_lists(prefix_a, prefix_b)
    changed.sort(key=lambda key: (len(key), key))
    write_tsv(out / "english-prefix.tsv", ["prefix", "before", "after"], [(key, " ".join(prefix_a.get(key, [])), " ".join(prefix_b.get(key, []))) for key in changed])
    summary["lists"]["english-prefix"] = {"compared": compared, "changed": len(changed), "top1_changed": top1}

    en_a, en_b = glosses(english_a, "en_zh_glosses", "english", "chinese_gloss"), glosses(english_b, "en_zh_glosses", "english", "chinese_gloss")
    words = top_english(english_a, english_words)
    rows = [(rank, word, en_a.get(word, ""), en_b.get(word, "")) for rank, word in enumerate(words, 1) if en_a.get(word) != en_b.get(word)]
    write_tsv(out / "english-en-zh.tsv", ["rank", "english", "before", "after"], rows)
    summary["lists"]["english-en-zh"] = {"compared": len(words), "changed": len(rows)}

    zh_a, zh_b = glosses(english_a, "zh_en_glosses", "chinese", "english_gloss"), glosses(english_b, "zh_en_glosses", "chinese", "english_gloss")
    weights = pinyin_weights(before / "msime-pinyin.db")
    changed = [word for word in set(zh_a) | set(zh_b) if zh_a.get(word) != zh_b.get(word)]
    changed.sort(key=lambda word: (-weights.get(word, -1), word))
    write_tsv(out / "english-zh-en.tsv", ["chinese", "pinyin_weight", "before", "after"], [(word, weights.get(word, ""), zh_a.get(word, ""), zh_b.get(word, "")) for word in changed])
    summary["lists"]["english-zh-en"] = {"compared": len(set(zh_a) | set(zh_b)), "changed": len(changed), "ranked_by_pinyin_weight": bool(weights)}
    english_a.close()
    english_b.close()

    (out / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return summary


def main(argv: list | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--top", type=int, default=5)
    parser.add_argument("--english-words", type=int, default=5000)
    args = parser.parse_args(argv)
    summary = run(args.before, args.after, args.out, args.top, args.english_words)
    for name, values in summary["lists"].items():
        print(f"{name}: " + ", ".join(f"{key} {value}" for key, value in values.items()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
