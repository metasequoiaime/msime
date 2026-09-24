#!/usr/bin/env python3
"""Build English glosses for Chinese words from CC-CEDICT, for the candidates english.db does not answer.

english.db's Chinese-to-English side is ECDICT reversed and deliberately conservative: it keeps Chinese words that are
the translation of a common English word, about 18 600 of them. The Japanese offline gloss, built the other way round
from Wiktionary's translation tables, covers 27 900, and 18 200 of those words — 芋头, 贺卡, 店员, 支票簿 — had a Japanese
line and no English one, and some english.db answers are loose (漂亮 "chic", 电源 "ps"). CC-CEDICT is written from the
Chinese side, 125 000 entries, CC BY-SA 4.0 like the offline glosses already shipped. This input method is also a
learning tool, so host-api shows its entry in place of english.db's; english.db answers what it lacks, and a gloss from
the user's own glossary is never replaced.

The output has the offline-gloss shape, so the bridge's candidate_target_glosses reads it unchanged:
``word-glosses/zh-en.db`` with ``zh_glosses(chinese, gloss, source)``, ``meta.target_language = en`` and
``PRAGMA user_version = 1``.

CC-CEDICT lines are ``traditional simplified [pinyin] /sense/sense/``. ``clean_senses`` drops what is not a translation
(classifiers, variant and see-also references, surnames, abbreviation notes, Taiwan pronunciations), removes the
cross-reference pinyin and ``(bound form)``-style notes, and keeps at most two senses of three phrases. Where one
simplified form has several entries (了 le / liǎo), a common-noun entry beats a proper noun (capitalised pinyin) and the
file's own order breaks ties. That order is alphabetical by pinyin, not by use (要 yāo before yào, 看 kān before kàn), so
single characters are asked of character-glosses/ (Unihan, meaning-first) before this table; here they only fill what
Unihan lacks.

usage: build_word_glosses.py [--out <directory>] [--zip <local cedict_1_0_ts_utf-8_mdbg.zip>]   (default: target/word-glosses)
"""
import argparse
import json
import re
import sqlite3
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True  # importing a sibling script must not leave scripts/__pycache__ behind
from fetch_wordbooks import fetch  # noqa: E402  (verified download)

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/word-glosses.lock.json"
DEFAULT_OUT = ROOT / "target/word-glosses"
DATABASE = "zh-en.db"
NOTICE = "word-glosses-NOTICE.txt"
SCHEMA_VERSION = 1
PAGE_SIZE = 4096
MAX_PHRASES_PER_SENSE = 3
MAX_SENSES = 2
MAX_PHRASE_CHARS = 40

LINE = re.compile(r"^(\S+) (\S+) \[([^\]]*)\] /(.*)/\s*$")
# Senses that point elsewhere or describe the entry rather than translate it.
NOT_A_TRANSLATION = re.compile(
    r"^(CL:|(old |archaic )?variant of|see |see also |also written|also pr\.|Taiwan pr\.|\(Tw\)|surname |abbr\. for|"
    r"used in |erhua variant|\(archaic\) variant|\(old\) variant|japanese variant|korean variant|\(dialect\) variant)",
    re.IGNORECASE,
)
# A cross-reference such as 無|无[wu2] or 得[de2]: the characters and their pinyin.
REFERENCE = re.compile(r"[^\s\[\]/;,()]+\[[^\]]*\]")
NOTE = re.compile(r"\((?:Taiwan pr\.[^)]*|also pr\.[^)]*|bound form|coll\.|colloquial|literary|lit\.|fig\.|formal|dialect|old|archaic|Tw|onom\.|polite|slang|pejorative|derog\.|loanword[^)]*|abbr\.[^)]*|usu\.[^)]*|esp\.[^)]*|i\.e\.[^)]*|e\.g\.[^)]*)\)\s*", re.IGNORECASE)


def clean_senses(senses: list[str]) -> str:
    """CC-CEDICT senses as a gloss line: at most two senses of at most three phrases, non-translations removed."""
    kept = []
    for sense in senses:
        sense = sense.strip()
        if not sense or NOT_A_TRANSLATION.match(sense):
            continue
        sense = NOTE.sub("", sense)
        # "fig. upright and unwilling to compromise", "lit. fish head": the label goes, the meaning stays.
        sense = re.sub(r"^(?:fig\.|lit\.|figuratively|literally)\s+", "", sense, flags=re.IGNORECASE)
        sense = REFERENCE.sub("", sense)
        # A sense that is only a grammatical description, such as "(completed action marker)", keeps its words.
        if sense.startswith("(") and sense.endswith(")") and sense.count("(") == 1:
            sense = sense[1:-1]
        phrases = []
        for phrase in sense.split(";"):
            phrase = " ".join(phrase.replace("  ", " ").split()).strip(" ,.")
            if not phrase or len(phrase) > MAX_PHRASE_CHARS or phrase.count("(") != phrase.count(")"):
                continue
            if phrase.lower() not in (existing.lower() for existing in phrases):
                phrases.append(phrase)
        if phrases:
            kept.append(", ".join(phrases[:MAX_PHRASES_PER_SENSE]))
        if len(kept) == MAX_SENSES:
            break
    return "; ".join(kept)


def entries(text: str):
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        match = LINE.match(line)
        if match:
            _, simplified, pinyin, senses = match.groups()
            yield simplified, pinyin, senses.split("/")


def collect(text: str) -> dict[str, str]:
    """The gloss of each simplified headword.

    One entry gives its first two senses. Several entries are several readings (便宜 biànyí "convenient" and piányi
    "cheap"; 地方 dìfāng "region" and dìfang "place"), and the file's order does not say which is common, so the first
    sense of each of the first two entries is shown. Proper nouns (capitalised pinyin: names, places) come last."""
    found: dict[str, list[tuple[tuple[int, int], str]]] = {}
    for order, (simplified, pinyin, senses) in enumerate(entries(text)):
        gloss = clean_senses(senses)
        if gloss:
            found.setdefault(simplified, []).append(((1 if pinyin[:1].isupper() else 0, order), gloss))
    rows: dict[str, str] = {}
    for simplified, readings in found.items():
        readings.sort()
        common = [gloss for (proper, _), gloss in readings if not proper] or [readings[0][1]]
        if len(common) == 1:
            rows[simplified] = common[0]
        else:
            firsts = []
            for gloss in common:
                first = gloss.split("; ", 1)[0]
                if first not in firsts:
                    firsts.append(first)
            rows[simplified] = "; ".join(firsts[:MAX_SENSES])
    return rows


def write_database(path: Path, rows: dict[str, str], meta: dict) -> None:
    path.unlink(missing_ok=True)
    database = sqlite3.connect(path)
    try:
        database.execute(f"PRAGMA page_size = {PAGE_SIZE}")
        database.execute("PRAGMA journal_mode = DELETE")
        database.execute("CREATE TABLE zh_glosses(chinese TEXT PRIMARY KEY, gloss TEXT NOT NULL, source TEXT NOT NULL) WITHOUT ROWID")
        database.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID")
        database.executemany("INSERT INTO zh_glosses VALUES (?, ?, 'cc-cedict')", sorted(rows.items()))
        database.executemany(
            "INSERT INTO meta VALUES (?, ?)",
            sorted({**meta, "target_language": "en", "key_count": str(len(rows))}.items()),
        )
        database.execute(f"PRAGMA user_version = {SCHEMA_VERSION}")
        database.commit()
        database.execute("VACUUM")
    finally:
        database.close()


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    parser.add_argument("--zip", help="a local copy of the pinned export; it is still checked against the lock")
    arguments = parser.parse_args(argv)

    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    out = Path(arguments.out)
    out.mkdir(parents=True, exist_ok=True)
    cache = Path(arguments.zip).parent if arguments.zip else out.parent / "word-glosses-source"
    source = fetch(lock["artifact"], cache)
    with zipfile.ZipFile(source) as archive:
        text = archive.read("cedict_ts.u8").decode("utf-8")
    rows = collect(text)
    if len(rows) < 50000:
        raise SystemExit(f"only {len(rows)} usable entries; the source layout changed")
    meta = {
        "kind": "zh_en_words",
        "source": lock["source"],
        "published": lock["artifact"]["published"],
        "license": lock["license"],
        "sqlite_version": sqlite3.sqlite_version,
    }
    write_database(out / DATABASE, rows, meta)
    (out / NOTICE).write_text(
        "zh-en.db is adapted from CC-CEDICT (https://cc-cedict.org/), the community-maintained Chinese-English\n"
        f"dictionary published by MDBG, export of {lock['artifact']['published']}. CEDICT - Copyright (C) 1997, 1998\n"
        "Paul Andrew Denisowski. Senses were shortened: references, classifiers and notes removed, at most two senses kept.\n"
        "Licensed under the Creative Commons Attribution-ShareAlike 4.0 International License:\n"
        "https://creativecommons.org/licenses/by-sa/4.0/ ; this adaptation is under the same license.\n",
        encoding="utf-8",
    )
    print(f"{len(rows)} word glosses -> {out / DATABASE} ({(out / DATABASE).stat().st_size / 1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
