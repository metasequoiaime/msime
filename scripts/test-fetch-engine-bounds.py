#!/usr/bin/env python3
"""Engine archive downloads reject bodies larger than the fixed resource limit."""

from __future__ import annotations

import json
import pathlib
import sys
import tempfile
from typing import Optional

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import fetch_engine  # noqa: E402
import relock_engine  # noqa: E402


class Response:
    def __init__(self, chunks: list[bytes], content_length: Optional[str] = None):
        self.chunks = iter(chunks)
        self.headers = {"Content-Length": content_length} if content_length is not None else {}

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def read(self, _size: int = -1) -> bytes:
        return next(self.chunks, b"")


def main() -> int:
    failures: list[str] = []
    original_fetch_limit = fetch_engine.MAX_ARCHIVE_BYTES
    original_relock_limit = relock_engine.MAX_ARCHIVE_BYTES
    original_urlopen = fetch_engine.urllib.request.urlopen
    fetch_engine.MAX_ARCHIVE_BYTES = 8
    relock_engine.MAX_ARCHIVE_BYTES = 8
    try:
        artifact = {
            "archive": "https://example.invalid/engine.tar.gz",
            "repository": "example/engine",
            "sha256": "0" * 64,
        }

        with tempfile.TemporaryDirectory() as directory:
            fetch_engine.urllib.request.urlopen = lambda *_args, **_kwargs: Response([], "9")
            try:
                fetch_engine.download_and_extract(artifact, pathlib.Path(directory) / "header")
                failures.append("fetch_engine accepted an oversized Content-Length")
            except RuntimeError as error:
                if "exceeds" not in str(error):
                    failures.append(f"fetch_engine rejected the header for the wrong reason: {error}")

            fetch_engine.urllib.request.urlopen = lambda *_args, **_kwargs: Response(
                [b"12345678", b"9"]
            )
            try:
                fetch_engine.download_and_extract(artifact, pathlib.Path(directory) / "stream")
                failures.append("fetch_engine accepted an oversized streamed body")
            except RuntimeError as error:
                if "exceeds" not in str(error):
                    failures.append(f"fetch_engine rejected the body for the wrong reason: {error}")

        relock_engine.urllib.request.urlopen = lambda *_args, **_kwargs: Response([], "9")
        try:
            relock_engine.fetch("https://example.invalid/engine.tar.gz")
            failures.append("relock_engine accepted an oversized Content-Length")
        except RuntimeError as error:
            if "exceeds" not in str(error):
                failures.append(f"relock_engine rejected the header for the wrong reason: {error}")

        relock_engine.urllib.request.urlopen = lambda *_args, **_kwargs: Response([b"12345678", b"9"])
        try:
            relock_engine.fetch("https://example.invalid/engine.tar.gz")
            failures.append("relock_engine accepted an oversized streamed body")
        except RuntimeError as error:
            if "exceeds" not in str(error):
                failures.append(f"relock_engine rejected the body for the wrong reason: {error}")

        lock = json.loads((ROOT / "engine-lock.json").read_text(encoding="utf-8"))
        try:
            relock_engine.relock(
                lock,
                "0123456789abcdef0123456789abcdef01234567",
                lambda _url: b"123456789",
            )
            failures.append("relock_engine accepted oversized bytes from a download callback")
        except RuntimeError as error:
            if "exceeds" not in str(error):
                failures.append(f"relock_engine rejected callback bytes for the wrong reason: {error}")
    finally:
        fetch_engine.MAX_ARCHIVE_BYTES = original_fetch_limit
        relock_engine.MAX_ARCHIVE_BYTES = original_relock_limit
        fetch_engine.urllib.request.urlopen = original_urlopen

    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        return 1
    print("engine archive downloads enforce a fixed response limit")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
