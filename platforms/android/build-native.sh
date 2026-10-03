#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
abi=${1:-arm64-v8a}
case "$abi" in
  arm64-v8a) rust_target=aarch64-linux-android; compiler_target=aarch64-linux-android; triplet=arm64-msime-android; cargo_linker=CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER ;;
  x86_64) rust_target=x86_64-linux-android; compiler_target=x86_64-linux-android; triplet=x64-msime-android; cargo_linker=CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER ;;
  *) echo "Supported ABIs: arm64-v8a, x86_64" >&2; exit 1 ;;
esac
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
ndk=${MSIME_ANDROID_NDK:-${android_sdk}/ndk/28.2.13676358}
vcpkg_root=${MSIME_VCPKG_ROOT:-$repo_root/target/tooling/vcpkg}
if [[ ! -f "$ndk/source.properties" ]] || ! grep -q '28.2.13676358' "$ndk/source.properties"; then
  echo "Install pinned NDK 28.2.13676358 and set ANDROID_SDK_ROOT or MSIME_ANDROID_NDK" >&2; exit 1
fi
if [[ ! -x "$vcpkg_root/vcpkg" ]] || [[ $(git -C "$vcpkg_root" rev-parse HEAD) != ef7dbf94b9198bc58f45951adcf1f041fcbc5ea0 ]]; then
  echo "Provide bootstrapped vcpkg at pinned commit ef7dbf94b9198bc58f45951adcf1f041fcbc5ea0 using MSIME_VCPKG_ROOT" >&2; exit 1
fi
case $(uname -s) in
  Darwin) host_tag=darwin-x86_64 ;;
  Linux) host_tag=linux-x86_64 ;;
  *) echo "Use this build script on macOS or Linux" >&2; exit 1 ;;
esac
toolchain="$ndk/toolchains/llvm/prebuilt/$host_tag"
compiler="$toolchain/bin/${compiler_target}28-clang"
# vcpkg provides only nlohmann-json, which shared/voice/LocalAsr.cpp includes. Manifest installs reconcile their root, so the ABIs are kept apart and one build cannot uninstall the other ABI's copy.
deps="$repo_root/target/android-deps/$abi"
ANDROID_NDK_HOME="$ndk" VCPKG_DISABLE_METRICS=1 "$vcpkg_root/vcpkg" install \
  --x-manifest-root="$repo_root/platforms/android" --x-install-root="$deps" \
  --overlay-triplets="$repo_root/platforms/android/triplets" --triplet="$triplet"
env "CC_${rust_target//-/_}=$compiler" "CXX_${rust_target//-/_}=$compiler++" \
  "AR_${rust_target//-/_}=$toolchain/bin/llvm-ar" "$cargo_linker=$compiler" \
  CARGO_TARGET_DIR="$repo_root/target/android-cargo" \
  RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384" \
  cargo build -p msime-host-api --target "$rust_target" --release --locked
output="$repo_root/target/android/jniLibs/$abi"
mkdir -p "$output"
cp "$repo_root/target/android-cargo/$rust_target/release/libmsime_host_api.so" "$output/"
cp "$toolchain/sysroot/usr/lib/$compiler_target/libc++_shared.so" "$output/"
# On-device speech recognition: the sherpa-onnx C API and ONNX Runtime come prebuilt from the pinned release in resources/voice-runtime.lock.json (fetched and hash-checked, never committed). Only the two native libraries are taken from the .aar; LocalAsr loads the C API with dlopen, so a missing runtime disables local recognition instead of the keyboard.
python3 scripts/fetch_voice_runtime.py --platform android --out "$repo_root/target/voice-runtime/android"
voice_aar=$(ls "$repo_root"/target/voice-runtime/android/sherpa-onnx-*.aar)
unzip -o -j -q "$voice_aar" "jni/$abi/libsherpa-onnx-c-api.so" "jni/$abi/libonnxruntime.so" -d "$output"
"$compiler++" -std=c++17 -shared -fPIC -Wall -Wextra -Werror \
  -Wl,--no-undefined -Wl,-z,max-page-size=16384 -Wl,-soname,libmsime_android.so \
  platforms/android/native/client_jni.cpp shared/voice/LocalAsr.cpp \
  -Icrates/host-api/include -Ishared -Ishared/voice/third_party -I"$deps/$triplet/include" \
  -L"$output" -lmsime_host_api -ldl -o "$output/libmsime_android.so"
bash platforms/android/verify-native.sh "$toolchain/bin/llvm-readelf" "$output" "$abi"
notices="$repo_root/target/android/notices/$abi"
mkdir -p "$notices"
for copyright_file in "$deps/$triplet"/share/*/copyright; do
  package=$(basename "$(dirname "$copyright_file")")
  cp "$copyright_file" "$notices/$package.txt"
done
cp "$toolchain/NOTICE" "$notices/ndk-toolchain.txt"
cp shared/voice/third_party/sherpa-onnx/LICENSE "$notices/sherpa-onnx.txt"
cp platforms/linux/data/licenses/onnxruntime-MIT.txt "$notices/onnxruntime.txt"
cp platforms/linux/data/licenses/onnxruntime-ThirdPartyNotices.txt "$notices/onnxruntime-third-party.txt"
cp "$toolchain/sysroot/NOTICE" "$notices/ndk-sysroot.txt"
# The input engine in libmsime_host_api.so embeds the Korean Hanja table from libhangul's data/hanja/hanja.txt, which is BSD-3-Clause: clause 2 requires its notice in every binary distribution, so it travels with the library's other notices into assets/native-notices.
cp resources/licenses/libhangul-hanja-BSD-3-Clause.txt "$notices/libhangul-hanja.txt"
# The engine's Cantonese and Zhuyin schemes take their Jyutping and bopomofo syllables and words from data derived from rime-cantonese (CC BY 4.0, which requires attribution) and libchewing-data (LGPL-2.1-or-later, which requires the licence text and a source pointer). build-apk.sh and build-client-apk.sh package the dictionaries themselves, each beside its own licence text, when target/language-dictionaries (or MSIME_LANGUAGE_DICTIONARIES) holds them; the notices travel with every engine build either way so that one notice list covers every platform.
cp resources/licenses/rime-cantonese-CC-BY-4.0.txt "$notices/rime-cantonese.txt"
cp resources/licenses/libchewing-data-LGPL-2.1.txt "$notices/libchewing-data.txt"
# The Stroke scheme's stroke orders come from rime-stroke (LGPL-3.0, whose main table also carries the CNS11643 attribution); its notice travels the same way.
cp resources/licenses/rime-stroke-LGPL-3.0.txt "$notices/rime-stroke.txt"
# 越南文和藏文方案分别用 vi crate（MIT）和 ewts crate（MIT OR Apache-2.0，按 MIT 使用）编进 libmsime_host_api.so；ewts 没有自带许可证文件，仓库里这份全文是它的声明唯一能随二进制分发的途径。
cp resources/licenses/vi-MIT.txt "$notices/vi.txt"
cp resources/licenses/ewts-MIT.txt "$notices/ewts.txt"
echo "Android native libraries built: $output (not yet device-verified)"
