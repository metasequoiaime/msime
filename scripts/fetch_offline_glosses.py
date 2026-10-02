#!/usr/bin/env python3
"""Fetch the offline candidate gloss dictionaries for the non-English translation targets.

``resources/offline-glosses.lock.json`` pins one ``zh-<lang>.db`` per language (fr, ja, es, ru, de, ko) built by ``scripts/build_offline_glosses.py``, plus ``offline-glosses-NOTICE.txt``, which must travel with them because they adapt CC BY-SA 4.0 Wiktionary text. The staging scripts, the Android build scripts and the Windows installer all read ``target/offline-glosses`` and package nothing when it is absent; Linux takes the directory through ``-DMSIME_OFFLINE_GLOSSES``. The lock's ``filtered_input`` is the reduced dump the files were built from and is not fetched here: it is only needed to rebuild them.

The lock pins a SHA-256 and a size for every file, and a download that does not match them is discarded rather than installed. Idempotent: a file already present and matching is left alone, so packaging can call this unconditionally.

usage: fetch_offline_glosses.py [--out <directory>]   (default: target/offline-glosses)
"""
import argparse
import hashlib
import json
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/offline-glosses.lock.json"
DEFAULT_OUT = ROOT / "target/offline-glosses"
CHUNK = 1 << 20


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(CHUNK), b""):
            sha.update(block)
    return sha.hexdigest()


def fetch(artifact: dict, destination: Path) -> None:
    url = artifact["url"]
    if not url.startswith("https://"):
        raise SystemExit(f"{artifact['name']}: refusing a non-HTTPS source")
    print(f"  fetching {artifact['name']} ({artifact['size'] / 1e6:.1f} MB)", file=sys.stderr)
    # Staged beside the destination, so a failed or mismatched download never leaves a partial file where a package script would pick it up as finished.
    staged = tempfile.NamedTemporaryFile(dir=destination.parent, delete=False)
    staged_path = Path(staged.name)
    try:
        with staged:
            with urllib.request.urlopen(url, timeout=300) as response:
                advertised = response.headers.get("Content-Length")
                if advertised is not None:
                    try:
                        if int(advertised) > artifact["size"]:
                            raise SystemExit(f"{artifact['name']}: response is larger than the lock")
                    except ValueError:
                        pass
                size = 0
                while block := response.read(CHUNK):
                    size += len(block)
                    if size > artifact["size"]:
                        raise SystemExit(f"{artifact['name']}: response is larger than the lock")
                    staged.write(block)
    except BaseException:
        # Deleted only once the with block has closed it: Windows refuses to delete an open file, and the PermissionError would replace the download's own error.
        staged_path.unlink(missing_ok=True)
        raise
    actual = digest(staged_path)
    size = staged_path.stat().st_size
    if actual != artifact["sha256"] or size != artifact["size"]:
        staged_path.unlink(missing_ok=True)
        raise SystemExit(f"{artifact['name']}: expected {artifact['sha256']} at {artifact['size']} bytes, got {actual} at {size}")
    staged_path.replace(destination)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    arguments = parser.parse_args()
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    if not lock["artifacts"]:
        raise SystemExit(f"{LOCK} pins no artifacts")
    arguments.out.mkdir(parents=True, exist_ok=True)
    for artifact in lock["artifacts"]:
        destination = arguments.out / artifact["name"]
        if destination.is_file() and destination.stat().st_size == artifact["size"] and digest(destination) == artifact["sha256"]:
            print(f"  {artifact['name']}: already at the locked digest", file=sys.stderr)
            continue
        fetch(artifact, destination)
    print(arguments.out)


if __name__ == "__main__":
    main()
