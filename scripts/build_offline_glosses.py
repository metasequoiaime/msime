#!/usr/bin/env python3
"""Build the offline candidate gloss dictionaries for the non-English targets from Wiktionary.

msime-english.db only glosses Chinese into English. For fr/ja/es/ru/de/ko this pairs, inside one translation table of the English Wiktionary (one English sense), the Mandarin row with the target-language rows, and writes one SQLite file per language: ``zh-<lang>.db`` with ``zh_glosses(chinese, gloss, source)``, a ``meta`` table naming the language and the input, and ``PRAGMA user_version = 1``. The bridge refuses a file whose version or language does not match what it was asked for, so a renamed file never shows French under Japanese.

The input is a Wiktextract JSONL dump from kaikki.org: either the postprocessed English edition (``kaikki.org-dictionary-English.jsonl.gz``) or the raw dump (``raw-wiktextract-data.jsonl.gz``), where only rows with ``lang_code == "en"`` are English entries. Wiktextract now places most translation rows under ``senses[].translations`` (a sample of the 2026-09-02 dump has six times as many there as in the deprecated top-level ``translations``, and no row in both), so both are read: sense rows first, in the page's sense order, then the top-level ones. The same table attached to several senses is kept once, since rows are grouped by their ``sense`` string anyway. Mandarin rows are ``lang == "Chinese Mandarin"``; the ``lang == "Chinese"`` rows are Hokkien, Dungan and other topolects. The last ``/`` segment of a Mandarin form is the simplified one (``"空閒 /空閑 /空闲"``).

Nothing is downloaded here. ``filter`` reduces a dump to the rows this build reads, which is the file worth keeping because kaikki overwrites its URLs every week; ``build`` accepts either the dump or that reduction, since the filter is idempotent.

The output is deterministic for the same input on the same SQLite: rows are inserted in key order into a fixed page size and the file is vacuumed. The SQLite version is written into ``meta`` because the file header records the library that last wrote it, so a different SQLite can produce different bytes from the same rows.

Wiktionary text is CC BY-SA 4.0; ``build`` writes ``offline-glosses-NOTICE.txt`` beside the databases and every row keeps the English page titles it came from.

usage:
  build_offline_glosses.py filter --input <dump.jsonl[.gz]> --out <filtered.jsonl[.gz]>
  build_offline_glosses.py build --input <dump-or-filtered> --out <directory> [--lang fr,ja,es,ru,de,ko]
      [--vocabulary msime-pinyin.db] [--frequency msime-english.db] [--dump-date YYYY-MM-DD] [--source-revision <wiktextract commit>]
      [--max-bytes N]
"""
import argparse
import gzip
import json
import re
import sqlite3
import sys
import unicodedata
from collections import defaultdict
from pathlib import Path

LANGUAGES = ("fr", "ja", "es", "ru", "de", "ko")
MANDARIN = "Chinese Mandarin"
SCHEMA_VERSION = 1
PAGE_SIZE = 4096
# The same bounds the display applies (candidate_gloss_display keeps two senses) and the learned glosses apply to a candidate (40 characters).
MAX_SENSES = 2
MAX_WORDS_PER_SENSE = 2
MAX_MANDARIN_PER_SENSE = 2
MAX_KEY_CHARS = 40
MAX_GLOSS_CHARS = 40
DEFAULT_MAX_BYTES = 4 << 20
# Forms nobody types or expects as the everyday rendering are dropped; register and regional variants are kept but ranked after the plain ones.
DROPPED_TAGS = {
    "archaic", "obsolete", "rare", "dated", "poetic", "literary", "uncommon", "Classical-Chinese",
    "historical", "nonstandard", "proscribed", "misspelling", "dialectal", "vulgar", "offensive",
    "derogatory", "slur", "euphemistic",
}
DEMOTED_TAGS = {"informal", "colloquial", "slang", "alternative", "familiar", "humorous"}
# Spanish from Spain and unmarked forms first; region-tagged forms (Mexico, Rioplatense, …) after them.
PREFERRED_REGIONS = {"Spain", "France", "Germany", "Russia", "Japan", "South-Korea"}
REGIONAL_TAG = re.compile(r"^[A-Z]")
PARENTHETICAL = re.compile(r"\s*\([^()]*\)\s*")
GENDER_TOKEN = re.compile(r"(?<=\S) (?:[mfn]|pl|m pl|f pl)(?= |$)")
STRESS_MARKS = {"\u0301", "\u0300"}


def open_text(path: Path):
    return gzip.open(path, "rt", encoding="utf-8") if path.suffix == ".gz" else path.open(encoding="utf-8")


def entries(path: Path):
    with open_text(path) as handle:
        for number, line in enumerate(handle, 1):
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError as error:
                raise SystemExit(f"{path}:{number}: not JSON ({error})")
            if entry.get("lang_code") == "en":
                yield entry


def translation_rows(entry: dict) -> list[dict]:
    groups = [sense.get("translations") for sense in entry.get("senses") or [] if isinstance(sense, dict)]
    groups.append(entry.get("translations"))
    rows, seen = [], set()
    for group in groups:
        if not isinstance(group, list):
            continue
        for row in group:
            if not isinstance(row, dict):
                continue
            tags = row.get("tags")
            identity = (row.get("lang"), row.get("code"), row.get("sense"), row.get("word"), tuple(tags) if isinstance(tags, list) else ())
            if identity not in seen:
                seen.add(identity)
                rows.append(row)
    return rows


def wanted_row(row: dict, languages) -> bool:
    return isinstance(row, dict) and (row.get("lang") == MANDARIN or row.get("code") in languages)


def reduce(entry: dict, languages) -> dict | None:
    rows = [
        {key: row[key] for key in ("lang", "code", "sense", "word", "tags") if key in row}
        for row in translation_rows(entry)
        if wanted_row(row, languages)
    ]
    if not any(row.get("lang") == MANDARIN for row in rows) or not any(row.get("code") in languages for row in rows):
        return None
    return {"word": entry.get("word", ""), "pos": entry.get("pos", ""), "lang_code": "en", "translations": rows}


def filter_command(arguments) -> int:
    kept = 0
    with arguments.out.open("wb") as raw:
        # mtime=0 and no file name so a gzip output is byte-identical across runs.
        output = gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) if arguments.out.suffix == ".gz" else raw
        with output:
            for entry in entries(arguments.input):
                reduced = reduce(entry, LANGUAGES)
                if reduced is None:
                    continue
                output.write((json.dumps(reduced, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8"))
                kept += 1
    if kept == 0:
        raise SystemExit("no English entry carries Mandarin and target translations; the source layout changed")
    print(f"{kept} entries -> {arguments.out}")
    return 0


def is_han(character: str) -> bool:
    return unicodedata.name(character, "").startswith(("CJK UNIFIED IDEOGRAPH", "CJK COMPATIBILITY IDEOGRAPH"))


def mandarin_keys(form: str, vocabulary) -> list[str]:
    simplified = form.split("/")[-1].strip()
    if not simplified or len(simplified) > MAX_KEY_CHARS or not all(is_han(ch) for ch in simplified):
        return []
    keys = []
    # Without a vocabulary there is nothing to tell a traditional-only form from a simplified one, so every well-formed key is kept; with one, keys the input method never offers are dropped and the adjective form 自由的 also answers for 自由.
    if vocabulary is None or simplified in vocabulary:
        keys.append(simplified)
    if vocabulary is not None and simplified.endswith("的") and len(simplified) > 2 and simplified[:-1] in vocabulary:
        keys.append(simplified[:-1])
    return keys


def clean_target(word: str, language: str) -> str:
    text = unicodedata.normalize("NFC", word).strip()
    if language == "ru":
        text = unicodedata.normalize("NFC", "".join(ch for ch in unicodedata.normalize("NFD", text) if ch not in STRESS_MARKS))
    if language == "ko":
        # 예약(豫約)하다 -> 예약하다: the hanja in parentheses is a reading aid, not part of the word.
        text = PARENTHETICAL.sub("", text)
    text = GENDER_TOKEN.sub("", text)
    text = " ".join(text.split())
    # candidate_gloss_display splits senses on semicolons, so a form containing one would be shown as two.
    if (not text or len(text) > MAX_GLOSS_CHARS or ";" in text or "；" in text
            or any(unicodedata.category(ch).startswith("C") for ch in text)):
        return ""
    return text


def rank(row: dict) -> int | None:
    tags = row.get("tags") or []
    if any(tag in DROPPED_TAGS for tag in tags):
        return None
    regional = any(REGIONAL_TAG.match(tag) and tag not in PREFERRED_REGIONS for tag in tags)
    return (1 if any(tag in DEMOTED_TAGS for tag in tags) else 0) + (2 if regional else 0)


def ranked_forms(rows):
    """The forms of the best tier present, in table order: an informal or regional form is used only when the table has nothing plainer."""
    forms = []
    for position, row in enumerate(rows):
        order = rank(row)
        word = row.get("word")
        if order is not None and isinstance(word, str) and word.strip():
            forms.append((order, position, word))
    best = min((order for order, _, _ in forms), default=None)
    return [word for order, _, word in sorted(forms) if order == best]


def english_weights(path: Path | None) -> dict[str, int]:
    if path is None:
        return {}
    with sqlite3.connect(f"file:{path}?mode=ro", uri=True) as database:
        return {word: weight for word, weight in database.execute("SELECT lower(word), MAX(weight) FROM english_words GROUP BY lower(word)")}


def input_vocabulary(path: Path | None) -> set[str] | None:
    if path is None:
        return None
    words = set()
    with sqlite3.connect(f"file:{path}?mode=ro", uri=True) as database:
        tables = [name for (name,) in database.execute("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'tbl\\_%' ESCAPE '\\' ORDER BY name")]
        for table in tables:
            words.update(value for (value,) in database.execute(f'SELECT value FROM "{table}"') if isinstance(value, str))
    if not words:
        raise SystemExit(f"{path}: no tbl_* word tables; not an msime-pinyin.db")
    return words


def collect(path: Path, languages, vocabulary, weights):
    """For each language, key -> [(sort key, gloss, English page)]."""
    found = {language: defaultdict(list) for language in languages}
    for sequence, entry in enumerate(entries(path)):
        page = entry.get("word", "")
        if not isinstance(page, str) or not page:
            continue
        weight = weights.get(page.lower(), 0)
        tables: dict[str, list[dict]] = {}
        for row in translation_rows(entry):
            if isinstance(row.get("sense"), str) and row["sense"]:
                tables.setdefault(row["sense"], []).append(row)
        for table_index, rows in enumerate(tables.values()):
            mandarin = ranked_forms(row for row in rows if row.get("lang") == MANDARIN)[:MAX_MANDARIN_PER_SENSE]
            keys = list(dict.fromkeys(key for form in mandarin for key in mandarin_keys(form, vocabulary)))
            if not keys:
                continue
            for language in languages:
                targets = []
                for form in ranked_forms(row for row in rows if row.get("code") == language):
                    text = clean_target(form, language)
                    if text and text not in targets:
                        targets.append(text)
                    if len(targets) == MAX_WORDS_PER_SENSE:
                        break
                if not targets:
                    continue
                gloss = ", ".join(targets)
                # A page's leading sense beats a minor sense of a more common word (呕吐 is "vomir" from vomit before the slang of cat's "vomit" sense); among equal positions the more frequent English word wins (天 is "day" before "sky").
                order = (table_index, -weight, page, sequence)
                for key in keys:
                    # A single character is mostly a bound morpheme; reached only from an English word the frequency list does not know, it is a rare reading (回 from gyrus, 里 from li), not what the candidate means.
                    if len(key) == 1 and weights and not weight:
                        continue
                    found[language][key].append((order, gloss, page))
    return found


def select(candidates):
    """Up to two senses; a word already shown is not repeated, so 猫 reads "chat, chatte; félin" and a sense adding nothing new is skipped."""
    glosses, pages, shown = [], [], set()
    for _, gloss, page in sorted(candidates):
        words = [word for word in gloss.split(", ") if word not in shown]
        if not words:
            continue
        shown.update(words)
        glosses.append(", ".join(words))
        pages.append(page)
        if len(glosses) == MAX_SENSES:
            break
    return "; ".join(glosses), "; ".join(pages)


def write_database(path: Path, language: str, rows, meta: dict) -> None:
    path.unlink(missing_ok=True)
    database = sqlite3.connect(path)
    try:
        database.execute(f"PRAGMA page_size = {PAGE_SIZE}")
        database.execute("PRAGMA journal_mode = DELETE")
        database.execute("CREATE TABLE zh_glosses(chinese TEXT COLLATE BINARY PRIMARY KEY, gloss TEXT NOT NULL, source TEXT NOT NULL) WITHOUT ROWID")
        database.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID")
        database.executemany("INSERT INTO zh_glosses VALUES (?, ?, ?)", rows)
        database.executemany("INSERT INTO meta VALUES (?, ?)", sorted({**meta, "target_language": language, "key_count": str(len(rows))}.items()))
        database.execute(f"PRAGMA user_version = {SCHEMA_VERSION}")
        database.commit()
        database.execute("VACUUM")
    finally:
        database.close()


NOTICE = """Offline candidate glosses (zh-fr.db, zh-ja.db, zh-es.db, zh-ru.db, zh-de.db, zh-ko.db)

These files contain translations taken from the English Wiktionary (https://en.wiktionary.org/), by Wiktionary contributors, as extracted by Wiktextract (https://github.com/tatuylonen/wiktextract) and published by kaikki.org (https://kaikki.org/dictionary/English/). The English page each row came from is recorded in its "source" column; the page is https://en.wiktionary.org/wiki/<source>.

Wiktionary edition: English
Wiktextract dump date: {dump_date}
Wiktextract revision: {source_revision}

Changes: only Mandarin and {languages} rows of the same translation table were paired; archaic, rare, dialectal and similar forms were removed; Russian stress marks, Korean hanja annotations and stray gender markers were removed; single characters reached only from English words outside the frequency list were dropped; each Chinese word keeps at most two senses of at most two words each; the result was converted to SQLite.

These files are licensed under the Creative Commons Attribution-ShareAlike 4.0 International License (https://creativecommons.org/licenses/by-sa/4.0/), the license of the Wiktionary text they adapt. They are provided as is, without warranties of any kind.
"""


def build_command(arguments) -> int:
    languages = tuple(dict.fromkeys(language.strip() for language in arguments.lang.split(",") if language.strip()))
    unknown = [language for language in languages if language not in LANGUAGES]
    if not languages or unknown:
        raise SystemExit(f"--lang takes a subset of {','.join(LANGUAGES)}")
    vocabulary = input_vocabulary(arguments.vocabulary)
    weights = english_weights(arguments.frequency)
    found = collect(arguments.input, languages, vocabulary, weights)
    arguments.out.mkdir(parents=True, exist_ok=True)
    meta = {
        "source": "https://kaikki.org/dictionary/English/",
        "edition": "en",
        "license": "CC-BY-SA-4.0",
        "dump_date": arguments.dump_date,
        "source_revision": arguments.source_revision,
        "sqlite_version": sqlite3.sqlite_version,
        "vocabulary": "msime-pinyin.db" if vocabulary is not None else "none",
    }
    for language in languages:
        rows = [(key, *select(candidates)) for key, candidates in sorted(found[language].items())]
        if not rows:
            raise SystemExit(f"no {language} glosses; the source layout changed")
        path = arguments.out / f"zh-{language}.db"
        write_database(path, language, rows, meta)
        size = path.stat().st_size
        if size > arguments.max_bytes:
            raise SystemExit(f"{path}: {size} bytes is over the {arguments.max_bytes} byte limit")
        print(f"zh-{language}.db {len(rows):7} keys {size:9} bytes")
    notice = NOTICE.format(dump_date=arguments.dump_date, source_revision=arguments.source_revision, languages="/".join(languages))
    (arguments.out / "offline-glosses-NOTICE.txt").write_text(notice, encoding="utf-8")
    return 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    filtering = commands.add_parser("filter")
    filtering.add_argument("--input", type=Path, required=True)
    filtering.add_argument("--out", type=Path, required=True)
    building = commands.add_parser("build")
    building.add_argument("--input", type=Path, required=True)
    building.add_argument("--out", type=Path, required=True)
    building.add_argument("--lang", default=",".join(LANGUAGES))
    building.add_argument("--vocabulary", type=Path)
    building.add_argument("--frequency", type=Path)
    building.add_argument("--dump-date", default="unknown")
    building.add_argument("--source-revision", default="unknown")
    building.add_argument("--max-bytes", type=int, default=DEFAULT_MAX_BYTES)
    arguments = parser.parse_args(argv)
    return filter_command(arguments) if arguments.command == "filter" else build_command(arguments)


if __name__ == "__main__":
    sys.exit(main())
