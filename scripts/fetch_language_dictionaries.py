#!/usr/bin/env python3
"""Fetch the Cantonese and Zhuyin dictionaries the macOS and Windows hosts ship beside their resource set.

``resources/language-dictionaries.lock.json`` pins ``cantonese.db`` and ``zhuyin.db``, built by ``msime-dict-build languages`` and published by ``.github/workflows/release-language-dictionaries.yml``, together with their licence texts (``rime_cantonese_LICENSE.txt``, ``libchewing_data_LICENSE.txt``), which must travel with them. ``platforms/macos/stage-resources.sh`` and ``platforms/windows/installer/Prepare-PackageFiles.ps1`` read ``target/language-dictionaries`` and stage nothing when it is absent; without the dictionaries Cantonese and Zhuyin are shown as unavailable and fall back, while Vietnamese needs no data.

Until a release is pinned the lock does not exist; this prints a "skipped" line and exits 0, so packaging can call it unconditionally. The lock pins a SHA-256 and a size for every file, and a download that does not match them is discarded rather than installed. Idempotent: a file already present and matching is left alone.

usage: fetch_language_dictionaries.py [--out <directory>]   (default: target/language-dictionaries)
"""
import argparse
import hashlib
import json
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/language-dictionaries.lock.json"
DEFAULT_OUT = ROOT / "target/language-dictionaries"
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
    if not LOCK.is_file():
        print(f"skipped: {LOCK.relative_to(ROOT)} missing; no language dictionaries are pinned yet", file=sys.stderr)
        return
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
