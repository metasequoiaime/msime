#!/usr/bin/env python3
"""保存 ELF 原生符号与构建清单，并校验模块身份后离线还原偏移。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import zipfile


def run(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def identity(path, prefix):
    text = run(prefix + "readelf", "-h", "-n", str(path))
    build_id = re.search(r"Build ID:\s*([0-9a-fA-F]+)", text)
    machine = re.search(r"Machine:\s*(.+)", text)
    if not build_id or not machine:
        raise ValueError("ELF 缺少 build-id 或目标架构: " + path.name)
    return build_id[1].lower(), machine[1].strip()


def revision():
    explicit = os.environ.get("MSIME_BUILD_REVISION")
    if explicit:
        return explicit
    root = Path(__file__).resolve().parent.parent
    try:
        if Path(run("git", "-C", str(root), "rev-parse", "--show-toplevel").strip()).resolve() == root:
            return run("git", "-C", str(root), "rev-parse", "HEAD").strip()
    except (OSError, subprocess.CalledProcessError):
        pass
    return "unknown"


def collect(args):
    modules = []
    payloads = {}
    for root in args.root:
        for path in sorted(root.rglob("*")):
            if path.is_symlink() or not path.is_file() or "CMakeFiles" in path.parts:
                continue
            # 只归档自己的最终模块，不收集目标文件、设备日志、系统库或模型。
            if not re.match(r"^(?:lib)?(?:msime|MSIME|fcitx5-msime)", path.name) or path.suffix in (".o", ".a", ".rlib"):
                continue
            with path.open("rb") as file:
                if file.read(4) != b"\x7fELF":
                    continue
            build_id, machine = identity(path, args.tool_prefix)
            sections = run(args.tool_prefix + "readelf", "-S", "-W", str(path))
            if ".debug_line" not in sections:
                raise ValueError("缺少行号调试信息，必须在 strip 前归档: " + path.name)
            hasher = hashlib.sha256()
            with path.open("rb") as file:
                for chunk in iter(lambda: file.read(1024 * 1024), b""):
                    hasher.update(chunk)
            digest = hasher.hexdigest()
            archive = "modules/" + digest[:16] + "/" + path.name
            if archive in payloads:
                continue
            payloads[archive] = path
            modules.append({"name": path.name, "build_id": build_id, "machine": machine,
                            "sha256": digest, "file": archive})
    if not modules:
        raise ValueError("未找到带调试信息的产品 ELF 模块")
    manifest = {"format": 1, "platform": args.platform, "version": args.version,
                "revision": revision(), "run": os.environ.get("MSIME_BUILD_RUN", os.environ.get("GITHUB_RUN_ID", "")),
                "attempt": os.environ.get("GITHUB_RUN_ATTEMPT", ""), "modules": modules}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(args.output, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
        for name, path in payloads.items():
            archive.write(path, name)
    print(str(args.output) + ": " + str(len(modules)) + " modules")


def symbolize(args):
    if not re.fullmatch(r"[0-9a-fA-F]+", args.build_id) or not re.fullmatch(r"0x[0-9a-fA-F]+", args.offset):
        raise ValueError("build-id 必须为十六进制，偏移必须以 0x 开头")
    with zipfile.ZipFile(args.archive) as archive:
        manifest = json.loads(archive.read("manifest.json"))
        # ELF 主程序的模块快照可能没有文件名；仍须按 build-id 唯一命中，不能猜用同名文件。
        modules = [module for module in manifest["modules"]
                   if (module["name"] == args.module or args.module == "<executable>")
                   and module["build_id"] == args.build_id.lower()]
        if len(modules) != 1:
            raise ValueError("模块与 build-id 未唯一命中，拒绝用其他构建猜测符号")
        module = modules[0]
        data = archive.read(module["file"])
        if hashlib.sha256(data).hexdigest() != module["sha256"]:
            raise ValueError("符号文件 SHA-256 不匹配")
    # 不解压清单提供的路径，只写入本次私有临时目录里的固定文件名。
    with tempfile.TemporaryDirectory(prefix="msime-symbolize-") as temporary:
        binary = Path(temporary) / "module.elf"
        binary.write_bytes(data)
        if identity(binary, args.tool_prefix)[0] != args.build_id.lower():
            raise ValueError("ELF 自身的 build-id 与清单不符")
        print(run(args.tool_prefix + "addr2line", "-f", "-C", "-i", "-e", str(binary), args.offset), end="")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    capture = commands.add_parser("collect")
    capture.add_argument("--platform", choices=("android", "linux", "harmony"), required=True)
    capture.add_argument("--version", required=True)
    capture.add_argument("--root", type=Path, action="append", required=True)
    capture.add_argument("--output", type=Path, required=True)
    capture.add_argument("--tool-prefix", default="")
    capture.set_defaults(action=collect)
    resolve = commands.add_parser("symbolize")
    resolve.add_argument("archive", type=Path)
    resolve.add_argument("module")
    resolve.add_argument("build_id")
    resolve.add_argument("offset")
    resolve.add_argument("--tool-prefix", default="")
    resolve.set_defaults(action=symbolize)
    args = parser.parse_args()
    args.action(args)


if __name__ == "__main__":
    main()
