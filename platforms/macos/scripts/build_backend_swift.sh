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
# CMake 把构建配置传到这里。package-release.sh 发布的是 Release 构建，所以它按整模块优化体积、做 dead strip，编完再剥掉局部符号；`strip -x` 保留输入法要绑定的全局 `@_cdecl` 入口（`_MSIME*`）和它按名字查找的 Objective-C 类。其他配置以及不经过 CMake 的运行都保持 swiftc 默认的 `-Onone`，便于调试。测试框架自己用显式的 `-Onone` 编译同一批源文件，不经过这个脚本。
release=false
if [[ ${MSIME_SWIFT_CONFIGURATION:-} == Release ]]; then release=true; fi
optimisation=()
if $release; then optimisation=(-Osize -wmo -gline-tables-only -Xlinker -dead_strip); fi

# Translation.framework 从 macOS 15 才提供，包仍支持 13，故弱链接；调用方在使用前检查系统版本。
# `-I shared/backend/rust` 让 Swift 看到 C 头文件的类型（`CMSIMEClient` 模块）；dylib 不链接 Rust，函数指针由输入法经 `MSIMEInstallRustClient` 交来。
compile() {
  xcrun swiftc -parse-as-library -emit-library -emit-module \
    -module-name MSIMEBackend -emit-module-path "${2%.dylib}.swiftmodule" \
    -I "$root/shared/backend/rust" \
    -target "$1" -Xlinker -install_name -Xlinker "@rpath/$(basename "$output")" \
    -Xlinker -weak_framework -Xlinker Translation \
    ${optimisation[@]+"${optimisation[@]}"} \
    -o "$2" "${sources[@]}"
}

# 要构建的架构，以空格分隔，为空时是宿主架构。CMake 从 CMAKE_OSX_ARCHITECTURES 传进来：universal 包要两种架构，swiftc 一次只编一种，所以逐个编译后用 lipo 合并。MSIME_SWIFT_TARGET 指定完整 target 时照旧只编那一种。
read -r -a architectures <<< "${MSIME_SWIFT_ARCHS:-$(uname -m)}"
if [[ -n ${MSIME_SWIFT_TARGET:-} || ${#architectures[@]} -eq 1 ]]; then
  compile "${MSIME_SWIFT_TARGET:-${architectures[0]}-apple-macosx$deployment}" "$output"
  if $release; then xcrun strip -x "$output"; fi
  exit
fi
slices=()
symbols=()
for architecture in "${architectures[@]}"; do
  mkdir -p "$module_dir/$architecture"
  compile "$architecture-apple-macosx$deployment" "$module_dir/$architecture/$(basename "$output")"
  slices+=("$module_dir/$architecture/$(basename "$output")")
  symbols+=("$module_dir/$architecture/$(basename "$output").dSYM")
done
lipo -create "${slices[@]}" -output "$output"
if $release; then
  # swiftc 已在临时对象消失之前产生 dSYM，按 UUID 归并原件后再剥离安装用的 dylib。
  python3 "$root/platforms/macos/scripts/crash_symbols.py" merge "$output" "$output.dSYM" "${symbols[@]}"
  xcrun strip -x "$output"
fi
