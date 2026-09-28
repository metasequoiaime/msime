#!/usr/bin/env python3
"""Point engine-lock.json at another MSIME-Engine commit: `relock_engine.py <commit>`.

Used by .github/workflows/engine-update.yml. Only `commit`, `archive` and `sha256` change; the
overlays and the `dependencies` pins are left as they are, so a commit that moves a submodule
pointer leaves those pins stale for the bump PR's CI and triage to catch.

The digest is recorded, not independently verified: it is whatever GitHub serves for the commit
today. The archive is checked to have the single `<repo>-<commit>/` root a GitHub tarball of that
commit has, so an error page or another repository's archive is not pinned. See fetch_engine.py's
docstring for why a digest that is merely stable is not proof of content.
"""
import hashlib
import io
import json
import re
import sys
import tarfile
import urllib.request
from pathlib import Path
from typing import Callable

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "engine-lock.json"
# Keep the scheduled relock download bounded even though the lock records only a digest.
MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
DOWNLOAD_CHUNK_BYTES = 1024 * 1024


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=300) as response:
        advertised = response.headers.get("Content-Length")
        if advertised is not None:
            try:
                if int(advertised) > MAX_ARCHIVE_BYTES:
                    raise RuntimeError(f"archive exceeds the {MAX_ARCHIVE_BYTES} byte limit")
            except ValueError:
                pass
        chunks = []
        size = 0
        while chunk := response.read(DOWNLOAD_CHUNK_BYTES):
            if size + len(chunk) > MAX_ARCHIVE_BYTES:
                raise RuntimeError(f"archive exceeds the {MAX_ARCHIVE_BYTES} byte limit")
            chunks.append(chunk)
            size += len(chunk)
        return b"".join(chunks)


def relock(lock: dict, commit: str, download: Callable[[str], bytes] = fetch) -> dict:
    """The lock at `commit`, with every other field and the key order unchanged."""
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError(f"not a full commit id: {commit!r}")
    archive = f"https://github.com/{lock['repository']}/archive/{commit}.tar.gz"
    data = download(archive)
    if len(data) > MAX_ARCHIVE_BYTES:
        raise RuntimeError(f"archive exceeds the {MAX_ARCHIVE_BYTES} byte limit")
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as source:
        roots = {member.name.split("/", 1)[0] for member in source.getmembers() if member.name}
    if len(roots) != 1 or not roots.pop().endswith(f"-{commit}"):
        raise RuntimeError(f"{archive} is not a single-root archive of {commit}")
    updated = dict(lock)
    updated.update(commit=commit, archive=archive, sha256=hashlib.sha256(data).hexdigest())
    return updated


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: relock_engine.py <commit>", file=sys.stderr)
        return 2
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    try:
        updated = relock(lock, sys.argv[1])
    except (ValueError, RuntimeError, OSError, tarfile.TarError) as error:
        print(f"relock_engine: {error}", file=sys.stderr)
        return 1
    LOCK.write_text(json.dumps(updated, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"engine-lock.json -> {updated['commit']} ({updated['sha256']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
