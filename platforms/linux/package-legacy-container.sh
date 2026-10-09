#!/usr/bin/env bash
# 在 Debian 10（buster，glibc 2.28）容器里构建 legacy .deb（#6311），给 UOS 20、Debian 10 这类装不上发布页 .deb 的旧系统用：只有 IBus 宿主、provider 程序和 msime-mcp，没有 Fcitx5 插件，也没有 Tauri 设置窗口。
#
# 与 package-container.sh 分开，因为基线不同：那边在 bookworm 里构建，Depends 按 bookworm 的库算出（glibc 2.35 起），还带 WebKitGTK 4.1 的设置窗口和 Fcitx5 插件，这三样 buster 都没有。这里的构建镜像是 tests/tools/Dockerfile.legacy，Rust 与 C++ 都在 buster 里编译，所以产物只引用 glibc 2.28 及以下的符号版本。
#
# 分两步：
#   1. 在构建镜像里以 Release 编译 Host API 库和 msime-mcp，收集 Rust 许可证，配置并编译原生宿主（-DMSIME_ENABLE_FCITX5=OFF，IBus 下限 1.5.19，Python 下限 3.7），用 CPack 打出 .deb，再核对包里每个 ELF 文件要求的 glibc 不超过 2.28、Depends 里没有 buster 不提供的东西。
#   2. 换一个只有系统基础包的 debian:buster 容器，用 apt 安装这个 .deb（依赖由 apt 从 buster 的源解析），确认包内每个 ELF 文件的共享库都能找到，再在装好包的容器里跑第 1 步构建树的 ctest。测试程序链接的库与包相同，只能由包的 Depends 带进来；ctest 用的 CMake 从构建镜像里拷出。
#
# Usage: platforms/linux/package-legacy-container.sh [VERSION]
#   VERSION 缺省取 platforms/linux/version.txt，与 package-container.sh 相同；包名仍是 msime-linux，与发布页的 .deb 是同一个包的不同构建，两者不能同时安装。
#   MSIME_LEGACY_ARCH=amd64|arm64 选择目标架构，缺省是本机架构。另一种架构经 docker 的 --platform 在模拟器里构建（需要 Docker 支持该平台，例如 binfmt/qemu 或 Rosetta），比原生慢得多。
#   CARGO_BUILD_JOBS 和 CMAKE_BUILD_PARALLEL_LEVEL 设置时传进容器，用来限制共享 Docker 虚拟机上的内存。
#   只构建 full 版本：其他版本的配置要用 edition_linux.py 改写脚本，它需要 Python 3.10 以上，buster 没有。
#
# Output: target/linux-package-legacy-<arch>/dist/{msime-linux_<VERSION>_<arch>.deb,SHA256SUMS}
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"

command -v docker >/dev/null 2>&1 || {
  echo "docker is required; run this on a host with docker" >&2
  exit 2
}

version="${1:-$(tr -d '[:space:]' < platforms/linux/version.txt)}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "version must be MAJOR.MINOR.PATCH: $version" >&2
  exit 2
}
arch="${MSIME_LEGACY_ARCH:-}"
if [ -z "$arch" ]; then
  case "$(uname -m)" in
    x86_64|amd64) arch=amd64 ;;
    aarch64|arm64) arch=arm64 ;;
    *) echo "no legacy build for $(uname -m); set MSIME_LEGACY_ARCH=amd64 or arm64" >&2; exit 2 ;;
  esac
fi
case "$arch" in
  amd64) voice_platform=linux-x86_64 ;;
  arm64) voice_platform=linux-aarch64 ;;
  *) echo "MSIME_LEGACY_ARCH must be amd64 or arm64: $arch" >&2; exit 2 ;;
esac
platform="linux/$arch"
# 每个架构一个构建目录：Cargo 的 target 目录和 CMake 构建树不能在两种架构之间复用。
build_root="$repo_root/target/linux-package-legacy-$arch"
mkdir -p "$build_root"

# sherpa-onnx 运行库是上游的预编译文件，在这里按锁下载，第 1 步核对它在 glibc 2.28 上能加载。在宿主上取而不是在容器里：取运行库的脚本用到 Python 3.8 起才有的语法，buster 的 Python 是 3.7。
python3 scripts/fetch_voice_runtime.py --platform "$voice_platform" --out "$build_root/voice-runtime"

# 镜像标签按检出和架构区分，理由与 build-container.sh 相同：几个 worktree 同时构建时不能用上别人的镜像。基础镜像按清单摘要固定，--platform 决定取哪个架构；同一个摘要在本机已有另一种架构的镜像时，不写 --platform 会静默用上那一个。
checkout_hash="$(printf %s "$repo_root" | shasum | cut -c1-12)"
image="msime-linux-legacy:$arch-$checkout_hash"
docker build -q --platform "$platform" -t "$image" \
  -f platforms/linux/tests/tools/Dockerfile.legacy platforms/linux/tests >/dev/null
echo "legacy image: $image ($platform)" >&2

docker run --rm --init --platform "$platform" \
  -v "$repo_root":/source \
  -v "$build_root":/build \
  -w /source \
  -e CARGO_TARGET_DIR=/build/cargo \
  -e MSIME_VERSION="$version" \
  ${CARGO_BUILD_JOBS:+-e CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS"} \
  ${CMAKE_BUILD_PARALLEL_LEVEL:+-e CMAKE_BUILD_PARALLEL_LEVEL="$CMAKE_BUILD_PARALLEL_LEVEL"} \
  "$image" bash -euo pipefail -c '
    # 与 package-container.sh 一样在这里去掉 Rust 二进制的符号表。
    export CARGO_PROFILE_RELEASE_STRIP=symbols
    cargo build --release --locked -p msime-host-api
    cargo build --release --locked -p msime-mcp-server --bin msime-mcp
    mkdir -p /build/notices
    python3 platforms/linux/collect-notices.py cargo /build/notices/rust-crates-NOTICES.txt msime-host-api msime-mcp-server
    # 离线释义与粤语、注音、笔画词库的取用方式与 package-container.sh 相同：暂存目录里有就装，没有就不装。
    extra_args=(-DMSIME_REQUIRE_LANGUAGE_DICTIONARIES=OFF)
    if compgen -G "target/offline-glosses/zh-*.db" >/dev/null && [ -f target/offline-glosses/offline-glosses-NOTICE.txt ]; then
      extra_args+=(-DMSIME_OFFLINE_GLOSSES=/source/target/offline-glosses)
    fi
    if [ -d target/language-dictionaries ]; then
      extra_args+=(-DMSIME_LANGUAGE_DICTIONARIES=/source/target/language-dictionaries)
    fi
    rm -rf /build/cmake /build/dist
    cmake -S platforms/linux -B /build/cmake -G Ninja \
      -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_INSTALL_PREFIX=/usr \
      -DMSIME_EDITION=full \
      -DBUILD_TESTING=ON \
      -DMSIME_ENABLE_PACKAGING=ON \
      -DMSIME_ENABLE_FCITX5=OFF \
      -DMSIME_IBUS_MIN_VERSION=1.5.19 \
      -DMSIME_PYTHON_MIN_VERSION=3.7 \
      -DMSIME_HOST_LIBRARY=/build/cargo/release/libmsime_host_api.so \
      -DMSIME_MCP_BINARY=/build/cargo/release/msime-mcp \
      -DMSIME_PACKAGE_VERSION="$MSIME_VERSION" \
      -DMSIME_RUST_NOTICES=/build/notices/rust-crates-NOTICES.txt \
      -DMSIME_VOICE_RUNTIME_DIR=/build/voice-runtime \
      -DMSIME_BUNDLE_HANDWRITING_MODEL=OFF \
      "${extra_args[@]}"
    cmake --build /build/cmake
    cpack --config /build/cmake/CPackConfig.cmake -G DEB -B /build/dist
    rm -rf /build/dist/_CPack_Packages
    deb=$(find /build/dist -maxdepth 1 -name "msime-linux_*.deb" -print -quit)
    [ -n "$deb" ] || { echo "cpack produced no msime-linux .deb" >&2; exit 1; }
    depends=$(dpkg-deb -f "$deb" Depends)
    echo "Depends: $depends"
    if grep -E "webkit2gtk|libsoup-3|fcitx5|libgtk-3" <<<"$depends"; then
      echo "the legacy package depends on something it must not ship: the settings window or the Fcitx5 addon" >&2
      exit 1
    fi
    # 包里每个 ELF 文件（含预编译的 sherpa-onnx 和 ONNX Runtime 库）要求的最高 glibc 符号版本不能超过 2.28。
    contents=$(mktemp -d)
    dpkg-deb -x "$deb" "$contents"
    newest=2.28
    while IFS= read -r path; do
      file "$path" | grep -q ": *ELF" || continue
      for needed in $(readelf -V "$path" | grep -oE "GLIBC_[0-9]+(\.[0-9]+)+" | sed "s/^GLIBC_//" | sort -uV); do
        if [ "$(printf "%s\n%s\n" "$needed" 2.28 | sort -V | tail -1)" != 2.28 ]; then
          echo "${path#"$contents"} needs GLIBC_$needed" >&2
          newest=$needed
        fi
      done
    done < <(find "$contents" -type f)
    rm -rf "$contents"
    [ "$newest" = 2.28 ] || { echo "the legacy package needs a glibc newer than 2.28" >&2; exit 1; }
    cd /build/dist
    sha256sum -- *.deb > SHA256SUMS
    cat SHA256SUMS
  '

# 第 2 步：干净的 buster 容器。基础镜像取自 Dockerfile.legacy 的 FROM，两步用同一个摘要。
base_image=$(sed -n "s/^FROM \(debian:buster@sha256:[0-9a-f]*\).*/\1/p" platforms/linux/tests/tools/Dockerfile.legacy)
[ -n "$base_image" ] || { echo "could not read the base image from Dockerfile.legacy" >&2; exit 2; }
# ctest 记下的是构建镜像里 /opt/cmake 下的 cmake 与 ctest，拷出来挂到同一个路径。
rm -rf "$build_root/toolchain"
mkdir -p "$build_root/toolchain"
docker run --rm --platform "$platform" "$image" tar -C /opt -cf - cmake | tar -xf - -C "$build_root/toolchain"

docker run --rm --init --platform "$platform" \
  -v "$repo_root":/source \
  -v "$build_root":/build \
  -v "$build_root/toolchain/cmake":/opt/cmake:ro \
  -w /source \
  "$base_image" bash -euo pipefail -c '
    printf "%s\n" \
      "deb http://archive.debian.org/debian buster main" \
      "deb http://archive.debian.org/debian buster-updates main" \
      "deb http://archive.debian.org/debian-security buster/updates main" \
      > /etc/apt/sources.list
    printf "Acquire::Check-Valid-Until \"false\";\n" > /etc/apt/apt.conf.d/99msime-archive
    apt-get update -qq
    deb=$(find /build/dist -maxdepth 1 -name "msime-linux_*.deb" -print -quit)
    # 不装推荐包：Recommends 里的 python3-websockets (>= 15) 在 buster 上本来就满足不了，录音工具也不是输入法启动所需。
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "$deb"
    dpkg -s msime-linux | sed -n "s/^\(Version\|Depends\):/&/p"
    missing=$(dpkg -L msime-linux | while IFS= read -r path; do
      [ -f "$path" ] && [ ! -L "$path" ] || continue
      head -c 4 "$path" | grep -q "ELF" || continue
      ldd "$path" 2>&1 | sed -n "s|^\(.*not found\)$|$path: \1|p"
    done)
    [ -z "$missing" ] || { echo "installed files miss shared libraries:" >&2; echo "$missing" >&2; exit 1; }
    msime-mcp --version
    python3 --version
    python3 /usr/bin/msime-linux-setup --help >/dev/null
    test -f /usr/share/ibus/component/msime-linux.xml
    if [ -e /usr/bin/msime-linux-settings ] || [ -e /usr/bin/msime-linux-desktop ]; then
      echo "the legacy package ships the settings window" >&2
      exit 1
    fi
    /opt/cmake/bin/ctest --test-dir /build/cmake --output-on-failure
    echo "legacy package installed and tested on $(sed -n "s/^PRETTY_NAME=//p" /etc/os-release), $(ldd --version | head -1)"
  '
