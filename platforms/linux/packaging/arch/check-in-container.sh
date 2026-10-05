#!/usr/bin/env bash
# 在干净的 archlinux 容器里用 makepkg 构建一个 PKGBUILD 目录（msime 或 msime-bin），跑完 check()，再用 namcap 检查 PKGBUILD 与产出的包，最后装上包核对插件能找到 Host API。
#
# 用法：platforms/linux/packaging/arch/check-in-container.sh <msime|msime-bin> [输出目录]
#   输出目录默认 target/arch-package/<包名>，构建好的 .pkg.tar.zst、.SRCINFO 与日志都留在那里。
#   需要 docker；Apple Silicon 上经 Rosetta/QEMU 跑 linux/amd64 镜像（archlinux 官方镜像只有 x86_64）。
#   CARGO_BUILD_JOBS 原样传进容器，用来在共用的 Docker 虚拟机上限制内存。
#   MSIME_REUSE_HOME=1 不清空上一次的家目录，沿用其中的 rustup 工具链、Cargo 下载和 target/，用于改 PKGBUILD 后反复验证；默认每次从空目录开始。
#   MSIME_MAKEPKG_ARGS 追加给 makepkg 的参数，例如 --nocheck。
#   Docker 不是 x86_64 主机（Apple Silicon、aarch64 Linux）时镜像经 QEMU/Rosetta 模拟运行。linux-replaced-program 靠 /proc/self/exe 的「(deleted)」标记判断程序被替换，模拟器伪造 /proc/self/exe、这个标记永远不出现，这条测试在模拟下必然失败，只能在原生 x86_64 上验证。所以模拟时 msime 的 check() 改由本脚本代跑：makepkg 加 --nocheck，构建完再对同一个构建目录跑排除了这一条的 ctest，其余检查照常。
#   MSIME_SOURCE_DIR=<检出目录> 只对 msime 有效：不下载标签的源码归档，而是把这个目录（不含 target、node_modules 与 .git）打成同名归档，跳过校验和构建，用来在发版前验证 PKGBUILD 对当前代码仍然成立。
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$here/../../../.." && pwd)
pkg=${1:?usage: check-in-container.sh <msime|msime-bin> [output-dir]}
[ -f "$here/$pkg/PKGBUILD" ] || { echo "no PKGBUILD in $here/$pkg" >&2; exit 2; }
out=${2:-$repo_root/target/arch-package/$pkg}
mkdir -p "$out"
out=$(cd "$out" && pwd)
# 构建用户的家目录（源码、Cargo 产物、rustup 工具链、pnpm 仓库）放在宿主的输出目录里，不占 Docker 虚拟机自己的磁盘：完整构建要十几 GB。
if [ "${MSIME_REUSE_HOME:-0}" != 1 ]; then
  rm -rf "$out/home"
else
  # 上一次 makepkg 留下的打包目录与包文件不沿用，只留源码树和缓存。
  rm -rf "$out/home/pkg/pkg" "$out"/home/pkg/*.pkg.tar.*
fi
mkdir -p "$out/home"

emulated=0
[ "$(docker info --format '{{.Architecture}}' 2>/dev/null)" = x86_64 ] || emulated=1

docker run --rm --init --platform linux/amd64 \
  -v "$here/$pkg":/pkg:ro \
  -v "$out":/out \
  -v "$out/home":/home/builder \
  ${CARGO_BUILD_JOBS:+-e CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS"} \
  -e MSIME_MAKEPKG_ARGS="${MSIME_MAKEPKG_ARGS:-}" \
  -e MSIME_EMULATED="$emulated" \
  -e MSIME_PKG="$pkg" \
  ${MSIME_SOURCE_DIR:+-v "$(cd "$MSIME_SOURCE_DIR" && pwd)":/source:ro -e MSIME_LOCAL_SOURCE=1} \
  archlinux:latest bash -euo pipefail -c '
    # pacman 7 的下载沙箱要 seccomp 与 Landlock，跨架构模拟的容器里两者都用不了。
    sed -i "/^\[options\]/a DisableSandbox" /etc/pacman.conf
    pacman -Syu --noconfirm --needed base-devel namcap dbus >/dev/null
    useradd -M -d /home/builder builder
    chown builder: /home/builder
    echo "builder ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/builder
    mkdir -p /home/builder/pkg
    cp -r /pkg/. /home/builder/pkg/
    chown -R builder: /home/builder/pkg
    cd /home/builder/pkg
    makepkg_args="-s --noconfirm $MSIME_MAKEPKG_ARGS"
    ctest_after=0
    if [ "$MSIME_EMULATED" = 1 ] && [ "$MSIME_PKG" = msime ] && [[ " $MSIME_MAKEPKG_ARGS " != *" --nocheck "* ]]; then
      makepkg_args="$makepkg_args --nocheck"
      ctest_after=1
    fi
    if [ "${MSIME_LOCAL_SOURCE:-}" = 1 ]; then
      pkgver=$(. ./PKGBUILD && echo "$pkgver")
      tar -C /source --exclude=./target --exclude=./node_modules --exclude=./.git \
        --transform "s|^\.|msime-linux-v$pkgver|" -czf "msime-$pkgver.tar.gz" .
      chown builder: "msime-$pkgver.tar.gz"
      makepkg_args="$makepkg_args --skipchecksums"
    fi
    # makepkg 拒绝以 root 运行；-s 经 sudo 装 depends/makedepends/checkdepends。
    su builder -c "makepkg --printsrcinfo > .SRCINFO && makepkg $makepkg_args 2>&1" | tee /out/makepkg.log
    if [ "$ctest_after" = 1 ]; then
      echo "== ctest (emulated linux/amd64: linux-replaced-program excluded)"
      su builder -c "ctest --test-dir /home/builder/pkg/src/build --output-on-failure -E \"^linux-replaced-program\$\" 2>&1" | tee /out/ctest.log
    fi
    cp .SRCINFO /out/
    pkgfile=$(ls -1 ./*.pkg.tar.zst | grep -v -- "-debug-" | head -1)
    cp ./*.pkg.tar.zst /out/
    echo "== namcap PKGBUILD"
    namcap PKGBUILD | tee /out/namcap-PKGBUILD.txt
    echo "== namcap $pkgfile"
    namcap "$pkgfile" | tee /out/namcap-package.txt
    # 装进容器本身，按真实路径再核对一次：Fcitx5 按 RUNPATH 解析出的 Host API 必须是包里的那份。
    pacman -U --noconfirm "$pkgfile" >/dev/null
    resolved=$(ldd /usr/lib/fcitx5/libmsime-fcitx5.so | awk "\$1 == \"libmsime_host_api.so\" { print \$3 }")
    echo "installed: libmsime_host_api.so => $resolved"
    [ "$(realpath -- "$resolved")" = /usr/lib/msime-client/libmsime_host_api.so ]
    missing=$(for f in /usr/bin/msime-* /usr/lib/msime-client/*; do ldd "$f" 2>/dev/null || true; done | grep "not found" || true)
    if [ -n "$missing" ]; then
      echo "installed binaries have unresolved libraries:" >&2
      echo "$missing" >&2
      exit 1
    fi
    pacman -Qi "$(basename "$pkgfile" | sed -E "s/-[^-]+-[^-]+-[^-]+\.pkg\.tar\.zst$//")" | sed -n "1,4p"
    pacman -R --noconfirm "$(pacman -Qqo /usr/lib/fcitx5/libmsime-fcitx5.so)" >/dev/null
    echo "installed and removed cleanly"
  '
