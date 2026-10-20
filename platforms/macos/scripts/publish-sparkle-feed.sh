#!/usr/bin/env bash
# 把一个 macOS 版本的 Sparkle 更新包和 appcast 发布到滚动的 macos-appcast release。release-macos.yml 在 main 的正式版之后调用它，relocate-sparkle-archives.sh 迁移旧版本时也调用它。
#
# 顺序就是安全性：先在本地核对 appcast 签名、更新包长度与签名和 sparkle-SHA256SUMS，再确认不会把已发布的 appcast 退回更旧的构建；然后先传更新包、按 appcast 里的公开地址下载回来核对摘要，确认都下得到之后才传 appcast，下载回来比对；最后删掉 macos-appcast 上不再被任何 appcast 指向的旧更新包。任何一步失败都停下，已经在线的 appcast 和它指向的更新包保持原样。
#
# Usage: publish-sparkle-feed.sh <dir> <version>
#   <dir> 里有 appcast-*.xml、它们指向的 *-input-method.zip / *-settings.zip 和列出这些更新包的 sparkle-SHA256SUMS（package-release.sh 的输出目录就是这样）。
# Environment: GH_TOKEN；GH_REPO 在工作目录不是仓库检出时指明仓库。MSIME_SPARKLE_DOWNLOAD_URL_PREFIX 与 MSIME_SPARKLE_APPCAST_BASE_URL 与 package-release.sh 的同名变量一致，缺省都是 macos-appcast release 的下载地址。
# 按 bash 3.2 写（macOS runner 可能用它）：不用关联数组和 mapfile。
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: publish-sparkle-feed.sh <dir> <version>" >&2; exit 2; }
dir=$1 version=$2
tag=macos-appcast
download_prefix="${MSIME_SPARKLE_DOWNLOAD_URL_PREFIX:-https://github.com/metasequoiaime/msime/releases/download/macos-appcast/}"
appcast_base="${MSIME_SPARKLE_APPCAST_BASE_URL:-https://github.com/metasequoiaime/msime/releases/download/macos-appcast/}"
here="$(cd "$(dirname "$0")" && pwd)"
feed_tool="$here/sparkle_feed.py"
sums="$dir/sparkle-SHA256SUMS"
scratch="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/msime-sparkle-publish.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

fail() { echo "publish-sparkle-feed.sh: $*" >&2; exit 1; }
# GitHub 刚上传的资产偶尔要几秒才下得到；curl -o 在重试前截断输出文件，不会把两次的字节拼在一起。
fetch() { curl --fail --silent --show-error --location --retry 5 --retry-delay 3 --retry-all-errors --output "$2" "$1"; }

shopt -s nullglob
appcasts=("$dir"/appcast-*.xml)
[[ ${#appcasts[@]} -gt 0 ]] || fail "$dir 里没有 appcast-*.xml"
[[ -f "$sums" ]] || fail "$dir 里没有 sparkle-SHA256SUMS"

# ---- 本地核对 ----
python3 "$feed_tool" enclosures "${appcasts[@]}" > "$scratch/enclosures"
archives=()
while IFS=$'\t' read -r feed build short url; do
  [[ "$short" == "$version" ]] || fail "$feed 是 $short 的 appcast，不是 $version"
  name="${url##*/}"
  [[ "$url" == "$download_prefix$name" ]] || fail "$feed 指向 $url，不在 $download_prefix 下"
  case "$name" in *-input-method.zip|*-settings.zip) ;; *) fail "$feed 指向的 $name 不是输入法或设置应用的更新包" ;; esac
  archives+=("$name")
done < "$scratch/enclosures"
python3 "$feed_tool" verify --archives "$dir" --url-prefix "$download_prefix" "${appcasts[@]}"
# 校验文件列出的恰好是这些更新包，并且摘要对得上。
printf '%s\n' "${archives[@]}" | sort > "$scratch/archives"
sed -n 's/^[0-9a-f]\{64\}  //p' "$sums" | sort > "$scratch/summed"
[[ "$(wc -l < "$sums" | tr -d ' ')" -eq "$(wc -l < "$scratch/summed" | tr -d ' ')" ]] || fail "sparkle-SHA256SUMS 有无法识别的行"
cmp -s "$scratch/archives" "$scratch/summed" || { diff "$scratch/archives" "$scratch/summed" >&2 || true; fail "sparkle-SHA256SUMS 列出的文件与 appcast 指向的更新包不一致"; }
(cd "$dir" && shasum -a 256 -c sparkle-SHA256SUMS)

# ---- 不退回更旧的构建 ----
if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" --prerelease --latest=false --title "macOS Sparkle appcasts" \
    --notes "macOS 输入法与完整设置应用的 Sparkle 更新通道：每个发行版本各有 appcast-<edition>.xml 和 appcast-settings-<edition>.xml，由 main 上的正式发布更新；它们指向的更新包也放在这里，只保留当前版本的，摘要见 sparkle-SHA256SUMS。"
fi
gh release view "$tag" --json assets --jq '.assets[].name' > "$scratch/published"
mkdir -p "$scratch/current"
if grep -q '^appcast-.*\.xml$' "$scratch/published"; then
  gh release download "$tag" --pattern 'appcast-*.xml' --dir "$scratch/current" --clobber
fi
while IFS=$'\t' read -r feed build short url; do
  current="$scratch/current/$feed"
  [[ -f "$current" ]] || continue
  old_build="$(python3 "$feed_tool" enclosures "$current" | cut -f2)"
  if [[ "$old_build" -gt "$build" ]]; then
    echo "::error::$feed already offers build $old_build, newer than $build" >&2
    exit 1
  fi
done < "$scratch/enclosures"

# ---- 先传更新包，确认下得到 ----
archive_paths=()
for name in "${archives[@]}"; do archive_paths+=("$dir/$name"); done
gh release upload "$tag" "${archive_paths[@]}" "$sums" --clobber
for name in "${archives[@]}"; do
  fetch "$download_prefix$name" "$scratch/download"
  expected="$(awk -v name="$name" '$2 == name {print $1}' "$sums")"
  actual="$(shasum -a 256 "$scratch/download" | cut -d' ' -f1)"
  [[ -n "$expected" && "$actual" == "$expected" ]] || fail "$download_prefix$name 下载到的内容与 sparkle-SHA256SUMS 不符"
  echo "$download_prefix$name 可下载，SHA-256 $actual"
  rm -f "$scratch/download"
done

# ---- 再传 appcast，下载回来比对 ----
gh release upload "$tag" "${appcasts[@]}" --clobber
for appcast in "${appcasts[@]}"; do
  feed="$(basename "$appcast")"
  fetch "$appcast_base$feed" "$scratch/download"
  cmp -s "$scratch/download" "$appcast" || fail "$appcast_base$feed 下载到的内容与刚上传的不同"
  echo "$appcast_base$feed 已更新"
  rm -f "$scratch/download"
done

# ---- 删掉没有 appcast 指向的旧更新包 ----
# 按 release 上全部 appcast 实际指向的文件判断，而不是按版本号：这次没有发布的版本（例如从 MACOS_EDITIONS 里去掉的）仍指着自己的更新包，不能删。
rm -rf "$scratch/current"
mkdir -p "$scratch/current"
gh release download "$tag" --pattern 'appcast-*.xml' --dir "$scratch/current"
python3 "$feed_tool" enclosures "$scratch/current"/appcast-*.xml | cut -f4 \
  | awk -v prefix="$download_prefix" 'index($0, prefix) == 1 {print substr($0, length(prefix) + 1)}' | sort -u > "$scratch/referenced"
gh release view "$tag" --json assets --jq '.assets[].name' > "$scratch/published"
while IFS= read -r name; do
  case "$name" in *-input-method.zip|*-settings.zip) ;; *) continue ;; esac
  if ! grep -qxF -- "$name" "$scratch/referenced"; then
    gh release delete-asset "$tag" "$name" --yes
    echo "已删除不再被引用的 $name"
  fi
done < "$scratch/published"
