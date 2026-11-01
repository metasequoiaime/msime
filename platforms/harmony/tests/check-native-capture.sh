#!/usr/bin/env bash
# 复用 NDK 做小型目标编译；不另起 Rust 构建树，不把宿主编译当成鸿蒙验证。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../.." && pwd)
ndk=${MSIME_OHOS_NDK:-}
if [[ -z "$ndk" ]]; then
  for candidate in "$HOME/command-line-tools/sdk/default/openharmony/native" \
    /Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native; do
    if [[ -x "$candidate/llvm/bin/aarch64-unknown-linux-ohos-clang++" ]]; then
      ndk=$candidate
      break
    fi
  done
fi
if [[ -z "$ndk" ]]; then
  echo "skipped: Harmony native capture needs MSIME_OHOS_NDK or an installed DevEco native SDK"
  exit 0
fi
cd "$repo_root"
output=target/harmony-tests/native-capture
mkdir -p "$output"
for triple in aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos; do
  for source in platforms/common/CrashRecorder.cpp platforms/harmony/native/client_napi.cpp; do
    "$ndk/llvm/bin/$triple-clang++" -std=c++17 -g -fPIC -Wall -Wextra -Werror \
      -Icrates/host-api/include -c "$source" -o "$output/$triple-$(basename "${source%.cpp}").o"
  done
  echo "Harmony native capture: $triple compiles"
done
