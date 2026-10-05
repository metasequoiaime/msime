#!/usr/bin/env bash
# Build the installable macOS release: the Tauri settings app with the pinned core dictionaries (EngineResources), the licence files and the InputMethodKit bundle 水杉输入法.app embedded as resources, packed into a DMG with a SHA256SUMS beside it.
#
# 包里只带装好就能打中文的核心词库（msime-pinyin.db、msime-wubi.db、bigram/trigram、msime-english.db、SCOWL 许可声明 msime-scowl_Copyright.txt、msime-others.db、sentence-model、helpcodes/、msime-dictionary-manifest.json）。其余三个资源包由 App 在首次用到时下载到 <state_root>/resource-packs/<id>/（state_root 默认 ~/Library/Application Support/app.msime.macos），查找时下载的优先、包内或旧版本记录的副本次之，两者都没有时对应功能显示为不可用：
#   japanese              msime-japanese.dat 与两份 Mozc 许可文本（mozc_dictionary_oss_README、mozc_LICENSE），用户选日文方案时下载
#   language-dictionaries 粤拼、注音与笔画词库及其许可证（resources/language-dictionaries.lock.json），用户选粤拼、注音或笔画时下载
#   handwriting           手写模型及其许可证（resources/handwriting-model.lock.json），首次打开手写面板时下载
# 打包时在编译之前先把三个资源包按 App 用的同一套安装器、URL 和哈希装一遍，链接失效或内容漂移的资源包不会随发布包出去。
#
# The same steps run locally and in release-macos.yml, so a package that passes here is the package CI publishes.
#
# Usage: platforms/macos/package-release.sh [--editions ID[,ID...]] [VERSION] [OUT_DIR]
#   --editions 是版本表 shared/contracts/editions.json 里的版本 id，缺省只打 full，产物与引入版本之前相同。几个版本共用一次编译：输入法、msime-mcp 和设置应用的可执行文件都只编一遍，每个版本只重新暂存自己的资源、用 platforms/macos/scripts/edition_bundle.py 把输入法 bundle 改成该版本的身份、按该版本的 identifier 和 productName 打一个设置应用，再各出一个 DMG。多个版本可以同时安装，彼此完全隔离。
#   VERSION defaults to platforms/macos/version.txt, the version release-macos.yml tags as macos-vVERSION. It becomes the version the settings app reports, so the in-app update check compares like with like, and it must equal the input method's CFBundleShortVersionString (platforms/macos/Info.plist.in).
#   OUT_DIR defaults to target/macos-package/dist and receives one <dmg_prefix>-VERSION-universal.dmg per edition (full: msime-macos-VERSION-universal.dmg) and SHA256SUMS.
#
# Environment:
#   MSIME_SPARKLE_ROOT            required; directory containing the pinned Sparkle.framework (see README.md)
#   CARGO_TARGET_DIR              defaults to target/ in the repository
#   MSIME_MACOS_BUILD_DIR         CMake build tree; defaults to target/macos-release
#   MACOS_SIGNING_IDENTITY        a "Developer ID Application: ... (TEAMID)" identity in the keychain. Without it everything is signed ad-hoc: the package builds and the settings app runs, but macOS will not register the embedded input method as an input source (see scripts/install.sh)
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_SPECIFIC_PASSWORD
#                                 when all three are set (and an identity is), the DMG is notarized and stapled
#
# 产物是 universal 的：Apple 芯片和 Intel Mac 用同一个包。Rust 产物按两个 target 各编一次再用 lipo 合并，CMake 的目标和 Swift 后端经 CMAKE_OSX_ARCHITECTURES 编出双架构，打包后 check_app 逐个核对包里的 Mach-O 都含两种架构。需要 rustup target add aarch64-apple-darwin x86_64-apple-darwin。
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"

editions=(full)
if [ "${1:-}" = --editions ]; then
  [ -n "${2:-}" ] || { echo "--editions needs a comma-separated list of edition ids" >&2; exit 2; }
  IFS=, read -r -a editions <<< "$2"
  shift 2
fi
edition_tool="$repo_root/platforms/macos/scripts/edition_bundle.py"
# 每个版本 id 都要在版本表里且带 macOS 标识，否则在编译之前就停下。
for edition in "${editions[@]}"; do
  python3 "$edition_tool" field --edition "$edition" input_method_bundle_id >/dev/null
done

version="${1:-$(tr -d '[:space:]' < platforms/macos/version.txt)}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "version must be MAJOR.MINOR.PATCH: $version" >&2
  exit 2
}
out_dir="${2:-$repo_root/target/macos-package/dist}"
: "${MSIME_SPARKLE_ROOT:?set MSIME_SPARKLE_ROOT to the directory containing Sparkle.framework 2.9.6}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/target}"
build_dir="${MSIME_MACOS_BUILD_DIR:-$repo_root/target/macos-release}"
identity="${MACOS_SIGNING_IDENTITY:-}"
arch=universal
architectures=(arm64 x86_64)
rust_targets=(aarch64-apple-darwin x86_64-apple-darwin)
entitlements="$repo_root/platforms/macos/resources/VoiceInput.entitlements"

work="$(mktemp -d "${TMPDIR:-/tmp}/msime-macos-package.XXXXXX")"
mount_point=""
cleanup() {
  if [ -n "$mount_point" ]; then hdiutil detach -quiet "$mount_point" || true; fi
  rm -rf "$work"
}
trap cleanup EXIT

# 按 rust_targets 各编一次（其余参数原样传给 cargo build），再把每个 target 的同名产物用 lipo 合并到第一个参数指定的路径。
cargo_universal() {
  local output="$1" name="$2" target slices=()
  shift 2
  for target in "${rust_targets[@]}"; do
    cargo build --release --locked --target "$target" "$@"
    slices+=("$CARGO_TARGET_DIR/$target/release/$name")
  done
  mkdir -p "$(dirname "$output")"
  lipo -create "${slices[@]}" -output "$output"
}
universal_dir="$CARGO_TARGET_DIR/universal/release"

if [ -z "$identity" ]; then
  echo "warning: MACOS_SIGNING_IDENTITY is not set; signing ad-hoc. The settings app will run, but macOS will not register the embedded input method as an input source until it is re-signed with a Developer ID (platforms/macos/scripts/install.sh)." >&2
fi

# 带安全时间戳的 codesign。时间戳要现场向 Apple 的时间戳服务器取，服务器偶尔会回 "The timestamp service is not available."，这时重试即可，不能让一次抖动白白废掉整次发版（2026-10-05 的 Release macOS 就是在 wubi 版签名时这样失败的）。其他错误不是抖动，原样失败。
codesign_timestamped() {
  local attempt output
  for attempt in 1 2 3 4 5; do
    if output="$(codesign --timestamp "$@" 2>&1)"; then
      [ -z "$output" ] || printf '%s\n' "$output"
      return 0
    fi
    printf '%s\n' "$output" >&2
    case "$output" in
      *"timestamp service is not available"*) ;;
      *) return 1 ;;
    esac
    if [ "$attempt" -lt 5 ]; then
      echo "codesign: the timestamp service is not available, retrying in $((attempt * 15))s" >&2
      sleep $((attempt * 15))
    fi
  done
  return 1
}

# codesign for one path. A Developer ID signature carries a secure timestamp, which notarization requires; an ad-hoc signature cannot carry one.
sign() {
  if [ -n "$identity" ]; then
    codesign_timestamped --force --options runtime --sign "$identity" "$@"
  else
    codesign --force --options runtime --sign - "$@"
  fi
}

# The first element of a glob, failing when there is none or more than one. The bundle names are Chinese or contain spaces, and a literal path to a Chinese name can miss on APFS because of NFC/NFD normalisation; matching with a glob sidesteps both.
only() {
  if [ "$#" -ne 1 ] || [ ! -e "$1" ]; then
    echo "expected exactly one match, found: $*" >&2
    exit 1
  fi
  printf '%s\n' "$1"
}

# ---- Core dictionaries ----
# install_resources prints progress on stderr and the verified directory as its last stdout line. Each edition's stage-resources.sh run (below) re-verifies it and stages target/macos/EngineResources, which tauri.macos.conf.json embeds.
resources="$(cargo run --quiet --locked -p msime-client-core --example install_resources -- "$work/desktop-resources" | tail -n 1)"

# ---- On-demand resource packs ----
# 编译之前先用 App 运行时的同一个安装器、同一组 URL 和 SHA-256 把三个资源包装一遍，失败就在这里停下：资源包不在包里，发布出去的 App 只能靠这些地址补齐，地址失效或内容漂移的发布包不该出去。不接管道，安装器的退出码就是这一步的结果。
cargo run --quiet --locked -p msime-client-core --example install_resource_pack -- "$work/pack-check" >/dev/null

# ---- Input method bundle ----
# The minimum system version goes to the C/C++ compilers and to CMake separately, never as MACOSX_DEPLOYMENT_TARGET: rustc applies that to host proc-macro dylibs too, which then fail to load (README.md, 构建与本地测试).
CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" \
  cargo_universal "$universal_dir/libmsime_host_api.a" libmsime_host_api.a -p msime-host-api
# MSIME_HOST_LIBRARY is explicit: the CMake default is target/debug, which would link the debug Rust library into a Release bundle.
cmake -S platforms/macos -B "$build_dir" -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$(brew --prefix)" \
  -DCMAKE_OSX_ARCHITECTURES="$(IFS=';'; echo "${architectures[*]}")" \
  -DMSIME_SPARKLE_ROOT="$MSIME_SPARKLE_ROOT" -DMSIME_HOST_LIBRARY="$universal_dir/libmsime_host_api.a"
# An explicit job count: a bare --parallel with the Makefile generator starts every compile at once and runs a 7 GB runner out of memory (ci-macos.yml).
cmake --build "$build_dir" --config Release --parallel "$(sysctl -n hw.logicalcpu)"
ctest --test-dir "$build_dir" --no-tests=error --output-on-failure -R '^bundle-contents$'

built_bundle="$(only "$build_dir"/*.app)"
imk_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$built_bundle/Contents/Info.plist")"
if [ "$imk_version" != "$version" ]; then
  echo "the input method reports $imk_version but the package is $version; bump platforms/macos/Info.plist.in and version.txt together" >&2
  exit 1
fi

# ---- MCP server ----
# The same compiler flags as the input method: msime-mcp links the Engine through msime-host-api, and those objects are shared with the build above. Nothing in the app starts it; an agent's MCP configuration runs Contents/MacOS/msime-mcp over stdio.
# MSIME_VERSION 是 msime-mcp --version 和 MCP 握手报告的版本（crates/mcp-server/build.rs），与这个包的版本相同。
MSIME_VERSION="$version" CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" \
  cargo_universal "$universal_dir/msime-mcp" msime-mcp -p msime-mcp-server --bin msime-mcp

# tauri-build checks every resource path in tauri.macos.conf.json while it compiles the settings app, so full's input method bundle and EngineResources are staged once before the compile below; package_edition stages each edition's own again before bundling.
MSIME_EDITION=full MSIME_MACOS_OMIT_ON_DEMAND=1 bash platforms/macos/stage-resources.sh "$resources"
mkdir -p target/macos
find target/macos -maxdepth 1 -name '*.app' -exec rm -rf {} +
ditto "$built_bundle" "target/macos/$(python3 "$edition_tool" field --edition full bundle_name)"

# ---- Settings app executable ----
# 所有版本共用这一个可执行文件，编进去的是 full 的 Tauri 配置；不是 full 的版本在运行时按 Contents/Resources/edition.json 换上自己的 identifier 和 productName（apply_edition_to_config），打包时 tauri bundle --config 写进包里的是同样的值。
pnpm install --frozen-lockfile
pnpm --filter @msime/desktop build
# Compiled with cargo and only then bundled by `tauri bundle`, not with `tauri build`: tauri build exports MACOSX_DEPLOYMENT_TARGET from bundle.macOS.minimumSystemVersion, rustc applies it to the host proc-macro dylibs as well, and on current macOS those come out with a mis-aligned LINKEDIT string pool that dlopen rejects, so the build fails with "can't find crate". cargo leaves the variable out of its fingerprint, so a broken proc-macro would also be reused by later builds. tauri/custom-protocol is what tauri build would enable (the binary serves the embedded frontend instead of devUrl), and TAURI_CONFIG sets the version the app reports, as package-container.sh does for Linux.
# cargo_universal 是 shell 函数，env 只能执行外部程序，所以在子 shell 里 unset 再调用。合并后的 universal 可执行文件放在 target/release/msime-desktop，tauri bundle 从那里取。
(
  unset MACOSX_DEPLOYMENT_TARGET
  TAURI_CONFIG="{\"version\":\"$version\"}" \
    CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" \
    cargo_universal "$CARGO_TARGET_DIR/release/msime-desktop" msime-desktop -p msime-desktop --bin msime-desktop --features tauri/custom-protocol
)
tauri_bundle_dir="$CARGO_TARGET_DIR/release/bundle/macos"
# Non-English candidate glosses (scripts/fetch_offline_glosses.py), copied here rather than listed in tauri.macos.conf.json because Tauri fails on a resource path that does not exist and the package must still build without them. The input method reads them beside EngineResources.
glosses="$repo_root/target/macos/offline-glosses"

mkdir -p "$out_dir"
rm -f "$out_dir/SHA256SUMS"
dmgs=()

# 打一个版本：暂存该版本的资源和输入法 bundle，打设置应用，出 DMG 并在挂载后的 DMG 里再检查一遍。下面这些变量对当前版本有效，check_app 也读它们。
package_edition() {
edition="$1"
bundle_name="$(python3 "$edition_tool" field --edition "$edition" bundle_name)"
bundle_id="$(python3 "$edition_tool" field --edition "$edition" input_method_bundle_id)"
display_name="$(python3 "$edition_tool" field --edition "$edition" display_name.zh-Hans)"
dmg_prefix="$(python3 "$edition_tool" field --edition "$edition" dmg_prefix)"
# 非英文离线释义按中文候选查，不提供中文方案的版本（日文、越南文和藏文版）不带：stage-resources.sh 不暂存，check_app 再确认包里没有。
offline_glosses="$(python3 "$edition_tool" field --edition "$edition" features.offline_glosses)"
echo "packaging edition $edition: $bundle_name ($bundle_id)"

# ---- Core dictionaries for this edition ----
# 只暂存核心词库：日文词典那一组文件（词典与两份 Mozc 许可文本）不进 EngineResources，粤拼、注音、笔画词库和手写模型也不再取回，三者都由 App 按需下载。MSIME_EDITION 让不是 full 的版本只带自己资源锁里的文件。
MSIME_EDITION="$edition" MSIME_MACOS_OMIT_ON_DEMAND=1 bash platforms/macos/stage-resources.sh "$resources"

# ---- Input method bundle for this edition ----
mkdir -p target/macos
find target/macos -maxdepth 1 -name '*.app' -exec rm -rf {} +
# ditto keeps the framework symlinks that cp -R and cmake -E copy_directory would flatten; a flattened Sparkle.framework cannot be sealed.
ditto "$built_bundle" "target/macos/$bundle_name"
staged_bundle="$(only target/macos/*.app)"
# 编出来的是 full 的 bundle；不是 full 的版本换上该版本的 Info.plist、InfoPlist.strings 和可执行文件名，签名在这之后。full 什么也不改。
python3 "$edition_tool" apply --edition "$edition" "$staged_bundle"
# --deep, as scripts/install.sh does: Sparkle arrives signed by its publisher, and under the hardened runtime a process cannot load a library whose Team ID differs from its own. The entitlements carry microphone access for voice input.
if [ -n "$identity" ]; then
  codesign_timestamped --force --deep --options runtime --entitlements "$entitlements" --sign "$identity" "$staged_bundle"
else
  codesign --force --deep --options runtime --entitlements "$entitlements" --sign - "$staged_bundle"
fi
codesign --verify --deep --strict "$staged_bundle"

# ---- Settings app for this edition ----
# full 只传版本号，与引入版本之前相同。其他版本另外换掉 identifier 和 productName（决定包的 Info.plist 和 .app 文件名），把资源里 full 的输入法 bundle 和设置应用的 InfoPlist.strings 换成该版本的，再放进版本声明 edition.json。--config 按 JSON merge patch 合并，null 删掉 tauri.macos.conf.json 里的同名条目。
bundle_config="$(python3 "$edition_tool" tauri-config --edition "$edition" --version "$version")"
if [ "$edition" != full ]; then
  edition_stage="$repo_root/target/macos/edition-$edition"
  rm -rf "$edition_stage"
  mkdir -p "$edition_stage"
  python3 "$edition_tool" marker --edition "$edition" --output "$edition_stage/edition.json"
  for locale in zh-Hans en; do
    python3 "$edition_tool" settings-strings --edition "$edition" \
      --input "$repo_root/apps/desktop/src-tauri/macos/$locale.lproj/InfoPlist.strings" \
      --output "$edition_stage/$locale.lproj/InfoPlist.strings"
  done
  bundle_config="$(python3 - "$bundle_config" "$edition" "$bundle_name" <<'PY'
import json, sys
config, edition, bundle_name = json.loads(sys.argv[1]), sys.argv[2], sys.argv[3]
stage = f"../../../target/macos/edition-{edition}"
config["bundle"] = {"resources": {
    "../../../target/macos/水杉输入法.app": None,
    f"../../../target/macos/{bundle_name}": bundle_name,
    "macos/en.lproj/InfoPlist.strings": None,
    "macos/zh-Hans.lproj/InfoPlist.strings": None,
    f"{stage}/en.lproj/InfoPlist.strings": "en.lproj/InfoPlist.strings",
    f"{stage}/zh-Hans.lproj/InfoPlist.strings": "zh-Hans.lproj/InfoPlist.strings",
    f"{stage}/edition.json": "edition.json",
}}
print(json.dumps(config, ensure_ascii=False))
PY
)"
fi
rm -rf "$tauri_bundle_dir"
# No APPLE_SIGNING_IDENTITY: Tauri would sign the nested input method again without its entitlements. The outer app is signed below instead. tauri.macos.conf.json is merged automatically on macOS and is what embeds EngineResources and the input method.
env -u APPLE_SIGNING_IDENTITY -u APPLE_CERTIFICATE \
  pnpm --filter @msime/desktop exec tauri bundle --bundles app --config "$bundle_config"
app="$(only "$tauri_bundle_dir"/*.app)"
app_name="$(basename "$app")"
# Tauri copies resources by following symlinks, which turns Sparkle.framework's links into duplicate files and drops the directory links, and codesign then rejects the nested bundle ("invalid Info.plist (plist or signature have been modified)"). The signed bundle staged above is copied back over it with ditto, which keeps the links; the settings app's installer recreates them in ~/Library/Input Methods the same way.
find "$app/Contents/Resources" -maxdepth 1 -name '*.app' -exec rm -rf {} +
ditto "$staged_bundle" "$app/Contents/Resources/$bundle_name"
# A helper executable beside the app's own is signed on its own first, and the outer signature below seals it.
ditto "$universal_dir/msime-mcp" "$app/Contents/MacOS/msime-mcp"
sign "$app/Contents/MacOS/msime-mcp"
if [ -d "$glosses" ]; then
  ditto "$glosses" "$app/Contents/Resources/offline-glosses"
fi
# Without --deep, so the input method keeps the signature and entitlements it was given above; the outer signature seals it as a nested resource.
sign "$app"
codesign --verify --deep --strict "$app"

# The package is only worth shipping if it holds what the install button and first run read. Checked on the built app and again on the copy inside the mounted DMG.
check_app() {
  local root="$1"
  local resources_dir="$root/Contents/Resources"
  test -d "$resources_dir/EngineResources"
  cargo run --quiet --locked -p msime-client-core --example verify_resources -- --omit-on-demand --edition "$edition" "$resources_dir/EngineResources" >/dev/null
  for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt NOTICE.md NOTICE-jiajia.md; do
    test -f "$resources_dir/EngineResources/helpcodes/$table"
  done
  # 按需下载的资源包不该出现在包里：日文词典、粤拼、注音与笔画词库、手写模型都由 App 下载到 resource-packs/<id>/，不提供手写的版本（日文、越南文和藏文版）连手写模型也不下载（macos_resource_packs.rs）。识别器代码的 Zinnia 许可证仍由 tauri.macos.conf.json 放进包里：Zinnia 的移植编在共用的 host 库里，每个版本都带着这份代码。
  test ! -e "$resources_dir/EngineResources/msime-japanese.dat"
  test ! -e "$resources_dir/EngineResources/msime-mozc_dictionary_oss_README.txt"
  test ! -e "$resources_dir/EngineResources/msime-mozc_LICENSE.txt"
  # SCOWL 的条款要求它的版权与许可声明随 msime-english.db 一起分发；verify_resources 已按锁文件要求它在场，这里再明确查一次，免得有人把它当成可有可无的文本挪走。
  test -f "$resources_dir/EngineResources/msime-scowl_Copyright.txt"
  test ! -e "$resources_dir/language-dictionaries"
  test ! -e "$resources_dir/handwriting/handwriting-zh_CN.model"
  test -f "$resources_dir/handwriting/Zinnia-LICENSE.txt"
  # 核心词库的体积预算（KiB）。按需资源包被误放回 EngineResources，或者核心词库意外变大，都会在这里报出来。dict-v2.0.5 把五笔码表拆进单独的 msime-wubi.db，它与 msime-pinyin.db 合计比 dict-v2.0.2 的 msime.db 大约 5.7 MB，当时核心文件加 helpcodes/ 约 116900 KiB，预算 121000 KiB。dict-v2.0.7 的 msime-english.db 并入 SCOWL 英文词表，从 1626112 字节涨到 4521984 字节（约 +2.8 MB），另加 4 KiB 的 msime-scowl_Copyright.txt；核心文件按锁文件大小逐个向上取整到 4 KiB 合计 119224 KiB，加上 helpcodes/ 的 528 KiB 约 119752 KiB，原预算只剩约 1.2 MB 余量，所以抬到 124000 KiB，保持与此前相同的约 4 MB 余量。dict-v2.0.10 的 msime-wubi.db 并入 86 五笔词组补充表，从 13688832 字节涨到 15720448 字节（约 +2.0 MB），拼音库与二元、三元模型也略有变化，核心文件合计 121208 KiB，加上 helpcodes/ 约 121736 KiB，余量只剩约 2.2 MB，所以再抬到 126000 KiB，仍保持约 4 MB 余量。
  local engine_kib
  engine_kib="$(du -sk "$resources_dir/EngineResources" | cut -f1)"
  test "$engine_kib" -le 126000 || {
    echo "EngineResources is ${engine_kib} KiB, over the 126000 KiB core-dictionary budget: $resources_dir/EngineResources" >&2
    exit 1
  }
  test -f "$resources_dir/Licenses/THIRD_PARTY_NOTICES.txt"
  test -x "$root/Contents/MacOS/msime-mcp"
  codesign --verify --strict "$root/Contents/MacOS/msime-mcp"
  if [ "$offline_glosses" != true ]; then
    test ! -e "$resources_dir/offline-glosses"
  elif [ -d "$glosses" ]; then
    test -f "$resources_dir/offline-glosses/offline-glosses-NOTICE.txt"
  fi
  local nested
  nested="$(only "$resources_dir"/*.app)"
  test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$nested/Contents/Info.plist")" = "$bundle_id"
  # 版本声明：full 的包不带，其他版本的声明必须是本版本。
  if [ "$edition" = full ]; then
    test ! -e "$resources_dir/edition.json"
  else
    python3 -c 'import json, sys; assert json.load(open(sys.argv[1])) == {"edition": sys.argv[2]}' "$resources_dir/edition.json" "$edition"
  fi
  test -L "$nested/Contents/Frameworks/Sparkle.framework/Versions/Current"
  # On-device voice models run in this helper, which loads the sherpa-onnx runtime beside it; the CMake build fetches the runtime from resources/voice-runtime.lock.json and stages both.
  test -x "$nested/Contents/MacOS/msime-voice-local"
  test -s "$nested/Contents/Frameworks/libsherpa-onnx-c-api.dylib"
  codesign -d --entitlements - --xml "$nested" 2>/dev/null | grep -q 'com.apple.security.device.audio-input' || {
    echo "the embedded input method lost its entitlements: $nested" >&2
    exit 1
  }
  codesign --verify --deep --strict "$nested"
  codesign --verify --deep --strict "$root"
  # universal 包里任何一个只含单一架构的 Mach-O，都会让另一种 Mac 上的输入法、设置应用或某个功能起不来，而单一架构的开发机上看不出来。
  local file
  while IFS= read -r -d '' file; do
    if file -b "$file" | grep -q '^Mach-O'; then
      # lipo -verify_arch 一次只接受一种架构（传多个会报 requires exactly one input file），所以逐个核对。
      local architecture
      for architecture in "${architectures[@]}"; do
        lipo "$file" -verify_arch "$architecture" || {
          echo "missing $architecture ($(lipo -archs "$file")): $file" >&2
          exit 1
        }
      done
    fi
  done < <(find "$root" -type f -print0)
}
check_app "$app"

# ---- DMG ----
# The macOS convention for a drag-to-install app: the app beside a link to /Applications.
stage="$work/dmg-$edition"
mkdir -p "$stage"
ditto "$app" "$stage/$app_name"
ln -s /Applications "$stage/Applications"
# Dragging the app is only half the install: the input method appears once the app has been opened and its install window's 立即安装 pressed, and on macOS 27 the user then adds it in System Settings, which the app walks them through. Finder shows the app by its localised name (水杉输入法 for full, apps/desktop/src-tauri/macos/*.lproj/InfoPlist.strings; each edition's own name otherwise), so the instructions call it that rather than by its file name.
printf '%s\n' \
  "$display_name macOS 安装说明" \
  '' \
  "1. 把「${display_name}」拖到「应用程序」文件夹。" \
  "2. 打开「应用程序」里的「${display_name}」，点「立即安装」把输入法安装到本机，再按设置页的提示在「系统设置」→「键盘」→「文字输入」→「输入法」中添加它。" \
  "3. 添加后在菜单栏的输入法菜单中选「${display_name}」，或按 Control+空格 切换。" \
  '' \
  "只把「${display_name}」拖进「应用程序」而不打开它，系统里不会出现这个输入法。" \
  > "$stage/安装说明.txt"
dmg="$out_dir/$dmg_prefix-$version-$arch.dmg"
rm -f "$dmg"
hdiutil create -quiet -volname "$display_name $version" -srcfolder "$stage" -format UDZO -fs HFS+ "$dmg"
if [ -n "$identity" ]; then
  codesign_timestamped --force --sign "$identity" "$dmg"
  codesign --verify --strict "$dmg"
fi

if [ -n "$identity" ] && [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]; then
  xcrun notarytool submit "$dmg" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --wait
  xcrun stapler staple "$dmg"
  spctl -a -t open --context context:primary-signature -v "$dmg"
else
  echo "warning: not notarized; Gatekeeper will quarantine the downloaded app until the user opens it with right-click Open" >&2
fi

mount_point="$work/mount"
mkdir -p "$mount_point"
hdiutil attach -quiet -nobrowse -readonly -mountpoint "$mount_point" "$dmg"
check_app "$(only "$mount_point"/*.app)"
test -L "$mount_point/Applications"
test -f "$mount_point/安装说明.txt"
hdiutil detach -quiet "$mount_point"
mount_point=""

# 体积报告：DMG 本身和 App 的 Contents/Resources。在 GitHub Actions 里同时写进这一步的摘要，方便逐次对比。
# /usr/bin/stat, not whatever is first on PATH: a GNU coreutils stat reads -f as --file-system and fails, after the package is already built.
dmg_bytes="$(/usr/bin/stat -f %z "$dmg")"
resources_kib="$(du -sk "$app/Contents/Resources" | cut -f1)"
echo "DMG size: $dmg_bytes bytes; Contents/Resources: $resources_kib KiB"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  printf '### macOS package size\n\n- %s: %s bytes\n- Contents/Resources: %s KiB\n' "$(basename "$dmg")" "$dmg_bytes" "$resources_kib" >> "$GITHUB_STEP_SUMMARY"
fi
dmgs+=("$dmg")
}

for edition in "${editions[@]}"; do
  package_edition "$edition"
done

(cd "$out_dir" && shasum -a 256 -- "${dmgs[@]##*/}" > SHA256SUMS && shasum -a 256 -c SHA256SUMS)
for dmg in "${dmgs[@]}"; do
  echo "macOS package: $dmg"
done
