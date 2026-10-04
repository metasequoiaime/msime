#!/usr/bin/env bash
# 在 gentoo/stage3 容器里检查本目录的 overlay：用 pkgcheck 扫 msime-9999.ebuild 与 render.py 渲染出的版本 ebuild，再把 live ebuild 跑到 src_unpack 结束（克隆、cargo vendor、pnpm 安装与锁定资源下载）。
#
# 用法：platforms/linux/packaging/gentoo/check-in-container.sh [VERSION]
#   VERSION 是拿来渲染版本 ebuild 的版本号，默认 platforms/linux/version.txt；渲染用的是当前检出的 Cargo.lock 与锁文件。
#   不做完整的 emerge：WebKitGTK、Fcitx5 与 IBus 在容器里都要从源码编译，跨架构模拟下要好几个小时。
#   需要 docker；依赖尽量从 Gentoo 官方二进制仓库取。
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$here/../../../.." && pwd)
version=${1:-$(tr -d '[:space:]' < "$repo_root/platforms/linux/version.txt")}
portage=msime-gentoo-portage-$$

docker create --platform linux/amd64 --name "$portage" gentoo/portage:latest true >/dev/null
trap 'docker rm -f "$portage" >/dev/null 2>&1 || true' EXIT

docker run --rm --init --platform linux/amd64 \
  --volumes-from "$portage" \
  -v "$repo_root":/src:ro \
  -e MSIME_VERSION="$version" \
  gentoo/stage3:latest bash -euo pipefail -c '
    # 容器里用不了 Portage 的命名空间沙箱。
    cat >> /etc/portage/make.conf <<EOF
FEATURES="-ipc-sandbox -mount-sandbox -network-sandbox -pid-sandbox -sandbox -usersandbox"
EMERGE_DEFAULT_OPTS="--getbinpkg --quiet-build --jobs=4"
ACCEPT_KEYWORDS="~amd64"
EOF
    emerge --noreplace dev-util/pkgcheck app-portage/pycargoebuild dev-vcs/git >/dev/null

    # overlay 放到 /var/db/repos/msime，渲染的版本 ebuild 写进同一处。
    cp -r /src/platforms/linux/packaging/gentoo /var/db/repos/msime
    mkdir -p /etc/portage/repos.conf
    printf "[msime]\nlocation = /var/db/repos/msime\n" > /etc/portage/repos.conf/msime.conf
    cp -r /src /tmp/source
    python3 /tmp/source/platforms/linux/packaging/gentoo/render.py "$MSIME_VERSION" --out /var/db/repos/msime
    ebuild_file=/var/db/repos/msime/app-i18n/msime/msime-$MSIME_VERSION.ebuild
    echo "== rendered $(basename "$ebuild_file"): $(grep -c "^	[a-z0-9_-]*@" "$ebuild_file") crates"
    grep -n "^GIT_CRATES" -A3 "$ebuild_file" || true

    echo "== pkgcheck scan"
    cd /var/db/repos/msime
    # 版本 ebuild 的 Manifest 要下载全部 distfile，前端归档在发布流程补上之前也还不存在，这里不生成，所以跳过与 Manifest 相关的检查。
    pkgcheck scan --exit error --keywords=-UnknownManifest,-MissingManifest app-i18n/msime

    echo "== live ebuild through src_unpack"
    emerge --noreplace --oneshot dev-lang/rust-bin net-libs/nodejs >/dev/null
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
