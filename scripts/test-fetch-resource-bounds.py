#!/usr/bin/env python3
"""Locked model and voice downloads enforce their sizes while streaming."""

import hashlib
import pathlib
import sys
import tempfile
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import fetch_handwriting_model  # noqa: E402
import fetch_settled_model  # noqa: E402
import fetch_voice_runtime  # noqa: E402


class Response:
    def __init__(self, chunks, content_length=None):
        self.chunks = iter(chunks)
        self.headers = {"Content-Length": content_length} if content_length is not None else {}
        self.reads = 0

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def read(self, _size):
        self.reads += 1
        return next(self.chunks, b"")


def exercise(module, failures):
    expected = b"abc"
    artifact = {
        "name": "synthetic.bin",
        "url": "https://example.invalid/synthetic.bin",
        "sha256": hashlib.sha256(expected).hexdigest(),
        "size": len(expected),
    }
    original_urlopen = urllib.request.urlopen
    fetcher = getattr(module, "fetch", None) or module.download
    try:
        with tempfile.TemporaryDirectory() as directory:
            destination = pathlib.Path(directory) / artifact["name"]
            for label, response in (
                ("header", Response([], "4")),
                ("stream", Response([b"ab", b"cd"])),
            ):
                urllib.request.urlopen = lambda *_args, **_kwargs: response
                try:
                    fetcher(artifact, destination)
                    failures.append(f"{module.__name__} accepted oversized {label}")
                except SystemExit as error:
                    if "larger than the lock" not in str(error):
                        failures.append(f"{module.__name__} rejected {label} for the wrong reason: {error}")
                if label == "header" and response.reads:
                    failures.append(f"{module.__name__} read after oversized Content-Length")
                if destination.exists() or list(pathlib.Path(directory).iterdir()):
                    failures.append(f"{module.__name__} left a partial file after oversized {label}")

            urllib.request.urlopen = lambda *_args, **_kwargs: Response([expected])
            fetcher(artifact, destination)
            if destination.read_bytes() != expected:
                failures.append(f"{module.__name__} rejected a valid response without Content-Length")
    finally:
        urllib.request.urlopen = original_urlopen


def main():
    failures = []
    exercise(fetch_handwriting_model, failures)
    exercise(fetch_settled_model, failures)
    exercise(fetch_voice_runtime, failures)
    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        return 1
    print("settled model and voice downloads enforce lock sizes while streaming")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
