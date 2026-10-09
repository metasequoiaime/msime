#!/usr/bin/env bash
# 在断网的新容器里使用仓库的工具链声明，缺失组件不能靠临时下载补齐。
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: bash $0 <image> [image ...]" >&2
  exit 2
fi

repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
for image in "$@"; do
  echo "检查镜像工具链：$image"
  docker run --rm --network none \
    -v "$repo_root":/source:ro -w /source \
    "$image" bash -euo pipefail -c '
      cargo --version
      cargo fmt --version
      cargo clippy --version
    '
done
