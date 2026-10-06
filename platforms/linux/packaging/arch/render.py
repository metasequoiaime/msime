#!/usr/bin/env python3
"""把 msime 与 msime-bin 两个 PKGBUILD 改写到某个 Linux 发布版本，并重新生成 .SRCINFO。

用法：render.py VERSION [--sha256sums FILE] [--source-sha256 HEX] [--no-srcinfo]

    VERSION          发布版本 MAJOR.MINOR.PATCH，对应 linux-vVERSION 标签与发布。
    --sha256sums     该发布的 SHA256SUMS（release-linux.yml 上传的那份）；不给时从 GitHub 发布下载。msime-bin 的 x86_64 与 aarch64 两个 .rpm 的校验值取自这里，不另算一遍；缺了哪一个都失败。
    --source-sha256  linux-vVERSION 源码归档的 SHA-256；不给时下载 GitHub 生成的归档自己算。
    --no-srcinfo     不生成 .SRCINFO。生成它要 makepkg（Arch 上以非 root 用户运行），只在没有 Arch 环境时用来检查改写结果。

pkgrel 一律重置为 1。改写只动 pkgver、pkgrel、_rpmrel 与校验值这几行，PKGBUILD 的其余部分保持原样。
"""
from __future__ import annotations

import argparse
import hashlib
import re
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = "metasequoiaime/msime"
# CPack 生成的 .rpm 的 Release 字段，packaging.cmake 没有设置 CPACK_RPM_PACKAGE_RELEASE，默认是 1。
RPM_RELEASE = "1"
# msime.install 在安装后运行 `msime-linux-setup --register`，这个选项从 0.10.0 才有；更早的发布还按已失效的地址取方言词库（langdict-v1.0.0 已删除），源码包在 prepare() 就会失败。仓库里提交的 PKGBUILD 在第一次渲染之前停在 0.9.1，只是占位，不能推到 AUR。
FIRST_SUPPORTED = (0, 10, 0)


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=300) as response:
        return response.read()


def source_sha256(version: str) -> str:
    digest = hashlib.sha256()
    url = f"https://github.com/{REPO}/archive/refs/tags/linux-v{version}.tar.gz"
    with urllib.request.urlopen(url, timeout=300) as response:
        while chunk := response.read(1 << 20):
            digest.update(chunk)
    return digest.hexdigest()


def rpm_sha256(version: str, sums: str, arch: str) -> str:
    name = f"msime-linux-{version}-{RPM_RELEASE}.{arch}.rpm"
    for line in sums.splitlines():
        parts = line.split()
        if len(parts) == 2 and parts[1].lstrip("*") == name:
            return parts[0]
    sys.exit(f"SHA256SUMS lists no {name}")


def rewrite(path: Path, replacements: dict[str, str]) -> None:
    text = path.read_text(encoding="utf-8")
    for key, value in replacements.items():
        pattern = re.compile(rf"^{re.escape(key)}=.*$", re.MULTILINE)
        if len(pattern.findall(text)) != 1:
            sys.exit(f"{path}: expected exactly one {key}= line")
        text = pattern.sub(lambda _: f"{key}={value}", text)
    path.write_text(text, encoding="utf-8")


def write_srcinfo(directory: Path) -> None:
    result = subprocess.run(["makepkg", "--printsrcinfo"], cwd=directory, check=True, capture_output=True, text=True)
    (directory / ".SRCINFO").write_text(result.stdout, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("version")
    parser.add_argument("--sha256sums", type=Path)
    parser.add_argument("--source-sha256")
    parser.add_argument("--no-srcinfo", action="store_true")
    arguments = parser.parse_args()
    version = arguments.version
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        sys.exit(f"version must be MAJOR.MINOR.PATCH: {version}")
    if tuple(int(part) for part in version.split(".")) < FIRST_SUPPORTED:
        sys.exit(f"{version} predates msime-linux-setup --register, which msime.install runs; the AUR packages need {'.'.join(map(str, FIRST_SUPPORTED))} or later")
    if not arguments.no_srcinfo and shutil.which("makepkg") is None:
        sys.exit("makepkg is required to generate .SRCINFO; pass --no-srcinfo only to inspect the PKGBUILDs")

    if arguments.sha256sums:
        sums = arguments.sha256sums.read_text(encoding="utf-8")
    else:
        sums = fetch(f"https://github.com/{REPO}/releases/download/linux-v{version}/SHA256SUMS").decode()
    source = arguments.source_sha256 or source_sha256(version)
    if not re.fullmatch(r"[0-9a-f]{64}", source):
        sys.exit(f"not a SHA-256: {source}")

    rewrite(HERE / "msime" / "PKGBUILD", {"pkgver": version, "pkgrel": "1", "sha256sums": f"('{source}')"})
    rewrite(HERE / "msime-bin" / "PKGBUILD", {
        "pkgver": version,
        "pkgrel": "1",
        "_rpmrel": RPM_RELEASE,
        "sha256sums_x86_64": f"('{rpm_sha256(version, sums, 'x86_64')}')",
        "sha256sums_aarch64": f"('{rpm_sha256(version, sums, 'aarch64')}')",
    })
    if not arguments.no_srcinfo:
        for name in ("msime", "msime-bin"):
            write_srcinfo(HERE / name)
    for name in ("msime", "msime-bin"):
        print(HERE / name / "PKGBUILD")


if __name__ == "__main__":
    main()
