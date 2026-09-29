#!/usr/bin/env python3
"""Linux setup rejects oversized download and extracted resource bodies."""

import importlib.machinery
import importlib.util
import pathlib
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "platforms/linux/scripts/msime-linux-setup"
SPEC = importlib.util.spec_from_file_location(
    "msime_linux_setup", SOURCE, loader=importlib.machinery.SourceFileLoader("msime_linux_setup", str(SOURCE))
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


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


def main() -> int:
    failures = []
    original = MODULE.urllib.request.urlopen
    try:
        with tempfile.TemporaryDirectory() as directory:
            destination = pathlib.Path(directory) / "download"
            for label, response in (("header", Response([], "4")), ("stream", Response([b"ab", b"cd"]))):
                MODULE.urllib.request.urlopen = lambda *_args, **_kwargs: response
                try:
                    MODULE.fetch("https://example.invalid/resource", destination, 3)
                    failures.append(f"accepted oversized {label} response")
                except SystemExit as error:
                    if "超过" not in str(error):
                        failures.append(f"rejected {label} for the wrong reason: {error}")
                if label == "header" and response.reads:
                    failures.append("read body after oversized Content-Length")
                if destination.exists():
                    failures.append(f"left a partial file after oversized {label}")

            MODULE.urllib.request.urlopen = lambda *_args, **_kwargs: Response([b"abc"])
            MODULE.fetch("https://example.invalid/resource", destination, 3)
            if destination.read_bytes() != b"abc":
                failures.append("rejected a valid response")
    finally:
        MODULE.urllib.request.urlopen = original

    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        return 1
    print("Linux setup downloads enforce response bounds")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
