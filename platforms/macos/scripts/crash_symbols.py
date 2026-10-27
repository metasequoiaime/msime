#!/usr/bin/env python3
"""归并并校验自有 Mach-O 的 dSYM；不把空符号包当作成功。"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path


def run(*args: str) -> str:
    return subprocess.check_output(args, text=True)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def identities(path: Path) -> dict[str, str]:
    rows = re.findall(r"UUID: ([0-9A-Fa-f-]+) \(([^)]+)\)", run("xcrun", "dwarfdump", "--uuid", str(path)))
    result = {arch: uuid.upper() for uuid, arch in rows}
    if not result or len(result) != len(rows):
        raise ValueError(f"缺少或重复的 Mach-O UUID：{path.name}")
    return result


def dwarf_file(dsym: Path) -> Path:
    files = list((dsym / "Contents/Resources/DWARF").glob("*"))
    if len(files) != 1 or not files[0].is_file():
        raise ValueError(f"dSYM 必须包含唯一的 DWARF 文件：{dsym.name}")
    return files[0]


def has_lines(dsym: Path, arch: str) -> bool:
    # Rust 行表可能很大，只读到第一条有源码行的指令，不把整份 DWARF 输出留在内存。
    with tempfile.TemporaryFile() as errors:
        child = subprocess.Popen(
            ["xcrun", "dwarfdump", "--debug-line", "--arch", arch, str(dsym)],
            stdout=subprocess.PIPE, stderr=errors, text=True,
        )
        found = False
        try:
            for line in child.stdout:
                if re.match(r"^0x[0-9a-fA-F]+\s+[1-9][0-9]*\s+\d+\s+\d+", line) and "end_sequence" not in line:
                    found = True
                    break
        finally:
            if found:
                child.terminate()
            child.communicate()
        if not found and child.returncode:
            errors.seek(0)
            raise ValueError(f"无法读取 {arch} 行表：{errors.read().decode(errors='replace')}")
        return found


def validate(binary: Path, dsym: Path) -> dict[str, str]:
    expected = identities(binary)
    if identities(dwarf_file(dsym)) != expected:
        raise ValueError(f"dSYM 的架构或 UUID 不匹配：{binary.name}")
    for arch in expected:
        if not has_lines(dsym, arch):
            raise ValueError(f"dSYM 没有可还原的源码行：{binary.name} ({arch})")
    return expected


def merge(binary: Path, output: Path, inputs: list[Path]) -> None:
    expected = identities(binary)
    actual: dict[str, str] = {}
    for source in inputs:
        for arch, uuid in identities(dwarf_file(source)).items():
            if arch in actual or expected.get(arch) != uuid:
                raise ValueError(f"dSYM 分片重复或不属于本次构建：{source.name} ({arch})")
            if not has_lines(source, arch):
                raise ValueError(f"dSYM 分片缺少源码行：{source.name} ({arch})")
            actual[arch] = uuid
    if actual != expected:
        raise ValueError(f"dSYM 分片未覆盖全部架构：{binary.name}")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".msime-dsym-", dir=output.parent) as directory:
        staged = Path(directory) / output.name
        shutil.copytree(inputs[0], staged)
        dwarf = staged / "Contents/Resources/DWARF"
        for path in dwarf.iterdir():
            path.unlink()
        subprocess.run(["xcrun", "lipo", "-create", *[str(dwarf_file(source)) for source in inputs],
                        "-output", str(dwarf / binary.name)], check=True)
        validate(binary, staged)
        if output.exists():
            shutil.rmtree(output)
        staged.rename(output)


def collect(binary: Path, output: Path, source: Path | None, version: str) -> None:
    output.mkdir(parents=True, exist_ok=True)
    target = output / (binary.name + ".dSYM")
    with tempfile.TemporaryDirectory(prefix=".msime-dsym-", dir=output) as directory:
        staged = Path(directory) / target.name
        if source is not None:
            shutil.copytree(source, staged)
        else:
            # 仅用于仍保留对象文件的原生 CMake 构建。Swift/Rust 必须传入编译器原始 dSYM。
            subprocess.run(["xcrun", "dsymutil", str(binary), "-o", str(staged)], check=True)
        uuids = validate(binary, staged)
        digest = sha256(dwarf_file(staged))
        if target.exists():
            shutil.rmtree(target)
        staged.rename(target)
    manifest_path = output / "manifest.json"
    if manifest_path.exists():
        manifest = json.loads(manifest_path.read_text())
    else:
        revision = run("git", "rev-parse", "HEAD").strip()
        manifest = {"format": 1, "platform": "macos", "version": version, "source_revision": revision,
                    "build_run": os.environ.get("GITHUB_RUN_ID", ""),
                    "build_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", ""), "modules": []}
    if manifest["version"] != version:
        raise ValueError("同一符号目录不能混入不同版本")
    manifest["modules"] = [row for row in manifest["modules"] if row["name"] != binary.name]
    manifest["modules"].append({"name": binary.name, "uuids": uuids, "dsym": target.name,
                                "dwarf_sha256": digest})
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")


def symbolize(archive: Path, module: str, uuid: str, arch: str, offset: int) -> str:
    manifest = json.loads((archive / "manifest.json").read_text())
    normalized = uuid.removeprefix("0x").replace("-", "").lower()
    matches = [row for row in manifest["modules"] if row["name"] == module
               and row["uuids"].get(arch, "").replace("-", "").lower() == normalized]
    if len(matches) != 1 or offset < 0:
        raise ValueError("找不到唯一匹配的模块、架构和 UUID")
    row = matches[0]
    dsym = (archive / row["dsym"]).resolve()
    dsym.relative_to(archive.resolve())
    dwarf = dwarf_file(dsym).resolve()
    dwarf.relative_to(dsym)
    if sha256(dwarf) != row["dwarf_sha256"] or identities(dwarf) != row["uuids"]:
        raise ValueError("符号文件的哈希或 UUID 不匹配")
    # 记录器按加载后的 Mach header 保存偏移；atos 读取链接时地址，补回该架构的 __TEXT 基址。
    layout = run("xcrun", "otool", "-arch", arch, "-l", str(dwarf))
    text = re.search(r"segname __TEXT\s+vmaddr (0x[0-9a-fA-F]+)", layout)
    if not text:
        raise ValueError("符号文件缺少 __TEXT 基址")
    address = int(text.group(1), 16) + offset
    return run("xcrun", "atos", "-arch", arch, "-o", str(dwarf), hex(address)).strip()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    combine = commands.add_parser("merge")
    combine.add_argument("binary", type=Path)
    combine.add_argument("output", type=Path)
    combine.add_argument("inputs", type=Path, nargs="+")
    archive = commands.add_parser("collect")
    archive.add_argument("binary", type=Path)
    archive.add_argument("output", type=Path)
    archive.add_argument("--dsym", type=Path)
    archive.add_argument("--version", required=True)
    resolve = commands.add_parser("symbolize", help="还原解压后的符号目录中的模块偏移")
    resolve.add_argument("archive", type=Path)
    resolve.add_argument("--module", required=True)
    resolve.add_argument("--uuid", required=True)
    resolve.add_argument("--arch", required=True)
    resolve.add_argument("--offset", type=lambda value: int(value, 0), required=True)
    args = parser.parse_args()
    if args.command == "merge":
        merge(args.binary, args.output, args.inputs)
    elif args.command == "collect":
        collect(args.binary, args.output, args.dsym, args.version)
    else:
        print(symbolize(args.archive, args.module, args.uuid, args.arch, args.offset))


if __name__ == "__main__":
    main()
