#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
abi=${1:-arm64-v8a}
case "$abi" in
  arm64-v8a) rust_target=aarch64-unknown-linux-ohos; runtime_triple=aarch64-linux-ohos ;;
  armeabi-v7a) rust_target=armv7-unknown-linux-ohos; runtime_triple=arm-linux-ohos ;;
  x86_64) rust_target=x86_64-unknown-linux-ohos; runtime_triple=x86_64-linux-ohos ;;
  *) echo "Supported OpenHarmony ABIs: arm64-v8a, armeabi-v7a, x86_64" >&2; exit 1 ;;
esac
# DevEco Studio ships the NDK inside the app bundle. A standalone command-line SDK works too, as long as it points at the directory holding sysroot and build/cmake/ohos.toolchain.cmake.
ndk=${MSIME_OHOS_NDK:-/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native}
if [[ ! -f "$ndk/build/cmake/ohos.toolchain.cmake" ]]; then
  echo "OpenHarmony NDK not found. Install DevEco Studio or set MSIME_OHOS_NDK to a native SDK containing build/cmake/ohos.toolchain.cmake" >&2
  exit 1
fi
# The NDK names its wrappers after the Rust target triple, and each one carries its own --target and --sysroot. Using them keeps this script from restating either. The same wrapper is the C compiler for the SQLite amalgamation that rusqlite's bundled feature builds, so no device dependency has to be prepared by hand.
compiler="$ndk/llvm/bin/${rust_target}-clang"
if [[ ! -x "$compiler" ]]; then
  echo "Missing $compiler; this NDK does not provide a wrapper for $rust_target" >&2
  exit 1
fi
cargo_linker="CARGO_TARGET_$(printf '%s' "$rust_target" | tr '[:lower:]-' '[:upper:]_')_LINKER"
underscored=${rust_target//-/_}
env "CC_${underscored}=$compiler" "CXX_${underscored}=${compiler}++" \
  "AR_${underscored}=$ndk/llvm/bin/llvm-ar" "$cargo_linker=$compiler" \
  CARGO_TARGET_DIR="$repo_root/target/ohos-cargo" \
  cargo build -p msime-host-api --target "$rust_target" --release --locked
output="$repo_root/target/ohos/libs/$abi"
mkdir -p "$output"
cp "$repo_root/target/ohos-cargo/$rust_target/release/libmsime_host_api.so" "$output/"
# 不用 grep -q：它匹配到就提前退出，llvm-readobj 还在写时收到 EPIPE，pipefail 下整条管道判失败，而且时有时无（本机 2026-10-03 出现过 `write on a pipe with no reader`，脚本在编 NAPI 库之前就退出了）。
"$ndk/llvm/bin/llvm-readobj" --file-headers "$output/libmsime_host_api.so" | grep "EM_AARCH64\|EM_ARM\|EM_X86_64" >/dev/null
# miniaudio decodes and pitches the key-sound samples (native/key_sound_render.cpp). It is the single header the Windows host already pins, and like there its implementation is third-party code compiled at the toolchain's default warning level, outside the -Werror set below.
miniaudio="platforms/windows/third_party/miniaudio"
"${compiler}++" -std=c++17 -fPIC -O2 -c platforms/harmony/native/miniaudio.cpp -I"$miniaudio" \
  -o "$output/miniaudio.o"
# The ArkTS side reaches the C ABI through this module. --no-undefined keeps a missing binding a link error here rather than a failed import on the device.
"${compiler}++" -std=c++17 -shared -fPIC -Wall -Wextra -Werror \
  -Wl,--no-undefined -Wl,-soname,libmsimeclient.so \
  platforms/harmony/native/client_napi.cpp platforms/harmony/native/key_sound_render.cpp \
  "$output/miniaudio.o" -Icrates/host-api/include -I"$miniaudio" \
  -L"$output" -lmsime_host_api -lace_napi.z -lz -lm -o "$output/libmsimeclient.so"
"$ndk/llvm/bin/llvm-nm" -D --defined-only "$output/libmsimeclient.so" | grep RegisterClientModule >/dev/null
# The C++ runtime has to travel with the module. OpenHarmony does not expose a system libc++_shared.so to applications, so leaving it out makes the NAPI import fail on the device with "Error loading shared library libc++_shared.so" while the build itself stays perfectly green.
runtime="$ndk/llvm/lib/$runtime_triple/libc++_shared.so"
[[ -f "$runtime" ]] || { echo "Missing $runtime in this NDK" >&2; exit 1; }
cp "$runtime" "$output/"
# hvigor packs whatever sits in entry/libs/<abi> into the HAP, so stage every object there. The directory is build output, not source, and is ignored.
staged="$repo_root/platforms/harmony/entry/libs/$abi"
mkdir -p "$staged"
cp "$output/libmsime_host_api.so" "$output/libmsimeclient.so" "$output/libc++_shared.so" "$staged/"
echo "OpenHarmony native libraries built: $output"
echo "Staged for the HAP: $staged"
echo "Next: platforms/harmony && hvigorw assembleHap (not yet device-verified)"
