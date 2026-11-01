#!/usr/bin/env python3
"""Offline check of build_target_pronunciations.py: what the filter keeps, which reading each word gets, and the databases.

The entries copy the shape of the 2026-10-25 kaikki.org dumps (French, Japanese, Spanish, Russian, German, Korean), cut down to the fields the build reads plus a few it must ignore. Everything runs on temporary files; nothing is downloaded, so --quick can run it offline.
"""
import gzip
import json
import sqlite3
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
import build_target_pronunciations as generator  # noqa: E402

failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def entry(language, word, pos="noun", sounds=(), forms=(), senses=({"glosses": ["x"]},), **extra):
    return {"word": word, "lang_code": language, "lang": "x", "pos": pos, "sounds": list(sounds), "forms": list(forms), "senses": list(senses), **extra}


AUDIO = {"audio": "LL-Q150 (fra)-x.wav", "ogg_url": "https://upload.wikimedia.org/x.ogg", "tags": ["France"]}
DUMP = [
    # A phonemic transcription beats the bracketed phonetic one; audio rows are dropped by the filter.
    entry("fr", "chat", sounds=[{"ipa": "[ʃa]"}, {"ipa": "/ʃa/"}, AUDIO]),
    # The noun's reading wins over the verb entry that comes first in the dump.
    entry("fr", "est", pos="verb", sounds=[{"ipa": "/ɛ/"}]),
    entry("fr", "est", pos="noun", sounds=[{"ipa": "/ɛst/"}]),
    # An inflected form has its own IPA but no gloss names it.
    entry("fr", "chats", sounds=[{"ipa": "/ʃa/"}], senses=[{"form_of": [{"word": "chat"}], "tags": ["form-of", "plural"]}]),
    # Two variants in one cell, a dialectal reading, and an untagged one after them.
    entry("fr", "pomme de terre", sounds=[{"ipa": "/pɔm də tɛʁ/, /pɔm t‿tɛʁ/"}, {"ipa": "/pɔm dœ tɛʁ/", "tags": ["dialectal"]}, {"ipa": "/pɔm də tɛʁ/"}]),
    # Not in any gloss: left out of the restricted table.
    entry("fr", "chien", sounds=[{"ipa": "/ʃjɛ̃/"}]),
    # A romanization page and a symbol are not words a gloss teaches.
    entry("fr", "Chat", pos="romanization", sounds=[{"ipa": "/tʃat/"}]),
    # Castilian first, as the Spanish glosses prefer Spain; the Latin American reading is regional.
    entry("es", "cielo", sounds=[{"ipa": "/ˈθjelo/", "tags": ["Spain"]}, {"ipa": "/ˈsjelo/", "tags": ["Latin-America"]}]),
    entry("es", "gato", sounds=[{"ipa": "/ˈɡato/"}, {"ipa": "[ˈɡa.t̪o]"}]),
    # Russian and Korean have only bracketed transcriptions; the key is the unstressed headword.
    entry("ru", "кошка", sounds=[{"ipa": "[ˈkoʂkə]"}, {"tags": ["Moscow"], "audio": "x.ogg"}]),
    entry("ko", "고양이", sounds=[{"ipa": "[ko̞ja̠ŋi]"}, {"other": "고양이", "tags": ["Revised-Romanization"]}]),
    # German nouns keep their capital; a note left in the cell rejects it.
    entry("de", "Katze", sounds=[{"ipa": "/ˈkatsə/"}]),
    entry("de", "Kater", sounds=[{"ipa": "/ˈkaːtɐ/ (Austria)"}]),
    # Japanese reads in Hepburn from the romanization form; the narrow IPA is never used. The kana spelling of the kanji entry reads the katakana gloss too.
    entry("ja", "林檎", sounds=[{"ipa": "[ɾʲiŋɡo̞]"}, {"other": "りんご"}], forms=[{"form": "林檎", "tags": ["canonical"]}, {"form": "ringo", "tags": ["romanization"]}]),
    entry("ja", "猫", sounds=[{"ipa": "[ne̞ko̞]"}, {"other": "ねこ"}], forms=[{"form": "neko", "tags": ["romanization"]}]),
    entry("ja", "リンゴ", sounds=[], forms=[], senses=[{"alt_of": [{"word": "林檎"}], "tags": ["alt-of"]}]),
    # A romanization containing kana is not romaji.
    entry("ja", "空", sounds=[{"other": "そら"}], forms=[{"form": "そら", "tags": ["romanization"]}]),
    # English and other languages are not this build's.
    entry("en", "cat", sounds=[{"ipa": "/kæt/"}]),
    {"word": "", "lang_code": "fr", "sounds": [{"ipa": "/x/"}]},
]

GLOSSES = {
    "fr": ["chat, chatte; félin", "pomme de terre", "est; orient", "Chat"],
    "es": ["gato, gata", "cielo; día"],
    "ru": ["кошка, кот"],
    "de": ["Katze, Kater"],
    "ko": ["고양이"],
    "ja": ["林檎, リンゴ", "家猫, 猫", "空, 大空"],
}


def write_glosses(directory: Path) -> None:
    directory.mkdir(parents=True)
    for language, glosses in GLOSSES.items():
        with sqlite3.connect(directory / f"zh-{language}.db") as database:
            database.execute("CREATE TABLE zh_glosses(chinese TEXT PRIMARY KEY, gloss TEXT NOT NULL, source TEXT NOT NULL)")
            database.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            database.executemany("INSERT INTO zh_glosses VALUES (?, ?, 'x')", [(str(index), gloss) for index, gloss in enumerate(glosses)])
            database.executemany("INSERT INTO meta VALUES (?, ?)", [("target_language", language), ("dump_date", "2026-09-02")])


def table(path: Path, language: str) -> dict[str, str]:
    with sqlite3.connect(path) as database:
        return dict(database.execute(f"SELECT word, phonetic FROM {language}_phonetics"))


def main() -> int:
    check(generator.clean_ipa("/ʃa/") == "/ʃa/", "a slashed transcription is kept with its delimiters")
    check(generator.clean_ipa("[ˈkoʂkə]") == "[ˈkoʂkə]", "a bracketed transcription is kept")
    check(generator.clean_ipa("/ʃa/, /ʃat/") is None, "two variants in one cell are not one reading")
    check(generator.clean_ipa("/ˈkaːtɐ/ (Austria)") is None, "a note after the transcription rejects it")
    check(generator.clean_ipa("ʃa") is None, "a cell without delimiters is not a transcription")
    check(generator.clean_ipa("/ʃa3/") is None, "digits are not IPA")
    check(generator.clean_ipa("/ʃ(a/") is None, "unbalanced brackets reject the cell")
    check(generator.clean_romaji("ōkii") == "ōkii" and generator.clean_romaji("konnichi wa") == "konnichi wa", "Hepburn with macrons and spaces is romaji")
    check(generator.clean_romaji("そら") is None and generator.clean_romaji("") is None, "kana is not romaji")
    check(generator.hiragana("リンゴ") == "りんご" and generator.hiragana("コンピューター") == "こんぴゅーたー", "katakana folds to hiragana, the long-vowel mark kept")

    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        dump = root / "dump.jsonl.gz"
        with gzip.open(dump, "wt", encoding="utf-8") as handle:
            for item in DUMP:
                handle.write(json.dumps(item, ensure_ascii=False) + "\n")
        filtered = root / "filtered.jsonl.gz"
        check(generator.main(["filter", "--input", str(dump), "--out", str(filtered)]) == 0, "filter runs")
        first = filtered.read_bytes()
        check(generator.main(["filter", "--input", str(dump), "--out", str(filtered)]) == 0 and filtered.read_bytes() == first, "the filtered dump is byte-identical across runs")
        kept = [json.loads(line) for line in gzip.open(filtered, "rt", encoding="utf-8")]
        check(all(item["lang_code"] in generator.LANGUAGES for item in kept), "only the six languages are kept")
        check(not any("audio" in sound or "ogg_url" in sound for item in kept for sound in item["sounds"]), "audio rows are dropped")
        check(not any(item["word"] == "リンゴ" for item in kept), "an entry without any reading is dropped")
        check(any(item["word"] == "chats" and item.get("form_of") for item in kept), "an inflected form is marked")
        check(all(set(item) <= {"word", "lang_code", "pos", "sounds", "romanizations", "form_of"} for item in kept), "only the fields the build reads are kept")

        glosses = root / "glosses"
        write_glosses(glosses)
        out = root / "out"
        arguments = ["build", "--input", str(filtered), "--out", str(out), "--glosses", str(glosses), "--min-words", "1",
                     "--dump-date", "2026-10-25", "--source-revision", "f9fe161b9dd1e898352c51bef7295e774bba2ef1"]
        check(generator.main(arguments) == 0, "build runs")
        french = table(out / "fr-phonetic.db", "fr")
        check(french.get("chat") == "/ʃa/", f"a phonemic transcription beats a phonetic one: {french.get('chat')!r}")
        check(french.get("est") == "/ɛst/", f"the noun entry beats an earlier verb entry: {french.get('est')!r}")
        check(french.get("pomme de terre") == "/pɔm də tɛʁ/", f"multi-variant and dialectal cells are skipped: {french.get('pomme de terre')!r}")
        check("chats" not in french and "chien" not in french, "inflected forms and words outside the glosses are left out")
        check("Chat" not in french, "a romanization page gives no reading")
        spanish = table(out / "es-phonetic.db", "es")
        check(spanish == {"gato": "/ˈɡato/", "cielo": "/ˈθjelo/"}, f"Spanish keeps the phonemic and the standard reading: {spanish}")
        check(table(out / "ru-phonetic.db", "ru") == {"кошка": "[ˈkoʂkə]"}, "Russian keeps its bracketed transcription")
        check(table(out / "ko-phonetic.db", "ko") == {"고양이": "[ko̞ja̠ŋi]"}, "Korean keeps its bracketed transcription")
        check(table(out / "de-phonetic.db", "de") == {"Katze": "/ˈkatsə/"}, "German keeps the capital and drops a noted cell")
        japanese = table(out / "ja-phonetic.db", "ja")
        check(japanese == {"林檎": "ringo", "猫": "neko", "リンゴ": "ringo"}, f"Japanese reads in Hepburn, katakana through the kanji entry's kana: {japanese}")
        with sqlite3.connect(out / "ja-phonetic.db") as database:
            meta = dict(database.execute("SELECT key, value FROM meta"))
            version = database.execute("PRAGMA user_version").fetchone()[0]
        check(meta.get("kind") == "ja_phonetic" and meta.get("language") == "ja", "meta names the kind and language the bridge checks")
        check(meta.get("license") == "CC-BY-SA-4.0" and meta.get("vocabulary") == "offline-glosses 2026-09-02", f"meta records the licence and the gloss vocabulary: {meta}")
        check(version == generator.SCHEMA_VERSION, "user_version is the schema version")
        notice = (out / generator.NOTICE_NAME).read_text(encoding="utf-8")
        check("Creative Commons Attribution-ShareAlike 4.0" in notice and "Wiktionary contributors" in notice, "the NOTICE carries the attribution and the licence")
        check("Wiktextract dump date: 2026-10-25" in notice and "f9fe161b9dd1e898352c51bef7295e774bba2ef1" in notice, "the NOTICE names the dump")
        digests = {path.name: path.read_bytes() for path in sorted(out.iterdir())}
        check(generator.main(arguments) == 0, "build runs again")
        check(digests == {path.name: path.read_bytes() for path in sorted(out.iterdir())}, "the same input gives the same bytes")
        unrestricted = root / "unrestricted"
        check(generator.main(["build", "--input", str(filtered), "--out", str(unrestricted), "--min-words", "1"]) == 0, "build without glosses runs")
        everything = table(unrestricted / "fr-phonetic.db", "fr")
        check("chien" in everything and "chats" not in everything, "without --glosses every lemma is kept, still without inflected forms")
        check(table(unrestricted / "ja-phonetic.db", "ja").get("りんご") == "ringo", "without --glosses the kana spellings are their own keys")
        try:
            generator.main(["build", "--input", str(filtered), "--out", str(root / "few"), "--glosses", str(glosses)])
            check(False, "a dump with too few readings must fail the build")
        except SystemExit as error:
            check("source layout changed" in str(error), f"too few readings fail with the layout message: {error}")
    return report()


def report() -> int:
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("target pronunciations: filter, reading choice and databases OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
