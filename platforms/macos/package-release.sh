#!/usr/bin/env bash
# Build the installable macOS release: the Tauri settings app with the pinned core dictionaries (EngineResources), the licence files and the InputMethodKit bundle 水杉输入法.app embedded as resources, packed into a DMG with a SHA256SUMS beside it.
#
# 包里只带装好就能打中文的核心词库（msime-pinyin.db、bigram/trigram、msime-english.db、SCOWL 许可声明 msime-scowl_Copyright.txt、msime-others.db、sentence-model、helpcodes/、msime-dictionary-manifest.json；五笔版另带 msime-wubi.db）。其余资源包由 App 在首次用到时下载到 <state_root>/resource-packs/<id>/（state_root 默认 ~/Library/Application Support/app.msime.macos），查找时下载的优先、包内或旧版本记录的副本次之，两者都没有时对应功能显示为不可用：
#   japanese              msime-japanese.dat 与两份 Mozc 许可文本（mozc_dictionary_oss_README、mozc_LICENSE），用户选日文方案时下载
#   wubi                  五笔码表 msime-wubi.db，用户选五笔方案时下载。五笔是默认方案的版本（五笔版）照旧随包带着它
#   language-dictionaries 粤拼、注音与笔画词库及其许可证（resources/language-dictionaries.lock.json），用户选粤拼、注音或笔画时下载
#   handwriting           手写模型及其许可证（resources/handwriting-model.lock.json），首次打开手写面板时下载
#   voice-runtime-macos   本地语音识别的 sherpa-onnx 运行库（resources/voice-runtime-macos.lock.json），用户选用本地识别时下载。只在锁文件固定了重签的运行库、并且这次用同一个 Team ID 签名时省略；否则运行库照旧放进输入法 bundle 的 Contents/Frameworks
# 打包时在编译之前先把这些资源包按 App 用的同一套安装器、URL 和哈希装一遍，链接失效或内容漂移的资源包不会随发布包出去。
#
# The same steps run locally and in release-macos.yml, so a package that passes here is the package CI publishes.
#
# Usage: platforms/macos/package-release.sh [--editions ID[,ID...]] [VERSION] [OUT_DIR]
#   --editions 是版本表 shared/contracts/editions.json 里的版本 id，缺省只打 full，产物与引入版本之前相同。几个版本共用一次编译：输入法、msime-mcp 和设置应用的可执行文件都只编一遍，每个版本只重新暂存自己的资源、用 platforms/macos/scripts/edition_bundle.py 把输入法 bundle 改成该版本的身份、按该版本的 identifier 和 productName 打一个设置应用，再各出一个 DMG。多个版本可以同时安装，彼此完全隔离。
#   VERSION defaults to platforms/macos/version.txt, the version release-macos.yml tags as macos-vVERSION. It becomes the version the settings app reports, so the in-app update check compares like with like, and it must equal the input method's CFBundleShortVersionString (platforms/macos/Info.plist.in).
#   OUT_DIR 缺省是 target/macos-package/dist，每个版本在里面得到一个 <dmg_prefix>-VERSION-universal.dmg（full 是 msime-macos-VERSION-universal.dmg），另有只列这些 DMG 的 SHA256SUMS：版本发布页上只放它们。
#   发布的可执行文件在剥离符号之前先生成 dSYM，放在 target/macos-package/dsym，每次运行开头清空；全部版本打完后再打成 OUT_DIR 里的 msime-macos-VERSION-dsym.zip，release-macos.yml 把它传到滚动的 macos-symbols release，不放进版本发布页。各版本带的是同样的四个可执行文件，一份覆盖全部版本。
#
# Environment:
#   MSIME_SPARKLE_ROOT            required; directory containing the pinned Sparkle.framework (see README.md)
#   MSIME_REQUIRE_OFFICIAL_PLUGINS 1 refuses to package without the official plugin packs in target/plugin-packs (scripts/build-official-plugins.sh builds them; release-macos.yml sets it); otherwise the bundle carries them when they are there
#   MSIME_REQUIRE_WORDBOOKS       1 时 target/wordbooks 里没有锁文件 resources/wordbook.lock.json 列出的全部内置词书和 wordbooks-NOTICE.txt 就不打包（python3 scripts/fetch_wordbooks.py 生成它们；release-macos.yml 和 ci-macos-package.yml 设它）；不设时有就带上，没有时背单词只列用户导入的词表
#   CARGO_TARGET_DIR              defaults to target/ in the repository
#   MSIME_MACOS_BUILD_DIR         CMake build tree; defaults to target/macos-release
#   MACOS_SIGNING_IDENTITY        a "Developer ID Application: ... (TEAMID)" identity in the keychain. Without it everything is signed ad-hoc: the package builds and the settings app runs, but macOS will not register the embedded input method as an input source (see scripts/install.sh)
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_SPECIFIC_PASSWORD
#                                 when all three are set (and an identity is), the DMG is notarized and stapled
#   MSIME_SPARKLE_PRIVATE_KEY     与 platforms/macos/sparkle-public-key.txt 对应的 base64 EdDSA 私钥（generate_keys -x 导出的 32 字节种子或旧版 96 字节格式）。同时具备私钥和签名身份时，每个发行版本生成输入法与完整设置应用的签名归档和独立 appcast；缺少时不配置更新通道，保留下载页入口
#   MSIME_SPARKLE_APPCAST_BASE_URL    where the appcast-<edition>.xml files are served; defaults to the macos-appcast release
#   MSIME_SPARKLE_DOWNLOAD_URL_PREFIX 更新包的下载地址前缀，缺省也是 macos-appcast release：更新包和 appcast 放在一起，不进版本发布页（见 platforms/macos/scripts/publish-sparkle-feed.sh）
#   MSIME_BUILD_NUMBER 发布工作流传入的唯一构建号；未设置时使用 platforms/macos/build-number.txt
#
# 产物是 universal 的：Apple 芯片和 Intel Mac 用同一个包。Rust 产物按两个 target 各编一次再用 lipo 合并，CMake 的目标和 Swift 后端经 CMAKE_OSX_ARCHITECTURES 编出双架构，打包后 check_app 逐个核对包里的 Mach-O 都含两种架构。需要 rustup target add aarch64-apple-darwin x86_64-apple-darwin。
set -Eeuo pipefail

# ERR 传入函数和子 shell，只记录行号、版本 id 和退出码，不打印可能带公证凭据的命令。
trap 'package_status=$?; printf "macOS 打包失败：行号=%s，版本=%s，退出码=%s\n" "$LINENO" "${edition:-shared}" "$package_status" >&2; exit "$package_status"' ERR

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
# Sparkle 分别更新 ~/Library/Input Methods 里的输入法和完整设置应用，各自使用签名归档与 appcast。新 bundle 必须与已安装副本出自同一签名身份，因此没有 Developer ID 时不开 feed。
sparkle_key="${MSIME_SPARKLE_PRIVATE_KEY:-}"
sparkle_appcast_base=""
if [ -n "$sparkle_key" ] && [ -z "$identity" ]; then
  echo "warning: MSIME_SPARKLE_PRIVATE_KEY is set but MACOS_SIGNING_IDENTITY is not; Sparkle cannot update an ad-hoc signed input method, so the package has no feed" >&2
  sparkle_key=""
fi
if [ -n "$sparkle_key" ]; then
  sparkle_appcast_base="${MSIME_SPARKLE_APPCAST_BASE_URL:-https://github.com/metasequoiaime/msime/releases/download/macos-appcast/}"
  sparkle_download_prefix="${MSIME_SPARKLE_DOWNLOAD_URL_PREFIX:-https://github.com/metasequoiaime/msime/releases/download/macos-appcast/}"
  # 私钥与仓库里的公钥对不上时，发出去的每个更新包都会被 Sparkle 当作签名无效拒绝，而发版本身照常成功；所以在编译之前从私钥推出公钥比一遍。macOS 自带的 LibreSSL 不支持 Ed25519，用 Homebrew 的 OpenSSL 3。
  # Sparkle 认两种私钥（common_cli/Secret.swift）：generate_keys 现在导出的 32 字节种子，公钥由它推出；以前的 96 字节格式是 64 字节私钥接 32 字节公钥。
  case "$(printf '%s' "$sparkle_key" | /usr/bin/base64 -D | wc -c | tr -d '[:space:]')" in
    32)
      openssl3="$(brew --prefix openssl@3)/bin/openssl"
      derived_public_key="$(printf '%s' "$sparkle_key" | /usr/bin/base64 -D | { printf '\x30\x2e\x02\x01\x00\x30\x05\x06\x03\x2b\x65\x70\x04\x22\x04\x20'; cat; } \
        | "$openssl3" pkey -inform DER -pubout -outform DER | tail -c 32 | /usr/bin/base64)" ;;
    96) derived_public_key="$(printf '%s' "$sparkle_key" | /usr/bin/base64 -D | tail -c 32 | /usr/bin/base64)" ;;
    *) echo "MSIME_SPARKLE_PRIVATE_KEY is neither a 32-byte EdDSA seed nor Sparkle's older 96-byte key" >&2; exit 1 ;;
  esac
  if [ "$derived_public_key" != "$(tr -d '[:space:]' < platforms/macos/sparkle-public-key.txt)" ]; then
    echo "MSIME_SPARKLE_PRIVATE_KEY does not belong to the public key in platforms/macos/sparkle-public-key.txt" >&2
    exit 1
  fi
fi
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

# 用法：cargo_universal 输出目录 产物名... -- cargo build 参数...。按 rust_targets 各编一次（`--` 之后的参数原样传给 cargo build），再把每个 target 的各个同名产物用 lipo 合并到输出目录。一次调用编几个包时，它们共用的依赖按同一组 feature 只编一遍。
cargo_universal() {
  local output_dir="$1" target name names=() slices
  shift
  while [ "$1" != -- ]; do
    names+=("$1")
    shift
  done
  shift
  for target in "${rust_targets[@]}"; do
    cargo build --release --locked --target "$target" "$@"
  done
  mkdir -p "$output_dir"
  for name in "${names[@]}"; do
    slices=()
    for target in "${rust_targets[@]}"; do
      slices+=("$CARGO_TARGET_DIR/$target/release/$name")
    done
    lipo -create "${slices[@]}" -output "$output_dir/$name"
  done
}
universal_dir="$CARGO_TARGET_DIR/universal/release"

if [ -z "$identity" ]; then
  echo "warning: MACOS_SIGNING_IDENTITY is not set; signing ad-hoc. The settings app will run, but macOS will not register the embedded input method as an input source until it is re-signed with a Developer ID (platforms/macos/scripts/install.sh)." >&2
fi
# 有签名身份和三项公证凭据时公证并装订每个 DMG。
notarize=""
if [ -n "$identity" ] && [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]; then
  notarize=1
fi

# 公证分两步：每个版本的 DMG 一做好就提交（notary_submit，不等结果），接着打下一个版本；全部版本提交之后再逐个取结果（notary_result）。Apple 处理一份要一到两分钟，以前 `submit --wait` 让六个版本依次各等一遍。两个函数都不回显命令行，免得把公证密码打进日志。
notary_ids=()
# 从 notarytool 的 --output-format json 输出里取一个字段。整段不是 JSON 时（前面夹着别的提示行）取最后一行 JSON。
notary_field() {
  python3 -c '
import json, sys
text = sys.stdin.read()
try:
    data = json.loads(text)
except ValueError:
    data = json.loads([line for line in text.splitlines() if line.lstrip().startswith("{")][-1])
print(data[sys.argv[1]])
' "$1"
}
notary_submit() {
  local output id
  output="$(xcrun notarytool submit "$1" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --output-format json)"
  id="$(notary_field id <<< "$output")"
  echo "notarization submitted: $(basename "$1") id=$id"
  notary_ids+=("$id")
}
# 打印一个提交的最终状态（Accepted、Invalid 或 Rejected）。`notarytool wait` 因网络中断提前返回时还拿不到最终状态，这时隔 15 秒再等，最多五轮；状态以 `notarytool info` 为准，不看 wait 的退出码。
notary_result() {
  local id="$1" attempt status=""
  for attempt in 1 2 3 4 5; do
    xcrun notarytool wait "$id" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" >&2 || true
    status="$(xcrun notarytool info "$id" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --output-format json \
      | notary_field status)" || status=""
    case "$status" in
      Accepted | Invalid | Rejected) break ;;
    esac
    echo "notarization $id has no final status yet (${status:-unknown}); waiting again ($attempt/5)" >&2
    sleep 15
  done
  printf '%s\n' "${status:-unknown}"
}

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

# 发布的 Mach-O 在签名之前剥掉局部符号：几个 universal 可执行文件在磁盘上合计少约 45 MB，每个 DMG 也小几 MB。`strip -x` 保留全局符号，所以 dylib 的导出（MSIMEBackend 的 `_MSIME*` 入口、sherpa-onnx 的 C API）和 dyld 要绑定的导入都还在。可执行文件先写出 dSYM，崩溃报告仍能符号化；dylib 和以前一样不生成 dSYM。
dsym_dir="$repo_root/target/macos-package/dsym"
rm -rf "$dsym_dir"
mkdir -p "$dsym_dir"
strip_executable() {
  dsymutil "$1" -o "$dsym_dir/$(basename "$1").dSYM"
  strip -x "$1"
}

# The first element of a glob, failing when there is none or more than one. The bundle names are Chinese or contain spaces, and a literal path to a Chinese name can miss on APFS because of NFC/NFD normalisation; matching with a glob sidesteps both.
only() {
  if [ "$#" -ne 1 ] || [ ! -e "$1" ]; then
    echo "expected exactly one match, found: $*" >&2
    exit 1
  fi
  printf '%s\n' "$1"
}

# 磁盘映像服务偶发失败时重新创建；每次都清掉半成品，持续失败仍阻止上传。
create_dmg() {
  local output="$1" attempt status
  shift
  for attempt in 1 2 3; do
    rm -f "$output"
    if hdiutil create "$@" "$output"; then
      return 0
    else
      status=$?
    fi
    printf 'DMG creation failed: edition=%s attempt=%s status=%s\n' "$edition" "$attempt" "$status" >&2
    if [ "$attempt" -lt 3 ]; then sleep "$((attempt * 5))"; fi
  done
  rm -f "$output"
  return "$status"
}

# ---- Core dictionaries ----
# install_resources prints progress on stderr and the verified directory as its last stdout line. Each edition's stage-resources.sh run (below) re-verifies it and stages target/macos/EngineResources, which tauri.macos.conf.json embeds.
resources="$(cargo run --quiet --locked -p msime-client-core --example install_resources -- "$work/desktop-resources" | tail -n 1)"

# ---- Speech runtime ----
# 输入法和 msime-voice-local 开着 hardened runtime，库验证只加载同一 Team ID 签名的库。resources/voice-runtime-macos.lock.json 固定了用本项目 Developer ID 重签的运行库、并且这次也用这个 Team ID 签名时，运行库不进包，由设置应用作为 voice-runtime-macos 资源包下载；锁文件还没固定（重签的运行库还没发布）、没有签名身份（ad-hoc）或身份属于别的团队时照旧内置，否则装出来的输入法加载不了下载的库。
voice_runtime_team="$(python3 -I -c 'import json, sys; lock = json.load(open(sys.argv[1])); print(lock["team_id"] if lock["archive"] else "")' "$repo_root/resources/voice-runtime-macos.lock.json")"
omit_voice_runtime=0
if [ -n "$voice_runtime_team" ]; then
  case "$identity" in
    *"($voice_runtime_team)") omit_voice_runtime=1 ;;
    *) echo "warning: the downloadable speech runtime is signed by Team $voice_runtime_team, which this package is not; bundling the runtime instead" >&2 ;;
  esac
fi

# ---- On-demand resource packs ----
# 编译之前先用 App 运行时的同一个安装器、同一组 URL 和 SHA-256 把 macOS 会下载的资源包（日文词典、语言词库、手写模型、桌面神经联想模型，省略运行库时还有语音运行库）装一遍，失败就在这里停下：资源包不在包里，发布出去的 App 只能靠这些地址补齐，地址失效或内容漂移的发布包不该出去。离线释义和 Android 的语音运行库只给 Android 按需下载，不在这里装。不接管道，安装器的退出码就是这一步的结果。
on_demand_packs=(japanese wubi language-dictionaries handwriting settled-model)
if [ "$omit_voice_runtime" = 1 ]; then
  on_demand_packs+=(voice-runtime-macos)
fi
cargo run --quiet --locked -p msime-client-providers --example install_resource_pack -- "$work/pack-check" "${on_demand_packs[@]}" >/dev/null
if [ "$omit_voice_runtime" = 1 ]; then
  # 下载到的运行库必须是这个团队签的、两种架构都在：否则省略之后的输入法在任何一台 Mac 上都加载不了它。
  downloaded_runtime="$work/pack-check/resource-packs/voice-runtime-macos/libsherpa-onnx-c-api.dylib"
  codesign --verify --strict "$downloaded_runtime"
  test "$(codesign -dv "$downloaded_runtime" 2>&1 | sed -n 's/^TeamIdentifier=//p')" = "$voice_runtime_team"
  for architecture in "${architectures[@]}"; do
    lipo "$downloaded_runtime" -verify_arch "$architecture"
  done
fi

# ---- Input method bundle ----
# The minimum system version goes to the C/C++ compilers and to CMake separately, never as MACOSX_DEPLOYMENT_TARGET: rustc applies that to host proc-macro dylibs too, which then fail to load (README.md, 构建与本地测试).
# host 库和 msime-mcp 在同一次 cargo 调用里编：分开编时两次的 feature 集合不同（msime-mcp 多开了 tokio 的 io-std 和 macros、futures-util 的 async-await 和 channel、tracing 的 attributes 等），engine、client-core、host-api 和半数第三方依赖每个 target 要编两遍。合在一起后 msime-mcp 的 feature 集合与单独编时完全相同，host 库只多出这几项纯增量的 API，行为不变。设置应用（msime-desktop）不并进来：它会给 objc2 打开 exception、给 serde_json 打开 unbounded_depth，这些会改变 host 库和 msime-mcp 的行为。
# The same compiler flags for both: msime-mcp links the Engine through msime-host-api. Nothing in the app starts msime-mcp; an agent's MCP configuration runs Contents/MacOS/msime-mcp over stdio. MSIME_VERSION 是 msime-mcp --version 和 MCP 握手报告的版本（crates/mcp-server/build.rs），与这个包的版本相同；host-api 不读它。
MSIME_VERSION="$version" CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" \
  cargo_universal "$universal_dir" libmsime_host_api.a msime-mcp -- -p msime-host-api -p msime-mcp-server
# MSIME_HOST_LIBRARY is explicit: the CMake default is target/debug, which would link the debug Rust library into a Release bundle.
cmake -S platforms/macos -B "$build_dir" -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$(brew --prefix)" \
  -DCMAKE_OSX_ARCHITECTURES="$(IFS=';'; echo "${architectures[*]}")" \
  -DMSIME_SPARKLE_ROOT="$MSIME_SPARKLE_ROOT" -DMSIME_HOST_LIBRARY="$universal_dir/libmsime_host_api.a" \
  -DMSIME_BUILD_NUMBER="${MSIME_BUILD_NUMBER:-}" -DMSIME_INPUT_SOURCE_REVISION="$(git rev-parse HEAD)" \
  -DMSIME_SPARKLE_APPCAST_BASE_URL="$sparkle_appcast_base" \
  -DMSIME_BUNDLE_VOICE_RUNTIME="$([ "$omit_voice_runtime" = 1 ] && echo OFF || echo ON)" \
  -DMSIME_REQUIRE_OFFICIAL_PLUGINS="$([ "${MSIME_REQUIRE_OFFICIAL_PLUGINS:-}" = 1 ] && echo ON || echo OFF)"
# An explicit job count: a bare --parallel with the Makefile generator starts every compile at once and runs a 7 GB runner out of memory (ci-macos.yml).
# 只编包里要的目标：输入法 bundle（连同它依赖的 Swift 后端、msime-voice-local 和语音运行库）和下面 ctest 要跑的 backend-library-load-test。CMakeLists.txt 里另有上百个测试可执行文件和 Swift 测试，发布包一个都不用，全编要多花约 10 分钟；CI 和本地开发照常不带 --target 全编、跑完整 ctest。
cmake --build "$build_dir" --config Release --parallel "$(sysctl -n hw.logicalcpu)" --target MSIMEClientInputMethod backend-library-load-test
# `backend-library-load` 加载 bundle 里的 MSIMEBackend.dylib（Release 构建会对它做优化、dead strip 和剥离符号，见 scripts/build_backend_swift.sh），确认输入法按名字查找的 Objective-C 类和 selector 都还在。
ctest --test-dir "$build_dir" --no-tests=error --output-on-failure -R '^(bundle-contents|backend-library-load)$'

built_bundle="$(only "$build_dir"/*.app)"
imk_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$built_bundle/Contents/Info.plist")"
if [ "$imk_version" != "$version" ]; then
  echo "the input method reports $imk_version but the package is $version; the bundle takes its version from platforms/macos/version.txt, so pass the release version as VERSION or bump version.txt" >&2
  exit 1
fi
# 构建目录里的 bundle 保持不剥离，重跑时即使什么都没重新链接，`dsymutil` 也还读得到符号；打包用的是剥离过的副本。Sparkle.framework 保持发布方签名时的原样。
mkdir -p "$work/input-method"
ditto "$built_bundle" "$work/input-method/$(basename "$built_bundle")"
built_bundle="$(only "$work/input-method"/*.app)"
strip_executable "$built_bundle/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$built_bundle/Contents/Info.plist")"
strip_executable "$built_bundle/Contents/MacOS/msime-voice-local"
# dylib 只用 `-x`：完全剥离会删掉使用方要绑定的导出符号。Release 构建已经剥离过 MSIMEBackend.dylib，除非构建目录配置成了别的配置，这一步对它不起作用。下载的语音运行库在重签之前已经同样剥离过（scripts/sign-voice-runtime.sh）。
strip -x "$built_bundle/Contents/Frameworks/MSIMEBackend.dylib"
if [ "$omit_voice_runtime" != 1 ]; then
  strip -x "$built_bundle/Contents/Frameworks/libsherpa-onnx-c-api.dylib"
fi

# ---- MCP server ----
# 已随 host 库编好（见上面的 cargo_universal）。
strip_executable "$universal_dir/msime-mcp"

# 背单词的内置词书：要求带上时在编译之前就查，缺了不白编一遍。stage-resources.sh 把它们暂存到 target/macos/wordbooks，下面拷进设置应用的 Contents/Resources/wordbooks，即 EngineResources 的同级，设置应用按 resource_dir 读这里。
wordbook_ids="$(python3 -c 'import json, sys; print(" ".join(book["id"] for book in json.load(open(sys.argv[1], encoding="utf-8"))["books"]))' resources/wordbook.lock.json)"
if [ "${MSIME_REQUIRE_WORDBOOKS:-}" = 1 ]; then
  for id in $wordbook_ids; do
    test -f "target/wordbooks/$id.json" || { echo "MSIME_REQUIRE_WORDBOOKS is set but target/wordbooks/$id.json is missing; run python3 scripts/fetch_wordbooks.py first" >&2; exit 1; }
  done
  test -f target/wordbooks/wordbooks-NOTICE.txt || { echo "MSIME_REQUIRE_WORDBOOKS is set but target/wordbooks/wordbooks-NOTICE.txt is missing; run python3 scripts/fetch_wordbooks.py first" >&2; exit 1; }
fi

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
    cargo_universal "$CARGO_TARGET_DIR/release" msime-desktop -- -p msime-desktop --bin msime-desktop --features tauri/custom-protocol
)
strip_executable "$CARGO_TARGET_DIR/release/msime-desktop"
tauri_bundle_dir="$CARGO_TARGET_DIR/release/bundle/macos"
# Non-English candidate glosses (scripts/fetch_offline_glosses.py), copied here rather than listed in tauri.macos.conf.json because Tauri fails on a resource path that does not exist and the package must still build without them. The input method reads them beside EngineResources.
glosses="$repo_root/target/macos/offline-glosses"

mkdir -p "$out_dir"
rm -f "$out_dir/SHA256SUMS" "$out_dir/sparkle-SHA256SUMS" "$out_dir"/appcast-*.xml "$out_dir"/*-input-method.zip "$out_dir"/*-settings.zip
dmgs=()
# 更新 zip 和 appcast 都挂在滚动的 macos-appcast 上，不进版本发布页；zip 的摘要写进单独的 sparkle-SHA256SUMS，随它们一起上传。
update_zips=()

# 同一种签名与校验流程分别生成输入法和完整设置应用的更新归档。
create_sparkle_update() {
  local bundle="$1" archive_name="$2" feed_name="$3"
  local update_zip="$out_dir/$archive_name" appcast_dir="$work/$feed_name"
  local appcast="$out_dir/$feed_name" feed build_version
  rm -f "$update_zip"
  ditto -c -k --sequesterRsrc --keepParent "$bundle" "$update_zip"
  rm -rf "$appcast_dir"
  mkdir -p "$appcast_dir"
  cp "$update_zip" "$appcast_dir/"
  printf '%s' "$sparkle_key" | "$MSIME_SPARKLE_ROOT/bin/generate_appcast" --ed-key-file - \
    --download-url-prefix "$sparkle_download_prefix" -o "$appcast" "$appcast_dir"
  feed="$(/usr/libexec/PlistBuddy -c 'Print :SUFeedURL' "$bundle/Contents/Info.plist")"
  [ "$feed" = "${sparkle_appcast_base}$feed_name" ] || { echo "unexpected Sparkle feed for $edition" >&2; exit 1; }
  build_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$bundle/Contents/Info.plist")"
  grep -qF "url=\"$sparkle_download_prefix$(basename "$update_zip")\"" "$appcast" || { echo "$appcast points at a different archive" >&2; exit 1; }
  grep -qF "<sparkle:version>$build_version</sparkle:version>" "$appcast" || { echo "$appcast offers a different build" >&2; exit 1; }
  grep -qF 'sparkle:edSignature=' "$appcast" || { echo "$appcast has no archive signature" >&2; exit 1; }
  grep -qF 'sparkle-signatures:' "$appcast" || { echo "$appcast has no feed signature" >&2; exit 1; }
  update_zips+=("$update_zip")
}

# 一个版本的身份与内容开关。package_edition 和 finish_edition 都先调用它，check_app 读的版本变量都在这里设，两处不会各算一套。
load_edition() {
  edition="$1"
  bundle_name="$(python3 "$edition_tool" field --edition "$edition" bundle_name)"
  bundle_id="$(python3 "$edition_tool" field --edition "$edition" input_method_bundle_id)"
  display_name="$(python3 "$edition_tool" field --edition "$edition" display_name.zh-Hans)"
  dmg_prefix="$(python3 "$edition_tool" field --edition "$edition" dmg_prefix)"
  # 非英文离线释义按中文候选查，不提供中文方案的版本（日文、越南文和藏文版）不带：stage-resources.sh 不暂存，check_app 再确认包里没有。
  offline_glosses="$(python3 "$edition_tool" field --edition "$edition" features.offline_glosses)"
  wubi_bundled="$(python3 -I -c 'import json, sys; print(int(next(edition["default_scheme"] for edition in json.load(open(sys.argv[1]))["editions"] if edition["id"] == sys.argv[2]) == "wubi"))' "$repo_root/shared/contracts/editions.json" "$edition")"
}

# 打一个版本：暂存该版本的资源和输入法 bundle，打设置应用，出 DMG、签名并提交公证；挂载 DMG 的最终检查在全部版本装订之后（finish_edition）。下面这些变量对当前版本有效，check_app 也读它们。
package_edition() {
load_edition "$1"
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
# 英文释义表也随独立的 InputMethodKit bundle 安装。旧配置可能仍使用较早的 EngineResources，
# InputController 会把这个 bundle 的 Resources 作为补充词库只读回退。
for table in character-glosses word-glosses; do
  source="$repo_root/target/macos/$table"
  if [ -d "$source" ]; then
    ditto "$source" "$staged_bundle/Contents/Resources/$table"
  fi
done
# --deep, as scripts/install.sh does: Sparkle arrives signed by its publisher, and under the hardened runtime a process cannot load a library whose Team ID differs from its own. The entitlements carry microphone access for voice input.
if [ -n "$identity" ]; then
  codesign_timestamped --force --deep --options runtime --entitlements "$entitlements" --sign "$identity" "$staged_bundle"
else
  codesign --force --deep --options runtime --entitlements "$entitlements" --sign - "$staged_bundle"
fi
codesign --verify --deep --strict "$staged_bundle"

# ---- Sparkle 输入法更新 ----
if [ -n "$sparkle_key" ]; then
  create_sparkle_update "$staged_bundle" "$dmg_prefix-$version-input-method.zip" "appcast-$edition.xml"
fi

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
# No APPLE_SIGNING_IDENTITY: Tauri would sign the nested input method again without its entitlements. The outer app is signed below instead. tauri.macos.conf.json is merged automatically on macOS and is what embeds EngineResources and the input method. MSIME_PLATFORM_BUILD=1 passes apps/desktop/scripts/require-platform-build.mjs in case the Tauri CLI runs beforeBuildCommand here.
env -u APPLE_SIGNING_IDENTITY -u APPLE_CERTIFICATE MSIME_PLATFORM_BUILD=1 \
  pnpm --filter @msime/desktop exec tauri bundle --bundles app --config "$bundle_config"
app="$(only "$tauri_bundle_dir"/*.app)"
app_name="$(basename "$app")"
# Tauri copies resources by following symlinks, which turns Sparkle.framework's links into duplicate files and drops the directory links, and codesign then rejects the nested bundle ("invalid Info.plist (plist or signature have been modified)"). The signed bundle staged above is copied back over it with ditto, which keeps the links; the settings app's installer recreates them in ~/Library/Input Methods the same way.
find "$app/Contents/Resources" -maxdepth 1 -name '*.app' -exec rm -rf {} +
ditto "$staged_bundle" "$app/Contents/Resources/$bundle_name"
# 完整应用沿用输入法的构建号、公钥与框架，但读取独立的设置应用 feed。
python3 platforms/macos/scripts/settings_updater.py --app "$app" --input-method "$staged_bundle" --edition "$edition"
# A helper executable beside the app's own is signed on its own first, and the outer signature below seals it.
ditto "$universal_dir/msime-mcp" "$app/Contents/MacOS/msime-mcp"
sign "$app/Contents/MacOS/msime-mcp"
if [ -d "$glosses" ]; then
  ditto "$glosses" "$app/Contents/Resources/offline-glosses"
fi
# The English pronunciation table (scripts/build_pronunciations.py), optional and read beside EngineResources in the same way.
pronunciations="$repo_root/target/macos/pronunciations"
if [ -d "$pronunciations" ]; then
  ditto "$pronunciations" "$app/Contents/Resources/pronunciations"
fi
# Single-character English glosses (scripts/build_character_glosses.py), optional and read beside EngineResources.
characters="$repo_root/target/macos/character-glosses"
if [ -d "$characters" ]; then
  ditto "$characters" "$app/Contents/Resources/character-glosses"
fi
# English glosses for words english.db does not cover (scripts/build_word_glosses.py), optional and read beside EngineResources.
words="$repo_root/target/macos/word-glosses"
if [ -d "$words" ]; then
  ditto "$words" "$app/Contents/Resources/word-glosses"
fi
# 背单词的内置词书（scripts/fetch_wordbooks.py），设置应用在 EngineResources 的同级读 wordbooks/。
wordbooks="$repo_root/target/macos/wordbooks"
if [ -d "$wordbooks" ]; then
  ditto "$wordbooks" "$app/Contents/Resources/wordbooks"
fi
# Without --deep, so the input method keeps the signature and entitlements it was given above; the outer signature seals it as a nested resource.
sign "$app"
codesign --verify --deep --strict "$app"

# The package is only worth shipping if it holds what the install button and first run read. Checked on the built app and again on the copy inside the mounted DMG.
check_app() {
  local root="$1"
  local resources_dir="$root/Contents/Resources"
  if [ -n "$sparkle_key" ]; then
    # 两份应用必须保持同一个单调构建号，设置应用只读取自己的已签名通道。
    test "$(/usr/libexec/PlistBuddy -c 'Print :SUFeedURL' "$root/Contents/Info.plist")" = "${sparkle_appcast_base}appcast-settings-$edition.xml"
    test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$root/Contents/Info.plist")" = "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$staged_bundle/Contents/Info.plist")"
    test "$(/usr/libexec/PlistBuddy -c 'Print :SUPublicEDKey' "$root/Contents/Info.plist")" = "$(/usr/libexec/PlistBuddy -c 'Print :SUPublicEDKey' "$staged_bundle/Contents/Info.plist")"
    test -d "$root/Contents/Frameworks/Sparkle.framework"
  fi
  test -d "$resources_dir/EngineResources"
  cargo run --quiet --locked -p msime-client-core --example verify_resources -- --omit-on-demand --platform macos --edition "$edition" "$resources_dir/EngineResources" >/dev/null
  for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt wubi86_helpcode.txt NOTICE.md NOTICE-jiajia.md NOTICE-wubi86.md; do
    test -f "$resources_dir/EngineResources/helpcodes/$table"
  done
  # 按需下载的资源包不该出现在包里：日文词典、粤拼、注音与笔画词库、手写模型都由 App 下载到 resource-packs/<id>/，不提供手写的版本（日文、越南文和藏文版）连手写模型也不下载（desktop_resource_packs.rs）。识别器代码的 Zinnia 许可证仍由 tauri.macos.conf.json 放进包里：Zinnia 的移植编在共用的 host 库里，每个版本都带着这份代码。
  test ! -e "$resources_dir/EngineResources/msime-japanese.dat"
  test ! -e "$resources_dir/EngineResources/msime-mozc_dictionary_oss_README.txt"
  test ! -e "$resources_dir/EngineResources/msime-mozc_LICENSE.txt"
  # 五笔码表只随五笔是默认方案的版本（五笔版）发布，其余版本由 App 在用户选五笔时下载到 resource-packs/wubi。
  if [ "$wubi_bundled" = 1 ]; then
    test -f "$resources_dir/EngineResources/msime-wubi.db"
  else
    test ! -e "$resources_dir/EngineResources/msime-wubi.db"
  fi
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
  if [ -d "$pronunciations" ]; then
    test -f "$resources_dir/pronunciations/pronunciations-NOTICE.txt"
  fi
  if [ -d "$characters" ]; then
    test -f "$resources_dir/character-glosses/character-glosses-NOTICE.txt"
  fi
  if [ "${MSIME_REQUIRE_WORDBOOKS:-}" = 1 ] || [ -d "$wordbooks" ]; then
    for id in $wordbook_ids; do
      test -f "$resources_dir/wordbooks/$id.json"
    done
    test -f "$resources_dir/wordbooks/wordbooks-NOTICE.txt"
  fi
  if [ -d "$words" ]; then
    test -f "$resources_dir/word-glosses/word-glosses-NOTICE.txt"
  fi
  local nested
  nested="$(only "$resources_dir"/*.app)"
  if [ -d "$characters" ]; then
    test -f "$nested/Contents/Resources/character-glosses/character-glosses-NOTICE.txt"
  fi
  if [ -d "$words" ]; then
    test -f "$nested/Contents/Resources/word-glosses/word-glosses-NOTICE.txt"
  fi
  test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$nested/Contents/Info.plist")" = "$bundle_id"
  # 版本声明：full 的包不带，其他版本的声明必须是本版本。
  if [ "$edition" = full ]; then
    test ! -e "$resources_dir/edition.json"
  else
    python3 -c 'import json, sys; assert json.load(open(sys.argv[1])) == {"edition": sys.argv[2]}' "$resources_dir/edition.json" "$edition"
  fi
  test -L "$nested/Contents/Frameworks/Sparkle.framework/Versions/Current"
  # On-device voice models run in this helper, which loads the sherpa-onnx runtime beside it; the CMake build fetches the runtime from resources/voice-runtime.lock.json and stages both. 省略运行库的包里不能有它：输入法把下载的 voice-runtime-macos 资源包交给辅助进程。
  test -x "$nested/Contents/MacOS/msime-voice-local"
  if [ "$omit_voice_runtime" = 1 ]; then
    test ! -e "$nested/Contents/Frameworks/libsherpa-onnx-c-api.dylib"
  else
    test -s "$nested/Contents/Frameworks/libsherpa-onnx-c-api.dylib"
  fi
  grep -q 'com.apple.security.device.audio-input' <<< "$(codesign -d --entitlements - --xml "$nested" 2>/dev/null)" || {
    echo "the embedded input method lost its entitlements: $nested" >&2
    exit 1
  }
  codesign --verify --deep --strict "$nested"
  codesign --verify --deep --strict "$root"
  # 每个发布的可执行文件都在签名之前剥离（`strip_executable`），`strip -x` 之后任何架构里都不应再有局部符号（nm 类型 t、d、b、s）。不能拿调试映射（OSO stabs）来判断：Cargo 的 release profile 本来就去掉了 msime-mcp 和设置应用的调试信息，但没人剥离时它们仍带着几万个局部符号。
  local executable input_method_executable architecture local_symbols
  input_method_executable="$nested/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$nested/Contents/Info.plist")"
  for executable in "$root/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$root/Contents/Info.plist")" "$root/Contents/MacOS/msime-mcp" "$input_method_executable" "$nested/Contents/MacOS/msime-voice-local"; do
    for architecture in "${architectures[@]}"; do
      local_symbols="$(nm -a -arch "$architecture" "$executable" | awk '$2 ~ /^[tdbs]$/' | wc -l | tr -d ' ')"
      test "$local_symbols" -eq 0 || {
        echo "not stripped: $executable ($architecture) has $local_symbols local symbols" >&2
        exit 1
      }
    done
  done
  # 输入法从 MSIMEBackend.dylib 绑定的 `_MSIME*` 入口，在每种架构里都必须经过 `-Osize`、dead strip 和 `strip -x` 后依然存在。只有 `_MSIMEFetchOnDeviceCandidateGlosses` 允许缺失：Swift 6.2 之前的工具链不编它，CMakeLists.txt 也允许输入法在没有它时链接。
  local backend_architecture backend_exports backend_import backend_imports
  for backend_architecture in "${architectures[@]}"; do
    backend_exports="$(nm -gU -arch "$backend_architecture" "$nested/Contents/Frameworks/MSIMEBackend.dylib" | awk '{print $NF}')"
    backend_imports="$(nm -u -arch "$backend_architecture" "$input_method_executable" | grep '^_MSIME' | grep -vx '_MSIMEFetchOnDeviceCandidateGlosses' || true)"
    test -n "$backend_imports" || {
      echo "the input method imports no _MSIME* symbols from MSIMEBackend.dylib ($backend_architecture): $input_method_executable" >&2
      exit 1
    }
    while IFS= read -r backend_import; do
      grep -qx -- "$backend_import" <<< "$backend_exports" || {
        echo "MSIMEBackend.dylib ($backend_architecture) does not export $backend_import, which the input method binds" >&2
        exit 1
      }
    done <<< "$backend_imports"
  done
  # universal 包里任何一个只含单一架构的 Mach-O，都会让另一种 Mac 上的输入法、设置应用或某个功能起不来，而单一架构的开发机上看不出来。
  local file
  while IFS= read -r -d '' file; do
    if grep -q '^Mach-O' <<< "$(file -b "$file")"; then
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
# Dragging the app is only half the install: the input method appears once the app has been opened and its install window's 立即安装 pressed, and on macOS 27 the user then adds it in System Settings, which the app walks them through. On a first install the add dialog only lists the input method after the next login (platforms/macos/README.md), so the instructions say so for a user who goes straight to System Settings. Finder shows the app by its localised name (水杉输入法 for full, apps/desktop/src-tauri/macos/*.lproj/InfoPlist.strings; each edition's own name otherwise), so the instructions call it that rather than by its file name.
printf '%s\n' \
  "$display_name macOS 安装说明" \
  '' \
  "1. 把「${display_name}」拖到「应用程序」文件夹。" \
  "2. 打开「应用程序」里的「${display_name}」，点「立即安装」把输入法安装到本机，再按设置页的提示在「系统设置」→「键盘」→「文字输入」→「输入法」中添加它。" \
  "3. 添加后在菜单栏的输入法菜单中选「${display_name}」，或按 Control+空格 切换。" \
  '' \
  "第一次安装后如果安装窗口提示要重新登录，或在「系统设置」的添加列表里找不到「${display_name}」：macOS 要到下次登录才会列出新装的输入法。请在苹果菜单中选「退出登录」，重新登录后再去添加。以后更新不需要再重新登录。" \
  '' \
  "只把「${display_name}」拖进「应用程序」而不打开它，系统里不会出现这个输入法。" \
  > "$stage/安装说明.txt"
dmg="$out_dir/$dmg_prefix-$version-$arch.dmg"
rm -f "$dmg"
# 用 ULMO（lzma）而不是 UDZO（zlib）：同样内容的 DMG 小约三分之一。ULMO 在 macOS 10.15 及以后都能挂载，而应用本身要求 13.0。DMG 创建之后才签名、公证和装订，事后再转换格式会丢掉签名，所以在创建时就选定格式。
create_dmg "$dmg" -volname "$display_name $version" -srcfolder "$stage" -format ULMO -fs HFS+
if [ -n "$identity" ]; then
  codesign_timestamped --force --sign "$identity" "$dmg"
  codesign --verify --strict "$dmg"
fi

if [ -n "$notarize" ]; then
  notary_submit "$dmg"
else
  echo "warning: not notarized; Gatekeeper will quarantine the downloaded app until the user opens it with right-click Open" >&2
fi

# 体积报告里 App 的 Contents/Resources 在这里量：下一个版本打包前会清掉 Tauri 的 bundle 目录。DMG 本身的大小要等装订之后（finish_edition）。
resources_kibs+=("$(du -sk "$app/Contents/Resources" | cut -f1)")
# 供设置应用直接安装的归档取的是这个已签名的应用本身。公证票据不装订进应用，所以不必等公证结果，内容与等到之后再打相同；任何一个版本公证失败整个打包都失败，归档不会单独发出去。
if [ -n "$sparkle_key" ]; then
  create_sparkle_update "$app" "$dmg_prefix-$version-settings.zip" "appcast-settings-$edition.xml"
fi
dmgs+=("$dmg")
}

# 装订之后对一个版本做最终检查：Gatekeeper 评估、挂载 DMG 后再跑一遍 check_app，再报告体积。参数是版本在 editions 里的下标。
finish_edition() {
  load_edition "${editions[$1]}"
  dmg="${dmgs[$1]}"
  # check_app 拿设置应用的构建号和公钥跟输入法比对。这个版本暂存在 target/macos 的输入法已被后面的版本清掉，改用它 DMG 暂存目录里那份 ditto 出来的同一个 bundle。
  staged_bundle="$(only "$work/dmg-$edition"/*.app/Contents/Resources/*.app)"
  if [ -n "$notarize" ]; then
    spctl -a -t open --context context:primary-signature -v "$dmg"
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
  local dmg_bytes resources_kib="${resources_kibs[$1]}"
  dmg_bytes="$(/usr/bin/stat -f %z "$dmg")"
  echo "DMG size ($edition): $dmg_bytes bytes; Contents/Resources: $resources_kib KiB"
  if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    printf '### macOS package size\n\n- %s: %s bytes\n- Contents/Resources: %s KiB\n' "$(basename "$dmg")" "$dmg_bytes" "$resources_kib" >> "$GITHUB_STEP_SUMMARY"
  fi
}

resources_kibs=()
for edition in "${editions[@]}"; do
  package_edition "$edition"
done

# ---- 公证结果与装订 ----
# 全部版本都已提交，先提交的多半已经处理完。任何一个没有通过时，先把每个版本的结果和失败提交的公证日志都打出来，再让打包失败。
if [ -n "$notarize" ]; then
  notary_failed=0
  for index in "${!editions[@]}"; do
    edition="${editions[$index]}"
    notary_status="$(notary_result "${notary_ids[$index]}")"
    echo "notarization $edition (${notary_ids[$index]}): $notary_status"
    if [ "$notary_status" != Accepted ]; then
      notary_failed=1
      xcrun notarytool log "${notary_ids[$index]}" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" >&2 || true
    fi
  done
  if [ "$notary_failed" -ne 0 ]; then
    echo "notarization failed for at least one edition; see the notarytool log above" >&2
    exit 1
  fi
  # 全部公证通过之后才逐个装订，不同时向 CloudKit 取几份票据。staple-ticket.sh 已对票据下载断线重试过；某个版本仍然失败时先装订其余版本，过一分钟再给失败的补一轮，已经编好、公证过的其他版本不因一个版本的票据下载断线而作废。补一轮仍失败才让打包失败。
  staple_failed=()
  for dmg in "${dmgs[@]}"; do
    if ! bash "$repo_root/platforms/macos/scripts/staple-ticket.sh" "$dmg"; then
      staple_failed+=("$dmg")
    fi
  done
  if [ "${#staple_failed[@]}" -gt 0 ]; then
    echo "stapling failed for ${#staple_failed[@]} DMG(s); retrying them once more in 60s" >&2
    sleep 60
    for dmg in "${staple_failed[@]}"; do
      bash "$repo_root/platforms/macos/scripts/staple-ticket.sh" "$dmg"
    done
  fi
fi

for index in "${!editions[@]}"; do
  finish_edition "$index"
done

# 所有版本的 DMG 都打完之后才把 dSYM 打成一个 zip：各版本带的是同一组可执行文件，只编一次、只剥离一次，所以一份 dSYM 覆盖全部版本。zip 长期留在 macos-symbols release 上，用来符号化这个版本的崩溃报告；它不进 SHA256SUMS，上传时以 GitHub 记录的摘要核对。
dsym_zip="$out_dir/msime-macos-$version-dsym.zip"
rm -f "$dsym_zip"
ditto -c -k --keepParent "$dsym_dir" "$dsym_zip"

(cd "$out_dir" && shasum -a 256 -- "${dmgs[@]##*/}" > SHA256SUMS && shasum -a 256 -c SHA256SUMS)
if [ ${#update_zips[@]} -gt 0 ]; then
  (cd "$out_dir" && shasum -a 256 -- "${update_zips[@]##*/}" > sparkle-SHA256SUMS && shasum -a 256 -c sparkle-SHA256SUMS)
fi
for dmg in "${dmgs[@]}"; do
  echo "macOS package: $dmg"
done
