#!/usr/bin/env python3
"""Fetch the sherpa-onnx runtime a platform's on-device speech recognition loads.

Every platform recognizes locally through the same pinned sherpa-onnx release, so a model that works on one works on all of them. The runtime is a prebuilt upstream artifact rather than something this repository compiles: onnxruntime alone is a larger build than the rest of the tree, and the upstream binaries are what its own CI tests.

`resources/voice-runtime.lock.json` pins one artifact per platform by SHA-256 and size. A download that does not match is discarded before anything is extracted. The desktop hosts load the C API with dlopen/LoadLibrary (see `shared/voice/LocalAsr.cpp`), so the output for them is just the libraries, flat; iOS gets the whole xcframework tree, and Android and HarmonyOS get the package (`.aar` / `.har`) untouched for their build systems to consume.

Idempotent: the verified archive is cached under `<out>/.archive/`, and outputs already present are rewritten from it rather than downloaded again.

usage: fetch_voice_runtime.py --platform <name> [--out <directory>]   (default: target/voice-runtime/<name>)
       fetch_voice_runtime.py --list
"""
import argparse
import hashlib
import json
import shutil
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path, PurePosixPath

import download_retry

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/voice-runtime.lock.json"
CHUNK = 1 << 20


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(CHUNK), b""):
            sha.update(block)
    return sha.hexdigest()


def matches(path: Path, artifact: dict) -> bool:
    return path.is_file() and path.stat().st_size == artifact["size"] and digest(path) == artifact["sha256"]


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
        raise SystemExit(
            f"{artifact['name']}: expected {artifact['sha256']} at {artifact['size']} bytes, got {actual} at {size}"
        )
    staged_path.replace(destination)


def safe_member(name: str) -> bool:
    path = PurePosixPath(name)
    return not path.is_absolute() and ".." not in path.parts


def extract(archive: Path, kind: str, into: Path) -> None:
    if kind == "zip":
        with zipfile.ZipFile(archive) as bundle:
            for member in bundle.namelist():
                if not safe_member(member):
                    raise SystemExit(f"{archive.name}: unsafe member {member}")
            bundle.extractall(into)
        # zipfile drops the executable bit; the Mach-O images inside need it for codesign and dlopen alike.
        for path in into.rglob("*"):
            if path.is_file() and path.suffix in ("", ".dylib"):
                path.chmod(0o755)
    elif kind == "tar.bz2":
        with tarfile.open(archive, "r:bz2") as bundle:
            if hasattr(tarfile, "data_filter"):
                bundle.extractall(into, filter="data")
            else:
                # No extraction filters before 3.9.17/3.10.12/3.11.4 (macOS's stock 3.9.6): check names and links by hand.
                for member in bundle.getmembers():
                    if not safe_member(member.name) or ((member.issym() or member.islnk()) and not safe_member(member.linkname)):
                        raise SystemExit(f"{archive.name}: unsafe member {member.name}")
                bundle.extractall(into)
    else:
        raise SystemExit(f"{archive.name}: unknown archive kind {kind}")


def install(platform: str, artifact: dict, out: Path) -> None:
    cache = out / ".archive" / artifact["name"]
    if matches(cache, artifact):
        print(f"  {artifact['name']}: already at the locked digest", file=sys.stderr)
    else:
        download(artifact, cache)
    if artifact["archive"] == "none":
        target = out / artifact["name"]
        shutil.copyfile(cache, target)
        return
    with tempfile.TemporaryDirectory(dir=out) as scratch:
        scratch_path = Path(scratch)
        extract(cache, artifact["archive"], scratch_path)
        if artifact["tree"]:
            source = scratch_path / artifact["tree"]
            target = out / PurePosixPath(artifact["tree"]).name
            if target.exists():
                shutil.rmtree(target)
            shutil.move(str(source), target)
        for library in artifact["libraries"]:
            source = scratch_path / library
            if not source.is_file():
                raise SystemExit(f"{platform}: {library} is missing from {artifact['name']}")
            shutil.move(str(source), out / PurePosixPath(library).name)


def main() -> None:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    parser = argparse.ArgumentParser()
    parser.add_argument("--platform", choices=sorted(lock["platforms"]))
    parser.add_argument("--out", type=Path)
    parser.add_argument("--list", action="store_true", help="print the platform names and exit")
    arguments = parser.parse_args()
    if arguments.list:
        print("\n".join(sorted(lock["platforms"])))
        return
    if not arguments.platform:
        parser.error("--platform is required")
    out = arguments.out or ROOT / "target/voice-runtime" / arguments.platform
    out.mkdir(parents=True, exist_ok=True)
    install(arguments.platform, lock["platforms"][arguments.platform], out)
    print(out)


if __name__ == "__main__":
    main()
