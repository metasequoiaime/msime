#!/usr/bin/env bash
# 只在打包镜像变化时清除编译结果，下载缓存仍留在既有挂载中。
set -euo pipefail

build_root="${1:-/build}"
image="${MSIME_PACKAGE_IMAGE:?MSIME_PACKAGE_IMAGE is required}"
previous="$(cat "$build_root/.package-image" 2>/dev/null || true)"
if [ "$previous" != "$image" ]; then
  rm -rf "$build_root/cargo" "$build_root/cmake" "$build_root"/cmake-*
  printf '%s\n' "$image" > "$build_root/.package-image"
fi
