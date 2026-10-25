#!/usr/bin/env bash
# 把一个 macOS 版本的 Sparkle 更新包和 appcast 发布到滚动的 macos-appcast release。release-macos.yml 在 main 的正式版（stable）、develop 的测试版（beta）和 nightly.yml 的每日构建（nightly）之后调用它，relocate-sparkle-archives.sh 迁移旧版本时也调用它。
#
# 顺序就是安全性：先在本地核对 appcast 签名、更新包长度与签名和 sparkle-SHA256SUMS，再确认不会把已发布的 appcast 退回更旧的构建；然后先传更新包、按 appcast 里的公开地址下载回来核对摘要，确认都下得到之后才传 appcast，下载回来比对；再把 appcast 复制到构建更旧（或还没有 appcast）的更宽通道，并用在线的稳定版 appcast 补齐其他版本还不存在的更宽通道 feed；最后删掉 macos-appcast 上属于这个通道、不再被任何 appcast 指向的旧更新包。任何一步失败都停下，已经在线的 appcast 和它指向的更新包保持原样。本通道的 appcast 已经都是这个构建时（重跑、或每日构建没有新提交的日子）跳过上传，但仍做后两步：上一次运行可能在复制到更宽通道时失败，重跑要能补上。
#
# 每个通道有自己的 appcast 文件（命名规则见 sparkle_feed.py 的 feed_name）：稳定版 appcast-<版本 id>.xml 与引入通道之前相同，测试版和每日构建是 appcast-beta-<版本 id>.xml、appcast-nightly-<版本 id>.xml，设置应用同理。选了更宽通道的用户也该拿到更窄通道里更新的构建，所以发布正式版时，测试版和每日构建的 appcast 若是更旧的构建（或还不存在）也换成这一份，发布测试版时同样更新每日构建的；那边已有同样新或更新的构建时不动。appcast 里只有更新包地址，不含自己的文件名，同一份签过名的文件换个名字照样有效。
# 一次运行只写自己通道的 appcast 和更宽通道的复制品，只删自己通道的更新包，同时进行的另一个通道的发布不会删掉它刚传上去、还没有 appcast 指向的更新包。
# 更新包的摘要文件也按通道分开：稳定版沿用 sparkle-SHA256SUMS，测试版和每日构建是 sparkle-SHA256SUMS-beta、sparkle-SHA256SUMS-nightly，各自只列本通道的更新包。三个通道共用一个文件名时，每晚的每日构建都会把它换成只列 nightly 更新包的版本，稳定版的更新包就在已发布的摘要里查不到了。更新包名里带着通道（sparkle_feed.py 的 archive_channel），某个更新包的摘要在哪个文件里由它自己的名字决定，复制到更宽通道的 appcast 指向的仍是原通道的更新包。
#
# Usage: publish-sparkle-feed.sh <dir> <version> [channel]
#   <dir> 里有 appcast-*.xml、它们指向的 *-input-method.zip / *-settings.zip 和列出这些更新包的 sparkle-SHA256SUMS（package-release.sh 的输出目录就是这样）。
#   channel 是 stable（缺省）、beta 或 nightly，必须与 <dir> 里 appcast 和更新包的文件名一致（package-release.sh 的 MSIME_UPDATE_CHANNEL）。
# Environment: GH_TOKEN；GH_REPO 在工作目录不是仓库检出时指明仓库。MSIME_SPARKLE_SKIP_WIDER=1 时不复制到更宽的通道（relocate-release-extras.sh 只重签已有的稳定版 appcast）。MSIME_SPARKLE_DOWNLOAD_URL_PREFIX 与 MSIME_SPARKLE_APPCAST_BASE_URL 与 package-release.sh 的同名变量一致，缺省都是 macos-appcast release 的下载地址。
# 按 bash 3.2 写（macOS runner 可能用它）：不用关联数组和 mapfile。
set -euo pipefail

[[ $# -eq 2 || $# -eq 3 ]] || { echo "usage: publish-sparkle-feed.sh <dir> <version> [stable|beta|nightly]" >&2; exit 2; }
dir=$1 version=$2 channel=${3:-stable}
case "$channel" in
  stable|beta|nightly) ;;
  *) echo "publish-sparkle-feed.sh: 不认识的更新通道 $channel" >&2; exit 2 ;;
esac
tag=macos-appcast
download_prefix="${MSIME_SPARKLE_DOWNLOAD_URL_PREFIX:-https://github.com/metasequoiaime/msime/releases/download/macos-appcast/}"
appcast_base="${MSIME_SPARKLE_APPCAST_BASE_URL:-https://github.com/metasequoiaime/msime/releases/download/macos-appcast/}"
here="$(cd "$(dirname "$0")" && pwd)"
feed_tool="$here/sparkle_feed.py"
sums="$dir/sparkle-SHA256SUMS"
# 上传到 macos-appcast 时的名字，见开头的说明。
if [[ "$channel" == stable ]]; then sums_asset=sparkle-SHA256SUMS; else sums_asset="sparkle-SHA256SUMS-$channel"; fi
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
# 文件名必须是这个通道的 appcast，免得把测试版的构建写进稳定版的 feed。
python3 "$feed_tool" feeds --channel "$channel" "${appcasts[@]}" > /dev/null
python3 "$feed_tool" enclosures "${appcasts[@]}" > "$scratch/enclosures"
archives=()
while IFS=$'\t' read -r feed build short url; do
  [[ "$short" == "$version" ]] || fail "$feed 是 $short 的 appcast，不是 $version"
  name="${url##*/}"
  [[ "$url" == "$download_prefix$name" ]] || fail "$feed 指向 $url，不在 $download_prefix 下"
  case "$name" in *-input-method.zip|*-settings.zip) ;; *) fail "$feed 指向的 $name 不是输入法或设置应用的更新包" ;; esac
  archives+=("$name")
done < "$scratch/enclosures"
python3 "$feed_tool" archive-channel "${archives[@]}" > "$scratch/archive-channels"
while IFS=$'\t' read -r name archive_channel; do
  [[ "$archive_channel" == "$channel" ]] || fail "$name 是 $archive_channel 通道的更新包，不是 $channel"
done < "$scratch/archive-channels"
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
    --notes "macOS 输入法与完整设置应用的 Sparkle appcast：每个发行版本各有 appcast-<edition>.xml 和 appcast-settings-<edition>.xml（正式版，由 main 上的正式发布更新），以及测试版的 appcast-beta-<edition>.xml、appcast-settings-beta-<edition>.xml（develop 的测试版发布）和每日构建的 appcast-nightly-<edition>.xml、appcast-settings-nightly-<edition>.xml（每日构建），更宽的通道也收到更窄通道里更新的构建；它们指向的更新包也放在这里，只保留仍被引用的，摘要见 sparkle-SHA256SUMS（正式版）、sparkle-SHA256SUMS-beta 和 sparkle-SHA256SUMS-nightly，按更新包名里的通道查。"
fi
gh release view "$tag" --json assets --jq '.assets[].name' > "$scratch/published"
mkdir -p "$scratch/current"
if grep -q '^appcast-.*\.xml$' "$scratch/published"; then
  gh release download "$tag" --pattern 'appcast-*.xml' --dir "$scratch/current" --clobber
fi
same=0
while IFS=$'\t' read -r feed build short url; do
  current="$scratch/current/$feed"
  [[ -f "$current" ]] || continue
  IFS=$'\t' read -r _ old_build _ old_url < <(python3 "$feed_tool" enclosures "$current")
  if [[ "$old_build" -gt "$build" ]]; then
    echo "::error::$feed already offers build $old_build, newer than $build" >&2
    exit 1
  fi
  # 同一个构建、指向同一个地址，它指向的更新包也还在：这份 appcast 已经发布过了。
  if [[ "$old_build" -eq "$build" && "$old_url" == "$url" ]] && grep -qxF -- "${url##*/}" "$scratch/published"; then same=$((same + 1)); fi
done < "$scratch/enclosures"
# 构建号由提交时间算出（release-macos.yml），每日构建在没有新提交的日子编出的是同一个构建。全部 appcast 都已经是这个构建时不传更新包和本通道的 appcast：重传同名更新包会在 appcast 换上新签名之前让正在下载的用户验签失败。后面复制到更宽通道和清理旧更新包照常做，上一次运行若在那两步失败，重跑就在这里补上。
already_published=0
if [[ "$same" -eq ${#appcasts[@]} ]]; then
  already_published=1
  echo "$tag 上 $channel 通道的 appcast 已经都是这个构建，不再上传更新包和本通道的 appcast"
fi

if [[ "$already_published" -eq 0 ]]; then
  # ---- 先传更新包，确认下得到 ----
  archive_paths=()
  for name in "${archives[@]}"; do archive_paths+=("$dir/$name"); done
  mkdir -p "$scratch/sums"
  cp "$sums" "$scratch/sums/$sums_asset"
  gh release upload "$tag" "${archive_paths[@]}" "$scratch/sums/$sums_asset" --clobber
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
fi

# ---- 复制到更宽的通道 ----
# 那边已有同样新或更新的构建（例如测试版之后又发了更新的每日构建）时不动；没有那份 appcast 时新建，第一次发布某个通道之前，选了更宽通道的用户也有 feed 可读。
# 复制的是已经在线的那份 appcast。这次上传过时它与本地文件相同（上面下载回来比对过）；跳过上传时两者可能不同：重跑重新打出的更新包不一定逐字节相同，本地 appcast 里的长度和签名对应的是本地的更新包，而在线的更新包是上一次运行传的，只有在线的 appcast 与它配套。
mkdir -p "$scratch/wider"
if [[ "${MSIME_SPARKLE_SKIP_WIDER:-}" == 1 ]]; then
  : > "$scratch/enclosures-wider"
else
  cp "$scratch/enclosures" "$scratch/enclosures-wider"
fi
while IFS=$'\t' read -r feed build short url; do
  # 循环体里的命令都不读标准输入，标准输入留给外层逐行读的 enclosures。
  for target in $(python3 "$feed_tool" wider "$feed" < /dev/null); do
    current="$scratch/current/$target"
    if [[ -f "$current" ]]; then
      old_build="$(python3 "$feed_tool" enclosures "$current" < /dev/null | cut -f2)"
      if [[ "$old_build" -ge "$build" ]]; then
        echo "$target 已经提供构建 ${old_build}，不用 $feed 的构建 $build 覆盖"
        continue
      fi
    fi
    source_feed="$dir/$feed"
    if [[ "$already_published" -eq 1 ]]; then source_feed="$scratch/current/$feed"; fi
    cp "$source_feed" "$scratch/wider/$target"
    gh release upload "$tag" "$scratch/wider/$target" --clobber < /dev/null
    fetch "$appcast_base$target" "$scratch/download" < /dev/null
    cmp -s "$scratch/download" "$source_feed" || fail "$appcast_base$target 下载到的内容与刚上传的不同"
    echo "$appcast_base$target 已换成 $feed 的构建 $build"
    rm -f "$scratch/download"
  done
done < "$scratch/enclosures-wider"

# ---- 补齐还不存在的更宽通道 feed ----
# 上面的复制只覆盖这次发布的版本。某个版本的测试版或每日构建 appcast 还不存在时（刚引入通道、或某个通道还没发过这个版本），选了那个通道的输入法和设置应用每次检查都读到 404：后台检查静默失败，手动检查报读取出错。所以每次发布（每晚的每日构建最频繁）都把在线的每份稳定版 appcast 复制到它还不存在的更宽通道；已经存在的一律不动，哪怕更旧，那由正常的发布和上面的复制负责。复制品指向稳定版的更新包，下面的清理按全部 appcast 的引用判断，不会把它删掉。
if [[ "${MSIME_SPARKLE_SKIP_WIDER:-}" != 1 ]]; then
  rm -rf "$scratch/online" "$scratch/seed"
  mkdir -p "$scratch/online" "$scratch/seed"
  gh release download "$tag" --pattern 'appcast-*.xml' --dir "$scratch/online"
  for stable_feed in "$scratch/online"/appcast-*.xml; do
    stable_name="$(basename "$stable_feed")"
    # 只从稳定版 appcast 补：parse_feed_name 认不出或不是稳定版的跳过。
    python3 "$feed_tool" feeds --channel stable "$stable_feed" > /dev/null 2>&1 || continue
    for target in $(python3 "$feed_tool" wider "$stable_name"); do
      [[ -f "$scratch/online/$target" ]] && continue
      cp "$stable_feed" "$scratch/seed/$target"
      gh release upload "$tag" "$scratch/seed/$target"
      fetch "$appcast_base$target" "$scratch/download"
      cmp -s "$scratch/download" "$stable_feed" || fail "$appcast_base$target 下载到的内容与刚上传的不同"
      echo "$appcast_base$target 还不存在，先用稳定版 $stable_name 补上"
      rm -f "$scratch/download"
    done
  done
fi

# ---- 删掉没有 appcast 指向的旧更新包 ----
# 按 release 上全部 appcast 实际指向的文件判断，而不是按版本号：这次没有发布的版本（例如从 MACOS_EDITIONS 里去掉的）仍指着自己的更新包，不能删。只删这个通道的更新包（文件名里的通道见 sparkle_feed.py 的 archive_channel）：别的通道的发布可能正在进行，它先传更新包、后传 appcast，中间这段时间它的更新包还没有被引用。
rm -rf "$scratch/current"
mkdir -p "$scratch/current"
gh release download "$tag" --pattern 'appcast-*.xml' --dir "$scratch/current"
python3 "$feed_tool" enclosures "$scratch/current"/appcast-*.xml | cut -f4 \
  | awk -v prefix="$download_prefix" 'index($0, prefix) == 1 {print substr($0, length(prefix) + 1)}' | sort -u > "$scratch/referenced"
gh release view "$tag" --json assets --jq '.assets[].name' > "$scratch/published"
while IFS= read -r name; do
  case "$name" in *-input-method.zip|*-settings.zip) ;; *) continue ;; esac
  [[ "$(python3 "$feed_tool" archive-channel "$name" < /dev/null | cut -f2)" == "$channel" ]] || continue
  if ! grep -qxF -- "$name" "$scratch/referenced"; then
    gh release delete-asset "$tag" "$name" --yes
    echo "已删除不再被引用的 $name"
  fi
done < "$scratch/published"
