#!/usr/bin/env python3
"""Writes a small msime-zhuyin.db into the directory named by the first argument, for the host tests that drive the Zhuyin scheme through IBus and Fcitx5.

The schema and format version are the engine's (crates/engine/src/language_dictionary.rs). The rows are a few Dachen syllables: 1 8 is ㄅㄚ, so the digit 1 spells rather than picks a candidate, and , is ㄝ, so the comma spells rather than writes Chinese punctuation."""

import sqlite3
import sys
from pathlib import Path

SCHEMA = """
CREATE TABLE metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE syllables(syllable TEXT PRIMARY KEY) WITHOUT ROWID;
CREATE TABLE entries(key TEXT NOT NULL, text TEXT NOT NULL, weight INTEGER NOT NULL, PRIMARY KEY(key, text)) WITHOUT ROWID;
CREATE INDEX entries_by_key_weight ON entries(key, weight DESC);
"""

SYLLABLES = ["ㄅㄚ", "ㄝ"]
ENTRIES = [("ㄅㄚ", "八", 900), ("ㄅㄚ", "巴", 500), ("ㄝ", "欸", 50)]


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: zhuyin_dictionary.py <directory>", file=sys.stderr)
        return 2
    target = Path(sys.argv[1]) / "msime-zhuyin.db"
    partial = target.with_name("msime-zhuyin.db.partial")
    partial.unlink(missing_ok=True)
    connection = sqlite3.connect(partial)
    with connection:
        connection.executescript(SCHEMA)
        connection.execute("INSERT INTO metadata VALUES ('format_version', '1')")
        connection.executemany("INSERT INTO syllables VALUES (?)", [(syllable,) for syllable in SYLLABLES])
        connection.executemany("INSERT INTO entries VALUES (?, ?, ?)", ENTRIES)
    connection.close()
    # Renamed into place so a host that looks for the file never sees a half-written one.
    partial.replace(target)
    return 0


if __name__ == "__main__":
    sys.exit(main())
