#!/bin/bash
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
output=${1:-"$root/.build/macos/libMSIMEBackend.dylib"}
module_dir=$(dirname "$output")
mkdir -p "$module_dir"
sources=()
while IFS= read -r source; do
  if [[ ${source##*/} != Package.swift ]]; then sources+=("$source"); fi
done < <(find "$root/shared/backend/account" "$root/shared/backend/clients" "$root/shared/backend/content" "$root/shared/backend/storage" "$root/shared/backend-ui" "$root/platforms/macos/src/backend" -type f -name '*.swift' -print | sort)
deployment=${MACOSX_DEPLOYMENT_TARGET:-13.0}

# Translation.framework first ships with macOS 15 and the package runs on 13, where a strong link would stop the whole backend from loading. Its only user checks #available(macOS 26) before touching it.
compile() {
  xcrun swiftc -parse-as-library -emit-library -emit-module \
    -module-name MSIMEBackend -emit-module-path "${2%.dylib}.swiftmodule" \
    -target "$1" -Xlinker -install_name -Xlinker "@rpath/$(basename "$output")" \
    -Xlinker -weak_framework -Xlinker Translation \
    -o "$2" "${sources[@]}"
}

# 要构建的架构，以空格分隔，为空时是宿主架构。CMake 从 CMAKE_OSX_ARCHITECTURES 传进来：universal 包要两种架构，swiftc 一次只编一种，所以逐个编译后用 lipo 合并。MSIME_SWIFT_TARGET 指定完整 target 时照旧只编那一种。
read -r -a architectures <<< "${MSIME_SWIFT_ARCHS:-$(uname -m)}"
if [[ -n ${MSIME_SWIFT_TARGET:-} || ${#architectures[@]} -eq 1 ]]; then
  compile "${MSIME_SWIFT_TARGET:-${architectures[0]}-apple-macosx$deployment}" "$output"
  exit
fi
slices=()
for architecture in "${architectures[@]}"; do
  mkdir -p "$module_dir/$architecture"
  compile "$architecture-apple-macosx$deployment" "$module_dir/$architecture/$(basename "$output")"
  slices+=("$module_dir/$architecture/$(basename "$output")")
done
lipo -create "${slices[@]}" -output "$output"
