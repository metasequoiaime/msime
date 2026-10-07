#!/usr/bin/env python3
"""Harmony local ASR text reads must bind the trusted path to one descriptor."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKER = ROOT / "platforms/harmony/entry/src/main/ets/workers/LocalAsrWorker.ets"


def main() -> int:
    source = WORKER.read_text(encoding="utf-8")
    start = source.index("const textReaderApi:")
    body = source[start : source.index("\n};", start) + 3]
    required = {
        "text path is checked without following links": "const expected: fs.Stat = fs.lstatSync(path);" in body
        and "expected.isSymbolicLink()" in body,
        "text descriptor is bound to the checked path": "const opened: fs.Stat = fs.statSync(file.fd);" in body
        and "opened.ino !== expected.ino" in body,
        "text descriptor is closed on a failed bind": "fs.closeSync(file);" in body,
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("harmony local ASR reader binding: missing " + ", ".join(missing))
        return 1
    print("harmony local ASR text reads bind trusted paths before reading")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
