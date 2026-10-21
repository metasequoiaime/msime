#!/usr/bin/env bash
# 把一次发布六个版本的符号包传到滚动的 windows-symbols release（预发布、不设为 latest），供解析崩溃转储。版本发布页只放给用户下载的安装程序，符号包不进去；workflow artifact 90 天后过期，所以每个版本的符号包都长期留在 windows-symbols 上，文件名带发布名，互不覆盖。
#
# 上传后用 GitHub 为资产记录的 SHA-256（`digest`）与本地文件逐个比对，对不上就失败。
#
# Usage: publish-symbols.sh <version> <release> <symbols-zip>...
#   每个 zip 是 Collect-Symbols.ps1 写出的 msime-windows-<edition>-<version>-symbols.zip，上传时名字里的 <version> 换成 <release>：预发布带后缀（msime-windows-full-0.3.0-beta-symbols.zip），免得与同一版本号的正式版互相覆盖。
# Environment: GH_TOKEN；GH_REPO 在工作目录不是仓库检出时指明仓库。
set -euo pipefail

[[ $# -ge 3 ]] || { echo "usage: publish-symbols.sh <version> <release> <symbols-zip>..." >&2; exit 2; }
version=$1
release=$2
shift 2
tag=windows-symbols
[[ "$release" == "$version" || "$release" == "$version"-* ]] || { echo "publish-symbols.sh: 发布名 $release 不是以版本号 $version 开头" >&2; exit 1; }

scratch="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/msime-symbols.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
names=()
for file in "$@"; do
  [[ -f "$file" ]] || { echo "publish-symbols.sh: $file 不存在" >&2; exit 1; }
  base="$(basename "$file")"
  [[ "$base" =~ ^msime-windows-([a-z][a-z0-9]*)-${version//./\\.}-symbols\.zip$ ]] || { echo "publish-symbols.sh: $base 不是 msime-windows-<版本>-$version-symbols.zip" >&2; exit 1; }
  name="msime-windows-${BASH_REMATCH[1]}-$release-symbols.zip"
  cp "$file" "$scratch/$name"
  names+=("$name")
done

if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" --prerelease --latest=false --title "Windows symbols" \
    --notes "Windows 各版本的调试符号（msime-windows-<版本 id>-<发布名>-symbols.zip），用来解析崩溃转储，用户不需要下载。每个发布版本一份，长期保留。"
fi
gh release upload "$tag" "${names[@]/#/$scratch/}" --clobber

for name in "${names[@]}"; do
  expected="sha256:$(sha256sum "$scratch/$name" | cut -d' ' -f1)"
  # 摘要偶尔要过一会儿才出现在 API 里。
  actual=""
  for _ in 1 2 3 4 5; do
    actual="$(gh release view "$tag" --json assets --jq ".assets[] | select(.name == \"$name\") | .digest")"
    [[ -n "$actual" && "$actual" != null ]] && break
    sleep 3
  done
  [[ "$actual" == "$expected" ]] || { echo "publish-symbols.sh: $tag 上的 $name 摘要是 ${actual:-（无）}，本地是 $expected" >&2; exit 1; }
  echo "$name 已在 $tag 上，$expected"
done
