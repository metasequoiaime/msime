#!/usr/bin/env python3
"""Language dictionary downloads skip without a lock, refuse non-HTTPS sources, stop at the lock size and leave no partial or mismatched file; --list-databases names the pinned databases."""

import contextlib
import hashlib
import io
import json
import pathlib
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import fetch_language_dictionaries  # noqa: E402


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


def rejects(failures, label, call, reason):
    try:
        call()
        failures.append(f"accepted {label}")
    except SystemExit as error:
        if reason not in str(error):
            failures.append(f"rejected {label} for the wrong reason: {error}")


def main():
    expected = b"abc"
    artifact = {
        "name": "zhuyin.db",
        "url": "https://example.invalid/zhuyin.db",
        "sha256": hashlib.sha256(expected).hexdigest(),
        "size": len(expected),
    }
    module = fetch_language_dictionaries
    original_urlopen = module.urllib.request.urlopen
    original_lock = module.LOCK
    original_argv = sys.argv
    failures = []
    try:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            destination = root / artifact["name"]

            # No lock yet: packaging calls the fetcher unconditionally, so it must skip with exit 0 and download nothing.
            module.LOCK = module.ROOT / "target" / "no-such-language-dictionaries.lock.json"
            sys.argv = ["fetch_language_dictionaries.py", "--out", str(root / "out")]
            module.urllib.request.urlopen = lambda *_args, **_kwargs: failures.append("downloaded without a lock")
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                module.main()
            if "skipped:" not in stderr.getvalue():
                failures.append(f"no skipped line without a lock: {stderr.getvalue()!r}")
            if (root / "out").exists():
                failures.append("created the output directory without a lock")
            # --list-databases without a lock prints nothing, so a release that requires the dictionaries fails rather than requiring none.
            sys.argv = ["fetch_language_dictionaries.py", "--list-databases"]
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                module.main()
            if stdout.getvalue():
                failures.append(f"listed databases without a lock: {stdout.getvalue()!r}")

            # --list-databases names only the pinned *.db files, in lock order, never a licence, and downloads nothing: the release staging requires exactly these.
            synthetic_lock = root / "language-dictionaries.lock.json"
            synthetic_lock.write_text(json.dumps({"artifacts": [
                {"name": "alpha.db"}, {"name": "alpha_LICENSE.txt"}, {"name": "beta.db"}, {"name": "beta_LICENSE.txt"},
            ]}), encoding="utf-8")
            module.LOCK = synthetic_lock
            module.urllib.request.urlopen = lambda *_args, **_kwargs: failures.append("downloaded while listing databases")
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                module.main()
            if stdout.getvalue().split() != ["alpha.db", "beta.db"]:
                failures.append(f"--list-databases printed {stdout.getvalue()!r}")
            synthetic_lock.unlink()
            module.LOCK = original_lock
            sys.argv = original_argv

            module.urllib.request.urlopen = lambda *_args, **_kwargs: failures.append("opened a non-HTTPS source")
            rejects(failures, "a non-HTTPS source", lambda: module.fetch({**artifact, "url": "http://example.invalid/zhuyin.db"}, destination), "non-HTTPS")

            with contextlib.redirect_stderr(io.StringIO()):
                for label, response in [
                    ("an oversized Content-Length", Response([], "4")),
                    ("an oversized stream", Response([b"ab", b"cd"])),
                ]:
                    module.urllib.request.urlopen = lambda *_args, **_kwargs: response
                    rejects(failures, label, lambda: module.fetch(artifact, destination), "larger than the lock")
                    if label == "an oversized Content-Length" and response.reads:
                        failures.append("read a body after an oversized Content-Length")
                    if list(root.iterdir()):
                        failures.append(f"left a partial file after {label}")

                module.urllib.request.urlopen = lambda *_args, **_kwargs: Response([b"abd"])
                rejects(failures, "a digest mismatch", lambda: module.fetch(artifact, destination), "expected")
                if list(root.iterdir()):
                    failures.append("left a file after a digest mismatch")

                module.urllib.request.urlopen = lambda *_args, **_kwargs: Response([expected])
                module.fetch(artifact, destination)
            if not destination.is_file() or destination.read_bytes() != expected:
                failures.append("rejected a valid response without Content-Length")
    finally:
        module.urllib.request.urlopen = original_urlopen
        module.LOCK = original_lock
        sys.argv = original_argv

    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        return 1
    print("language dictionary downloads skip without a lock and enforce HTTPS, the lock size and the digest; --list-databases names the pinned databases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
