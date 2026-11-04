#!/usr/bin/env python3
"""从词库 dry-run artifact 的 `msime-SHA256SUMS.txt` 生成一份临时资源锁，用来在候选词库上跑评测。

为什么要有它：评测二进制（`convert_eval`、`typing_sim`）按资源锁核对资源目录，目录里的文件必须与锁逐一对应、长度和 SHA-256 都对上（`ResourceStore::verify`），否则拒绝运行。候选词库来自词库仓库 `release-built-dictionaries.yml` 的 workflow_dispatch（publish=false）artifact，还没有发布地址，提交的 `resources/desktop-dictionary.lock.json` 也不认识它。锁的校验只要求 URL 是 https，并不下载核对，所以临时锁的 URL 默认写成 `https://dry-run.invalid/<文件名>`：`.invalid` 永远解析不出来，这份锁不可能被误用来下载东西。

做法：以提交的桌面锁为模板，保持文件清单和顺序；清单里在 SUMS 中出现的文件换成 artifact 里的 SHA-256 和实际长度（先逐个核对文件与 SUMS 一致），`sentence-model.safetensors` 不在词库 artifact 里，原样保留，`--without-model` 时去掉，就是方案 4.2 说的不挂模型的那一臂。`source_commit` 取 artifact 里 `msime-dictionary-manifest.json` 的 `source.commit`（构建这份词库的 msime 提交），与桌面锁的口径相同。artifact 里有、模板里没有的文件（粤拼、注音等语言词库）不进锁，打印出来。

`--stage <目录>` 另外建一个只含锁里文件的资源目录（硬链接，跨盘时复制），挂模型的那一臂还要 `--model` 指向一份 `sentence-model.safetensors`（通常是现有已校验资源目录里的那一份）。artifact 目录本身多出语言词库，不能直接当资源目录用。

用法（在仓库根目录）：

    python3 scripts/resource_lock_from_sums.py --artifact <解压后的 dry-run artifact 目录> --out <临时锁.json> \\
        [--base-lock resources/desktop-dictionary.lock.json] [--url-base https://dry-run.invalid/] [--source-commit <40 位>] \\
        [--without-model] [--stage <新资源目录> [--model <sentence-model.safetensors>]]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SUMS = "msime-SHA256SUMS.txt"
MANIFEST = "msime-dictionary-manifest.json"
MODEL = "sentence-model.safetensors"
HEX = set("0123456789abcdef")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def is_hex(text: str, length: int) -> bool:
    return len(text) == length and set(text) <= HEX


def parse_sums(path: Path) -> dict:
    """`sha256sum` 的输出：每行 64 位十六进制、两个空格（或空格加 `*`）、文件名。不带目录的文件名才收，重复的名字报错。"""
    sums = {}
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        digest, _, name = line.partition(" ")
        name = name[1:] if name.startswith((" ", "*")) else name
        if not is_hex(digest, 64) or not name or "/" in name or name.startswith("."):
            raise SystemExit(f"{path}:{number}: not a sha256sum line: {line}")
        if name in sums:
            raise SystemExit(f"{path}:{number}: {name} listed twice")
        sums[name] = digest
    return sums


def build_lock(artifact: Path, base: dict, url_base: str, without_model: bool, source_commit: str | None) -> tuple:
    """返回（临时锁，artifact 里有但模板里没有的文件名）。"""
    if not url_base.startswith("https://"):
        raise SystemExit("--url-base must be https:// (ResourceSet::validate rejects anything else)")
    if not url_base.endswith("/"):
        url_base += "/"
    sums = parse_sums(artifact / SUMS)
    artifacts = []
    for entry in base["artifacts"]:
        name = entry["name"]
        if name == MODEL and name not in sums:
            if not without_model:
                artifacts.append(dict(entry))
            continue
        if name not in sums:
            raise SystemExit(f"{name} is in the base lock but not in {SUMS}; this artifact is not a full desktop build")
        path = artifact / name
        if not path.is_file():
            raise SystemExit(f"{path} is listed in {SUMS} but missing")
        actual = sha256(path)
        if actual != sums[name]:
            raise SystemExit(f"{path} has SHA-256 {actual}, {SUMS} says {sums[name]}")
        artifacts.append({"name": name, "url": url_base + name, "sha256": actual, "size": path.stat().st_size})
    if source_commit is None:
        manifest = json.loads((artifact / MANIFEST).read_text(encoding="utf-8"))
        source_commit = manifest["source"]["commit"]
    if not is_hex(source_commit, 40):
        raise SystemExit(f"source commit {source_commit!r} is not a 40-digit lowercase hex commit")
    extra = sorted(set(sums) - {entry["name"] for entry in base["artifacts"]})
    return {"source_commit": source_commit, "artifacts": artifacts}, extra


def stage(lock: dict, artifact: Path, model: Path | None, directory: Path) -> None:
    """建一个只含锁里文件的资源目录：硬链接，跨盘或文件系统不支持时复制。目录必须不存在或为空。"""
    if directory.exists() and any(directory.iterdir()):
        raise SystemExit(f"{directory} is not empty")
    directory.mkdir(parents=True, exist_ok=True)
    for entry in lock["artifacts"]:
        if entry["name"] == MODEL:
            if model is None:
                raise SystemExit(f"the lock keeps {MODEL}; pass --model <file> or --without-model")
            source = model
            if sha256(source) != entry["sha256"]:
                raise SystemExit(f"{source} does not match the locked {MODEL}")
        else:
            source = artifact / entry["name"]
        target = directory / entry["name"]
        try:
            os.link(source, target)
        except OSError:
            shutil.copyfile(source, target)
        os.chmod(target, 0o600)


def main(argv: list | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--artifact", type=Path, required=True, help=f"directory holding {SUMS} and the files it lists")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--base-lock", type=Path, default=ROOT / "resources/desktop-dictionary.lock.json")
    parser.add_argument("--url-base", default="https://dry-run.invalid/")
    parser.add_argument("--source-commit")
    parser.add_argument("--without-model", action="store_true", help=f"drop {MODEL}: the arm that measures the engine alone")
    parser.add_argument("--stage", type=Path, help="also create a resource directory holding exactly the locked files")
    parser.add_argument("--model", type=Path, help=f"the {MODEL} to stage when the lock keeps it")
    args = parser.parse_args(argv)

    base = json.loads(args.base_lock.read_text(encoding="utf-8"))
    lock, extra = build_lock(args.artifact, base, args.url_base, args.without_model, args.source_commit)
    args.out.write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8")
    print(f"{args.out}: {len(lock['artifacts'])} artifacts, source_commit {lock['source_commit']}", file=sys.stderr)
    if extra:
        print(f"not in the base lock, left out: {', '.join(extra)}", file=sys.stderr)
    if args.stage:
        stage(lock, args.artifact, args.model, args.stage)
        print(f"{args.stage}: staged; run the eval with --resources {args.stage} --resource-lock {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
