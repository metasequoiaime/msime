#!/usr/bin/env bash
# 把一个已发布的 Linux 正式版的源码包 msime-VERSION.tar.xz 与 msime-VERSION-vendor.tar.xz 下载到 DIR 并核对，供 publish-linux-copr.yml、publish-linux-obs.yml 与 publish-linux-ppa.yml 交给 render-definitions.sh 或直接上传。
#
# 源码包现在放在滚动的 linux-sources release 上（packaging/publish-sources.sh），校验值在同处的 msime-VERSION-SHA256SUMS 里。linux-sources 出现之前发布的版本（到 0.12.1 为止）仍把它们放在自己的 linux-vVERSION 发布页上，校验值在那里的 SHA256SUMS 里；这三个 workflow 可以手动触发去补发旧版本，所以 linux-sources 上没有这个版本时退回发布页。两种情况下 DIR 里的校验文件都保留原名，那个版本自己检出里的 render-definitions.sh 认得它：旧版本的认 SHA256SUMS，新版本的认 msime-VERSION-SHA256SUMS。
#
# Usage: download-sources.sh <version> <dir>
# Environment: GH_TOKEN；GH_REPO 在工作目录不是仓库检出时指明仓库。
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: download-sources.sh <version> <dir>" >&2; exit 2; }
version=$1 dir=$2
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "download-sources.sh: 版本号必须是 X.Y.Z：$version" >&2; exit 2; }
tarballs=("msime-$version.tar.xz" "msime-$version-vendor.tar.xz")
mkdir -p "$dir"

# linux-sources 不存在或其中没有这个版本时退回发布页。退错了也不会拿错文件：新版本的发布页上没有源码包，下面的核对会失败。
if gh release view linux-sources >/dev/null 2>&1; then
  assets=$(gh release view linux-sources --json assets --jq '.assets[].name')
else
  assets=""
fi
if grep -qxF "msime-$version-SHA256SUMS" <<<"$assets"; then
  tag=linux-sources sums="msime-$version-SHA256SUMS"
else
  tag="linux-v$version" sums=SHA256SUMS
  echo "linux-sources 上没有 $version 的源码包，从 $tag 发布页下载" >&2
fi
patterns=(--pattern "$sums")
for name in "${tarballs[@]}"; do patterns+=(--pattern "$name"); done
gh release download "$tag" --dir "$dir" --clobber "${patterns[@]}"

for name in "${tarballs[@]}"; do
  [[ -f "$dir/$name" ]] || { echo "download-sources.sh: $tag 上没有 $name" >&2; exit 1; }
  line=$(grep -E "^[0-9a-f]{64} [ *]${name//./\\.}\$" "$dir/$sums" || true)
  [[ -n "$line" ]] || { echo "download-sources.sh: $tag 的 $sums 没有列出 $name" >&2; exit 1; }
  (cd "$dir" && sha256sum -c --quiet - <<<"$line")
done
echo "${tarballs[*]} 来自 ${tag}，已对着 ${sums} 核对"
