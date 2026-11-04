#!/usr/bin/env python3
"""检查 scripts/dictionary_topn_diff.py：五笔按权重取前 N、补全按前缀、英文补全照引擎 `PREFIX_SQL` 的次序，清单只列变了的行，首选变了单独计数。

用合成的小数据库，不需要真实词库。
"""
import importlib.util
import json
import sqlite3
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("topn_diff", ROOT / "scripts/dictionary_topn_diff.py")
topn_diff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(topn_diff)

# 英文补全的排序与引擎逐字相同，引擎改了这里要跟着改。
english_rs = (ROOT / "crates/engine/src/dictionary/english.rs").read_text(encoding="utf-8")
engine_order = "ORDER BY CASE WHEN word = ?1 THEN 0 ELSE 1 END, weight DESC, length(word), word, display LIMIT "
assert engine_order in english_rs, "PREFIX_SQL changed in english.rs"
assert engine_order.strip() in topn_diff.ENGLISH_PREFIX_SQL


def make(directory: Path, wubi: list, english: list, en_zh: dict, zh_en: dict, pinyin: list) -> None:
    directory.mkdir()
    connection = sqlite3.connect(directory / "msime-wubi.db")
    for table in topn_diff.WUBI_TABLES:
        connection.execute(f'CREATE TABLE {table} ("key" TEXT NOT NULL, "value" TEXT NOT NULL, "weight" INTEGER NOT NULL DEFAULT 0, UNIQUE("key", "value"))')
        connection.executemany(f"INSERT INTO {table} VALUES (?, ?, ?)", wubi)
    connection.commit()
    connection.close()
    connection = sqlite3.connect(directory / "msime-english.db")
    connection.execute("CREATE TABLE english_words (word TEXT NOT NULL, display TEXT NOT NULL, weight INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (word, display))")
    connection.execute("CREATE TABLE en_zh_glosses (english TEXT PRIMARY KEY, chinese_gloss TEXT NOT NULL)")
    connection.execute("CREATE TABLE zh_en_glosses (chinese TEXT PRIMARY KEY, english_gloss TEXT NOT NULL)")
    connection.executemany("INSERT INTO english_words VALUES (?, ?, ?)", english)
    connection.executemany("INSERT INTO en_zh_glosses VALUES (?, ?)", en_zh.items())
    connection.executemany("INSERT INTO zh_en_glosses VALUES (?, ?)", zh_en.items())
    connection.commit()
    connection.close()
    connection = sqlite3.connect(directory / "msime-pinyin.db")
    connection.execute("CREATE TABLE tbl_2_y (key TEXT, jp TEXT, value TEXT, weight INTEGER)")
    connection.executemany("INSERT INTO tbl_2_y VALUES (?, ?, ?, ?)", pinyin)
    connection.commit()
    connection.close()


with tempfile.TemporaryDirectory(prefix="msime-topn-diff-") as temporary:
    root = Path(temporary)
    before, after = root / "a", root / "b"
    make(before,
         [("g", "一", 100), ("gg", "五", 50), ("ggll", "一二", 10), ("bqsy", "版权所有", 5), ("aaaa", "工", 9)],
         [("is", "is", 900), ("island", "island", 50), ("it", "it", 800), ("isis", "ISIS", 10)],
         {"is": "信息系统", "it": "它"}, {"一点": "ghost", "影响": "influence"},
         [("yi'dian", "yd", "一点", 300), ("ying'xiang", "yx", "影响", 900)])
    make(after,
         [("g", "一", 100), ("gg", "五", 50), ("ggll", "一二", 10), ("bqsy", "版权所有", 5), ("aaaa", "工", 9), ("g", "第五", 444000000)],
         [("is", "is", 900), ("island", "island", 50), ("it", "it", 800), ("isis", "ISIS", 10), ("isle", "isle", 100)],
         {"is": "是", "it": "它"}, {"一点": "a little", "影响": "influence"},
         [("yi'dian", "yd", "一点", 300), ("ying'xiang", "yx", "影响", 900)])
    out = root / "out"
    summary = topn_diff.run(before, after, out, 3, 10)

    # 五笔：g 的首选被高权重的 第五 换掉；没变的码不进清单。
    exact = (out / "wubi86-exact.tsv").read_text(encoding="utf-8").splitlines()
    assert exact == ["code\tbefore\tafter", "g\t一\t第五 一"], exact
    assert summary["lists"]["wubi86-exact"] == {"compared": 5, "changed": 1, "top1_changed": 1}
    completion = (out / "wubi86-completion.tsv").read_text(encoding="utf-8").splitlines()
    assert completion[1] == "g\t一(g) 五(gg) 一二(ggll)\t第五(g) 一(g) 五(gg)", completion
    # 英文补全：完全相同的词排第一，再按权重；isle 进了 is 和 isl 的前 3。
    prefix = dict(line.split("\t", 1) for line in (out / "english-prefix.tsv").read_text(encoding="utf-8").splitlines()[1:])
    assert prefix["is"] == "is island ISIS\tis isle island", prefix
    assert prefix["i"] == "is it island\tis it isle", prefix
    assert prefix["isl"] == "island\tisle island", prefix
    assert summary["lists"]["english-prefix"]["top1_changed"] == 1, "isl now offers isle first"
    # 释义：只列变了的；中译英按拼音词库权重排，常用词在前。
    assert (out / "english-en-zh.tsv").read_text(encoding="utf-8").splitlines()[1:] == ["1\tis\t信息系统\t是"]
    assert (out / "english-zh-en.tsv").read_text(encoding="utf-8").splitlines()[1:] == ["一点\t300\tghost\ta little"]
    assert json.loads((out / "summary.json").read_text(encoding="utf-8"))["lists"]["english-zh-en"]["ranked_by_pinyin_weight"]

    # 同一份比自己：每份清单都是空的。
    same = topn_diff.run(before, before, root / "same", 3, 10)
    assert all(values["changed"] == 0 for values in same["lists"].values()), same

print("dictionary top-N diff: ok")
