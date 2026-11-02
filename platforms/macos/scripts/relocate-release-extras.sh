#!/usr/bin/env bash
# 只适用于 0.54.0 前后发 DMG 的旧版本（现在的版本发布页只放 pkg，不需要搬）。把一个已经发布的 macOS 版本里用户用不到的资产从 macos-vVERSION 搬走，供 relocate-macos-release-extras.yml 调用：Sparkle 更新包（*-input-method.zip、*-settings.zip）搬到 macos-appcast，dSYM（msime-macos-VERSION-dsym.zip）搬到 macos-symbols。之后版本发布页只剩 DMG 和只列 DMG 的 SHA256SUMS，与 release-macos.yml 当时发布的一样。0.54.0 是需要这样搬的版本。
#
# appcast 带签名（SURequireSignedFeed），只改地址就要重新签，所以 Sparkle 更新包这样搬：
#   1. 从 macos-vVERSION 下载更新包和 SHA256SUMS，按 SHA256SUMS 核对；从 macos-appcast 下载现行 appcast，用 sparkle-public-key.txt 核对它们的签名，以及它们记录的更新包长度和 sparkle:edSignature 与下载到的文件一致。
#   2. 把每个更新包的修改时间设回原来的 pubDate，以 macos-appcast 的地址为前缀用钉住的 Sparkle 的 generate_appcast 重新生成；核对新 appcast 除地址外与原来逐项相同（构建号、长度、edSignature 都没变），并且新的签名用公钥核对通过。
#   3. 交给 publish-sparkle-feed.sh：先传更新包并按公开地址下载核对，再传 appcast，再清理 macos-appcast 上没有 appcast 指向的更新包。
#   4. 从 macos-appcast 重新下载 appcast 再核对一遍，确认都指向 macos-appcast、就是刚发布的那份。
# dSYM 按 SHA256SUMS 核对后交给 publish-dsym.sh，它用 GitHub 记录的摘要确认上传无误。
# 最后才改 macos-vVERSION：先把 SHA256SUMS 换成只列 DMG 的版本，再删掉搬走的资产。前面任何一步失败都不会走到这里。--dry-run 做完第 2 步和 dSYM 的核对就停下，不上传、不删除。
# 已经搬过的部分（发布页上没有更新包，或没有 dSYM）跳过，只核对现行 appcast；可以重跑。
#
# Usage: relocate-release-extras.sh [--dry-run] <version> <work-dir>
# Environment: MSIME_SPARKLE_ROOT（含 bin/generate_appcast 的 Sparkle 2.9.6 目录）和 MSIME_SPARKLE_PRIVATE_KEY（只经标准输入交给 generate_appcast），只在要搬更新包时需要；GH_TOKEN，以及 publish-sparkle-feed.sh 认的变量。
set -euo pipefail

dry_run=false
if [[ "${1:-}" == --dry-run ]]; then dry_run=true; shift; fi
[[ $# -eq 2 ]] || { echo "usage: relocate-release-extras.sh [--dry-run] <version> <work-dir>" >&2; exit 2; }
version=$1 work=$2
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "版本号应为 X.Y.Z：$version" >&2; exit 2; }
release="macos-v$version"
download_prefix="${MSIME_SPARKLE_DOWNLOAD_URL_PREFIX:-https://github.com/metasequoiaime/msime/releases/download/macos-appcast/}"
old_prefix="https://github.com/metasequoiaime/msime/releases/download/$release/"
dsym="msime-macos-$version-dsym.zip"
here="$(cd "$(dirname "$0")" && pwd)"
feed_tool="$here/sparkle_feed.py"

fail() { echo "relocate-release-extras.sh: $*" >&2; exit 1; }

rm -rf "$work"
mkdir -p "$work/release" "$work/current" "$work/regenerate" "$work/publish" "$work/final" "$work/upload"
gh release view "$release" --json assets --jq '.assets[].name' | sort > "$work/release-assets"
grep -E -- '-(input-method|settings)\.zip$' "$work/release-assets" > "$work/release-archives" || true
has_dsym=false
grep -qxF -- "$dsym" "$work/release-assets" && has_dsym=true
# 要从版本发布页删掉的资产：更新包和 dSYM。
cat "$work/release-archives" > "$work/to-remove"
[[ "$has_dsym" == true ]] && echo "$dsym" >> "$work/to-remove"
sort -o "$work/to-remove" "$work/to-remove"

# 现行 appcast 全部指向 macos-appcast，签名有效。
verify_published() {
  rm -rf "$work/final"
  mkdir -p "$work/final"
  gh release download macos-appcast --pattern 'appcast-*.xml' --dir "$work/final"
  python3 "$feed_tool" verify --url-prefix "$download_prefix" "$work/final"/appcast-*.xml
}

if [[ ! -s "$work/to-remove" ]]; then
  echo "$release 上已经只有 DMG 和 SHA256SUMS，只核对现行 appcast"
  verify_published
  exit 0
fi

gh release download "$release" --pattern SHA256SUMS --dir "$work/release"
# 要搬走的每个文件都必须在 SHA256SUMS 里，下载后逐个核对。
awk 'NR == FNR {remove[$0] = 1; next} ($2 in remove)' "$work/to-remove" "$work/release/SHA256SUMS" > "$work/moved-SHA256SUMS"
sed -n 's/^[0-9a-f]\{64\}  //p' "$work/moved-SHA256SUMS" | sort > "$work/moved-summed"
cmp -s "$work/to-remove" "$work/moved-summed" || { diff "$work/to-remove" "$work/moved-summed" >&2 || true; fail "$release 上要搬走的资产与它的 SHA256SUMS 列出的不一致"; }
while IFS= read -r name; do
  gh release download "$release" --pattern "$name" --dir "$work/release"
done < "$work/to-remove"
(cd "$work/release" && shasum -a 256 -c ../moved-SHA256SUMS)

# ---- Sparkle 更新包：核对现行 appcast，换前缀重新生成 ----
feeds=()
if [[ -s "$work/release-archives" ]]; then
  grep -E -- '-(input-method|settings)\.zip$' "$work/moved-SHA256SUMS" > "$work/publish/sparkle-SHA256SUMS"
  gh release download macos-appcast --pattern 'appcast-*.xml' --dir "$work/current"
  shopt -s nullglob
  current=("$work/current"/appcast-*.xml)
  [[ ${#current[@]} -gt 0 ]] || fail "macos-appcast 上没有 appcast"
  python3 "$feed_tool" enclosures "${current[@]}" > "$work/enclosures"
  : > "$work/referenced-archives"
  while IFS=$'\t' read -r feed build short url; do
    name="${url##*/}"
    # 测试版和每日构建的 appcast（appcast-beta-*、appcast-nightly-* 等）由各自的发布维护，迁移只重签稳定版的；和别的版本一样，它们不能指向这次要删的文件。
    if [[ "$short" != "$version" || "$feed" =~ ^appcast-(settings-)?(beta|nightly)- ]]; then
      # 别的版本的 appcast 与这次迁移无关，但它不能指向这次要删的文件。
      ! grep -qxF -- "$name" "$work/release-archives" || fail "$feed 是 $short 的 appcast，却指向 $release 的 $name"
      continue
    fi
    # 上一次运行可能已经把 appcast 换成 macos-appcast 的地址、只差删除没做完，两种地址都认。
    [[ "$url" == "$old_prefix$name" || "$url" == "$download_prefix$name" ]] || fail "$feed（构建 $build）指向意料之外的地址 $url"
    echo "$name" >> "$work/referenced-archives"
    feeds+=("$feed")
  done < "$work/enclosures"
  sort -o "$work/referenced-archives" "$work/referenced-archives"
  cmp -s "$work/release-archives" "$work/referenced-archives" || { diff "$work/release-archives" "$work/referenced-archives" >&2 || true; fail "$version 的 appcast 指向的更新包与 $release 上的不是同一组"; }
  for feed in "${feeds[@]}"; do
    python3 "$feed_tool" verify --archives "$work/release" "$work/current/$feed"
  done

  : "${MSIME_SPARKLE_ROOT:?set MSIME_SPARKLE_ROOT to the directory containing Sparkle 2.9.6}"
  : "${MSIME_SPARKLE_PRIVATE_KEY:?MSIME_SPARKLE_PRIVATE_KEY is required to re-sign the appcasts}"
  for feed in "${feeds[@]}"; do
    url="$(python3 "$feed_tool" enclosures "$work/current/$feed" | cut -f4)"
    name="${url##*/}"
    source_dir="$work/regenerate/$feed"
    mkdir -p "$source_dir"
    cp "$work/release/$name" "$source_dir/"
    python3 "$feed_tool" set-mtime "$work/current/$feed" "$source_dir/$name"
    # 原来的 appcast 在 UTC 的 runner 上生成，pubDate 带 +0000；时区不同会让 pubDate 的写法变掉。
    printf '%s' "$MSIME_SPARKLE_PRIVATE_KEY" | TZ=UTC "$MSIME_SPARKLE_ROOT/bin/generate_appcast" --ed-key-file - \
      --download-url-prefix "$download_prefix" -o "$work/publish/$feed" "$source_dir"
    python3 "$feed_tool" compare "$work/current/$feed" "$work/publish/$feed"
    cp "$work/release/$name" "$work/publish/$name"
  done
  python3 "$feed_tool" verify --archives "$work/publish" --url-prefix "$download_prefix" "$work/publish"/appcast-*.xml
fi

# ---- 改写后的 SHA256SUMS：只列 DMG，并且正好是删除之后留下的资产 ----
awk '$2 ~ /\.dmg$/' "$work/release/SHA256SUMS" > "$work/upload/SHA256SUMS"
sed -n 's/^[0-9a-f]\{64\}  //p' "$work/upload/SHA256SUMS" | sort > "$work/kept-summed"
grep -vxF SHA256SUMS "$work/release-assets" | grep -vxF -f "$work/to-remove" > "$work/kept-assets" || true
cmp -s "$work/kept-assets" "$work/kept-summed" || { diff "$work/kept-assets" "$work/kept-summed" >&2 || true; fail "删除之后 $release 留下的资产不全是 SHA256SUMS 里的 DMG"; }
[[ -s "$work/kept-assets" ]] || fail "$release 上没有 DMG"

if [[ "$dry_run" == true ]]; then
  echo "dry run：核对全部通过。重新生成的 appcast 在 $work/publish，改写后的 SHA256SUMS 在 $work/upload；没有上传或删除任何东西。将要从 $release 删除："
  cat "$work/to-remove"
  exit 0
fi

# ---- 发布到 macos-appcast 和 macos-symbols ----
if [[ ${#feeds[@]} -gt 0 ]]; then
  # 只重新发布原有的稳定版 appcast（换了更新包地址），不往测试版和每日构建的 appcast 里复制。
  MSIME_SPARKLE_SKIP_WIDER=1 bash "$here/publish-sparkle-feed.sh" "$work/publish" "$version" stable
  verify_published
  for feed in "${feeds[@]}"; do
    cmp -s "$work/final/$feed" "$work/publish/$feed" || fail "macos-appcast 上的 $feed 不是刚发布的那份"
  done
fi
if [[ "$has_dsym" == true ]]; then
  bash "$here/publish-dsym.sh" "$work/release/$dsym"
fi

# ---- 最后才从 macos-vVERSION 拿掉它们 ----
gh release upload "$release" "$work/upload/SHA256SUMS" --clobber
while IFS= read -r name; do
  gh release delete-asset "$release" "$name" --yes
  echo "已从 $release 删除 $name"
done < "$work/to-remove"
echo "$release 现在的资产："
gh release view "$release" --json assets --jq '.assets[].name'
