#!/usr/bin/env bash
# Build Windows GNU DLL + native tests; excludes the SDK C++/WinRT demo.
# Never runs Windows executables.
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
arch=${1:?usage: build-cross.sh x64|x86 [edition]}
# 产品版本（shared/contracts/editions.json 里有 Windows 段的 id），缺省是 full。full 的输出仍在 target/windows-full/<arch>，其他版本在 target/windows-<id>/<arch>，host DLL 按版本表改名并生成同名导入库。
edition=${2:-full}
[[ "$edition" =~ ^[a-z][a-z0-9]*$ ]] || { echo "Expected an edition id" >&2; exit 2; }
case "$arch" in
  x64) triple=x86_64-pc-windows-gnu; compiler=x86_64-w64-mingw32; linker_var=CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER ;;
  x86) triple=i686-pc-windows-gnu; compiler=i686-w64-mingw32; linker_var=CARGO_TARGET_I686_PC_WINDOWS_GNU_LINKER ;;
  *) echo "Expected x64 or x86" >&2; exit 2 ;;
esac
command -v "$compiler-g++" >/dev/null
command -v "$compiler-gcc" >/dev/null
if [[ "$arch" = x86 ]]; then
  case "$("$compiler-g++" -dM -E -x c++ /dev/null)" in
    *__USING_SJLJ_EXCEPTIONS__*)
      echo "x86 Rust GNU needs DWARF unwinding; this MinGW uses SJLJ. Use a compatible toolchain, not panic=abort." >&2
      exit 1 ;;
  esac
fi
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) host_triplet=arm64-osx ;;
  Darwin-x86_64) host_triplet=x64-osx ;;
  Linux-x86_64) host_triplet=x64-linux ;;
  Linux-aarch64|Linux-arm64) host_triplet=arm64-linux ;;
  *) echo "Set up dependencies manually on this host" >&2; exit 1 ;;
esac
# Check compiler/host compatibility before any network preparation.
rustup target add "$triple"
if [[ -z ${MSIME_VCPKG_ROOT:-} ]]; then
  bash "$repo_root/platforms/windows/bootstrap-vcpkg.sh"
fi
vcpkg_root=${MSIME_VCPKG_ROOT:-$repo_root/target/tooling/vcpkg}
[[ "$vcpkg_root" = /* && -x "$vcpkg_root/vcpkg" ]] || { echo "Provide an absolute bootstrapped MSIME_VCPKG_ROOT" >&2; exit 1; }
[[ $(git -C "$vcpkg_root" rev-parse HEAD) = ef7dbf94b9198bc58f45951adcf1f041fcbc5ea0 ]] || { echo "vcpkg must match the manifest baseline" >&2; exit 1; }
git -C "$vcpkg_root" diff --quiet HEAD -- || { echo "vcpkg has tracked changes" >&2; exit 1; }
# Separate manifest install roots: vcpkg removes other target triplets when a
# manifest is reinstalled in the same root. Do not run this script concurrently
# against the same vcpkg checkout (it holds a filesystem lock).
#
# MSIME_WINDOWS_DEPS_ROOT shares the built dependencies across checkouts. This repository is worked in one short-lived worktree per task, and each would otherwise rebuild curl from source before it could compile a line of this project - minutes of work per worktree, for an identical answer every time. The manifest is the same file in every worktree, so one tree per architecture serves all of them; the per-arch split above is unchanged.
deps_root="${MSIME_WINDOWS_DEPS_ROOT:-$repo_root/target/windows-native-deps}/$arch"
prefix="$deps_root/$arch-mingw-static"
VCPKG_DISABLE_METRICS=1 "$vcpkg_root/vcpkg" install \
  --triplet "$arch-mingw-static" --host-triplet "$host_triplet" \
  --x-manifest-root="$repo_root/platforms/windows" --x-install-root="$deps_root"
env "$linker_var=$compiler-gcc" \
  cargo build --locked -p msime-host-api --target "$triple"
output="$repo_root/target/windows-$edition/$arch"
host_dll=msime_host_api.dll
host_library="$repo_root/target/$triple/debug/libmsime_host_api.dll.a"
if [[ "$edition" != full ]]; then
  # 两个版本的 TIP 被同一个应用加载时，按导入表找 DLL 会拿到先加载的那一个，所以不是 full 的版本换一个 DLL 名，再用 dlltool 按原 DLL 的导出表生成同名的导入库。
  host_dll=$(python3 platforms/windows/scripts/edition_windows.py field --edition "$edition" host_dll)
  mkdir -p "$output"
  python3 platforms/windows/scripts/edition_windows.py host-def --edition "$edition" \
    --dll "$repo_root/target/$triple/debug/msime_host_api.dll" --output "$output/${host_dll%.dll}.def"
  host_library="$output/lib$host_dll.a"
  "$compiler-dlltool" -d "$output/${host_dll%.dll}.def" -D "$host_dll" -l "$host_library"
fi
# compile_commands.json is what lets the same sources be re-checked for the
# other architecture with the flags they are really built with, rather than a
# second hand-maintained list that drifts.
cmake -S platforms/windows -B "$output" \
  -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_CXX_COMPILER="$compiler-g++" \
  -DCMAKE_C_COMPILER="$compiler-gcc" \
  -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
  -DCMAKE_BUILD_TYPE=Debug -DCMAKE_PREFIX_PATH="$prefix" \
  -DMSIME_WINDOWS_PIPE_ONLY=OFF \
  -DMSIMEUI_BUILD_HANDWRITING_DEMO=OFF \
  -DMSIME_EDITION="$edition" \
  -DMSIME_HOST_LIBRARY="$host_library"
cmake --build "$output" --parallel 4
cmake -E copy_if_different "$repo_root/target/$triple/debug/msime_host_api.dll" "$output/$host_dll"
echo "$arch $edition Windows GNU host/TSF DLLs, Server and native tests linked; SDK C++/WinRT handwriting demo excluded; Windows execution not performed; MinGW runtime DLLs are not bundled."
