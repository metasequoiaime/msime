#!/usr/bin/env python3
"""Fetch the offline handwriting model the Linux package ships for `msime-linux-handwriting --local`.

The model is zinnia's binary format (Tegaki zh_CN), read by the Rust port in crates/engine/src/handwriting. It is 26.8 MB, so it is not committed: ``resources/handwriting-model.lock.json`` pins it, and its LGPL-2.1 licence beside it, to the msime-engine commit they were last published from, each with a SHA-256 and a size. A download that does not match both is discarded rather than installed, and nothing is written in place, so an interrupted run never leaves a partial file where a build would take it as finished. Idempotent: files already present and matching are left alone, so packaging can call this unconditionally.

usage: fetch_handwriting_model.py [--out <directory>]   (default: target/handwriting-model)
"""
import argparse
import hashlib
import json
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/handwriting-model.lock.json"
DEFAULT_OUT = ROOT / "target/handwriting-model"
CHUNK = 1 << 20


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(CHUNK), b""):
            sha.update(block)
    return sha.hexdigest()


def matches(path: Path, artifact: dict) -> bool:
    return (
        path.is_file()
        and not path.is_symlink()
        and path.stat().st_size == artifact["size"]
        and digest(path) == artifact["sha256"]
    )


def fetch(artifact: dict, destination: Path) -> None:
    url = artifact["url"]
    if not url.startswith("https://"):
        raise SystemExit(f"{artifact['name']}: refusing a non-HTTPS source")
    print(f"  fetching {artifact['name']} ({artifact['size'] / 1e6:.1f} MB)", file=sys.stderr)
    staged = tempfile.NamedTemporaryFile(dir=destination.parent, delete=False)
    staged_path = Path(staged.name)
    try:
        with staged:
            with urllib.request.urlopen(url, timeout=300) as response:
                advertised = response.headers.get("Content-Length")
                if advertised is not None and advertised.isdigit() and int(advertised) > artifact["size"]:
                    raise SystemExit(f"{artifact['name']}: response is larger than the lock")
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
    if not matches(staged_path, artifact):
        actual, size = digest(staged_path), staged_path.stat().st_size
        staged_path.unlink(missing_ok=True)
        raise SystemExit(
            f"{artifact['name']}: expected {artifact['sha256']} at {artifact['size']} bytes, got {actual} at {size}"
        )
    staged_path.chmod(0o644)
    staged_path.replace(destination)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    arguments = parser.parse_args()
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    arguments.out.mkdir(parents=True, exist_ok=True)
    for artifact in lock["artifacts"]:
        destination = arguments.out / artifact["name"]
        if matches(destination, artifact):
            print(f"  {artifact['name']}: already at the locked digest", file=sys.stderr)
            continue
        fetch(artifact, destination)
    print(arguments.out)


if __name__ == "__main__":
    main()
