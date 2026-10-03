#!/usr/bin/env python3
"""Writes a small stroke.db into the directory named by the first argument, for the host tests that drive the Stroke scheme through IBus and Fcitx5.

The schema and format version are the engine's (crates/engine/src/language_dictionary.rs); `syllables` holds the five stroke letters, as dict-builder writes them. The rows are synthetic: h s gives 十 exactly and then the longer codes that start with it, and h x (x is the wildcard) gives the two-stroke codes 十 and 二 before anything longer."""

import sqlite3
import sys
from pathlib import Path

SCHEMA = """
CREATE TABLE metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE syllables(syllable TEXT PRIMARY KEY) WITHOUT ROWID;
CREATE TABLE entries(key TEXT NOT NULL, text TEXT NOT NULL, weight INTEGER NOT NULL, PRIMARY KEY(key, text)) WITHOUT ROWID;
CREATE INDEX entries_by_key_weight ON entries(key, weight DESC);
"""

SYLLABLES = ["h", "s", "p", "n", "z"]
ENTRIES = [
    ("h", "一", 900),
    ("hs", "十", 800),
    ("hh", "二", 700),
    ("hspn", "木", 600),
    ("hszhh", "古", 500),
    ("pn", "人", 650),
]


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: stroke_dictionary.py <directory>", file=sys.stderr)
        return 2
    target = Path(sys.argv[1]) / "stroke.db"
    partial = target.with_name("stroke.db.partial")
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
