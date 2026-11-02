#!/usr/bin/env python3
"""Fetch the minimal ONNX Runtime static library the keystroke rerank backend links, verify it, and print the directory `ORT_LIB_LOCATION` should name.

`resources/onnxruntime-minimal.lock.json` pins one gzip-compressed library per Rust target triple: its URL, the archive's SHA-256 and size, and the SHA-256 and size of the library inside it (`libonnxruntime.a`, or `onnxruntime.lib` for MSVC targets, the names ort-sys looks for). A download that does not match is discarded before anything is unpacked, and an unpacked library that does not match is discarded before it replaces anything. How the libraries are built is in `tools/onnxruntime-minimal/README.md`.

Idempotent: the verified archive is cached under `<out>/.archive/`, and nothing is done when the library in `<out>` is already the locked one. ort-sys copies the library into its own build directory and does not copy it again when it changes, so after this script replaces a library run `cargo clean -p ort-sys`.

usage: fetch_onnxruntime.py --target <triple> [--out <directory>]   (default: target/onnxruntime/<triple>)
       fetch_onnxruntime.py --list
"""
import argparse
import gzip
import hashlib
import json
import sys
import tempfile
from pathlib import Path

import download_retry

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/onnxruntime-minimal.lock.json"
CHUNK = 1 << 20


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(CHUNK), b""):
            sha.update(block)
    return sha.hexdigest()


def matches(path: Path, sha256: str, size: int) -> bool:
    return path.is_file() and path.stat().st_size == size and digest(path) == sha256


def download(artifact: dict, destination: Path) -> None:
    url = artifact["url"]
    if not url.startswith("https://"):
        raise SystemExit(f"{artifact['name']}: refusing a non-HTTPS source")
    print(f"  fetching {artifact['name']} ({artifact['size'] / 1e6:.1f} MB)", file=sys.stderr)
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Staged beside the destination so a failed or mismatched download never sits where the next run would take it as finished.
    staged = tempfile.NamedTemporaryFile(dir=destination.parent, delete=False)
    staged_path = Path(staged.name)
    try:
        with staged:
            with download_retry.urlopen(url, timeout=300) as response:
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
        raise SystemExit(f"{artifact['name']}: expected {artifact['sha256']} at {artifact['size']} bytes, got {actual} at {size}")
    staged_path.replace(destination)


def unpack(artifact: dict, archive: Path, library: Path) -> None:
    staged = tempfile.NamedTemporaryFile(dir=library.parent, delete=False)
    staged_path = Path(staged.name)
    try:
        with staged, gzip.open(archive, "rb") as source:
            size = 0
            while block := source.read(CHUNK):
                size += len(block)
                # A verified archive cannot grow, but the bound keeps a lock with a wrong library size from filling the disk before the digest check.
                if size > artifact["library_size"]:
                    raise SystemExit(f"{artifact['name']}: unpacks larger than the lock's {artifact['library']}")
                staged.write(block)
    except BaseException:
        staged_path.unlink(missing_ok=True)
        raise
    if not matches(staged_path, artifact["library_sha256"], artifact["library_size"]):
        actual, size = digest(staged_path), staged_path.stat().st_size
        staged_path.unlink(missing_ok=True)
        raise SystemExit(f"{artifact['name']}: unpacked {artifact['library']} should be {artifact['library_sha256']} at {artifact['library_size']} bytes, got {actual} at {size}")
    staged_path.replace(library)


def install(artifact: dict, out: Path) -> Path:
    library = out / artifact["library"]
    if matches(library, artifact["library_sha256"], artifact["library_size"]):
        print(f"  {artifact['library']}: already the locked build", file=sys.stderr)
        return out
    out.mkdir(parents=True, exist_ok=True)
    archive = out / ".archive" / artifact["name"]
    if not matches(archive, artifact["sha256"], artifact["size"]):
        download(artifact, archive)
    replacing = library.exists()
    unpack(artifact, archive, library)
    if replacing:
        print(f"  replaced a different {artifact['library']}; run `cargo clean -p ort-sys` so the next build links this one", file=sys.stderr)
    return out


def main() -> None:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--target", choices=sorted(lock["targets"]), help="Rust target triple")
    parser.add_argument("--out", type=Path, help="output directory (default: target/onnxruntime/<triple>)")
    parser.add_argument("--list", action="store_true", help="list the locked target triples")
    arguments = parser.parse_args()
    if arguments.list:
        print("\n".join(sorted(lock["targets"])))
        return
    if not arguments.target:
        parser.error("--target is required")
    out = arguments.out or ROOT / "target/onnxruntime" / arguments.target
    print(install(lock["targets"][arguments.target], out.resolve()))


if __name__ == "__main__":
    main()
