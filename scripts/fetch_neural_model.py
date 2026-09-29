#!/usr/bin/env python3
"""Fetch the pinned neural sentence models.

The keyboard model is already part of the verified dictionary resource set.  This manifest keeps
the two model artifacts together for hosts that want to prepare them independently, and also
provides the desktop settled model used beside that resource set.  Downloads are written to a
temporary file and renamed only after both the locked byte count and SHA-256 match.

usage: fetch_neural_model.py [--out <directory>] [--force]
       (default output: target/neural-model)
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/neural-model.lock.json"
DEFAULT_OUT = ROOT / "target/neural-model"
CHUNK_BYTES = 1 << 20
MAX_MODEL_BYTES = 64 * 1024 * 1024


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(CHUNK_BYTES), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def validate_artifact(artifact: dict) -> tuple[str, int, str]:
    name = artifact.get("file")
    size = artifact.get("size")
    sha256 = artifact.get("sha256")
    if (
        not isinstance(name, str)
        or not name
        or Path(name).name != name
        or Path(name).is_absolute()
        or ".." in Path(name).parts
        or not name.endswith(".safetensors")
    ):
        raise RuntimeError(f"invalid neural model file name: {name!r}")
    if not isinstance(size, int) or not 0 < size <= MAX_MODEL_BYTES:
        raise RuntimeError(f"invalid size for {name}: {size!r}")
    if not isinstance(sha256, str) or len(sha256) != 64 or any(
        character not in "0123456789abcdefABCDEF" for character in sha256
    ):
        raise RuntimeError(f"invalid SHA-256 for {name}")
    return name, size, sha256.lower()


def download(artifact: dict, destination: Path) -> None:
    name, expected_size, expected_digest = validate_artifact(artifact)
    url = artifact.get("url")
    if not isinstance(url, str) or not url.startswith("https://"):
        raise RuntimeError(f"{name}: refusing a non-HTTPS source")
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=destination.parent,
            prefix=f".{name}.",
            delete=False,
        ) as temporary:
            temporary_path = Path(temporary.name)
            request = urllib.request.Request(
                url,
                headers={"User-Agent": "MSIME neural model fetch"},
            )
            with urllib.request.urlopen(request, timeout=300) as response:
                advertised = response.headers.get("Content-Length")
                if advertised is not None:
                    try:
                        if int(advertised) > expected_size:
                            raise RuntimeError(f"{name}: response is larger than the lock")
                    except ValueError:
                        pass
                received = 0
                while chunk := response.read(CHUNK_BYTES):
                    received += len(chunk)
                    if received > expected_size:
                        raise RuntimeError(f"{name}: response is larger than the lock")
                    temporary.write(chunk)
            temporary.flush()
            os.fsync(temporary.fileno())
        actual_size = temporary_path.stat().st_size
        actual_digest = digest(temporary_path)
        if actual_size != expected_size or actual_digest != expected_digest:
            raise RuntimeError(
                f"{name}: expected {expected_digest} at {expected_size} bytes, "
                f"got {actual_digest} at {actual_size}"
            )
        temporary_path.replace(destination)
        temporary_path = None
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def fetch(output: Path, force: bool = False) -> None:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    models = lock.get("models")
    if not isinstance(models, list) or not models:
        raise RuntimeError("neural model lock has no models")
    output.mkdir(parents=True, exist_ok=True)
    for artifact in models:
        name, expected_size, expected_digest = validate_artifact(artifact)
        destination = output / name
        if (
            not force
            and destination.is_file()
            and destination.stat().st_size == expected_size
            and digest(destination) == expected_digest
        ):
            print(f"{name}: already at the locked digest")
            continue
        print(f"fetching {name}")
        download(artifact, destination)
        print(f"{name}: installed")
    print(output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--force", action="store_true", help="redownload matching files")
    args = parser.parse_args()
    fetch(args.out, args.force)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
