#!/usr/bin/env bash
# 在断网的新容器中按真实仓库声明检查工具链，并链接两个 Windows GNU 目标。
set -euo pipefail

image=${1:?usage: check-cross-image-toolchain.sh <image>}
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
mkdir -p "$repo_root/target/windows-cross"

# 按被检查镜像的架构运行，分别覆盖原生 ARM64 和 amd64 镜像。
platform=$(docker image inspect --format '{{.Os}}/{{.Architecture}}' "$image")
docker run --rm --platform "$platform" --network none \
  -v "$repo_root":/repo:ro \
  -v "$repo_root/target/windows-cross":/output \
  -w /repo "$image" bash -euo pipefail -c '
    cargo --version
    cargo fmt --version
    cargo clippy --version
    for arch in i686 x86_64; do
      target="$arch-pc-windows-gnu"
      # 构建脚本执行同一命令；目标已预装时断网调用必须成功。
      rustup target add "$target"
      mkdir -p "/output/$arch"
      binary="/output/$arch/toolchain-probe.exe"
      printf "%s\n" "fn main() { assert_eq!(std::thread::spawn(|| vec![1, 2, 3].len()).join().unwrap(), 3); }" |
        rustc --edition 2021 --crate-name msime_toolchain_probe --target "$target" \
          -C "linker=$arch-w64-mingw32-gcc" -o "$binary" -
      case "$arch" in
        i686) format=pei-i386 ;;
        x86_64) format=pei-x86-64 ;;
      esac
      description=$("$arch-w64-mingw32-objdump" -f "$binary")
      printf "%s\n" "$description"
      grep -q "file format $format$" <<<"$description"
    done
  '
