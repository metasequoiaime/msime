#!/usr/bin/env python3
"""在构建目录生成原生词库窗口测试需要的合成 SQLite 资源。"""

import sqlite3
import sys
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("需要 SQL 来源路径与输出目录")
    source, output = Path(sys.argv[1]), Path(sys.argv[2])
    output.mkdir(parents=True, exist_ok=True)
    pinyin = output / "msime-pinyin.db"
    english = output / "msime-english.db"
    pinyin.unlink(missing_ok=True)
    english.unlink(missing_ok=True)
    with sqlite3.connect(pinyin) as database:
        database.executescript(source.read_text(encoding="utf-8"))
        database.execute("CREATE TABLE tbl_2_h(key TEXT,jp TEXT,value TEXT,weight INTEGER)")
        database.execute("INSERT INTO tbl_2_h VALUES(?,?,?,?)", ("he'cheng", "hc", "合成", 321))
    with sqlite3.connect(english) as database:
        database.execute("CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER)")
        database.execute("CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT)")
        database.execute("CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english TEXT)")
        database.execute("INSERT INTO english_words VALUES(?,?,?)", ("synthetic", "Synthetic", 123))


if __name__ == "__main__":
    main()
