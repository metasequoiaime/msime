#!/usr/bin/env bash
# Run build-cross.sh inside the cross container.
#
# Use this when the host has no toolchain the architecture can be built with.
# On macOS that is x86: Homebrew's i686 MinGW uses SJLJ exceptions and Rust's
# i686-pc-windows-gnu needs DWARF unwinding, so build-cross.sh refuses. The
# container carries Debian's i686 MinGW, which is built with DWARF, and the
# build then runs unchanged.
#
# The repository is mounted rather than copied, so the outputs land in the
# usual target/ directories. vcpkg and its dependency tree are built inside the
# container under target/, which means they are kept between runs and are not
# mixed with the host's own (a host-built vcpkg tree carries host binaries).
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
arch=${1:-x86}

if ! command -v docker >/dev/null 2>&1; then
  echo "skipped: docker is not installed"; exit 0
fi
if ! daemon_platform=$(docker info --format '{{.OSType}}/{{.Architecture}}' 2>/dev/null); then
  echo "skipped: the docker daemon is not running"; exit 0
fi
# 以 daemon 而非客户端架构选择编译器，避免 ARM 主机上的整套工具链仿真。
# amd64 保留已有缓存；ARM64 的 vcpkg 可执行文件和宿主依赖单独存放。
case "$daemon_platform" in
  linux/aarch64|linux/arm64) platform=linux/arm64; cache_suffix=/arm64; image=msime-cross:local-arm64 ;;
  linux/*) platform=linux/amd64; cache_suffix=; image=msime-cross:local ;;
  *) echo "Unsupported cross-build Docker platform: $daemon_platform" >&2; exit 1 ;;
esac
docker build --platform "$platform" -t "$image" "$root/platforms/windows/cross" >/dev/null 2>&1 || {
  echo "skipped: could not build the cross image"; exit 0; }

# Keep the container's tooling apart from the host's. A vcpkg checkout
# bootstrapped on macOS holds a macOS vcpkg binary, which cannot run in here,
# and the dependency trees are built for different hosts. bootstrap-vcpkg.sh
# writes to target/tooling by construction, so that path is bind-mounted to a
# container-only directory rather than teaching the script a second location.
# `CARGO_HOME` 放在现有仓库挂载内，保留临时容器运行后下载的 registry 与 Git 源码。
mkdir -p "$root/target/tooling-linux$cache_suffix" "$root/target/windows-native-deps-linux$cache_suffix" \
  "$root/target/windows-cross/cargo-home"
docker run --rm --platform "$platform" \
  -v "$root":/repo -v "$root/target/tooling-linux$cache_suffix":/repo/target/tooling -w /repo \
  -e MSIME_WINDOWS_DEPS_ROOT="/repo/target/windows-native-deps-linux$cache_suffix" \
  -e CARGO_HOME=/repo/target/windows-cross/cargo-home \
  "$image" bash platforms/windows/build-cross.sh "$arch"
