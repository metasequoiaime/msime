#!/usr/bin/env bash
# 在干净的 archlinux 容器里用 makepkg 构建一个 PKGBUILD 目录（msime 或 msime-bin），跑完 check()，再用 namcap 检查 PKGBUILD 与产出的包，最后装上包核对插件能找到 Host API。
#
# 用法：platforms/linux/packaging/arch/check-in-container.sh <msime|msime-bin> [输出目录]
#   输出目录默认 target/arch-package/<包名>，构建好的 .pkg.tar.zst、.SRCINFO 与日志都留在那里。
#   需要 docker；Apple Silicon 上经 Rosetta/QEMU 跑 linux/amd64 镜像（archlinux 官方镜像只有 x86_64）。
#   CARGO_BUILD_JOBS 原样传进容器，用来在共用的 Docker 虚拟机上限制内存。
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$here/../../../.." && pwd)
pkg=${1:?usage: check-in-container.sh <msime|msime-bin> [output-dir]}
[ -f "$here/$pkg/PKGBUILD" ] || { echo "no PKGBUILD in $here/$pkg" >&2; exit 2; }
out=${2:-$repo_root/target/arch-package/$pkg}
mkdir -p "$out"
out=$(cd "$out" && pwd)
# 构建用户的家目录（源码、Cargo 产物、rustup 工具链、pnpm 仓库）放在宿主的输出目录里，不占 Docker 虚拟机自己的磁盘：完整构建要十几 GB。每次从空目录开始。
rm -rf "$out/home"
mkdir -p "$out/home"

docker run --rm --init --platform linux/amd64 \
  -v "$here/$pkg":/pkg:ro \
  -v "$out":/out \
  -v "$out/home":/home/builder \
  ${CARGO_BUILD_JOBS:+-e CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS"} \
  archlinux:latest bash -euo pipefail -c '
    # pacman 7 的下载沙箱要 seccomp 与 Landlock，跨架构模拟的容器里两者都用不了。
    sed -i "/^\[options\]/a DisableSandbox" /etc/pacman.conf
    pacman -Syu --noconfirm --needed base-devel namcap >/dev/null
    useradd -M -d /home/builder builder
    chown builder: /home/builder
    echo "builder ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/builder
    cp -r /pkg /home/builder/pkg
    chown -R builder: /home/builder/pkg
    cd /home/builder/pkg
    # makepkg 拒绝以 root 运行；-s 经 sudo 装 depends/makedepends/checkdepends。
    su builder -c "makepkg --printsrcinfo > .SRCINFO && makepkg -s --noconfirm 2>&1" | tee /out/makepkg.log
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
