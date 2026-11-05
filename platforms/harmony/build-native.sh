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
# 发布与原生构建复用此入口；已选 ABI 的符号回归缺工具时直接失败。
MSIME_OHOS_NDK="$ndk" python3 scripts/test-harmony-crash-symbols.py --require-tools --target "$rust_target"
cargo_linker="CARGO_TARGET_$(printf '%s' "$rust_target" | tr '[:lower:]-' '[:upper:]_')_LINKER"
underscored=${rust_target//-/_}
# 整句联想「增强」的逐键重排跑在 ONNX Runtime 上（host-api 的 `onnx-rerank` 特性，见 .agents/notes/implemented/architecture/2026-11-01-onnx-rerank-backend.md）：精简构建的静态库（tools/onnxruntime-minimal 的 ohos-arm64）按 resources/onnxruntime-minimal.lock.json 下载核对，链进 libmsime_host_api.so。只有 arm64：32 位不在锁里，x86_64（模拟器）的库编不出来（见 tools/onnxruntime-minimal/build.sh），这两个 ABI 照旧不开这个特性，退回随包的句子模型。ort-sys 不认识 ohos 目标，默认去链 libstdc++，鸿蒙没有它；ORT_CXX_STDLIB 换成 NDK 的 libc++_shared.so，也就是下面随 HAP 带的那一份。
ort_env=()
cargo_features=()
if [[ "$abi" == arm64-v8a ]]; then
  ort_location=$(python3 scripts/fetch_onnxruntime.py --target "$rust_target")
  # ort-sys 把静态库复制进自己的输出目录，库换了也不会重新复制；按锁里库的 SHA-256 记一个戳，变了就让它重新复制。
  ort_stamp="$repo_root/target/ohos-cargo/onnxruntime-$rust_target.sha256"
  ort_library_sha=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["targets"][sys.argv[2]]["library_sha256"])' resources/onnxruntime-minimal.lock.json "$rust_target")
  if [ -d "$repo_root/target/ohos-cargo/$rust_target" ] && [ "$(cat "$ort_stamp" 2>/dev/null)" != "$ort_library_sha" ]; then
    CARGO_TARGET_DIR="$repo_root/target/ohos-cargo" cargo clean -p ort-sys --target "$rust_target" --release
  fi
  ort_env=(ORT_LIB_LOCATION="$ort_location" ORT_CXX_STDLIB=c++_shared)
  cargo_features=(--features onnx-rerank)
fi
env "CC_${underscored}=$compiler" "CXX_${underscored}=${compiler}++" \
  "AR_${underscored}=$ndk/llvm/bin/llvm-ar" "$cargo_linker=$compiler" \
  CARGO_TARGET_DIR="$repo_root/target/ohos-cargo" \
  CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_STRIP=none \
  ${ort_env[@]+"${ort_env[@]}"} \
  cargo rustc -p msime-host-api ${cargo_features[@]+"${cargo_features[@]}"} --target "$rust_target" --release --locked -- \
    -C link-arg=-Wl,--build-id=sha1
if [[ "$abi" == arm64-v8a ]]; then
  mkdir -p "$repo_root/target/ohos-cargo"
  printf '%s\n' "$ort_library_sha" > "$ort_stamp"
fi
output="$repo_root/target/ohos/libs/$abi"
mkdir -p "$output"
cp "$repo_root/target/ohos-cargo/$rust_target/release/libmsime_host_api.so" "$output/"
# 不用 grep -q：它匹配到就提前退出，llvm-readobj 还在写时收到 EPIPE，pipefail 下整条管道判失败，而且时有时无（本机 2026-10-03 出现过 `write on a pipe with no reader`，脚本在编 NAPI 库之前就退出了）。
"$ndk/llvm/bin/llvm-readobj" --file-headers "$output/libmsime_host_api.so" | grep "EM_AARCH64\|EM_ARM\|EM_X86_64" >/dev/null
# 没链进 ONNX Runtime 时会话只能退回随包的句子模型，HAP 里的 rerank-model-small 永远用不上，而编译、打包和装机都不会报错。先把符号表读进变量再查，理由同上面的 llvm-readobj。
if [[ "$abi" == arm64-v8a ]]; then
  host_symbols=$("$ndk/llvm/bin/llvm-nm" --defined-only "$output/libmsime_host_api.so")
  if ! grep 'OrtGetApiBase' <<< "$host_symbols" >/dev/null; then
    echo "libmsime_host_api.so does not link ONNX Runtime; it must be built with --features onnx-rerank" >&2
    exit 1
  fi
fi
# miniaudio decodes and pitches the key-sound samples (native/key_sound_render.cpp). It is the single header the Windows host already pins, and like there its implementation is third-party code compiled at the toolchain's default warning level, outside the -Werror set below.
miniaudio="platforms/windows/third_party/miniaudio"
"${compiler}++" -std=c++17 -fPIC -O2 -c platforms/harmony/native/miniaudio.cpp -I"$miniaudio" \
  -o "$output/miniaudio.o"
# The ArkTS side reaches the C ABI through this module. --no-undefined keeps a missing binding a link error here rather than a failed import on the device.
"${compiler}++" -std=c++17 -g -Wl,--build-id=sha1 -shared -fPIC -Wall -Wextra -Werror \
  -Wl,--no-undefined -Wl,-soname,libmsimeclient.so \
  platforms/harmony/native/client_napi.cpp platforms/harmony/native/key_sound_render.cpp \
  platforms/common/CrashRecorder.cpp \
  "$output/miniaudio.o" -Icrates/host-api/include -I"$miniaudio" \
  -L"$output" -lmsime_host_api -lace_napi.z -lhilog_ndk.z -lz -lm -o "$output/libmsimeclient.so"
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

python3 scripts/crash-symbols.py collect --platform harmony --version "${MSIME_HARMONY_VERSION:-$(cat platforms/harmony/version.txt)}" \
  --root "$output" --tool-prefix "$ndk/llvm/bin/llvm-" \
  --output "target/harmony-symbols/harmony-${MSIME_HARMONY_VERSION:-$(cat platforms/harmony/version.txt)}-$abi-${MSIME_BUILD_RUN:-${GITHUB_RUN_ID:-local}}-${GITHUB_RUN_ATTEMPT:-1}.zip"
