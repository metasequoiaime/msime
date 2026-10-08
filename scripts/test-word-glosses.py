#!/usr/bin/env python3
"""Offline check of build_word_glosses.py against real CC-CEDICT lines.

The lines below are copied from the pinned 2026-09-23 export, including the references, classifiers and notes the
cleaner exists to drop and the multi-reading entries it merges. Everything runs on a temporary zip; nothing is
downloaded, so --quick can run it offline.
"""
import sqlite3
import sys
import tempfile
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
import build_word_glosses as generator  # noqa: E402

failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


LINES = """# CC-CEDICT fixture
芋頭 芋头 [yu4 tou5] /taro/
學校 学校 [xue2 xiao4] /school/CL:所[suo3]/
你好 你好 [ni3 hao3] /hello; hi/
愛 爱 [ai4] /to love; to be fond of; to like/affection/to be inclined (to do sth); to tend to (happen)/
了 了 [le5] /(completed action marker)/(modal particle indicating change of state, situation now)/
了 了 [liao3] /to finish/(used with 得[de2] or 不[bu4] after a verb to express (im)possibility, as in 忘不了[wang4 bu5 liao3] "cannot forget")/
便宜 便宜 [bian4 yi2] /convenient/
便宜 便宜 [pian2 yi5] /cheap; inexpensive/small advantages/to let sb off lightly/
李 李 [Li3] /surname Li/
李 李 [li3] /plum/
王 王 [Wang2] /surname Wang/
著 着 [zhao1] /(chess) move/trick; tactic/(Taiwan pr. [zhuo2])/
馬屁 马屁 [ma3 pi4] /horse hindquarters/flattery/boot-licking/
蔔 卜 [bo5] /see 蘿蔔|萝卜[luo2 bo5]/
"""

EXPECTED = {
    "芋头": "taro",
    "学校": "school",  # the classifier is not a translation
    "你好": "hello, hi",
    "爱": "to love, to be fond of, to like; affection",
    "了": "completed action marker; to finish",  # two readings: the first sense of each
    "便宜": "convenient; cheap, inexpensive",
    "李": "plum",  # the common noun, not the surname entry
    "马屁": "horse hindquarters; flattery",
}


def main() -> int:
    check(generator.clean_senses(["CL:所[suo3]"]) == "", "a classifier alone is nothing")
    check(generator.clean_senses(["see 蘿蔔|萝卜[luo2 bo5]"]) == "", "a see-reference is nothing")
    check(generator.clean_senses(["(bound form) clear in one's mind"]) == "clear in one's mind", "(bound form) is dropped")
    check(generator.clean_senses(["(Taiwan pr. [zhuo2])"]) == "", "a Taiwan pronunciation note is dropped")
    check(generator.clean_senses(["fish head", "fig. upright and unwilling to compromise"]) == "fish head; upright and unwilling to compromise",
          "a fig. label is dropped and its meaning kept")

    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "cedict.zip"
        with zipfile.ZipFile(path, "w") as archive:
            archive.writestr("cedict_ts.u8", LINES)
        with zipfile.ZipFile(path) as archive:
            rows = generator.collect(archive.read("cedict_ts.u8").decode("utf-8"))
        for word, expected in EXPECTED.items():
            check(rows.get(word) == expected, f"{word}: {rows.get(word)!r}, expected {expected!r}")
        check("王" not in rows, "a headword with only a surname entry is left out")
        check("卜" not in rows, "a headword with only a reference is left out")
        check(rows.get("着") == "(chess) move; trick, tactic", f"着: {rows.get('着')!r}")

        database = Path(directory) / "zh-en.db"
        generator.write_database(database, rows, {"kind": "zh_en_words"})
        first = database.read_bytes()
        generator.write_database(database, rows, {"kind": "zh_en_words"})
        check(first == database.read_bytes(), "the same rows give the same bytes")
        connection = sqlite3.connect(database)
        try:
            check(connection.execute("PRAGMA user_version").fetchone() == (1,), "user_version is 1")
            meta = dict(connection.execute("SELECT key, value FROM meta"))
            check(meta.get("target_language") == "en", f"meta.target_language: {meta.get('target_language')!r}")
        finally:
            connection.close()

    for failure in failures:
        print(f"FAIL {failure}")
    print(f"word glosses: {'ok' if not failures else f'{len(failures)} failure(s)'}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
