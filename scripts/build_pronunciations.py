#!/usr/bin/env python3
"""Build the offline English pronunciation table shown under candidate glosses.

The candidate gloss already tells a learner what a Chinese candidate means in English; this adds how
the English word is said. ``english.db`` carries no pronunciation — its build drops ECDICT's
``phonetic`` column — so the table is a sibling resource, the same way ``offline-glosses/`` is:
``resources/desktop-dictionary.lock.json`` is shared by all six platforms and
``ResourceStore::verify`` requires a resource directory to match it exactly.

ECDICT is MIT licensed and already pinned for the 背单词 wordbooks; ``resources/pronunciations.lock.json``
pins the same file, and the download is verified by ``fetch_wordbooks.fetch``.

ECDICT's ``phonetic`` cells are not uniform IPA. About 150 000 of them spell schwa with the Cyrillic
``ә`` (U+04D9), and most mark primary stress with an ASCII apostrophe, length with a colon and
secondary stress with a leading comma. ``clean_phonetic`` maps those to IPA, keeps only the first of
several variants, and drops a cell that still holds anything outside the IPA letters it knows —
the few dozen with Chinese notes, private-use characters or unbalanced brackets — rather than show
it.

The output is ``en-phonetic.db`` with ``en_phonetics(word, phonetic)`` keyed by the lowercase word,
a ``meta`` table naming the kind and the input, and ``PRAGMA user_version = 1``. The bridge refuses a
file whose version or kind does not match. Rows are inserted in key order into a fixed page size and
the file is vacuumed, so the same input on the same SQLite gives the same bytes.

usage: build_pronunciations.py [--out <directory>]   (default: target/pronunciations)
"""
import argparse
import csv
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True  # importing a sibling script must not leave scripts/__pycache__ behind
from fetch_wordbooks import fetch  # noqa: E402  (the verified ECDICT download)

import json  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/pronunciations.lock.json"
DEFAULT_OUT = ROOT / "target/pronunciations"
DATABASE = "en-phonetic.db"
NOTICE = "pronunciations-NOTICE.txt"
SCHEMA_VERSION = 1
PAGE_SIZE = 4096
MAX_WORD_CHARS = 40
MAX_PHONETIC_CHARS = 40

# ECDICT spellings that have an IPA equivalent. Anything else outside IPA_LETTERS rejects the cell.
REPLACEMENTS = {
    "ә": "ə",  # Cyrillic schwa -> IPA schwa
    "є": "ɛ",  # Cyrillic ukrainian ie -> open e
    "ε": "ɛ",  # Greek epsilon -> open e
    "ˊ": "ˈ",  # modifier acute -> primary stress
    "'": "ˈ",
    "’": "ˈ",
    ":": "ː",
    "‑": "-",
    "（": "(",
    "）": ")",
    "g": "ɡ",  # IPA uses script g; ECDICT mixes both
}
IPA_LETTERS = set("abcdefhijklmnopqrstuvwxyz") | set(
    "əɛæɔɪʃʌɑŋʒɒθʊ"
    "ˌˈɡːɜðɚɝɵɹɐʔ"
) | set("()-. ")


VOWELS = set("aeiouæɑɒɔəɜɪʊʌɛ")


def _stressed(word: str, index: int) -> bool:
    """Whether the vowel at ``index`` opens the syllable a stress mark (or the start of the word) points at."""
    start = max(word.rfind("ˈ", 0, index), word.rfind("ˌ", 0, index)) + 1
    return not any(character in VOWELS for character in word[start:index])


def _modernize_word(word: str) -> str:
    # NURSE and the old open-e spellings.
    word = word.replace("əː", "ɜː").replace("ɛə", "eə").replace("ɛ", "e")
    # ECDICT writes the THOUGHT vowel with the LOT letter (all ɒːl, more mɒː, water ˈwɒːtə); ɒ is only ever short, so
    # ɒː is always ɔː.
    word = word.replace("ɒː", "ɔː")
    # what hwɒt, why hwai: the older /hw/ of wh-words is /w/ in current British notation.
    stress = len(word) - len(word.lstrip("ˈˌ"))
    if word[stress:].startswith("hw"):
        word = word[:stress] + word[stress + 1 :]
    # Closing diphthongs: the second element is the lax vowel in current notation.
    for old, new in (("ei", "eɪ"), ("ai", "aɪ"), ("ɔi", "ɔɪ"), ("ɒi", "ɔɪ"), ("əu", "əʊ"), ("ou", "əʊ"), ("au", "aʊ")):
        word = word.replace(old, new)
    characters = list(word)
    for index, character in enumerate(characters):
        following = characters[index + 1] if index + 1 < len(characters) else ""
        after = characters[index + 2] if index + 2 < len(characters) else ""
        if character in "iu" and following == "ə" and after != "ʊ" and _stressed(word, index):
            # Centring diphthongs, in a stressed syllable only: here hiə -> hɪə, but media ˈmiːdiə keeps its i.
            characters[index] = "ɪ" if character == "i" else "ʊ"
        elif character in "iu" and following and following != "ː" and following not in VOWELS:
            # A short vowel before a consonant is the lax one; final and prevocalic i/u keep the happY convention.
            characters[index] = "ɪ" if character == "i" else "ʊ"
        elif character == "ɔ" and following not in ("ː", "ɪ"):
            characters[index] = "ɒ"
    return "".join(characters)


# Single-cell mistakes in ECDICT that no rule can tell apart from a correct cell: voiced th written voiceless, a dropped
# sound.
# Found by reading the 200 most frequent words and every th- function word; add to it the same way, by word.
CORRECTIONS = {
    "this": "ðɪs",
    "thou": "ðaʊ",
    "thither": "ˈðɪðə",
    "electric": "ɪˈlektrɪk",  # ECDICT: i'lektik
}


def _first_of_dotted_variants(text: str) -> str:
    """ECDICT's '.' is usually a syllable break (streetwalker ˈstriːt.wɔːkə) but sometimes separates two whole
    readings (live liv.laiv). A part after the dot that starts with the same sound as the cell is a second reading."""
    head = text.lstrip("ˈˌ")
    dot = head.find(".", 1)
    if dot > 0:
        rest = head[dot + 1 :].lstrip("ˈˌ")
        if rest[:1] and rest[:1] == head[:1]:
            return text[: len(text) - len(head) + dot]
    return text


def modernize(ipa: str) -> str:
    """ECDICT's older British notation (dei, həˈləu, bəːd, buk) in the notation current learner's dictionaries use
    (deɪ, həˈləʊ, bɜːd, bʊk). Text already in current notation is left as it is, so a cell mixing both is safe."""
    return " ".join(_modernize_word(word) for word in ipa.split(" "))


def clean_phonetic(raw: str) -> str | None:
    """The first variant of an ECDICT phonetic cell in modern IPA, or None when it is not one."""
    text = raw.strip()
    for separator in (";", "；", ", ", "，", ". "):
        text = text.split(separator, 1)[0]
    text = _first_of_dotted_variants(text.strip().strip("/[]").strip())
    if not text:
        return None
    # A comma that is not a variant separator is ECDICT's secondary-stress mark.
    text = text.replace(",", "ˌ")
    text = "".join(REPLACEMENTS.get(character, character) for character in text)
    if any(character not in IPA_LETTERS for character in text):
        return None
    if text.count("(") != text.count(")") or len(text) > MAX_PHONETIC_CHARS:
        return None
    return modernize(text)


def usable_word(word: str) -> bool:
    """A plain English word a gloss or an English candidate can be looked up by."""
    if not word or len(word) > MAX_WORD_CHARS or not word.isascii():
        return False
    if not (word[0].isalpha() and word[-1].isalpha()):
        return False
    return all(character.isalpha() or character in "-'" for character in word)


def collect(csv_path: Path) -> dict[str, str]:
    rows: dict[str, str] = {}
    exact: set[str] = set()
    csv.field_size_limit(1024 * 1024)
    with csv_path.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            word = (row.get("word") or "").strip()
            if not usable_word(word):
                continue
            phonetic = clean_phonetic(row.get("phonetic") or "")
            if not phonetic:
                continue
            key = word.lower()
            # "may" and "May" share a key; the lowercase headword is the common word.
            if key in rows and (key in exact or word != key):
                continue
            rows[key] = phonetic
            if word == key:
                exact.add(key)
    rows.update(CORRECTIONS)
    return rows


def write_database(path: Path, rows: dict[str, str], meta: dict) -> None:
    path.unlink(missing_ok=True)
    database = sqlite3.connect(path)
    try:
        database.execute(f"PRAGMA page_size = {PAGE_SIZE}")
        database.execute("PRAGMA journal_mode = DELETE")
        database.execute("CREATE TABLE en_phonetics(word TEXT PRIMARY KEY, phonetic TEXT NOT NULL) WITHOUT ROWID")
        database.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID")
        database.executemany("INSERT INTO en_phonetics VALUES (?, ?)", sorted(rows.items()))
        database.executemany("INSERT INTO meta VALUES (?, ?)", sorted({**meta, "key_count": str(len(rows))}.items()))
        database.execute(f"PRAGMA user_version = {SCHEMA_VERSION}")
        database.commit()
        database.execute("VACUUM")
    finally:
        database.close()


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    parser.add_argument("--csv", help="a local ecdict.csv; it is still checked against the lock")
    arguments = parser.parse_args(argv)

    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    out = Path(arguments.out)
    out.mkdir(parents=True, exist_ok=True)
    cache = Path(arguments.csv).parent if arguments.csv else out.parent / "wordbook-source"
    source = fetch(lock["artifact"], cache)
    # MIT asks for the copyright and permission notice to travel with the data, so the pinned
    # LICENSE is copied into the NOTICE verbatim rather than paraphrased.
    license_text = fetch(lock["license_file"], cache).read_text(encoding="utf-8")
    rows = collect(source)
    if len(rows) < 10000:
        raise SystemExit(f"only {len(rows)} usable pronunciations; the source layout changed")
    meta = {
        "kind": "en_phonetic",
        "source": lock["source"],
        "source_commit": lock["source_commit"],
        "license": lock["license"],
        "sqlite_version": sqlite3.sqlite_version,
    }
    write_database(out / DATABASE, rows, meta)
    (out / NOTICE).write_text(
        f"en-phonetic.db is derived from ECDICT ({lock['source']}), commit {lock['source_commit']}.\n"
        "Phonetic cells were normalised to IPA and only the first variant of each was kept.\n\n"
        + license_text,
        encoding="utf-8",
    )
    print(f"{len(rows)} pronunciations -> {out / DATABASE} ({(out / DATABASE).stat().st_size / 1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
