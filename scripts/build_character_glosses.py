#!/usr/bin/env python3
"""Build English glosses for single Chinese characters from Unihan's kDefinition.

english.db has no single-character keys on purpose: its Chinese-to-English side is built by reversing ECDICT, and a
single character turns up in the translations of so many English words that the reverse is noise (clean_ecdict.py
keeps 2 to 6 characters). A single character needs a source written per character, and Unihan's kDefinition is that:
an English definition for every common CJK character, versioned with Unicode and under the permissive Unicode License
v3.

The output has the same shape as an offline gloss file (scripts/build_offline_glosses.py), so the bridge's
candidate_target_glosses reads it unchanged: ``character-glosses/zh-en.db`` with ``zh_glosses(chinese, gloss,
source)``, ``meta.target_language = en`` and ``PRAGMA user_version = 1``. host-api asks it for a single-character
candidate english.db did not answer.

kDefinition mixes dictionary apparatus into the senses — "Kangxi radical 126", "surname", "5th lunar mansion",
"(simp. for 後)". ``clean_definition`` drops those, moves a sense that the simplified form took over from a traditional
character to the front (后: "behind, rear, after" before "queen, empress"), and keeps at most three words per sense.

usage: build_character_glosses.py [--out <directory>] [--zip <local Unihan.zip>]   (default: target/character-glosses)
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
from fetch_wordbooks import digest, fetch  # noqa: E402  (verified download)

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/character-glosses.lock.json"
DEFAULT_OUT = ROOT / "target/character-glosses"
DATABASE = "zh-en.db"
NOTICE = "character-glosses-NOTICE.txt"
SCHEMA_VERSION = 1
PAGE_SIZE = 4096
MAX_WORDS_PER_SENSE = 3
MAX_SENSES = 2

# Senses and words that describe the character rather than translate it.
APPARATUS = re.compile(
    r"^(kangxi radical|radical( number)?|surname|a surname|family name|numerary adjunct|classifier|"
    r"\d+(st|nd|rd|th) (lunar mansion|earthly branch|heavenly stem)|.*lunar mansion|determinative star|"
    r"(first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth|eleventh|twelfth) (earthly branch|heavenly stem)|"
    r"used in|used as|used for|interchangeable|variant|same as|ancient form|old form|simplified form|"
    r"phonetic|translit|transcription|name of|place name|proper name)",
    re.IGNORECASE,
)
# Characters whose Unihan definition leads with a sense modern Chinese rarely means. Most are simplified forms that
# merged traditional characters (几 = 几 + 幾, 里 = 里 + 裏) and are defined only in their own, older sense; no field
# in Unihan says which sense is common, so a rule that preferred the traditional sense made 了, 只 and 着 worse. Found
# by comparing each merged character's own and traditional senses; add to it the same way, by character.
CORRECTIONS = {
    "我": "I, me; we, us",
    "机": "machine; desk",
    "干": "dry; do, work",
    "几": "how many, several; small table",
    "里": "inside, within; unit of distance",
    "斗": "fight, struggle; a dipper",
    "谷": "valley; grain, cereal",
    "松": "pine; loose",
    "丑": "ugly; clown",
    "范": "pattern, model",
    "郁": "gloomy, depressed; fragrant",
    "征": "attack, conquer; levy, recruit",
    "叶": "leaf",
    "苹": "apple",
    "帘": "curtain, blind",
    "愿": "wish, desire; willing",
    "咸": "salty",
    "佣": "servant, hire; commission",
    "表": "show, express; watch",
    "困": "sleepy, tired; trapped",
}
SIMPLIFIED_NOTE = re.compile(r"\((?:simp\.|simplified)[^)]*\)", re.IGNORECASE)
NOTE = re.compile(r"\([^)]*\)")


def clean_definition(raw: str) -> str:
    """A Unihan kDefinition as a gloss line: at most two senses of at most three words, apparatus removed."""
    senses = []
    for index, sense in enumerate(raw.split(";")):
        took_over = bool(SIMPLIFIED_NOTE.search(sense))
        sense = NOTE.sub(" ", sense)
        words = []
        for word in sense.split(","):
            word = " ".join(word.split())
            if not word or APPARATUS.match(word) or any(ch.isdigit() for ch in word):
                continue
            word = re.sub(r"\bi\b", "I", word)
            if word.lower() not in (existing.lower() for existing in words):
                words.append(word)
        if words:
            # A sense the simplified form took over from a traditional character is its everyday meaning today.
            senses.append((0 if took_over else 1, index, ", ".join(words[:MAX_WORDS_PER_SENSE])))
    senses.sort()
    return "; ".join(text for _, _, text in senses[:MAX_SENSES])


def read_unihan(zip_path: Path) -> tuple[dict[str, str], dict[str, list[str]]]:
    definitions: dict[str, str] = {}
    traditional: dict[str, list[str]] = {}
    with zipfile.ZipFile(zip_path) as archive:
        for member in ("Unihan_Readings.txt", "Unihan_Variants.txt"):
            for line in archive.read(member).decode("utf-8").splitlines():
                if not line or line.startswith("#"):
                    continue
                codepoint, field, value = line.split("\t", 2)
                character = chr(int(codepoint[2:], 16))
                if field == "kDefinition":
                    definitions[character] = value
                elif field == "kTraditionalVariant":
                    traditional[character] = [chr(int(item[2:], 16)) for item in value.split()]
    return definitions, traditional


def collect(definitions: dict[str, str], traditional: dict[str, list[str]]) -> dict[str, str]:
    rows: dict[str, str] = {}
    for character in sorted(set(definitions) | set(traditional)):
        gloss = clean_definition(definitions.get(character, ""))
        if not gloss:
            # A simplified character Unihan defines only through its traditional form (涂 through 塗).
            gloss = next(
                (clean_definition(definitions[t]) for t in traditional.get(character, []) if t != character and t in definitions),
                "",
            )
        if gloss:
            rows[character] = gloss
    rows.update(CORRECTIONS)
    return rows


def write_database(path: Path, rows: dict[str, str], meta: dict) -> None:
    path.unlink(missing_ok=True)
    database = sqlite3.connect(path)
    try:
        database.execute(f"PRAGMA page_size = {PAGE_SIZE}")
        database.execute("PRAGMA journal_mode = DELETE")
        database.execute("CREATE TABLE zh_glosses(chinese TEXT PRIMARY KEY, gloss TEXT NOT NULL, source TEXT NOT NULL) WITHOUT ROWID")
        database.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID")
        database.executemany("INSERT INTO zh_glosses VALUES (?, ?, 'unihan:kDefinition')", sorted(rows.items()))
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
    parser.add_argument("--zip", help="a local Unihan.zip; it is still checked against the lock")
    arguments = parser.parse_args(argv)

    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    out = Path(arguments.out)
    out.mkdir(parents=True, exist_ok=True)
    cache = Path(arguments.zip).parent if arguments.zip else out.parent / "character-glosses-source"
    if arguments.zip and Path(arguments.zip).name != lock["artifact"]["name"]:
        raise SystemExit(f"--zip must be named {lock['artifact']['name']}")
    source = fetch(lock["artifact"], cache)
    license_path = ROOT / lock["license_file"]["path"]
    if digest(license_path) != lock["license_file"]["sha256"]:
        raise SystemExit(f"{license_path} does not match the lock")
    rows = collect(*read_unihan(source))
    if len(rows) < 10000:
        raise SystemExit(f"only {len(rows)} usable definitions; the source layout changed")
    meta = {
        "kind": "zh_en_characters",
        "source": lock["source"],
        "unicode_version": lock["unicode_version"],
        "license": lock["license"],
        "sqlite_version": sqlite3.sqlite_version,
    }
    write_database(out / DATABASE, rows, meta)
    (out / NOTICE).write_text(
        f"zh-en.db is derived from the Unicode Han Database (Unihan), kDefinition field, Unicode {lock['unicode_version']}.\n"
        "Definitions were shortened: dictionary apparatus removed, at most two senses of three words kept.\n\n"
        + license_path.read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    print(f"{len(rows)} character glosses -> {out / DATABASE} ({(out / DATABASE).stat().st_size / 1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
