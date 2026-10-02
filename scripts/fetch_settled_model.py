#!/usr/bin/env python3
"""Fetch the desktop settled-rerank model the three desktop installers ship.

The model that runs once typing stops is 25 MB and only the desktop platforms want it: it lifts
top-1 on the harvested failure set from 0.123 to 0.613, and it costs p95 153ms per keystroke, which
is why it runs on the settle timer rather than on the keystroke path. On iOS that 25 MB resident is
most of what a keyboard extension is allowed, which is the reason the small preset exists.

It is deliberately *not* in ``resources/desktop-dictionary.lock.json``. That lock is shared by all
six platforms and ``ResourceStore::verify`` requires a resource directory to match it exactly, so a
desktop-only entry there would mean teaching the manifest, the Rust verifier, the PowerShell
verifier, four staging scripts and a CMake parser what a platform is — all on the path that
guarantees a shipped dictionary is intact. A second one-artifact manifest costs none of that.

The same discipline applies regardless: the lock pins a SHA-256 and a size, and a download that
does not match them is discarded rather than installed. Idempotent — a file already present and
matching is left alone, so installers can call this unconditionally.

usage: fetch_settled_model.py [--out <directory>]   (default: target/settled-model)
"""
import argparse
import hashlib
import json
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/settled-model.lock.json"
DEFAULT_OUT = ROOT / "target/settled-model"
CHUNK = 1 << 20


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(CHUNK), b""):
            sha.update(block)
    return sha.hexdigest()


def fetch(artifact: dict, destination: Path) -> None:
    url = artifact["url"]
    if not url.startswith("https://"):
        raise SystemExit(f"{artifact['name']}: refusing a non-HTTPS source")
    print(f"  fetching {artifact['name']} ({artifact['size'] / 1e6:.1f} MB)", file=sys.stderr)
    # Staged beside the destination, so a failed or mismatched download never leaves a partial
    # file where an installer would pick it up as finished.
    staged = tempfile.NamedTemporaryFile(dir=destination.parent, delete=False)
    staged_path = Path(staged.name)
    try:
        with staged:
            with urllib.request.urlopen(url, timeout=300) as response:
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
        raise SystemExit(
            f"{artifact['name']}: expected {artifact['sha256']} at {artifact['size']} bytes, "
            f"got {actual} at {size}"
        )
    staged_path.replace(destination)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    arguments = parser.parse_args()
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    arguments.out.mkdir(parents=True, exist_ok=True)
    for artifact in lock["artifacts"]:
        destination = arguments.out / artifact["name"]
        if (
            destination.is_file()
            and destination.stat().st_size == artifact["size"]
            and digest(destination) == artifact["sha256"]
        ):
            print(f"  {artifact['name']}: already at the locked digest", file=sys.stderr)
            continue
        fetch(artifact, destination)
    print(arguments.out)


if __name__ == "__main__":
    main()
