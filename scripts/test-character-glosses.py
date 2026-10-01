#!/usr/bin/env python3
"""Offline check of build_character_glosses.py against real Unihan kDefinition values.

The definitions below are copied from Unihan 17.0.0, including the apparatus the cleaner exists to drop. Everything
runs on a temporary zip in Unihan's layout; nothing is downloaded, so --quick can run it offline.
"""
import sqlite3
import sys
import tempfile
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
import build_character_glosses as generator  # noqa: E402

failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


# (kDefinition, cleaned gloss)
DEFINITIONS = [
    ("love, be fond of, like", "love, be fond of, like"),
    ("sky, heaven; god, celestial", "sky, heaven; god, celestial"),
    ("water, liquid, lotion, juice", "water, liquid, lotion"),  # at most three words
    ("and; and then; and yet; but; Kangxi radical 126", "and; and then"),  # at most two senses
    ("self, private, personal; from; Kangxi radical 132", "self, private, personal; from"),
    ("queen, empress, sovereign; (simp. for 後) behind, rear, after", "behind, rear, after; queen, empress, sovereign"),
    ("heart; mind, intelligence; soul; 5th lunar mansion, determinative star", "heart; mind, intelligence"),
    ("numerary adjunct, piece; single", "piece; single"),
    ("in, on, at; go to; surname", "in, on, at; go to"),
    ("offspring, child; fruit, seed of; first earthly branch", "offspring, child; fruit, seed of"),
    ("our, us, i, me, my, we", "our, us, I"),  # a lone i is the pronoun
    ("Kangxi radical 1", ""),  # nothing left
]


def unihan_zip(path: Path, readings: list[str], variants: list[str]) -> None:
    header = "# Unihan fixture in the 17.0.0 layout\n"
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("Unihan_Readings.txt", header + "\n".join(readings) + "\n")
        archive.writestr("Unihan_Variants.txt", header + "\n".join(variants) + "\n")


def main() -> int:
    for raw, expected in DEFINITIONS:
        found = generator.clean_definition(raw)
        check(found == expected, f"clean_definition({raw!r}) = {found!r}, expected {expected!r}")

    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "Unihan.zip"
        unihan_zip(
            path,
            [
                "U+7231\tkDefinition\tlove, be fond of, like",  # 爱
                "U+5857\tkDefinition\tsmear, daub, apply, spread; paint",  # 塗
                "U+51E0\tkDefinition\tsmall table",  # 几
                "U+673A\tkDefinition\tdesk; machine",  # 机
                "U+5929\tkMandarin\ttiān",  # a field that is not read
            ],
            [
                "U+6D82\tkTraditionalVariant\tU+5857",  # 涂 -> 塗, no definition of its own
                "U+51E0\tkTraditionalVariant\tU+51E0 U+5E7E",  # 几 -> 几 幾
            ],
        )
        rows = generator.collect(*generator.read_unihan(path))
        check(rows.get("爱") == "love, be fond of, like", f"爱: {rows.get('爱')!r}")
        check(rows.get("涂") == "smear, daub, apply; paint", f"a character defined only through its traditional form: {rows.get('涂')!r}")
        check(rows.get("几") == "how many, several; small table", f"a correction wins over Unihan's order: {rows.get('几')!r}")
        check(rows.get("机") == "machine; desk", f"机: {rows.get('机')!r}")
        check("天" not in rows, "a character with no kDefinition is left out")

        database = Path(directory) / "zh-en.db"
        generator.write_database(database, rows, {"kind": "zh_en_characters"})
        first = database.read_bytes()
        generator.write_database(database, rows, {"kind": "zh_en_characters"})
        check(first == database.read_bytes(), "the same rows give the same bytes")
        connection = sqlite3.connect(database)
        try:
            check(connection.execute("PRAGMA user_version").fetchone() == (1,), "user_version is 1")
            meta = dict(connection.execute("SELECT key, value FROM meta"))
            # The bridge's candidate_target_glosses refuses a file whose target_language is not the one asked for.
            check(meta.get("target_language") == "en", f"meta.target_language: {meta.get('target_language')!r}")
            check(
                connection.execute("SELECT gloss FROM zh_glosses WHERE chinese = '爱'").fetchone() == ("love, be fond of, like",),
                "爱 is looked up by its character",
            )
        finally:
            connection.close()

    for failure in failures:
        print(f"FAIL {failure}")
    print(f"character glosses: {'ok' if not failures else f'{len(failures)} failure(s)'}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
