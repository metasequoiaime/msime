#!/usr/bin/env python3
"""Offline gloss downloads stop at the lock size and leave no partial file."""

import hashlib
import pathlib
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import fetch_offline_glosses  # noqa: E402


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


def main():
    expected = b"abc"
    artifact = {
        "name": "synthetic.db",
        "url": "https://example.invalid/synthetic.db",
        "sha256": hashlib.sha256(expected).hexdigest(),
        "size": len(expected),
    }
    original_urlopen = fetch_offline_glosses.urllib.request.urlopen
    failures = []
    try:
        with tempfile.TemporaryDirectory() as directory:
            destination = pathlib.Path(directory) / artifact["name"]
            for label, response in [
                ("header", Response([], "4")),
                ("stream", Response([b"ab", b"cd"])),
            ]:
                fetch_offline_glosses.urllib.request.urlopen = lambda *_args, **_kwargs: response
                try:
                    fetch_offline_glosses.fetch(artifact, destination)
                    failures.append(f"accepted oversized {label}")
                except SystemExit as error:
                    if "larger than the lock" not in str(error):
                        failures.append(f"rejected {label} for the wrong reason: {error}")
                if label == "header" and response.reads:
                    failures.append("read a body after an oversized Content-Length")
                if destination.exists() or list(pathlib.Path(directory).iterdir()):
                    failures.append(f"left a partial file after oversized {label}")

            fetch_offline_glosses.urllib.request.urlopen = lambda *_args, **_kwargs: Response([expected])
            fetch_offline_glosses.fetch(artifact, destination)
            if destination.read_bytes() != expected:
                failures.append("rejected a valid response without Content-Length")
    finally:
        fetch_offline_glosses.urllib.request.urlopen = original_urlopen

    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        return 1
    print("offline gloss downloads enforce the lock size while streaming")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
