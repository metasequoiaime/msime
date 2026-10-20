#!/usr/bin/env bash
# 按一个已发布的 linux-vVERSION 渲染各发行版的包定义，供维护者拿去发布到各自的仓库（步骤见 platforms/linux/README.md「包管理器」）。本脚本只产出文件，不向任何外部仓库推送。
#
# 用法：platforms/linux/packaging/render-definitions.sh VERSION OUTDIR
#   在 linux-vVERSION 那个提交的检出里运行：RPM 规格、debian/、PKGBUILD 与 ebuild 模板都取自当前检出，Gentoo 的 CRATES 取自当前检出的 Cargo.lock。release-linux.yml 的 package-definitions job 在发布之后以同一提交调用它。
#   需要 docker 与网络。每一步在各自发行版的官方容器里跑，OUTDIR 下得到：
#     rpm/      渲染好的 msime.spec、msime-rpmlintrc 与 msime-VERSION-1.src.rpm（COPR 直接构建它；OBS 用 spec 加两个 tarball）
#     debian/   msime_VERSION-1.dsc 与 msime_VERSION-1.debian.tar.xz（orig tarball 就是 linux-sources 上的 msime-VERSION.tar.xz 与 msime-VERSION-vendor.tar.xz 改名）
#     arch/     msime/ 与 msime-bin/，各含 PKGBUILD、.SRCINFO 与 msime.install，原样复制到 AUR 仓库
#     gentoo/   一个完整的 overlay：版本 ebuild、live ebuild、metadata.xml、Manifest、metadata/ 与 profiles/
#
# 环境变量：
#   MSIME_DEFINITIONS   要渲染的部分，逗号分隔，默认 rpm,debian,arch,gentoo。
#   MSIME_RELEASE_DIR   本地目录，里面放要用到的文件；给了就不下载。用于在发布前验证，或复用刚构建出的文件。rpm、debian 要 msime-VERSION-SHA256SUMS 与它列出的源码包和 vendor 包，gentoo 要它与前端包（都在 linux-sources 上，或是 make-source-tarballs.sh 的输出），arch 要版本发布页的 SHA256SUMS。
set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: $0 VERSION OUTDIR" >&2
  exit 2
fi
version=$1
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "version must be MAJOR.MINOR.PATCH: $version" >&2
  exit 2
}
here=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$here/../../.." && pwd)
mkdir -p "$2"
out=$(cd "$2" && pwd)
parts=",${MSIME_DEFINITIONS:-rpm,debian,arch,gentoo},"
# 版本发布页只放给用户下载的安装包和只列它们的 SHA256SUMS（AUR 的 msime-bin 重新打包其中的 .rpm）；发行版构建用的三个源码包在滚动的 linux-sources release 上，校验值在同处的 msime-VERSION-SHA256SUMS 里（packaging/publish-sources.sh）。
release_url="https://github.com/metasequoiaime/msime/releases/download/linux-v$version"
sources_url="https://github.com/metasequoiaime/msime/releases/download/linux-sources"
sources_sums="msime-$version-SHA256SUMS"
# 设置应用关于页的发布日期，与发布包编进去的同是当前检出 HEAD 的提交日期。AUR 的 msime 从标签归档构建，里面没有 .git，由 arch/render.py 写进 PKGBUILD；渲染它的容器里没有仓库，在这里算好传进去。
release_date=$(git -C "$repo_root" log -1 --format=%cs HEAD)
want() { [[ "$parts" == *",$1,"* ]]; }
# 容器以 root 往 OUTDIR 写文件；结束时（包括失败时）交回给调用者，免得 OUTDIR 删不掉。
reclaim() { docker run --rm -v "$out":/out debian:sid chown -R "$(id -u):$(id -g)" /out 2>/dev/null || true; }
trap reclaim EXIT

# 要用到的文件：本地目录优先，否则下载到 OUTDIR/.release。源码包必须列在 msime-VERSION-SHA256SUMS 里并对得上，校验值只认发布时算的那一份。
release=${MSIME_RELEASE_DIR:-$out/.release}
mkdir -p "$release"
release=$(cd "$release" && pwd)
fetch() {
  if [ ! -f "$release/$2" ]; then
    [ -z "${MSIME_RELEASE_DIR:-}" ] || { echo "$2 is not in MSIME_RELEASE_DIR ($release)" >&2; exit 1; }
    curl -fsSL --retry 3 -o "$release/$2.part" "$1/$2"
    mv "$release/$2.part" "$release/$2"
  fi
}
source_asset() {
  fetch "$sources_url" "$1"
  local line
  line=$(grep -E "^[0-9a-f]{64} [ *]${1//./\\.}\$" "$release/$sources_sums") || { echo "$1 is not listed in $sources_sums" >&2; exit 1; }
  (cd "$release" && sha256sum -c --quiet - <<<"$line")
}
# AUR 的 msime-bin 从版本发布页的 SHA256SUMS 取 .rpm 的校验值（arch/render.py --sha256sums）。
if want arch; then
  fetch "$release_url" SHA256SUMS
fi
if want rpm || want debian || want gentoo; then
  fetch "$sources_url" "$sources_sums"
fi
if want rpm || want debian; then
  source_asset "msime-$version.tar.xz"
  source_asset "msime-$version-vendor.tar.xz"
fi

if want rpm; then
  echo "== rpm"
  rm -rf "$out/rpm"
  mkdir -p "$out/rpm"
  python3 "$here/render-sources.py" --version "$version" --spec-out "$out/rpm/msime.spec"
  cp "$here/rpm/msime-rpmlintrc" "$out/rpm/"
  # 只打源码包，不构建：rpmbuild -bs 只要规格文件与各个 Source 在 _sourcedir 里，tarball 用符号链接，不复制一份。dist 置空，SRPM 不带 .fc44 这类标记，COPR 按各 chroot 重新构建。
  docker run --rm -v "$out/rpm":/d -v "$release":/release:ro fedora:44 bash -euo pipefail -c '
    dnf -y -q install rpm-build >/dev/null
    mkdir /sources
    ln -s /release/msime-'"$version"'.tar.xz /release/msime-'"$version"'-vendor.tar.xz /sources/
    cp /d/msime-rpmlintrc /sources/
    rpmbuild -bs --define "_sourcedir /sources" --define "_srcrpmdir /d" --define "dist %{nil}" /d/msime.spec
    test -f /d/msime-'"$version"'-1.src.rpm
  '
fi

if want debian; then
  echo "== debian"
  rm -rf "$out/debian"
  mkdir -p "$out/debian"
  python3 "$here/render-sources.py" --version "$version" --changelog-out "$out/debian/changelog"
  docker run --rm -v "$out/debian":/d -v "$release":/release:ro debian:sid bash -euo pipefail -c '
    v='"$version"'
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq && apt-get install -y -qq --no-install-recommends dpkg-dev xz-utils >/dev/null
    mkdir /work && cd /work
    cp /release/msime-$v.tar.xz msime_$v.orig.tar.xz
    cp /release/msime-$v-vendor.tar.xz msime_$v.orig-vendor.tar.xz
    tar xf msime_$v.orig.tar.xz
    mkdir msime-$v/vendor
    tar xf msime_$v.orig-vendor.tar.xz -C msime-$v/vendor --strip-components=1
    cp -a msime-$v/platforms/linux/packaging/debian msime-$v/debian
    mv /d/changelog msime-$v/debian/changelog
    dpkg-source -b msime-$v
    cp msime_$v-1.dsc msime_$v-1.debian.tar.xz /d/
  '
fi

if want arch; then
  echo "== arch"
  rm -rf "$out/arch"
  mkdir -p "$out/arch"
  # makepkg --printsrcinfo 拒绝以 root 运行，render.py 原地改写 PKGBUILD，所以把 arch 目录复制给一个普通用户再渲染。archlinux 官方镜像只有 x86_64。
  docker run --rm --platform linux/amd64 -v "$here/arch":/arch:ro -v "$release":/release:ro -v "$out/arch":/out archlinux:latest bash -euo pipefail -c '
    # pacman 7 的下载沙箱要 seccomp 与 Landlock，跨架构模拟的容器里两者都用不了。
    sed -i "/^\[options\]/a DisableSandbox" /etc/pacman.conf
    pacman -Syu --noconfirm --needed base-devel python >/dev/null
    useradd -m builder
    cp -r /arch /home/builder/arch
    chown -R builder: /home/builder/arch
    su builder -c "python3 /home/builder/arch/render.py '"$version"' --release-date '"$release_date"' --sha256sums /release/SHA256SUMS"
    for pkg in msime msime-bin; do
      mkdir -p /out/$pkg
      cp /home/builder/arch/$pkg/PKGBUILD /home/builder/arch/$pkg/.SRCINFO /home/builder/arch/$pkg/msime.install /out/$pkg/
    done
  '
fi

if want gentoo; then
  echo "== gentoo"
  rm -rf "$out/gentoo"
  mkdir -p "$out/gentoo" "$out/.gentoo-distfiles"
  # 前端包先核对过再放进 distfiles，Portage 生成 Manifest 时就不再下载它，Manifest 记下的就是 msime-VERSION-SHA256SUMS 里的那一份；发布前验证时它来自 MSIME_RELEASE_DIR。
  source_asset "msime-$version-frontend.tar.xz"
  cp "$release/msime-$version-frontend.tar.xz" "$out/.gentoo-distfiles/"
  image="msime-gentoo-tools:$(printf %s "$repo_root" | shasum | cut -c1-12)"
  docker pull -q gentoo/stage3:latest >/dev/null
  docker pull -q gentoo/portage:latest >/dev/null
  docker build -q -t "$image" - >/dev/null < "$here/gentoo/tools.Dockerfile"
  docker run --rm --init -v "$repo_root":/src:ro -v "$out/gentoo":/var/db/repos/msime -v "$out/.gentoo-distfiles":/var/cache/distfiles \
    "$image" bash -euo pipefail -c '
      v='"$version"'
      overlay=/var/db/repos/msime
      cp -r /src/platforms/linux/packaging/gentoo/metadata /src/platforms/linux/packaging/gentoo/profiles "$overlay/"
      # pycargoebuild 读 Cargo.lock 时会在检出里写文件，在副本里渲染。
      mkdir /tmp/source
      tar -C /src --exclude=./target --exclude=./node_modules --exclude=./.git -cf - . | tar -C /tmp/source -xf -
      python3 /tmp/source/platforms/linux/packaging/gentoo/render.py "$v" --out "$overlay"
      cp /tmp/source/platforms/linux/packaging/gentoo/app-i18n/msime/msime-9999.ebuild "$overlay/app-i18n/msime/"
      mkdir -p /etc/portage/repos.conf
      printf "[DEFAULT]\nmain-repo = gentoo\n\n[gentoo]\nlocation = /var/db/repos/gentoo\n" > /etc/portage/repos.conf/gentoo.conf
      printf "[msime]\nlocation = %s\n" "$overlay" > /etc/portage/repos.conf/msime.conf
      # Manifest 记下每个 distfile 的校验值：Portage 下载 SRC_URI 里还没有的文件，已在 distfiles 里的直接计算。下载以 portage 用户进行（userfetch），挂进来的目录要让它可写。这些文件不在 Gentoo 的镜像上，清空 GENTOO_MIRRORS 直接取上游地址，免得每个文件先在镜像上 404 一次。
      chown portage:portage /var/cache/distfiles
      GENTOO_MIRRORS="" ebuild "$overlay/app-i18n/msime/msime-$v.ebuild" manifest
      cd "$overlay"
      pkgcheck scan --exit error app-i18n/msime
    '
  reclaim
  rm -rf "$out/.gentoo-distfiles"
  docker rmi -f "$image" >/dev/null
fi

[ -n "${MSIME_RELEASE_DIR:-}" ] || rm -rf "$out/.release"
find "$out" -maxdepth 4 -type f ! -path "*/.release/*" | sort
echo "package definitions for $version are in $out"
