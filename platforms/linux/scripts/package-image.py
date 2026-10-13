#!/usr/bin/env python3
"""按构建配方复用不包含源码的 Linux 打包镜像，输出不可变镜像 ID。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]


def image_id(tag):
    result = subprocess.run(
        ["docker", "image", "inspect", "--format", "{{.Id}}", tag],
        capture_output=True, text=True,
    )
    return result.stdout.strip() if result.returncode == 0 else None


def output_reference(name, identity, tag_output):
    if not tag_output:
        return identity
    # BuildKit 的 FROM 不接受本地 sha256 ID；用 ID 派生标签固定父镜像，避免并发重建改写配方标签。
    tag = f"{name}:id-{identity.removeprefix('sha256:')}"
    subprocess.run(["docker", "image", "tag", identity, tag], check=True)
    return tag


def ensure_image(name, dockerfile, platform=None, build_args=(), tag_output=False):
    if not platform:
        platform = os.environ.get("DOCKER_DEFAULT_PLATFORM")
    if not platform:
        platform = subprocess.check_output(
            ["docker", "info", "--format", "{{.OSType}}/{{.Architecture}}"],
            text=True,
        ).strip()
    platform = platform.replace("/aarch64", "/arm64").replace("/x86_64", "/amd64")
    recipe = dockerfile.read_bytes()
    # 当前配方不读取本地文件；将来需要 COPY 时必须先扩展缓存输入契约。
    for line in recipe.decode().splitlines():
        if re.match(r"^\s*(COPY|ADD)\b", line, re.IGNORECASE):
            raise ValueError("打包镜像必须使用空上下文，不能包含 COPY 或 ADD")
    # Deb 配方的 FROM 使用父镜像标签，但缓存键还需锁住其当前 ID。
    parents = {}
    for argument in build_args:
        if argument.startswith("MSIME_BUILD_GATE_IMAGE="):
            parent = argument.split("=", 1)[1]
            parents[parent] = image_id(parent)
            if not parents[parent]:
                raise ValueError(f"找不到父镜像：{parent}")
    key = hashlib.sha256(json.dumps({
        "recipe": recipe.decode(),
        "platform": platform,
        "args": sorted(build_args),
        "parents": parents,
        "toolchain": (ROOT / "rust-toolchain.toml").read_text(),
    }, sort_keys=True).encode()).hexdigest()
    tag = f"{name}:{key}"
    cached = image_id(tag)
    if cached:
        print(f"reuse package image: {tag}", file=sys.stderr)
        return output_reference(name, cached, tag_output)
    command = ["docker", "build", "-q", "--platform", platform, "-t", tag]
    for argument in build_args:
        command += ["--build-arg", argument]
    print(f"build package image: {tag}", file=sys.stderr)
    # 标准输入是 Dockerfile，构建上下文为空，源码改动不会触发镜像重建。
    subprocess.run(command + ["-"], input=recipe, stdout=sys.stderr, check=True)
    built = image_id(tag)
    if not built:
        raise RuntimeError(f"构建后找不到镜像：{tag}")
    return output_reference(name, built, tag_output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("name")
    parser.add_argument("dockerfile", type=Path)
    parser.add_argument("--platform")
    parser.add_argument("--build-arg", action="append", default=[])
    parser.add_argument("--tag", action="store_true", help="输出标签，用于子镜像的 FROM")
    args = parser.parse_args()
    print(ensure_image(args.name, args.dockerfile, args.platform, args.build_arg, args.tag))


if __name__ == "__main__":
    main()
