#!/usr/bin/env python3
"""`scripts/fetch_rerank_model.py` and its lock: the lock names the two directories host-api reads with the three files each loads, all from chinese-ime-lm's `rerank-v1` release; an install lays them out under those names, leaves matching files alone and removes files the lock does not name."""

import hashlib
import json
import pathlib
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import fetch_rerank_model  # noqa: E402

RELEASE = "https://github.com/metasequoiaime/chinese-ime-lm/releases/download/rerank-v1/"


def check_lock(failures):
    lock = json.loads(fetch_rerank_model.LOCK.read_text(encoding="utf-8"))
    try:
        fetch_rerank_model.check_lock(lock)
    except SystemExit as error:
        failures.append(f"the committed lock fails its own check: {error}")
    if lock["release_tag"] != "rerank-v1" or lock["license"] != "Apache-2.0":
        failures.append("the lock should pin chinese-ime-lm's rerank-v1 release, Apache-2.0")
    for model in lock["models"]:
        for artifact in model["artifacts"]:
            if artifact["url"] != RELEASE + artifact["name"]:
                failures.append(f"{artifact['name']} is not downloaded from the rerank-v1 release under its own name")
            if not artifact["name"].startswith(f"rerank-{model['shape']}."):
                failures.append(f"{artifact['name']} does not belong to the {model['shape']} model")
            if len(artifact["sha256"]) != 64 or artifact["size"] <= 0:
                failures.append(f"{artifact['name']} has no usable SHA-256 or size")
    # A lock that dropped a file or named a fourth would still pass the size checks above; the shared check has to refuse it.
    broken = json.loads(json.dumps(lock))
    broken["models"][0]["artifacts"].pop()
    try:
        fetch_rerank_model.check_lock(broken)
        failures.append("a lock missing vocab.json was accepted")
    except SystemExit:
        pass


def check_install(failures):
    contents = {name: f"{name} bytes".encode() for name in fetch_rerank_model.FILES}
    lock = {
        "models": [
            {
                "directory": directory,
                "artifacts": [
                    {
                        "name": f"{directory}-{name}",
                        "file": name,
                        "url": f"https://example.invalid/{directory}/{name}",
                        "sha256": hashlib.sha256(data).hexdigest(),
                        "size": len(data),
                    }
                    for name, data in contents.items()
                ],
            }
            for directory in fetch_rerank_model.DIRECTORIES
        ]
    }
    downloads = []

    def download(artifact, destination):
        downloads.append(artifact["name"])
        destination.write_bytes(contents[artifact["file"]])

    original_lock, original_download = fetch_rerank_model.LOCK, fetch_rerank_model.download
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        lock_path = root / "lock.json"
        lock_path.write_text(json.dumps(lock), encoding="utf-8")
        out = root / "out"
        fetch_rerank_model.LOCK, fetch_rerank_model.download = lock_path, download
        try:
            fetch_rerank_model.install(out)
            if len(downloads) != 6:
                failures.append(f"a first install should download six files, downloaded {downloads}")
            for directory_name in fetch_rerank_model.DIRECTORIES:
                for name, data in contents.items():
                    if (out / directory_name / name).read_bytes() != data:
                        failures.append(f"{directory_name}/{name} was not installed under its runtime name")
            stray = out / "rerank-model" / "rerank-8x448.ort"
            stray.write_bytes(b"left by an earlier layout")
            (out / "rerank-model-small" / "vocab.json").write_bytes(b"tampered")
            downloads.clear()
            fetch_rerank_model.install(out)
            if downloads != ["rerank-model-small-vocab.json"]:
                failures.append(f"a second install should replace only the mismatched file, downloaded {downloads}")
            if stray.exists():
                failures.append("a file the lock does not name was left in rerank-model/")
        finally:
            fetch_rerank_model.LOCK, fetch_rerank_model.download = original_lock, original_download


def main():
    failures = []
    check_lock(failures)
    check_install(failures)
    for failure in failures:
        print(f"FAIL: {failure}")
    if failures:
        return 1
    print("rerank model lock and install layout are consistent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
