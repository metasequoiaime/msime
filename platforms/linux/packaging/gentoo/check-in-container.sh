#!/usr/bin/env bash
# 在 gentoo/stage3 容器里检查本目录的 overlay：用 pkgcheck 扫 msime-9999.ebuild 与 render.py 渲染出的版本 ebuild，再把 live ebuild 跑到 src_unpack 结束（克隆、cargo vendor、pnpm 安装与锁定资源下载）。
#
# 用法：platforms/linux/packaging/gentoo/check-in-container.sh [VERSION]
#   VERSION 是拿来渲染版本 ebuild 的版本号，默认 platforms/linux/version.txt；渲染用的是当前检出的 Cargo.lock 与锁文件。
#   不做完整的 emerge：WebKitGTK、Fcitx5 与 IBus 在容器里都要从源码编译，要好几个小时。
#   需要 docker；依赖尽量从 Gentoo 官方二进制仓库取。镜像用宿主的原生架构（amd64 或 arm64，ebuild 两者都支持）：跨架构模拟下 Portage 给构建进程开的伪终端不可用，emerge 直接报错。
#   工具镜像 msime-gentoo-check:<检出哈希> 按检出缓存，用完删掉：docker rmi msime-gentoo-check:<检出哈希>。
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$here/../../../.." && pwd)
version=${1:-$(tr -d '[:space:]' < "$repo_root/platforms/linux/version.txt")}
# 构建目录与 distfile 放在宿主上，不占 Docker 虚拟机自己的磁盘。每次从空目录开始。
scratch=$repo_root/target/gentoo-check
rm -rf "$scratch"
mkdir -p "$scratch/tmp" "$scratch/distfiles"

# 显式拉取，本地同名标签可能是之前为别的架构拉的。工具（pkgcheck、pycargoebuild、git、rust-bin、nodejs）装进按检出命名的镜像，反复运行时不必每次重装。
checkout_hash="$(printf %s "$repo_root" | shasum | cut -c1-12)"
image="msime-gentoo-check:$checkout_hash"
docker pull -q gentoo/stage3:latest >/dev/null
docker pull -q gentoo/portage:latest >/dev/null
docker build -q -t "$image" - >/dev/null < "$here/tools.Dockerfile"

docker run --rm --init \
  -v "$repo_root":/src:ro \
  -v "$scratch/tmp":/var/tmp/portage \
  -v "$scratch/distfiles":/var/cache/distfiles \
  -e MSIME_VERSION="$version" \
  "$image" bash -euo pipefail -c '
    # overlay 放到 /var/db/repos/msime，渲染的版本 ebuild 写进同一处。pkgcheck（pkgcore）只读 /etc/portage/repos.conf，建了这个目录就得把 gentoo 也写进去。
    cp -r /src/platforms/linux/packaging/gentoo /var/db/repos/msime
    mkdir -p /etc/portage/repos.conf
    printf "[DEFAULT]\nmain-repo = gentoo\n\n[gentoo]\nlocation = /var/db/repos/gentoo\n" > /etc/portage/repos.conf/gentoo.conf
    printf "[msime]\nlocation = /var/db/repos/msime\n" > /etc/portage/repos.conf/msime.conf
    mkdir /tmp/source
    tar -C /src --exclude=./target --exclude=./node_modules -cf - . | tar -C /tmp/source -xf -
    python3 /tmp/source/platforms/linux/packaging/gentoo/render.py "$MSIME_VERSION" --out /var/db/repos/msime
    ebuild_file=/var/db/repos/msime/app-i18n/msime/msime-$MSIME_VERSION.ebuild
    echo "== rendered $(basename "$ebuild_file"): $(grep -c "^	[a-z0-9_-]*@" "$ebuild_file") crates"
    grep -n -A3 "GIT_CRATES=" "$ebuild_file"
    grep -n "^LICENSE+=" "$ebuild_file"

    echo "== pkgcheck scan"
    cd /var/db/repos/msime
    # 版本 ebuild 的 Manifest 要下载全部 distfile，前端归档在发布流程补上之前也还不存在，这里不生成，所以跳过与 Manifest 相关的检查。
    pkgcheck scan --exit error --keywords=-UnknownManifest,-MissingManifest app-i18n/msime

    echo "== live ebuild through src_unpack"
    ebuild /var/db/repos/msime/app-i18n/msime/msime-9999.ebuild clean unpack
    work=/var/tmp/portage/app-i18n/msime-9999/work
    ls "$work"
    for dir in voice-runtime handwriting-model offline-glosses language-dictionaries; do
      echo "$dir: $(find "$work/$dir" -maxdepth 1 -type f | wc -l) files"
    done
    test -d "$work/msime-9999/node_modules"
    test -d "$work/cargo_home/gentoo"
    echo "live src_unpack complete"
  '
