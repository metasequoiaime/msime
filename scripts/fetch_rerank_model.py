#!/usr/bin/env python3
"""Fetch the two ONNX keystroke rerank models a host built with `onnx-rerank` ships, verify them, and lay them out the way host-api reads them.

`resources/rerank-model.lock.json` pins chinese-ime-lm's `rerank-v1` release: for each model the directory host-api looks in (`rerank-model` is 8x448, `rerank-model-small` is 6x256) and its three files, each with the release asset's URL, SHA-256 and size and the name it takes on disk (`lm_step.ort`, `model.json`, `vocab.json`). A download that does not match is discarded before it replaces anything. How host-api picks between the two is in `.agents/notes/implemented/architecture/2026-11-01-onnx-rerank-backend.md`.

The models are not in `resources/desktop-dictionary.lock.json` for the same reason as the settled model (`scripts/fetch_settled_model.py`): that lock is shared by every platform and `ResourceStore::verify` requires a resource directory to match it exactly, so host-api looks for these directories beside the resource directory, not inside it.

Idempotent: a file already present and matching is left alone, and a file the lock does not name is removed from the two model directories, so a host can call this unconditionally and stage the directories as they are.

usage: fetch_rerank_model.py [--out <directory>]   (default: target/rerank-model; prints the directory holding rerank-model/ and rerank-model-small/)
"""
import argparse
import hashlib
import json
import sys
import tempfile
from pathlib import Path

import download_retry

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/rerank-model.lock.json"
DEFAULT_OUT = ROOT / "target/rerank-model"
CHUNK = 1 << 20
FILES = ("lm_step.ort", "model.json", "vocab.json")
DIRECTORIES = ("rerank-model", "rerank-model-small")


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(CHUNK), b""):
            sha.update(block)
    return sha.hexdigest()


def matches(path: Path, artifact: dict) -> bool:
    return path.is_file() and path.stat().st_size == artifact["size"] and digest(path) == artifact["sha256"]


def download(artifact: dict, destination: Path) -> None:
    url = artifact["url"]
    if not url.startswith("https://"):
        raise SystemExit(f"{artifact['name']}: refusing a non-HTTPS source")
    print(f"  fetching {artifact['name']} ({artifact['size'] / 1e6:.1f} MB)", file=sys.stderr)
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Staged beside the destination so a failed or mismatched download never sits where a build would take it as finished.
    staged = tempfile.NamedTemporaryFile(dir=destination.parent, delete=False)
    staged_path = Path(staged.name)
    try:
        with staged:
            with download_retry.urlopen(url, timeout=300) as response:
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


def check_lock(lock: dict) -> None:
    """The lock names exactly the two directories host-api reads, each with exactly the three files it loads, all from HTTPS."""
    directories = [model["directory"] for model in lock["models"]]
    if tuple(directories) != DIRECTORIES:
        raise SystemExit(f"{LOCK}: expected the model directories {DIRECTORIES}, got {tuple(directories)}")
    for model in lock["models"]:
        files = sorted(artifact["file"] for artifact in model["artifacts"])
        if files != sorted(FILES):
            raise SystemExit(f"{LOCK}: {model['directory']} must hold exactly {FILES}, got {files}")
        for artifact in model["artifacts"]:
            if not artifact["url"].startswith("https://"):
                raise SystemExit(f"{artifact['name']}: refusing a non-HTTPS source")


def install(out: Path) -> Path:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    check_lock(lock)
    for model in lock["models"]:
        directory = out / model["directory"]
        directory.mkdir(parents=True, exist_ok=True)
        for stray in directory.iterdir():
            if stray.name not in FILES:
                print(f"  removing {stray} (not in the lock)", file=sys.stderr)
                stray.unlink()
        for artifact in model["artifacts"]:
            destination = directory / artifact["file"]
            if matches(destination, artifact):
                print(f"  {model['directory']}/{artifact['file']}: already at the locked digest", file=sys.stderr)
                continue
            download(artifact, destination)
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    arguments = parser.parse_args()
    print(install(arguments.out.resolve()))


if __name__ == "__main__":
    main()
