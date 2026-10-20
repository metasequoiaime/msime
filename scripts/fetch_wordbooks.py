#!/usr/bin/env python3
"""Fetch ECDICT and expand it into the 背单词 wordbooks the settings page offers.

The exam syllabuses are not something this project can derive. The shipped ``msime-english.db`` carries
Chinese glosses and corpus frequency, which is enough to build "the thousand commonest words" but
not enough to say a word is on the CET-4 list — that is a published syllabus, and inventing the
label would put a name on the page that nothing behind it supports.

ECDICT is that data and it is MIT licensed. Its ``tag`` column marks每个词 against zk/gk/cet4/cet6/
ky/ielts/toefl/gre, so eight books fall straight out of one file.

It is deliberately *not* in ``resources/desktop-dictionary.lock.json``. That lock is shared by all
six platforms and ``ResourceStore::verify`` requires a resource directory to match it exactly, so an
extra entry there would break the check whose job is to prove a shipped dictionary is intact — the
same reasoning ``fetch_settled_model.py`` records. A second one-artifact manifest costs none of it.

The source CSV is 66 MB and is never shipped: this script keeps only word, phonetic and the first
gloss line, which is what a card shows, and writes one compact JSON per book in exactly the shape
``client-services::vocabulary::wordbook::Wordbook`` deserialises. The eight books together are under
3 MB, so the hosts read them with serde_json and no host needs a CSV parser or a SQLite driver.

Idempotent: a download matching the pinned digest is reused, and books already written are only
rewritten when their contents change.

usage: fetch_wordbooks.py [--out <directory>] [--cache <directory>]   (defaults: target/wordbooks, <out>/../wordbook-source)
"""
import argparse
import csv
import hashlib
import json
import sys
import tempfile
from pathlib import Path

import download_retry

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/wordbook.lock.json"
# 随词书一起进包的许可证声明。宿主只按 `<id>.json` 读书（`builtin.rs`），这个文件放在同一目录里不会被当成一本书。
NOTICE = "wordbooks-NOTICE.txt"

# A card shows one line. ECDICT packs several senses and a "[网络]" section into ``translation``
# separated by newlines; the first line is the dictionary sense and the rest is noise on a card.
MAX_MEANING_CHARS = 256
MAX_PHONETIC_CHARS = 64
MAX_WORD_CHARS = 64


def read_lock() -> dict:
    return json.loads(LOCK.read_text(encoding="utf-8"))


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            sha.update(chunk)
    return sha.hexdigest()


def fetch(artifact: dict, cache: Path) -> Path:
    """The pinned CSV, downloaded once and verified every time."""
    cache.mkdir(parents=True, exist_ok=True)
    target = cache / artifact["name"]
    if target.is_file() and digest(target) == artifact["sha256"]:
        return target
    with tempfile.NamedTemporaryFile(dir=cache, delete=False) as handle:
        temporary = Path(handle.name)
    try:
        size = 0
        with download_retry.urlopen(artifact["url"], timeout=300) as response:
            with temporary.open("wb") as out:
                while True:
                    chunk = response.read(1024 * 1024)
                    if not chunk:
                        break
                    size += len(chunk)
                    # Stop a body that outgrows the lock rather than writing all of it first.
                    if size > artifact["size"]:
                        raise SystemExit("wordbook source is larger than the lock records")
                    out.write(chunk)
        if size != artifact["size"]:
            raise SystemExit(f"wordbook source is {size} bytes; the lock records {artifact['size']}")
        found = digest(temporary)
        if found != artifact["sha256"]:
            raise SystemExit(f"wordbook source digest {found} does not match the lock")
        temporary.replace(target)
        return target
    finally:
        temporary.unlink(missing_ok=True)


def gloss(translation: str) -> str:
    """The first dictionary sense, which is what fits on a card."""
    for line in translation.replace("\\n", "\n").splitlines():
        line = line.strip()
        # "[网络]" lines are crowd-sourced web senses; they are not what a learner should memorise.
        if line and not line.startswith("[网络]"):
            return line[:MAX_MEANING_CHARS]
    return ""


def usable(word: str) -> bool:
    # The store keys a card by its headword, so anything that is not a plain lowercase word would
    # either be invisible on the card or compare unequal to itself after a round trip.
    return word.isascii() and word.isalpha() and word.islower() and 3 <= len(word) <= MAX_WORD_CHARS


def build(csv_path: Path, books: list[dict], out: Path) -> None:
    wanted = {book["tag"]: book for book in books}
    collected: dict[str, list[dict]] = {book["id"]: [] for book in books}
    seen: dict[str, set[str]] = {book["id"]: set() for book in books}

    # csv's default field limit is smaller than some ECDICT definition cells.
    csv.field_size_limit(1024 * 1024)
    with csv_path.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            tags = (row.get("tag") or "").split()
            if not tags:
                continue
            word = (row.get("word") or "").strip()
            if not usable(word):
                continue
            meaning = gloss(row.get("translation") or "")
            if not meaning:
                continue
            phonetic = (row.get("phonetic") or "").strip()[:MAX_PHONETIC_CHARS]
            if phonetic:
                phonetic = f"/{phonetic}/"
            for tag in tags:
                book = wanted.get(tag)
                if book is None or word in seen[book["id"]]:
                    continue
                seen[book["id"]].add(word)
                collected[book["id"]].append(
                    {"word": word, "phonetic": phonetic, "meaning": meaning}
                )

    out.mkdir(parents=True, exist_ok=True)
    for book in books:
        entries = collected[book["id"]]
        if not entries:
            raise SystemExit(f"no usable entries for {book['id']}; the source layout changed")
        document = {"id": book["id"], "name": book["name"], "entries": entries}
        path = out / f"{book['id']}.json"
        encoded = json.dumps(document, ensure_ascii=False, separators=(",", ":"))
        if not path.is_file() or path.read_text(encoding="utf-8") != encoded:
            path.write_text(encoded, encoding="utf-8")
        print(f"{book['id']:6} {book['name']:6} {len(entries):6} 词  {path}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=str(ROOT / "target/wordbooks"))
    # 66 MB 的 ECDICT CSV 和 LICENSE 放在哪里。缺省是输出目录旁边的 wordbook-source；生成发行版 vendor 包时要放到包外，免得把源 CSV 打进去。
    parser.add_argument("--cache")
    arguments = parser.parse_args()

    lock = read_lock()
    out = Path(arguments.out)
    cache = Path(arguments.cache) if arguments.cache else Path(arguments.out).parent / "wordbook-source"
    source = fetch(lock["artifact"], cache)
    build(source, lock["books"], out)
    # MIT 要求版权和许可声明随数据一起分发，所以逐字复制锁定提交的 LICENSE，不做转述；内容不变时不重写。
    license_text = fetch(lock["license_file"], cache).read_text(encoding="utf-8")
    notice = (
        f"The bundled wordbooks are derived from ECDICT ({lock['source']}), commit {lock['source_commit']}.\n"
        "Each book keeps the words ECDICT tags for that syllabus, with the phonetic and the first dictionary sense.\n\n"
        + license_text
    )
    notice_path = out / NOTICE
    if not notice_path.is_file() or notice_path.read_text(encoding="utf-8") != notice:
        notice_path.write_text(notice, encoding="utf-8")
    print(f"wordbooks built from ECDICT {lock['source_commit'][:12]} ({lock['license']}): {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
