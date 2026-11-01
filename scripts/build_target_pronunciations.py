#!/usr/bin/env python3
"""Build the offline readings shown under the non-English candidate gloss lines (fr, ja, es, ru, de, ko).

``en-phonetic.db`` reads the English gloss line; this builds the same kind of table for each of the other gloss languages, from the same source as the glosses themselves: the English Wiktionary, extracted by Wiktextract and published by kaikki.org. The offline glosses (``build_offline_glosses.py``) come from the translation tables of the English entries, which carry no pronunciation; the readings come from the entries of the target-language words in the same edition, which kaikki.org publishes as one JSONL dump per language (``kaikki.org-dictionary-French.jsonl.gz`` and so on), in the same format as the English dump the glosses were built from.

A reading is what a learner of that language is taught to say the word with:

- French, Spanish, German, Russian and Korean use the IPA in ``sounds[].ipa``. A phonemic transcription between slashes is preferred to a phonetic one in brackets (Spanish has both; Russian and Korean have only the bracketed one), and an untagged transcription to a regional or register variant. The transcription is kept as Wiktionary writes it, delimiters included.
- Japanese uses the Hepburn romanization in ``forms[]`` (the form tagged ``romanization``). The dump's ``sounds`` has a narrow IPA (``[ne̞ko̞]``), which a Japanese learner is never taught, and the kana reading (``sounds[].other``). Romaji is what the macOS and Windows hosts already draw under a Japanese gloss line from the system tokenizer, so a host without one shows the same kind of reading; and unlike kana it also reads the katakana and hiragana lines, which are most of the Japanese glosses.

A word several entries spell the same (French ``fils`` the son and the threads) takes the reading of the first entry of the most common part of speech, in dump order; a word whose entries disagree is still read, because the gloss line is the common sense far more often than not.

The output is one SQLite file per language, ``<lang>-phonetic.db``, with ``<lang>_phonetics(word, phonetic)`` keyed by the word exactly as Wiktionary spells it (``Katze``, ``кот``, ``猫``), a ``meta`` table naming the kind (``<lang>_phonetic``) and the input, and ``PRAGMA user_version = 1``: the same layout as ``en-phonetic.db``, so the bridge refuses a file whose version or kind does not match what it was asked for. Rows are inserted in key order into a fixed page size and the file is vacuumed, so the same input on the same SQLite gives the same bytes.

The tables keep only the words the offline glosses use (``--glosses``, the ``zh-<lang>.db`` files pinned by ``resources/offline-glosses.lock.json``): every term of every gloss, and its lowercase. The hosts read the first term of a gloss line, and almost every line they draw comes from those files, so the whole lemma list (about 13 MB for the six languages) would mostly carry words no line shows; restricted, the six tables are under 4 MB. A line from an online translation is still read when its first term is one of these words. When the glosses are rebuilt from a newer dump, these tables are rebuilt from the same week's dumps.

Nothing is downloaded here. ``filter`` reduces the six dumps (about 420 MB) to the fields this build reads, which is the file worth keeping because kaikki.org overwrites its URLs every week; ``build`` reads that reduction.

Wiktionary text is CC BY-SA 4.0; ``build`` writes ``target-pronunciations-NOTICE.txt`` beside the databases.

usage:
  build_target_pronunciations.py filter --input <kaikki.org-dictionary-<Language>.jsonl.gz> [--input ...] --out <filtered.jsonl.gz>
  build_target_pronunciations.py build --input <filtered.jsonl[.gz]> --out <directory> [--glosses <offline-glosses directory>] [--lang fr,ja,es,ru,de,ko]
      [--dump-date YYYY-MM-DD] [--source-revision <wiktextract commit>] [--max-bytes N] [--min-words N]
"""
import argparse
import gzip
import json
import re
import sqlite3
import sys
import unicodedata
from pathlib import Path

LANGUAGES = ("fr", "ja", "es", "ru", "de", "ko")
SCHEMA_VERSION = 1
PAGE_SIZE = 4096
MAX_WORD_CHARS = 40
MAX_PHONETIC_CHARS = 60
DEFAULT_MAX_BYTES = 4 << 20
NOTICE_NAME = "target-pronunciations-NOTICE.txt"

# Parts of speech in the order a gloss word is most likely to be: a gloss is a noun, verb or adjective far more often than anything else. Anything not listed ranks after these, in dump order.
POS_ORDER = ("noun", "verb", "adj", "adv", "phrase", "intj", "pron", "num", "prep", "conj", "det", "particle", "name")
# Entries that are not a word a gloss teaches: a romanization page, a letter, a symbol, a bare affix.
SKIPPED_POS = {"romanization", "character", "symbol", "suffix", "prefix", "infix", "interfix", "circumfix", "affix", "soft-redirect", "punct", "combining_form"}
# A transcription with one of these tags is not the everyday pronunciation.
DROPPED_TAGS = {
    "archaic", "obsolete", "dated", "rare", "dialectal", "nonstandard", "proscribed", "poetic", "literary",
    "historical", "Middle", "Old", "Early",
}
# The standard pronunciation each language's learners are taught; any other region-tagged transcription ranks after untagged ones.
STANDARD_REGIONS = {
    "fr": {"France", "Paris", "Standard"},
    "es": {"Spain", "Castilian", "Standard", "Latin-America"},
    "de": {"Germany", "Standard", "Standard-German"},
    "ru": {"Standard"},
    "ko": {"Seoul", "Standard", "SK-Standard"},
    "ja": {"Tokyo", "Standard"},
}
REGIONAL_TAG = re.compile(r"^[A-Z]")
# Characters a reading may hold besides letters (any script's letters are fine; IPA uses Latin, Greek and IPA-extension letters and modifier letters). Anything else (digits, CJK, notes, templates left over) rejects the reading rather than show it.
READING_MARKS = set(" .ˈˌːˑ‿-‐()[]/|‖ʼ'ʔ̃͜͡")


def open_text(path: Path):
    return gzip.open(path, "rt", encoding="utf-8") if path.suffix == ".gz" else path.open(encoding="utf-8")


def entries(path: Path):
    with open_text(path) as handle:
        for number, line in enumerate(handle, 1):
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError as error:
                raise SystemExit(f"{path}:{number}: not JSON ({error})")


def reduce(entry: dict) -> dict | None:
    """The fields the build reads, or None for an entry it never uses."""
    language = entry.get("lang_code")
    word = entry.get("word")
    if language not in LANGUAGES or not isinstance(word, str) or not word.strip():
        return None
    sounds = []
    for sound in entry.get("sounds") or []:
        if not isinstance(sound, dict):
            continue
        kept = {key: sound[key] for key in ("ipa", "other", "tags") if key in sound}
        if "ipa" in kept or "other" in kept:
            sounds.append(kept)
    romanizations = [
        form["form"]
        for form in entry.get("forms") or []
        if language == "ja" and isinstance(form, dict) and isinstance(form.get("form"), str) and form.get("tags") == ["romanization"]
    ]
    if not sounds and not romanizations:
        return None
    reduced = {"word": word, "lang_code": language, "pos": entry.get("pos", ""), "sounds": sounds}
    senses = [sense for sense in entry.get("senses") or [] if isinstance(sense, dict)]
    # An inflected form (Russian кошки, French chattes) is its own entry with its own IPA; a gloss names the lemma, so these are marked and left out of the build.
    if senses and all("form_of" in sense or "form-of" in (sense.get("tags") or []) for sense in senses):
        reduced["form_of"] = True
    if romanizations:
        reduced["romanizations"] = romanizations
    return reduced


def filter_command(arguments) -> int:
    kept = {language: 0 for language in LANGUAGES}
    with arguments.out.open("wb") as raw:
        # mtime=0 and no file name so a gzip output is byte-identical across runs.
        output = gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) if arguments.out.suffix == ".gz" else raw
        with output:
            for path in arguments.input:
                for entry in entries(path):
                    reduced = reduce(entry)
                    if reduced is None:
                        continue
                    output.write((json.dumps(reduced, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8"))
                    kept[reduced["lang_code"]] += 1
    for language, count in kept.items():
        print(f"{language} {count:8} entries")
    if not any(kept.values()):
        raise SystemExit("no entry of fr/ja/es/ru/de/ko carries a pronunciation; the source layout changed")
    print(f"-> {arguments.out}")
    return 0


def usable_word(word: str) -> bool:
    return (
        0 < len(word) <= MAX_WORD_CHARS
        and word == word.strip()
        and not any(unicodedata.category(character)[0] in "CZ" and character != " " for character in word)
        and ";" not in word
    )


def clean_ipa(raw: str) -> str | None:
    """A transcription as Wiktionary writes it, or None when it is not one transcription of plain IPA."""
    text = unicodedata.normalize("NFC", " ".join(raw.split()))
    if not text or len(text) > MAX_PHONETIC_CHARS:
        return None
    # Two transcriptions in one cell ("/ʃa/, /ʃat/") or a note ("/ʃa/ (Quebec)") is not one reading.
    if "," in text or ";" in text or "~" in text:
        return None
    opening, closing = text[0], text[-1]
    if (opening, closing) not in (("/", "/"), ("[", "]")) or len(text) < 3:
        return None
    inner = text[1:-1]
    if "/" in inner or "[" in inner or "]" in inner or inner.count("(") != inner.count(")"):
        return None
    for character in inner:
        category = unicodedata.category(character)
        if category[0] == "L" and not _han_or_kana(character):
            continue
        if category in ("Mn", "Lm", "Sk") or character in READING_MARKS:
            continue
        return None
    return text


def _han_or_kana(character: str) -> bool:
    name = unicodedata.name(character, "")
    return name.startswith(("CJK", "HIRAGANA", "KATAKANA", "HANGUL"))


def clean_romaji(raw: str) -> str | None:
    """A Hepburn romanization (``neko``, ``ōkii``, ``konnichi wa``), or None for anything else."""
    text = unicodedata.normalize("NFC", " ".join(raw.split()))
    if not text or len(text) > MAX_PHONETIC_CHARS:
        return None
    for character in text:
        if character in " -'’.":
            continue
        base = unicodedata.normalize("NFD", character)[0]
        if not ("a" <= base.lower() <= "z"):
            return None
    return text if any(character.isalpha() for character in text) else None


def ipa_rank(sound: dict, language: str) -> int | None:
    tags = sound.get("tags") or []
    if not isinstance(tags, list) or any(tag in DROPPED_TAGS for tag in tags):
        return None
    regional = [tag for tag in tags if isinstance(tag, str) and REGIONAL_TAG.match(tag)]
    standard = STANDARD_REGIONS.get(language, set())
    region = 0 if not regional else 1 if any(tag in standard for tag in regional) else 2
    phonetic = 1 if str(sound.get("ipa", "")).startswith("[") else 0
    return region * 2 + phonetic


def entry_reading(entry: dict, language: str) -> str | None:
    if language == "ja":
        for form in entry.get("romanizations") or []:
            reading = clean_romaji(form)
            if reading:
                return reading
        return None
    ranked = []
    for position, sound in enumerate(entry.get("sounds") or []):
        if not isinstance(sound, dict) or not isinstance(sound.get("ipa"), str):
            continue
        order = ipa_rank(sound, language)
        reading = clean_ipa(sound["ipa"])
        if order is not None and reading:
            ranked.append((order, position, reading))
    return min(ranked)[2] if ranked else None


def is_kana(text: str) -> bool:
    return bool(text) and all("\u3041" <= character <= "\u309f" or "\u30a1" <= character <= "\u30fa" or character == "ー" for character in text)


def hiragana(text: str) -> str:
    """Katakana as hiragana (リンゴ -> りんご); the long-vowel mark and hiragana are kept."""
    return "".join(chr(ord(character) - 0x60) if "\u30a1" <= character <= "\u30f6" else character for character in text)


def pos_rank(pos: str) -> int:
    return POS_ORDER.index(pos) if pos in POS_ORDER else len(POS_ORDER)


def collect(path: Path, languages) -> dict[str, dict[str, str]]:
    """For each language, word -> reading."""
    best: dict[str, dict[str, tuple]] = {language: {} for language in languages}
    for sequence, entry in enumerate(entries(path)):
        language = entry.get("lang_code")
        word = entry.get("word")
        if language not in best or not isinstance(word, str):
            continue
        pos = entry.get("pos", "")
        if pos in SKIPPED_POS or entry.get("form_of"):
            continue
        word = unicodedata.normalize("NFC", word)
        if not usable_word(word):
            continue
        reading = entry_reading(entry, language)
        if not reading:
            continue
        keys = [((0, pos_rank(pos), sequence), word)]
        # A Japanese entry also gives the reading of its kana spelling (林檎 is りんご, ringo), ranked after every headword, so a gloss written in kana (リンゴ, りんご) is read too: the kana-only entries mostly point at the kanji one and carry no pronunciation of their own.
        if language == "ja":
            kana = {sound["other"] for sound in entry.get("sounds") or [] if isinstance(sound.get("other"), str) and is_kana(sound["other"])}
            if len(kana) == 1:
                keys.append(((1, pos_rank(pos), sequence), hiragana(kana.pop())))
        for order, key in keys:
            current = best[language].get(key)
            if current is None or order < current[0]:
                best[language][key] = (order, reading)
    return {language: {word: reading for word, (_, reading) in found.items()} for language, found in best.items()}


GLOSS_SEPARATORS = re.compile(r"[;；,，、/]")


def gloss_vocabulary(directory: Path, language: str) -> set[str]:
    """Every term of every gloss in ``zh-<language>.db``, and its lowercase."""
    path = directory / f"zh-{language}.db"
    if not path.is_file():
        raise SystemExit(f"{path}: missing; --glosses takes the directory scripts/fetch_offline_glosses.py writes")
    terms = set()
    with sqlite3.connect(f"file:{path}?mode=ro", uri=True) as database:
        stored = database.execute("SELECT value FROM meta WHERE key = 'target_language'").fetchone()
        if stored != (language,):
            raise SystemExit(f"{path}: not the {language} glosses")
        for (gloss,) in database.execute("SELECT gloss FROM zh_glosses"):
            for term in GLOSS_SEPARATORS.split(gloss):
                term = unicodedata.normalize("NFC", term.strip())
                if term:
                    terms.add(term)
                    terms.add(term.lower())
    return terms


def write_database(path: Path, language: str, rows: dict[str, str], meta: dict) -> None:
    path.unlink(missing_ok=True)
    database = sqlite3.connect(path)
    try:
        database.execute(f"PRAGMA page_size = {PAGE_SIZE}")
        database.execute("PRAGMA journal_mode = DELETE")
        database.execute(f"CREATE TABLE {language}_phonetics(word TEXT PRIMARY KEY, phonetic TEXT NOT NULL) WITHOUT ROWID")
        database.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID")
        database.executemany(f"INSERT INTO {language}_phonetics VALUES (?, ?)", sorted(rows.items()))
        values = {**meta, "kind": f"{language}_phonetic", "language": language, "key_count": str(len(rows))}
        database.executemany("INSERT INTO meta VALUES (?, ?)", sorted(values.items()))
        database.execute(f"PRAGMA user_version = {SCHEMA_VERSION}")
        database.commit()
        database.execute("VACUUM")
    finally:
        database.close()


NOTICE = """Candidate gloss readings ({files})

These files contain pronunciations taken from the English Wiktionary (https://en.wiktionary.org/), by Wiktionary contributors, as extracted by Wiktextract (https://github.com/tatuylonen/wiktextract) and published by kaikki.org (https://kaikki.org/dictionary/). Each row comes from the Wiktionary page of its word, https://en.wiktionary.org/wiki/<word>; a Japanese word written in kana takes the reading of the entry whose kana spelling it is (リンゴ from 林檎).

Wiktionary edition: English
Wiktextract dump date: {dump_date}
Wiktextract revision: {source_revision}

Changes: only the IPA transcriptions (French, Spanish, German, Russian, Korean) and the Hepburn romanization (Japanese) of each word were kept, one per word; archaic, dialectal and similar transcriptions and transcriptions holding more than one variant were removed; inflected forms were removed and only the words used by the offline candidate glosses were kept; the result was converted to SQLite.

These files are licensed under the Creative Commons Attribution-ShareAlike 4.0 International License (https://creativecommons.org/licenses/by-sa/4.0/), the license of the Wiktionary text they adapt. They are provided as is, without warranties of any kind.
"""


def gloss_dump_date(directory: Path) -> str:
    """The dump the offline glosses in ``directory`` were built from, as their ``meta`` records it."""
    dates = set()
    for path in sorted(directory.glob("zh-*.db")):
        with sqlite3.connect(f"file:{path}?mode=ro", uri=True) as database:
            row = database.execute("SELECT value FROM meta WHERE key = 'dump_date'").fetchone()
            dates.add(row[0] if row else "unknown")
    if len(dates) != 1:
        raise SystemExit(f"{directory}: the gloss files come from {len(dates)} dumps")
    return dates.pop()


def build_command(arguments) -> int:
    languages = tuple(dict.fromkeys(language.strip() for language in arguments.lang.split(",") if language.strip()))
    unknown = [language for language in languages if language not in LANGUAGES]
    if not languages or unknown:
        raise SystemExit(f"--lang takes a subset of {','.join(LANGUAGES)}")
    found = collect(arguments.input, languages)
    arguments.out.mkdir(parents=True, exist_ok=True)
    meta = {
        "source": "https://kaikki.org/dictionary/",
        "edition": "en",
        "license": "CC-BY-SA-4.0",
        "dump_date": arguments.dump_date,
        "source_revision": arguments.source_revision,
        "sqlite_version": sqlite3.sqlite_version,
    }
    if arguments.glosses is not None:
        meta["vocabulary"] = "offline-glosses " + gloss_dump_date(arguments.glosses)
    for language in languages:
        rows = found[language]
        if arguments.glosses is not None:
            vocabulary = gloss_vocabulary(arguments.glosses, language)
            restricted = {word: reading for word, reading in rows.items() if word in vocabulary}
            if language == "ja":
                for term in vocabulary:
                    if term not in restricted and is_kana(term) and hiragana(term) in rows:
                        restricted[term] = rows[hiragana(term)]
            rows = restricted
        if len(rows) < arguments.min_words:
            raise SystemExit(f"only {len(rows)} {language} readings; the source layout changed")
        path = arguments.out / f"{language}-phonetic.db"
        write_database(path, language, rows, meta)
        size = path.stat().st_size
        if size > arguments.max_bytes:
            raise SystemExit(f"{path}: {size} bytes is over the {arguments.max_bytes} byte limit")
        print(f"{language}-phonetic.db {len(rows):7} words {size:9} bytes")
    files = ", ".join(f"{language}-phonetic.db" for language in languages)
    notice = NOTICE.format(files=files, dump_date=arguments.dump_date, source_revision=arguments.source_revision)
    (arguments.out / NOTICE_NAME).write_text(notice, encoding="utf-8")
    return 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    filtering = commands.add_parser("filter")
    filtering.add_argument("--input", type=Path, action="append", required=True)
    filtering.add_argument("--out", type=Path, required=True)
    building = commands.add_parser("build")
    building.add_argument("--input", type=Path, required=True)
    building.add_argument("--out", type=Path, required=True)
    building.add_argument("--glosses", type=Path)
    building.add_argument("--lang", default=",".join(LANGUAGES))
    building.add_argument("--dump-date", default="unknown")
    building.add_argument("--source-revision", default="unknown")
    building.add_argument("--max-bytes", type=int, default=DEFAULT_MAX_BYTES)
    # A real dump gives every language over ten thousand words; fewer means the dump's layout changed. The offline test lowers it for its few rows.
    building.add_argument("--min-words", type=int, default=1000)
    arguments = parser.parse_args(argv)
    return filter_command(arguments) if arguments.command == "filter" else build_command(arguments)


if __name__ == "__main__":
    sys.exit(main())
