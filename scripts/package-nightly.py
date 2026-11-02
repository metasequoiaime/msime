#!/usr/bin/env python3
"""暂存 nightly 原生安装文件，并说明 iOS TestFlight 安装入口。"""

import argparse
from datetime import datetime, timezone
import hashlib
from pathlib import Path
import shutil


PLATFORM_SUFFIXES = {
    "android": (".apk",),
    "macos": (".pkg",),
    "linux": (".deb", ".rpm", ".pkg.tar.zst"),
    "harmony": (".hap",),
    "windows": (".exe",),
}
ASSET_LIMIT = 2 * 1024**3 - 1


def checksum(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def platform_files(source, platform, limit=ASSET_LIMIT):
    files = sorted(path for path in (source / platform).rglob("*")
                   if path.is_file() and path.name.endswith(PLATFORM_SUFFIXES[platform]))
    if not files:
        raise ValueError(f"缺少平台分发文件：{platform}")
    names = set()
    for path in files:
        if path.name in names:
            raise ValueError(f"分发文件重名，禁止覆盖：{path.name}")
        if path.stat().st_size > limit:
            raise ValueError(f"分发文件超出附件大小限制，禁止发布：{path.name}")
        names.add(path.name)
    return files


def package(source, output, source_sha, limit=ASSET_LIMIT):
    if limit < 1:
        raise ValueError("附件大小上限必须为正数")
    if output.exists() and any(output.iterdir()):
        raise ValueError("输出目录必须为空，避免上传上一次的产物")
    selected = {}
    for platform in PLATFORM_SUFFIXES:
        for path in platform_files(source, platform, limit):
            if path.name in selected:
                raise ValueError(f"分发文件重名，禁止覆盖：{path.name}")
            selected[path.name] = path
    output.mkdir(parents=True, exist_ok=True)
    for name, path in selected.items():
        shutil.copyfile(path, output / name)
    (output / "SHA256SUMS").write_text("".join(
        f"{checksum(output / name)}  {name}\n" for name in sorted(selected)
    ))
    (output / "README.txt").write_text(
        f"nightly source: {source_sha}\nprepared at: {datetime.now(timezone.utc).isoformat()}\n\n"
        "直接下载对应平台和版本的文件，无需解压外层归档。\n"
        "Android：安装 APK 后，在系统设置中启用水杉输入法。\n"
        "Windows：双击 Setup EXE，按安装向导操作。\n"
        "macOS：pkg 安装包已用 Developer ID 签名并公证，双击按提示安装（需要管理员密码），装好后在自动打开的应用里点「立即安装」。\n"
        "Linux：根据发行版和 CPU 架构选择 DEB、RPM 或 Arch 的 .pkg.tar.zst，用系统包管理器安装。\n"
        "iOS：真机通过现有外部 TestFlight 组安装，须等 Apple 审核批准；\n"
        "模拟器构建只保留在 Actions artifacts 中，不出现在普通下载页。\n"
        "HarmonyOS：unsigned HAP 供模拟器或开发签名使用，不能直接安装到零售设备。\n\n"
        "下载校验清单后，在文件所在目录运行 sha256sum --ignore-missing -c SHA256SUMS。\n"
    )
    if any(path.stat().st_size > limit for path in output.iterdir()):
        raise ValueError("仍有附件超出大小限制，禁止发布")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("source_sha")
    args = parser.parse_args()
    package(args.source, args.output, args.source_sha)
