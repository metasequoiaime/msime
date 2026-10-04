#!/usr/bin/env python3
"""Offline check of build_offline_glosses.py against a fixture of real Wiktextract rows.

The fixture is English Wiktionary entries as kaikki.org publishes them, cut down to the Chinese and target-language rows, plus a few rows in the same shape for cases the sample did not contain: a Chinese key reached from two English pages (天 from day and sky), a three-segment traditional/simplified form, a literary-only form, a non-English entry of the raw dump, and translations under senses[] (where Wiktextract now puts most of them), including a row repeated from the top level and a minor sense of a common word. Everything runs on temporary files; nothing is downloaded, so --quick can run it offline.
"""
import hashlib
import json
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / "scripts/build_offline_glosses.py"
FIXTURE = ROOT / "scripts/offline-glosses-fixture.jsonl"
LOCK = ROOT / "resources/offline-glosses.lock.json"
LANGUAGES = ("fr", "ja", "es", "ru", "de", "ko")
failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def run(*arguments: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(GENERATOR), *arguments], capture_output=True, text=True)


def build(out: Path, *extra: str, source: Path = FIXTURE) -> subprocess.CompletedProcess:
    return run("build", "--input", str(source), "--out", str(out), *extra)


def glosses(path: Path) -> dict[str, str]:
    with sqlite3.connect(path) as database:
        return dict(database.execute("SELECT chinese, gloss FROM zh_glosses"))


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    with tempfile.TemporaryDirectory() as scratch:
        scratch = Path(scratch)
        # The vocabulary stands in for msime-pinyin.db (tbl_* tables with a value column) and holds 辞林 so that its absence proves the Hokkien row was skipped rather than filtered by vocabulary.
        vocabulary = scratch / "msime-pinyin.db"
        with sqlite3.connect(vocabulary) as database:
            database.execute("CREATE TABLE tbl_2_a(key TEXT, jp TEXT, value TEXT, weight INTEGER)")
            database.execute("CREATE TABLE tbl_others_a(key TEXT, jp TEXT, value TEXT, weight INTEGER)")
            words = ["天", "天空", "猫", "猫儿", "词典", "辞典", "辞林", "字典", "自由", "自由的", "预订", "产品", "制品", "积", "橙", "橙子", "空闲", "余暇", "日", "呕吐", "回", "脑回"]
            database.executemany("INSERT INTO tbl_2_a VALUES ('', '', ?, 1)", [(word,) for word in words])
        frequency = scratch / "msime-english.db"
        with sqlite3.connect(frequency) as database:
            database.execute("CREATE TABLE english_words(word TEXT, display TEXT, weight INTEGER)")
            database.executemany("INSERT INTO english_words VALUES (?, ?, ?)", [("day", "day", 446236148), ("sky", "sky", 27281333), ("Day", "Day", 5), ("cat", "cat", 60133542), ("vomit", "vomit", 1296870), ("orange", "orange", 37316112)])

        out = scratch / "out"
        result = build(out, "--vocabulary", str(vocabulary), "--frequency", str(frequency), "--dump-date", "2026-09-20", "--source-revision", "abc123")
        check(result.returncode == 0, f"build failed: {result.stderr.strip()}")
        if result.returncode != 0:
            return report()

        for language in LANGUAGES:
            path = out / f"zh-{language}.db"
            check(path.is_file(), f"zh-{language}.db was not written")
            with sqlite3.connect(path) as database:
                check(database.execute("PRAGMA user_version").fetchone()[0] == 1, f"{language}: user_version is not 1")
                check(database.execute("PRAGMA page_size").fetchone()[0] == 4096, f"{language}: page size is not fixed")
                meta = dict(database.execute("SELECT key, value FROM meta"))
                check(meta.get("target_language") == language, f"{language}: meta names {meta.get('target_language')}")
                check(meta.get("license") == "CC-BY-SA-4.0" and meta.get("dump_date") == "2026-09-20" and meta.get("source_revision") == "abc123", f"{language}: meta lacks the input description: {meta}")
                check(meta.get("sqlite_version") == sqlite3.sqlite_version, f"{language}: meta lacks the SQLite version")
                rows = database.execute("SELECT chinese, gloss, source FROM zh_glosses").fetchall()
                check(meta.get("key_count") == str(len(rows)), f"{language}: key_count disagrees with the table")
            for key, gloss, source in rows:
                senses = gloss.split("; ")
                check(0 < len(key) <= 40, f"{language}: key {key!r} is out of bounds")
                check(1 <= len(senses) <= 2, f"{language}: {key} has {len(senses)} senses")
                check(len(source.split("; ")) == len(senses), f"{language}: {key} does not name a page per sense")
                for sense in senses:
                    words = sense.split(", ")
                    check(1 <= len(words) <= 2 and all(words), f"{language}: {key} sense {sense!r} is not one or two words")
                    check("；" not in sense and ";" not in sense, f"{language}: {key} sense {sense!r} holds a semicolon")

        fr = glosses(out / "zh-fr.db")
        check(fr.get("天") == "jour, journée; ciel", f"天 must rank day before sky: {fr.get('天')!r}")
        check(fr.get("猫") == "chat, chatte; félin", f"a row under both senses[] and the top level must not disturb the table: {fr.get('猫')!r}")
        check(fr.get("呕吐") == "vomir; dégobiller, débecter", f"senses[] must be read, and a page's leading sense must beat a minor sense of a more common word: {fr.get('呕吐')!r}")
        check(fr.get("词典") == "dictionnaire", f"an informal form must yield to a plain one: {fr.get('词典')!r}")
        check("辞林" not in fr, "a Hokkien row was taken as Mandarin")
        check("回" not in fr and fr.get("脑回") == "gyrus", f"a single character reached only from a rare English page must be dropped: {fr.get('回')!r}")
        check(fr.get("空闲") == "loisir", f"three-segment form or semicolon rejection: {fr.get('空闲')!r}")
        check("余暇" not in fr, "a literary form was kept")
        check(fr.get("自由的") == "libre" and fr.get("自由") == "libre", "的 must also key the bare word when the vocabulary has it")
        check("辞典" in fr and "字典" not in fr, "only the first two Mandarin forms of a table are keys")
        with sqlite3.connect(out / "zh-fr.db") as database:
            check(all("chat" not in source.split("; ") for (source,) in database.execute("SELECT source FROM zh_glosses")), "a non-English entry of the raw dump was read")
            check(database.execute("SELECT source FROM zh_glosses WHERE chinese = '天'").fetchone()[0] == "day; sky", "sources must follow the senses")

        ru = glosses(out / "zh-ru.db")
        check(ru.get("预订") == "бронировать, забронировать", f"Russian stress marks: {ru.get('预订')!r}")
        check(all("́" not in gloss and "̀" not in gloss for gloss in ru.values()), "a Russian stress mark survived")
        ko = glosses(out / "zh-ko.db")
        check(ko.get("预订") == "예약하다", f"Korean hanja annotation: {ko.get('预订')!r}")
        check(all("(" not in gloss for gloss in ko.values()), "a Korean parenthetical survived")
        es = glosses(out / "zh-es.db")
        check(es.get("橙") == "naranja", f"a regional form must yield to an unmarked one: {es.get('橙')!r}")
        de = glosses(out / "zh-de.db")
        check(de.get("词典") == "Wörterbuch", f"an Alemannic form must yield to standard German: {de.get('词典')!r}")
        ja = glosses(out / "zh-ja.db")
        check(ja.get("天") == "日, 一日; 空", f"ja 天: {ja.get('天')!r}")

        notice = (out / "offline-glosses-NOTICE.txt").read_text(encoding="utf-8")
        for needle in ("Wiktionary contributors", "Creative Commons Attribution-ShareAlike 4.0", "2026-09-20", "abc123", "Changes:", "without warranties"):
            check(needle in notice, f"NOTICE lacks {needle!r}")

        # Same input, same SQLite: same bytes. Through the filter: the same bytes again.
        again = scratch / "again"
        build(again, "--vocabulary", str(vocabulary), "--frequency", str(frequency), "--dump-date", "2026-09-20", "--source-revision", "abc123")
        filtered = scratch / "filtered.jsonl.gz"
        result = run("filter", "--input", str(FIXTURE), "--out", str(filtered))
        check(result.returncode == 0, f"filter failed: {result.stderr.strip()}")
        refiltered = scratch / "refiltered.jsonl.gz"
        run("filter", "--input", str(filtered), "--out", str(refiltered))
        check(sha(filtered) == sha(refiltered), "the filter is not idempotent")
        through = scratch / "through"
        build(through, "--vocabulary", str(vocabulary), "--frequency", str(frequency), "--dump-date", "2026-09-20", "--source-revision", "abc123", source=filtered)
        for language in LANGUAGES:
            name = f"zh-{language}.db"
            check(sha(out / name) == sha(again / name), f"{name} is not reproducible")
            check((through / name).is_file() and sha(out / name) == sha(through / name), f"{name} differs when built from the filtered input")

        bare = scratch / "bare"
        build(bare, "--lang", "fr")
        bare_fr = glosses(bare / "zh-fr.db")
        check("自由的" in bare_fr and "自由" not in bare_fr, "without a vocabulary no 的-stripped key may be invented")
        check(not (bare / "zh-ja.db").exists(), "--lang fr wrote other languages")

        result = build(scratch / "small", "--lang", "fr", "--max-bytes", "1000")
        check(result.returncode != 0 and "limit" in result.stderr, "the size limit was not enforced")
        foreign = scratch / "foreign.jsonl"
        foreign.write_text("".join(line for line in FIXTURE.read_text(encoding="utf-8").splitlines(keepends=True) if '"lang_code":"en"' not in line), encoding="utf-8")
        result = build(scratch / "empty", "--lang", "fr", source=foreign)
        check(result.returncode != 0 and "layout changed" in result.stderr, "an input without English entries was not refused")
        result = build(scratch / "unknown", "--lang", "fr,xx")
        check(result.returncode != 0, "an unknown language was accepted")

    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    check(lock.get("license") == "CC-BY-SA-4.0" and lock.get("source", "").startswith("https://kaikki.org/"), "the lock does not name the source and license")
    check({"edition", "url", "etag", "dump_date", "source_revision", "sha256", "size"} <= set(lock.get("input", {})), "the lock input block is incomplete")
    artifacts = lock.get("artifacts")
    check(isinstance(artifacts, list), "the lock has no artifact list")
    if artifacts:
        names = {artifact.get("name") for artifact in artifacts}
        check("offline-glosses-NOTICE.txt" in names, "a published lock must ship the NOTICE")
        for artifact in artifacts:
            check(str(artifact.get("url", "")).startswith("https://") and len(str(artifact.get("sha256", ""))) == 64 and isinstance(artifact.get("size"), int), f"{artifact.get('name')}: incomplete artifact")
    return report()


def report() -> int:
    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    if failures:
        return 1
    print("offline glosses: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
