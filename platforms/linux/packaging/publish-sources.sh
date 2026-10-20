#!/usr/bin/env bash
# 把一个 Linux 正式版的发行版源码包传到滚动的 linux-sources release（预发布、不设为 latest）。版本发布页 linux-vVERSION 只放给用户下载的安装包，COPR、OBS、Launchpad PPA 和 Gentoo 构建用的这三个 tarball 不进去，放在这里，每个版本一组，文件名带版本号，互不覆盖，长期保留：已发布的 RPM 规格、debian/watch 和 Gentoo 的 ebuild 都按地址引用它们，删掉旧版本的就会让那个版本的包没法重新构建。
#
# 上传的是 make-source-tarballs.sh 在 DIR 里写出的 msime-VERSION.tar.xz、msime-VERSION-vendor.tar.xz、msime-VERSION-frontend.tar.xz 和只列这三个文件的 msime-VERSION-SHA256SUMS。上传前先对着 msime-VERSION-SHA256SUMS 核对，上传后用 GitHub 为每个资产记录的 SHA-256（`digest`）与本地文件比对，对不上就失败。同一版本重跑发布时用 --clobber 覆盖，与 publish-release.sh 覆盖版本发布页的做法一致；make-source-tarballs.sh 对同一提交生成相同的字节，所以从同一提交重跑不改变任何一个地址下的内容。
#
# Usage: publish-sources.sh <dir> <version>
# Environment: GH_TOKEN；GH_REPO 在工作目录不是仓库检出时指明仓库。
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: publish-sources.sh <dir> <version>" >&2; exit 2; }
dir=$1 version=$2
tag=linux-sources
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "publish-sources.sh: 版本号必须是 X.Y.Z：$version" >&2; exit 2; }
sums="msime-$version-SHA256SUMS"
tarballs=("msime-$version.tar.xz" "msime-$version-vendor.tar.xz" "msime-$version-frontend.tar.xz")

for name in "${tarballs[@]}" "$sums"; do
  [[ -f "$dir/$name" ]] || { echo "publish-sources.sh: $dir/$name 不存在" >&2; exit 1; }
done
# 校验文件只列这三个 tarball，每个一行。
listed=$(sed -E 's/^[0-9a-f]{64} [ *]//' "$dir/$sums" | sort)
[[ "$listed" == "$(printf '%s\n' "${tarballs[@]}" | sort)" ]] || { echo "publish-sources.sh: $sums 应当只列 ${tarballs[*]}，实际是：$listed" >&2; exit 1; }
(cd "$dir" && sha256sum -c --quiet "$sums")

if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" --prerelease --latest=false --title "Linux distribution sources" \
    --notes "给发行版打包用的源码包，用户不需要下载，安装包在各个 linux-v 版本发布页上。每个 Linux 正式版一组：msime-<版本>.tar.xz（源码）、msime-<版本>-vendor.tar.xz（离线构建用的 crate、构建好的前端和按锁文件取回的数据）、msime-<版本>-frontend.tar.xz（只有前端，Gentoo 用）和只列这三个文件的 msime-<版本>-SHA256SUMS。Fedora COPR、openSUSE OBS、Launchpad PPA 与 Gentoo 的包定义按这里的地址取文件，长期保留。0.12.1 及更早版本的源码包仍在各自的 linux-v 发布页上。"
fi
uploads=()
for name in "${tarballs[@]}" "$sums"; do uploads+=("$dir/$name"); done
gh release upload "$tag" "${uploads[@]}" --clobber

for name in "${tarballs[@]}" "$sums"; do
  expected="sha256:$(sha256sum "$dir/$name" | cut -d' ' -f1)"
  # 摘要偶尔要过一会儿才出现在 API 里。
  actual=""
  for _ in 1 2 3 4 5; do
    actual=$(gh release view "$tag" --json assets --jq ".assets[] | select(.name == \"$name\") | .digest")
    [[ -n "$actual" && "$actual" != null ]] && break
    sleep 3
  done
  [[ "$actual" == "$expected" ]] || { echo "publish-sources.sh: $tag 上的 $name 摘要是 ${actual:-（无）}，本地是 $expected" >&2; exit 1; }
  echo "$name 已在 $tag 上，$expected"
done
