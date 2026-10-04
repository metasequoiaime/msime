#!/usr/bin/env python3
"""从 msime.ebuild.in 渲染某个 Linux 发布版本的 Gentoo ebuild。

用法：render.py VERSION [--out OVERLAY_DIR] [--no-crates]

    VERSION      发布版本 MAJOR.MINOR.PATCH，对应 linux-vVERSION 标签。必须在该标签的检出里运行：Cargo.lock、rust-toolchain.toml 和 resources/*.lock.json 都取自当前检出。
    --out        输出的 overlay 根目录，默认是本脚本所在的 gentoo 目录；ebuild 写到 <out>/app-i18n/msime/msime-VERSION.ebuild。
    --no-crates  不跑 pycargoebuild，CRATES、GIT_CRATES 与 crate 许可证留空。只用于没有 Gentoo 环境时检查渲染结果，产物不能发布。

渲染的内容：
    RUST_MIN_VER  rust-toolchain.toml 钉住的版本。
    SRC_URI       随包资源按锁文件里的地址列出，distfile 名带锁定 SHA-256 的前 12 位；MSIME_RESOURCES 记录每个 distfile 该放到哪里。语音运行库按架构区分。
    CRATES 等     由 pycargoebuild（app-portage/pycargoebuild）按 Cargo.lock 填入，它要 Gentoo 仓库里的许可证映射，所以完整渲染要在 Gentoo 容器里跑。

渲染之后还要生成 Manifest（`pkgdev manifest` 或 `ebuild msime-VERSION.ebuild manifest`），它会下载全部 distfile 并记下校验值；ebuild 里不再另抄一份哈希。
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
TEMPLATE = HERE / "app-i18n" / "msime" / "msime.ebuild.in"

# 随包资源：锁文件、构建时放到 WORKDIR 下的目录。与 package-container.sh 调用的 fetch 脚本一一对应。
RESOURCE_LOCKS = [
    ("handwriting-model.lock.json", "handwriting-model"),
    ("offline-glosses.lock.json", "offline-glosses"),
    ("language-dictionaries.lock.json", "language-dictionaries"),
]
# 语音运行库按 Gentoo 架构关键字取锁文件里对应的平台；fetch_voice_runtime.py 从 <out>/.archive/ 下的缓存解包。
VOICE_PLATFORMS = {"amd64": "linux-x86_64", "arm64": "linux-aarch64"}
# ebuild 构建的 workspace 成员：msime-host-api、msime-mcp-server 与 msime-desktop。
CARGO_MEMBERS = ["crates/host-api", "crates/mcp-server", "apps/desktop/src-tauri"]


def distfile(artifact: dict) -> str:
    return f"msime-res-{artifact['sha256'][:12]}-{artifact['name']}"


def rust_channel() -> str:
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))
    channel = toolchain["toolchain"]["channel"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", channel):
        sys.exit(f"rust-toolchain.toml pins {channel!r}, not a MAJOR.MINOR.PATCH release")
    return channel


def resources() -> tuple[list[str], list[str]]:
    src_uri: list[str] = []
    entries: list[str] = []

    def add(artifact: dict, directory: str, use: str | None = None) -> None:
        line = f"{artifact['url']} -> {distfile(artifact)}"
        src_uri.append(f"\t{use}? ( {line} )" if use else f"\t{line}")
        entries.append(f'\t"{directory} {artifact["name"]} {distfile(artifact)}"')

    for lock_name, directory in RESOURCE_LOCKS:
        lock = json.loads((ROOT / "resources" / lock_name).read_text(encoding="utf-8"))
        if not lock["artifacts"]:
            sys.exit(f"resources/{lock_name} pins no artifacts")
        for artifact in lock["artifacts"]:
            add(artifact, directory)
    voice = json.loads((ROOT / "resources" / "voice-runtime.lock.json").read_text(encoding="utf-8"))
    for keyword, platform in VOICE_PLATFORMS.items():
        add(voice["platforms"][platform], "voice-runtime/.archive", keyword)
    return src_uri, entries


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("version")
    parser.add_argument("--out", type=Path, default=HERE)
    parser.add_argument("--no-crates", action="store_true")
    arguments = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", arguments.version):
        sys.exit(f"version must be MAJOR.MINOR.PATCH: {arguments.version}")

    src_uri, entries = resources()
    text = TEMPLATE.read_text(encoding="utf-8")
    for placeholder, value in {
        "@RUST_MIN_VER@": rust_channel(),
        "@RESOURCE_SRC_URI@": "\n".join(src_uri),
        "@RESOURCES@": "\n".join(entries),
    }.items():
        if placeholder not in text:
            sys.exit(f"{TEMPLATE.name} has no {placeholder}")
        text = text.replace(placeholder, value)
    leftover = re.findall(r"@[A-Z_]+@", text)
    if leftover:
        sys.exit(f"unrendered placeholders in {TEMPLATE.name}: {sorted(set(leftover))}")

    package_dir = arguments.out / "app-i18n" / "msime"
    package_dir.mkdir(parents=True, exist_ok=True)
    output = package_dir / f"msime-{arguments.version}.ebuild"
    if arguments.no_crates:
        output.write_text(text, encoding="utf-8")
    else:
        if shutil.which("pycargoebuild") is None:
            sys.exit("pycargoebuild is required (emerge app-portage/pycargoebuild); pass --no-crates only to inspect the output")
        with tempfile.TemporaryDirectory() as scratch:
            staged = Path(scratch) / output.name
            staged.write_text(text, encoding="utf-8")
            # -C 把 crate 逐个列进 CRATES；-i 只改写已有的 CRATES、GIT_CRATES 与「# Dependent crate licenses」下的 LICENSE+=，其余保持模板原样；-M 不顺手生成 Manifest，留给发布步骤。pycargoebuild 不接受 workspace 根目录，传入实际构建的三个成员。
            members = [str(ROOT / member) for member in CARGO_MEMBERS]
            subprocess.run(["pycargoebuild", "-C", "-M", "--input", str(staged), "--output", str(output), *members], check=True)
    if arguments.out.resolve() != HERE:
        for name in ("metadata.xml",):
            shutil.copyfile(HERE / "app-i18n" / "msime" / name, package_dir / name)
    print(output)


if __name__ == "__main__":
    main()
