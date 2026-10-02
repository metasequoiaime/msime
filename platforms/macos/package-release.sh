#!/usr/bin/env bash
# Build the installable macOS release: the Tauri settings app with the pinned dictionaries (EngineResources), the handwriting model, the licence files and the InputMethodKit bundle 水杉输入法.app embedded as resources, packed into a DMG with a SHA256SUMS beside it.
#
# The same steps run locally and in release-macos.yml, so a package that passes here is the package CI publishes.
#
# Usage: platforms/macos/package-release.sh [VERSION] [OUT_DIR]
#   VERSION defaults to platforms/macos/version.txt, the version release-macos.yml tags as macos-vVERSION. It becomes the version the settings app reports, so the in-app update check compares like with like, and it must equal the input method's CFBundleShortVersionString (platforms/macos/Info.plist.in).
#   OUT_DIR defaults to target/macos-package/dist and receives msime-macos-VERSION-universal.dmg and SHA256SUMS.
#
# Environment:
#   MSIME_SPARKLE_ROOT            required; directory containing the pinned Sparkle.framework (see README.md)
#   CARGO_TARGET_DIR              defaults to target/ in the repository
#   MSIME_MACOS_BUILD_DIR         CMake build tree; defaults to target/macos-release
#   MACOS_SIGNING_IDENTITY        a "Developer ID Application: ... (TEAMID)" identity in the keychain. Without it everything is signed ad-hoc: the package builds and the settings app runs, but macOS will not register the embedded input method as an input source (see scripts/install.sh)
#   APPLE_ID, APPLE_TEAM_ID, APPLE_APP_SPECIFIC_PASSWORD
#                                 when all three are set (and an identity is), the DMG is notarized and stapled
#   MSIME_REQUIRE_LANGUAGE_DICTIONARIES
#                                 1 fails the package unless the Cantonese and Zhuyin dictionaries are staged and embedded; otherwise a package without them still builds, with both schemes shown as unavailable
#
# 产物是 universal 的：Apple 芯片和 Intel Mac 用同一个包。Rust 产物按两个 target 各编一次再用 lipo 合并，CMake 的目标和 Swift 后端经 CMAKE_OSX_ARCHITECTURES 编出双架构，打包后 check_app 逐个核对包里的 Mach-O 都含两种架构。需要 rustup target add aarch64-apple-darwin x86_64-apple-darwin。
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"

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
bundle_name="水杉输入法.app"
bundle_id="app.msime.inputmethod.MetasequoiaIME"
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

# codesign for one path. A Developer ID signature carries a secure timestamp, which notarization requires; an ad-hoc signature cannot carry one.
sign() {
  if [ -n "$identity" ]; then
    codesign --force --options runtime --timestamp --sign "$identity" "$@"
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

# ---- Dictionaries and the handwriting model ----
# The offline handwriting model the settings app's handwriting panel recognises with (apps/desktop/src-tauri/src/platform/macos/macos_handwriting.rs), pinned by resources/handwriting-model.lock.json. It is copied into the app below rather than listed in tauri.macos.conf.json, so a development build of the settings app does not need the 26.8 MB download.
handwriting_model="$repo_root/target/handwriting-model"
python3 scripts/fetch_handwriting_model.py --out "$handwriting_model"
# install_resources prints progress on stderr and the verified directory as its last stdout line. stage-resources.sh re-verifies it and stages target/macos/EngineResources, which tauri.macos.conf.json embeds.
resources="$(cargo run --quiet --locked -p msime-client-core --example install_resources -- "$work/desktop-resources" | tail -n 1)"
# The Cantonese and Zhuyin dictionaries pinned by resources/language-dictionaries.lock.json, into target/language-dictionaries where stage-resources.sh looks for them. Until a release is pinned the script prints a skipped line and fetches nothing.
python3 scripts/fetch_language_dictionaries.py >/dev/null
bash platforms/macos/stage-resources.sh "$resources"

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
mkdir -p target/macos
find target/macos -maxdepth 1 -name '*.app' -exec rm -rf {} +
# ditto keeps the framework symlinks that cp -R and cmake -E copy_directory would flatten; a flattened Sparkle.framework cannot be sealed.
ditto "$built_bundle" "target/macos/$bundle_name"
staged_bundle="$(only target/macos/*.app)"
# --deep, as scripts/install.sh does: Sparkle arrives signed by its publisher, and under the hardened runtime a process cannot load a library whose Team ID differs from its own. The entitlements carry microphone access for voice input.
if [ -n "$identity" ]; then
  codesign --force --deep --options runtime --timestamp --entitlements "$entitlements" --sign "$identity" "$staged_bundle"
else
  codesign --force --deep --options runtime --entitlements "$entitlements" --sign - "$staged_bundle"
fi
codesign --verify --deep --strict "$staged_bundle"

# ---- MCP server ----
# The same compiler flags as the input method: msime-mcp links the Engine through msime-host-api, and those objects are shared with the build above. Nothing in the app starts it; an agent's MCP configuration runs Contents/MacOS/msime-mcp over stdio.
CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" \
  cargo_universal "$universal_dir/msime-mcp" msime-mcp -p msime-mcp-server --bin msime-mcp

# ---- Settings app ----
pnpm install --frozen-lockfile
pnpm --filter @msime/desktop build
# Compiled with cargo and only then bundled by `tauri bundle`, not with `tauri build`: tauri build exports MACOSX_DEPLOYMENT_TARGET from bundle.macOS.minimumSystemVersion, rustc applies it to the host proc-macro dylibs as well, and on current macOS those come out with a mis-aligned LINKEDIT string pool that dlopen rejects, so the build fails with "can't find crate". cargo leaves the variable out of its fingerprint, so a broken proc-macro would also be reused by later builds. tauri/custom-protocol is what tauri build would enable (the binary serves the embedded frontend instead of devUrl), and TAURI_CONFIG sets the version the app reports, as package-container.sh does for Linux.
env -u MACOSX_DEPLOYMENT_TARGET TAURI_CONFIG="{\"version\":\"$version\"}" \
  CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 CMAKE_PREFIX_PATH="$(brew --prefix)" \
  cargo_universal "$CARGO_TARGET_DIR/release/msime-desktop" msime-desktop -p msime-desktop --bin msime-desktop --features tauri/custom-protocol
# 合并后的 universal 可执行文件放在 target/release/msime-desktop，tauri bundle 从那里取。
tauri_bundle_dir="$CARGO_TARGET_DIR/release/bundle/macos"
rm -rf "$tauri_bundle_dir"
# No APPLE_SIGNING_IDENTITY: Tauri would sign the nested input method again without its entitlements. The outer app is signed below instead. tauri.macos.conf.json is merged automatically on macOS and is what embeds EngineResources and the input method.
env -u APPLE_SIGNING_IDENTITY -u APPLE_CERTIFICATE \
  pnpm --filter @msime/desktop exec tauri bundle --bundles app --config "{\"version\":\"$version\"}"
app="$(only "$tauri_bundle_dir"/*.app)"
app_name="$(basename "$app")"
# Tauri copies resources by following symlinks, which turns Sparkle.framework's links into duplicate files and drops the directory links, and codesign then rejects the nested bundle ("invalid Info.plist (plist or signature have been modified)"). The signed bundle staged above is copied back over it with ditto, which keeps the links; the settings app's installer recreates them in ~/Library/Input Methods the same way.
find "$app/Contents/Resources" -maxdepth 1 -name '*.app' -exec rm -rf {} +
ditto "$staged_bundle" "$app/Contents/Resources/$bundle_name"
# A helper executable beside the app's own is signed on its own first, and the outer signature below seals it.
ditto "$universal_dir/msime-mcp" "$app/Contents/MacOS/msime-mcp"
sign "$app/Contents/MacOS/msime-mcp"
# Non-English candidate glosses (scripts/fetch_offline_glosses.py), copied here rather than listed in tauri.macos.conf.json because Tauri fails on a resource path that does not exist and the package must still build without them. The input method reads them beside EngineResources.
glosses="$repo_root/target/macos/offline-glosses"
if [ -d "$glosses" ]; then
  ditto "$glosses" "$app/Contents/Resources/offline-glosses"
fi
# The Cantonese and Zhuyin dictionaries, copied for the same reason as the glosses. The input method finds them beside EngineResources; without them both schemes fall back.
languages="$repo_root/target/macos/language-dictionaries"
if [ -d "$languages" ]; then
  ditto "$languages" "$app/Contents/Resources/language-dictionaries"
fi
# Beside the Zinnia licence tauri.macos.conf.json already put in Contents/Resources/handwriting.
mkdir -p "$app/Contents/Resources/handwriting"
cp "$handwriting_model/handwriting-zh_CN.model" "$handwriting_model/HandwritingModel-LICENSE.txt" "$app/Contents/Resources/handwriting/"
# Without --deep, so the input method keeps the signature and entitlements it was given above; the outer signature seals it as a nested resource.
sign "$app"
codesign --verify --deep --strict "$app"

# The package is only worth shipping if it holds what the install button and first run read. Checked on the built app and again on the copy inside the mounted DMG.
check_app() {
  local root="$1"
  local resources_dir="$root/Contents/Resources"
  test -d "$resources_dir/EngineResources"
  cargo run --quiet --locked -p msime-client-core --example verify_resources -- "$resources_dir/EngineResources" >/dev/null
  for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt NOTICE.md NOTICE-jiajia.md; do
    test -f "$resources_dir/EngineResources/helpcodes/$table"
  done
  test -f "$resources_dir/handwriting/handwriting-zh_CN.model"
  test -f "$resources_dir/handwriting/HandwritingModel-LICENSE.txt"
  test -f "$resources_dir/handwriting/Zinnia-LICENSE.txt"
  test -f "$resources_dir/Licenses/THIRD_PARTY_NOTICES.txt"
  test -x "$root/Contents/MacOS/msime-mcp"
  codesign --verify --strict "$root/Contents/MacOS/msime-mcp"
  if [ -d "$glosses" ]; then
    test -f "$resources_dir/offline-glosses/offline-glosses-NOTICE.txt"
  fi
  # A release build must carry both dictionaries; any other build carries whatever was staged, each dictionary with its licence.
  if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ] || [ -d "$languages" ]; then
    local pair
    for pair in cantonese.db:rime_cantonese_LICENSE.txt zhuyin.db:libchewing_data_LICENSE.txt; do
      if [ "${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" = 1 ] || [ -f "$languages/${pair%%:*}" ]; then
        test -s "$resources_dir/language-dictionaries/${pair%%:*}"
        test -f "$resources_dir/language-dictionaries/${pair#*:}"
      fi
    done
  fi
  local nested
  nested="$(only "$resources_dir"/*.app)"
  test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$nested/Contents/Info.plist")" = "$bundle_id"
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
      lipo "$file" -verify_arch "${architectures[@]}" || {
        echo "not universal ($(lipo -archs "$file")): $file" >&2
        exit 1
      }
    fi
  done < <(find "$root" -type f -print0)
}
check_app "$app"

# ---- DMG ----
# The macOS convention for a drag-to-install app: the app beside a link to /Applications.
stage="$work/dmg"
mkdir -p "$stage"
ditto "$app" "$stage/$app_name"
ln -s /Applications "$stage/Applications"
# Dragging the app is only half the install: the input method appears once the app has been opened and its install window's 立即安装 pressed, and on macOS 27 the user then adds it in System Settings, which the app walks them through. Finder shows the app as 水杉输入法 (apps/desktop/src-tauri/macos/Info.plist), so the instructions call it that rather than by its file name.
printf '%s\n' \
  '水杉输入法 macOS 安装说明' \
  '' \
  '1. 把「水杉输入法」拖到「应用程序」文件夹。' \
  '2. 打开「应用程序」里的「水杉输入法」，点「立即安装」把输入法安装到本机，再按设置页的提示在「系统设置」→「键盘」→「文字输入」→「输入法」中添加它。' \
  '3. 添加后在菜单栏的输入法菜单中选「水杉输入法」，或按 Control+空格 切换。' \
  '' \
  '只把「水杉输入法」拖进「应用程序」而不打开它，系统里不会出现这个输入法。' \
  > "$stage/安装说明.txt"
mkdir -p "$out_dir"
dmg="$out_dir/msime-macos-$version-$arch.dmg"
rm -f "$dmg" "$out_dir/SHA256SUMS"
hdiutil create -quiet -volname "水杉输入法 $version" -srcfolder "$stage" -format UDZO -fs HFS+ "$dmg"
if [ -n "$identity" ]; then
  codesign --force --timestamp --sign "$identity" "$dmg"
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

(cd "$out_dir" && shasum -a 256 -- *.dmg > SHA256SUMS && shasum -a 256 -c SHA256SUMS)
echo "macOS package: $dmg"
