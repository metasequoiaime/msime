#!/usr/bin/env python3
"""决定一次 Release 工作流运行发布什么：由触发它的 ref 推出版本号、发布名、是否预发布和是否发布。

发版就是推 tag（AGENTS.md「发版」）：
- ref 是 `refs/tags/<前缀>X.Y.Z`：正式版，发布名 `X.Y.Z`，不是预发布。
- ref 是 `refs/tags/<前缀>X.Y.Z-beta.N`（N 从 1 起，不带前导零）或兼容旧 tag 的 `<前缀>X.Y.Z-beta`：测试版，发布名带这个后缀，一律是预发布。`--stable-only` 的工作流（crates.io、npm、Windows 7 这类没有测试版通道的）不接受它。
- tag 运行在构建前核对两件事，任一不符就失败、什么都不发：tag 里的 `X.Y.Z` 等于被打 tag 的提交里的版本号（`--version-file` 或 `--tree-version`；网页引擎和打包工具没有仓库内的版本号，只核对第二条），被打 tag 的提交是 `origin/main` 的祖先。手动输入的 version 不为空时也必须等于它。
- 别的 ref（分支、PR、nightly.yml 的复用调用）只构建不发布：版本号取手动输入或仓库内的版本号，发布名就是版本号，tag 为空。

发布与否：tag 运行发布，除非手动触发时把 publish 输入设成 false（环境变量 PUBLISH_INPUT；推 tag 的运行没有输入，为空）。别的 ref 一律不发布，publish 输入为 true 时只提示一句。

用法：release-ref.py --prefix <tag 前缀> [--version-file <文件> | --tree-version <X.Y.Z>] [--requested <X.Y.Z>] [--stable-only]
读 GITHUB_REF 和 PUBLISH_INPUT；把 version、release、tag、prerelease、publish 逐行写成 key=value，写到 stdout，并追加到 GITHUB_OUTPUT（设置了的话）。
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import subprocess
import sys


VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")
TAG_RELEASE = re.compile(r"(?P<version>[0-9]+\.[0-9]+\.[0-9]+)(?P<suffix>-beta(?:\.[1-9][0-9]*)?)?")
MAIN = "refs/remotes/origin/main"


class ReleaseRefError(Exception):
    pass


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], check=check, capture_output=True, text=True)


def ensure_main_history() -> None:
    """取 origin/main 和足够判断祖先关系的历史：浅克隆先补全，否则 merge-base 会把缺历史当成不是祖先。"""
    unshallow = ["--unshallow"] if git("rev-parse", "--is-shallow-repository").stdout.strip() == "true" else []
    fetched = git("fetch", "--quiet", "--no-tags", *unshallow, "origin", f"+refs/heads/main:{MAIN}", check=False)
    if fetched.returncode != 0:
        raise ReleaseRefError(f"取不到 origin/main，无法核对 tag 是否在 main 上：{fetched.stderr.strip()}")


def is_on_main(commit: str = "HEAD") -> bool:
    ensure_main_history()
    return git("merge-base", "--is-ancestor", commit, MAIN, check=False).returncode == 0


def resolve(ref: str, prefix: str, tree_version: str | None, requested: str, publish_input: str,
            stable_only: bool, on_main=is_on_main) -> dict[str, str]:
    requested = requested.strip()
    if requested and not VERSION.fullmatch(requested):
        raise ReleaseRefError(f"version 必须是 X.Y.Z，不是 {requested!r}")
    if tree_version is not None and not VERSION.fullmatch(tree_version):
        raise ReleaseRefError(f"仓库里的版本号不是 X.Y.Z：{tree_version!r}")

    if not ref.startswith("refs/tags/"):
        version = requested or tree_version or ""
        if publish_input == "true":
            print(f"::notice::{ref} 不是 {prefix}X.Y.Z 这样的发布 tag，这次只构建、不发布。发版是在 main 上推 tag（AGENTS.md「发版」）。", file=sys.stderr)
        return {"version": version, "release": version, "tag": "", "prerelease": "false", "publish": "false"}

    tag = ref[len("refs/tags/"):]
    if not tag.startswith(prefix):
        raise ReleaseRefError(f"这个工作流只发布 {prefix}* tag，不是 {tag}")
    match = TAG_RELEASE.fullmatch(tag[len(prefix):])
    if not match:
        allowed = f"{prefix}X.Y.Z" if stable_only else f"{prefix}X.Y.Z 或 {prefix}X.Y.Z-beta.N（N 从 1 起）"
        raise ReleaseRefError(f"tag {tag} 不是 {allowed}")
    version, suffix = match.group("version"), match.group("suffix") or ""
    if suffix and stable_only:
        raise ReleaseRefError(f"{prefix}* 只发正式版，tag {tag} 带了 {suffix}")
    if tree_version is not None and tree_version != version:
        raise ReleaseRefError(f"tag {tag} 的版本号是 {version}，被打 tag 的提交里的版本号是 {tree_version}；先经 PR 把版本号改成 {version} 合进 main，再在那个提交上打 tag")
    if requested and requested != version:
        raise ReleaseRefError(f"手动输入的 version {requested} 与 tag {tag} 不一致；对 tag 手动触发时 version 留空")
    if not on_main():
        raise ReleaseRefError(f"tag {tag} 指向的提交不是 origin/main 的祖先；只发布已经合进 main 的提交")
    publish = "false" if publish_input == "false" else "true"
    return {"version": version, "release": version + suffix, "tag": tag,
            "prerelease": "true" if suffix else "false", "publish": publish}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--prefix", required=True, help="tag 前缀，例如 android-v")
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--version-file", type=Path, help="被打 tag 的提交里记版本号的文件")
    source.add_argument("--tree-version", help="被打 tag 的提交里的版本号（从 Cargo.toml 这类文件读出来的）")
    parser.add_argument("--requested", default="", help="手动触发时输入的 version")
    parser.add_argument("--stable-only", action="store_true", help="不接受 -beta tag")
    args = parser.parse_args()

    tree_version = args.tree_version
    if args.version_file is not None:
        tree_version = args.version_file.read_text(encoding="utf-8").strip()
    try:
        outputs = resolve(os.environ.get("GITHUB_REF", ""), args.prefix, tree_version, args.requested,
                          os.environ.get("PUBLISH_INPUT", ""), args.stable_only)
    except ReleaseRefError as error:
        print(f"::error::{error}", file=sys.stderr)
        return 1
    lines = "".join(f"{key}={value}\n" for key, value in outputs.items())
    sys.stdout.write(lines)
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write(lines)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
