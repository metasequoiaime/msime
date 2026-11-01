#!/usr/bin/env python3
"""Fetch the gloss tables the host reads beside its resource directory: character-glosses (Unihan, ``scripts/build_character_glosses.py``), word-glosses (CC-CEDICT, ``scripts/build_word_glosses.py``), pronunciations (ECDICT, ``scripts/build_pronunciations.py``) and target-pronunciations (the readings of the French, Japanese, Spanish, Russian, German and Korean gloss lines, from Wiktionary, ``scripts/build_target_pronunciations.py``).

Each table's lock in ``resources/`` lists under ``built`` the database and NOTICE that were built from its pinned source and published on metasequoiaime/chinese-ime-lm, and they land in ``target/<table>``, where the macOS staging script and the Windows installer pick them up. Without them single characters and long-tail words have no English gloss, English glosses carry no IPA, the other gloss lines carry no reading and sentence candidates get no word-by-word breakdown.

Downloads go through ``fetch_offline_glosses.fetch``: sizes are enforced while streaming, and a file that does not match its lock is discarded rather than installed. Idempotent: a file already present and matching is left alone, so packaging can call this unconditionally.

usage: fetch_gloss_tables.py [--out <directory>]   (default: target; each table goes into <directory>/<table>)
"""
import argparse
import json
import sys
from pathlib import Path

from fetch_offline_glosses import digest, fetch

ROOT = Path(__file__).resolve().parents[1]
TABLES = ("character-glosses", "word-glosses", "pronunciations", "target-pronunciations")
DEFAULT_OUT = ROOT / "target"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    arguments = parser.parse_args()
    for table in TABLES:
        lock_path = ROOT / "resources" / f"{table}.lock.json"
        built = json.loads(lock_path.read_text(encoding="utf-8")).get("built") or []
        if not built:
            raise SystemExit(f"{lock_path} pins no built files")
        directory = arguments.out / table
        directory.mkdir(parents=True, exist_ok=True)
        for artifact in built:
            destination = directory / artifact["name"]
            if destination.is_file() and destination.stat().st_size == artifact["size"] and digest(destination) == artifact["sha256"]:
                print(f"  {table}/{artifact['name']}: already at the locked digest", file=sys.stderr)
                continue
            fetch(artifact, destination)
        print(directory)


if __name__ == "__main__":
    main()
