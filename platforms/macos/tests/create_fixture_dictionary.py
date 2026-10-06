#!/usr/bin/env python3

import sqlite3
import sys
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("Usage: create_fixture_dictionary.py OUTPUT")
    output = Path(sys.argv[1]).resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.unlink(missing_ok=True)
    with sqlite3.connect(output) as database:
        database.execute("CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER)")
        database.execute("INSERT INTO tbl_2_n VALUES(?, ?, ?, ?)", ("ni'hao", "nh", "你好", 100))
    english = output.with_name("msime-english.db")
    english.unlink(missing_ok=True)
    with sqlite3.connect(english) as database:
        database.execute(
            "CREATE TABLE english_words(word TEXT COLLATE BINARY NOT NULL, display TEXT NOT NULL, "
            "weight INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(word, display)) WITHOUT ROWID"
        )
        database.execute(
            "CREATE TABLE en_zh_glosses(english TEXT COLLATE BINARY PRIMARY KEY, "
            "chinese_gloss TEXT NOT NULL) WITHOUT ROWID"
        )
        database.execute(
            "CREATE TABLE zh_en_glosses(chinese TEXT COLLATE BINARY PRIMARY KEY, "
            "english_gloss TEXT NOT NULL) WITHOUT ROWID"
        )
        database.execute("INSERT INTO english_words VALUES(?, ?, ?)", ("hello", "hello", 1))
        database.execute("INSERT INTO en_zh_glosses VALUES(?, ?)", ("hello", "你好"))
        database.execute("INSERT INTO zh_en_glosses VALUES(?, ?)", ("你好", "hello"))
        database.execute("INSERT INTO zh_en_glosses VALUES(?, ?)", ("水杉", "metasequoia"))
    print(output)
    print(english)


if __name__ == "__main__":
    main()
