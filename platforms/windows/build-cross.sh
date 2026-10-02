#!/usr/bin/env bash
# Build Windows GNU DLL + native tests; excludes the SDK C++/WinRT demo.
# Never runs Windows executables.
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
arch=${1:?usage: build-cross.sh x64|x86}
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
output="$repo_root/target/windows-full/$arch"
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
  -DMSIME_HOST_LIBRARY="$repo_root/target/$triple/debug/libmsime_host_api.dll.a"
cmake --build "$output" --parallel 4
cmake -E copy_if_different "$repo_root/target/$triple/debug/msime_host_api.dll" "$output"
echo "$arch Windows GNU host/TSF DLLs, Server and native tests linked; SDK C++/WinRT handwriting demo excluded; Windows execution not performed; MinGW runtime DLLs are not bundled."
