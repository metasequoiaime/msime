#!/usr/bin/env python3
"""Locked downloads close their staged file before deleting it, so a failed download on Windows reports its own error rather than WinError 32."""

import hashlib
import pathlib
import sys
import tempfile
import urllib.error

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import fetch_handwriting_model  # noqa: E402
import fetch_language_dictionaries  # noqa: E402
import fetch_offline_glosses  # noqa: E402
import fetch_settled_model  # noqa: E402
import fetch_voice_runtime  # noqa: E402

MODULES = (
    fetch_handwriting_model,
    fetch_language_dictionaries,
    fetch_offline_glosses,
    fetch_settled_model,
    fetch_voice_runtime,
)


def exercise(module, failures):
    expected = b"abc"
    artifact = {
        "name": "synthetic.bin",
        "url": "https://example.invalid/synthetic.bin",
        "sha256": hashlib.sha256(expected).hexdigest(),
        "size": len(expected),
    }
    fetcher = getattr(module, "fetch", None) or module.download
    staged = []
    original_named = tempfile.NamedTemporaryFile
    original_unlink = pathlib.Path.unlink
    original_urlopen = module.urllib.request.urlopen

    def named(*args, **kwargs):
        handle = original_named(*args, **kwargs)
        staged.append(handle)
        return handle

    # Windows refuses to delete a file that is still open; POSIX does not, so the rule is applied here by hand.
    def unlink(path, missing_ok=False):
        if any(not handle.closed and pathlib.Path(handle.name) == path for handle in staged):
            raise PermissionError(32, "The process cannot access the file because it is being used by another process", str(path))
        return original_unlink(path, missing_ok=missing_ok)

    def unavailable(url, *_args, **_kwargs):
        raise urllib.error.HTTPError(url, 500, "Internal Server Error", {}, None)

    module.tempfile.NamedTemporaryFile = named
    pathlib.Path.unlink = unlink
    module.urllib.request.urlopen = unavailable
    try:
        with tempfile.TemporaryDirectory() as directory:
            destination = pathlib.Path(directory) / artifact["name"]
            try:
                fetcher(artifact, destination)
                failures.append(f"{module.__name__} accepted a failed download")
            except urllib.error.HTTPError:
                pass
            except PermissionError as error:
                failures.append(f"{module.__name__} deleted its staged file while it was open: {error}")
            if list(pathlib.Path(directory).iterdir()):
                failures.append(f"{module.__name__} left a staged file after a failed download")
    finally:
        module.tempfile.NamedTemporaryFile = original_named
        pathlib.Path.unlink = original_unlink
        module.urllib.request.urlopen = original_urlopen


def main():
    failures = []
    for module in MODULES:
        exercise(module, failures)
    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        return 1
    print("locked downloads close their staged file before deleting it")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
