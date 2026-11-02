#!/usr/bin/env bash
# 把一个版本的 dSYM zip 传到滚动的 macos-symbols release（预发布、不设为 latest），供符号化崩溃报告。版本发布页只放给用户下载的安装包，dSYM 不进去；workflow artifact 90 天后过期，所以每个版本的 dSYM 都长期留在 macos-symbols 上，文件名带版本号，互不覆盖。
#
# 上传后用 GitHub 为资产记录的 SHA-256（`digest`）与本地文件比对，对不上就失败。
#
# Usage: publish-dsym.sh <dsym-zip> [<asset-name>]
#   <asset-name> 缺省是文件名；预发布用带后缀的名字（msime-macos-0.55.0-beta-dsym.zip），免得与同一版本号的正式版互相覆盖。
# Environment: GH_TOKEN；GH_REPO 在工作目录不是仓库检出时指明仓库。
set -euo pipefail

[[ $# -eq 1 || $# -eq 2 ]] || { echo "usage: publish-dsym.sh <dsym-zip> [<asset-name>]" >&2; exit 2; }
file=$1
name=${2:-$(basename "$file")}
tag=macos-symbols
[[ -f "$file" ]] || { echo "publish-dsym.sh: $file 不存在" >&2; exit 1; }
case "$name" in msime-macos-*-dsym.zip) ;; *) echo "publish-dsym.sh: $name 不是 msime-macos-<版本>-dsym.zip" >&2; exit 1 ;; esac

if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" --prerelease --latest=false --title "macOS dSYM" \
    --notes "macOS 各版本的 dSYM（msime-macos-<版本>-dsym.zip），用来符号化崩溃报告，用户不需要下载。每个发布版本一份，长期保留。"
fi
upload="$file"
scratch=""
if [[ "$name" != "$(basename "$file")" ]]; then
  scratch="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/msime-dsym.XXXXXX")"
  trap 'rm -rf "$scratch"' EXIT
  upload="$scratch/$name"
  cp "$file" "$upload"
fi
gh release upload "$tag" "$upload" --clobber
expected="sha256:$(shasum -a 256 "$file" | cut -d' ' -f1)"
# 摘要偶尔要过一会儿才出现在 API 里。
actual=""
for _ in 1 2 3 4 5; do
  actual="$(gh release view "$tag" --json assets --jq ".assets[] | select(.name == \"$name\") | .digest")"
  [[ -n "$actual" && "$actual" != null ]] && break
  sleep 3
done
[[ "$actual" == "$expected" ]] || { echo "publish-dsym.sh: $tag 上的 $name 摘要是 ${actual:-（无）}，本地是 $expected" >&2; exit 1; }
echo "$name 已在 $tag 上，$expected"
