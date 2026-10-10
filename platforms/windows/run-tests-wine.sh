#!/usr/bin/env bash
# Run the cross-built Windows test executables under Wine, in a container.
#
# These suites were built by build-cross.sh and then never run: executing a
# Windows binary needs Windows, so every report about them said "linked".
# Linking does not catch an assertion. Wine runs the resulting executables,
# which is the difference between a suite that compiles and one that passes.
#
# What it cannot run is recorded rather than hidden: anything that needs a
# compositor, a real monitor, or the installed dictionary bundle fails here for
# the environment, the same way it does on a Windows session with no desktop.
# Those names are compared against scripts/known-failures.txt, so the question
# this answers is the same one the rest of local verification answers - did
# this change break something that worked.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
arch=${1:-x64}
build="$root/target/windows-full/$arch"
image=msime-wine:local

if ! command -v docker >/dev/null 2>&1; then
  echo "skipped: docker is not installed"
  exit 0
fi
if ! docker info >/dev/null 2>&1; then
  echo "skipped: the docker daemon is not running"
  exit 0
fi
if [ ! -d "$build" ]; then
  echo "skipped: $build not built"
  echo "  run platforms/windows/build-cross.sh $arch first"
  exit 0
fi

# 新构建由实际生产编译器暂存运行时 DLL。旧目录没有这些文件时，
# x86 沿用 Debian DWARF 工具链，x64 沿用本机工具链；不能把 Homebrew
# i686 的 SJLJ 展开器配给 Rust 要求的 DWARF 产物。
runtime="$(mktemp -d)"
trap 'rm -rf "$runtime"' EXIT

# 优先复用构建方按实际编译器暂存的 DLL，保留旧构建目录的工具链回退。
unwind=libgcc_s_seh-1.dll
[ "$arch" = x86 ] && unwind=libgcc_s_dw2-1.dll
if [ -f "$build/libwinpthread-1.dll" ] && [ -f "$build/libstdc++-6.dll" ] && [ -f "$build/$unwind" ]; then
  cp "$build/libwinpthread-1.dll" "$build/libstdc++-6.dll" "$build/$unwind" "$runtime/" || exit 1
elif [ "$arch" = x86 ]; then
  cross=msime-cross:local
  docker build --platform linux/amd64 -t "$cross" "$root/platforms/windows/cross" >/dev/null 2>&1 || {
    echo "skipped: could not build the cross image"; exit 0; }
  # Ask the compiler for its own runtime rather than guessing which versioned
  # gcc directory the distribution used.
  docker run --rm --platform linux/amd64 -v "$runtime":/out "$cross" sh -c '
    for name in libwinpthread-1.dll libstdc++-6.dll libgcc_s_dw2-1.dll; do
      path=$(i686-w64-mingw32-g++ -print-file-name="$name")
      [ -f "$path" ] && cp "$path" /out/
    done' >/dev/null 2>&1
  found=$(ls -1 "$runtime" 2>/dev/null | wc -l | tr -d " ")
  if [ "$found" != 3 ]; then
    echo "skipped: the i686 MinGW runtime is not in the cross image"
    exit 0
  fi
else
  compiler=$(command -v x86_64-w64-mingw32-g++ 2>/dev/null) || {
    echo "skipped: x86_64-w64-mingw32-g++ is not installed"; exit 0; }
  # Ask the compiler where its own files live rather than searching a prefix: the
  # same package ships an i686 toolchain with identically named DLLs, and picking
  # those makes every x86_64 executable fail to load - which reads as the whole
  # suite failing rather than as a runner mistake.
  toolchain="$("$compiler" -print-sysroot 2>/dev/null)"
  [ -n "$toolchain" ] && [ -d "$toolchain" ] ||
    toolchain="$(dirname "$("$compiler" -print-libgcc-file-name 2>/dev/null)")"
  found=0
  for name in libwinpthread-1.dll libstdc++-6.dll libgcc_s_seh-1.dll; do
    path=$(find "$toolchain" -name "$name" -print -quit 2>/dev/null)
    [ -n "$path" ] && cp "$path" "$runtime/" && found=$((found + 1))
  done
  if [ "$found" -ne 3 ]; then
    echo "skipped: the MinGW runtime DLLs are not where this toolchain keeps them"
    exit 0
  fi
fi

docker build --platform linux/amd64 -t "$image" "$root/platforms/windows/wine" >/dev/null 2>&1 || {
  echo "skipped: could not build the Wine image"; exit 0; }

# windows-session-smoke takes an optional resource directory and needs one to
# get past its candidate-translation checks - the dictionaries are release
# artefacts the cross build does not stage. Point MSIME_WINE_RESOURCES at a
# directory holding what resources/desktop-dictionary.lock.json lists and it is
# passed through; without it the suite runs as far as it can.
# bash 3.2 treats an empty array as unset under `set -u`, so every expansion
# of it has to be guarded rather than written plainly.
#
# Default to the cache the rest of the repository already uses for this set - platforms/windows/installer/DesktopResources.md documents target/desktop-resources and the packaging scripts default to it - so a checkout that has it gets the full suite without anyone having to know this variable exists. Populate it with
#
#   cargo run -p msime-client-core --example install_resources -- target/desktop-resources
#
# install_resources writes into a generation directory named for the set's
# hash, so the artifacts are one level below what it is given. Resolve that
# here: what gets mounted has to be the directory the files are actually in.
if [ -z "${MSIME_WINE_RESOURCES:-}" ]; then
  cache="$root/target/desktop-resources"
  if [ -f "$cache/msime-pinyin.db" ]; then
    MSIME_WINE_RESOURCES="$cache"
  else
    for generation in "$cache"/*/; do
      if [ -f "$generation/msime-pinyin.db" ]; then
        MSIME_WINE_RESOURCES="${generation%/}"
        break
      fi
    done
  fi
fi
resources_mount=()
resources_argument=""
if [ -n "${MSIME_WINE_RESOURCES:-}" ] && [ -d "${MSIME_WINE_RESOURCES}" ]; then
  resources_mount=(-v "${MSIME_WINE_RESOURCES}":/res:ro)
  resources_argument='Z:\\res'
  echo "resources: ${MSIME_WINE_RESOURCES}"
else
  echo "resources: none; windows-session-smoke will stop at its documented input"
fi

# windows-installer-launch reads the real installer script rather than a copy of
# its arguments, so it needs the repository. That is this runner's to supply -
# the directory is right here - and not a property of the test.
installer="$root/platforms/windows/installer"
installer_mount=()
installer_argument=""
if [ -d "$installer" ]; then
  installer_mount=(-v "$installer":/installer:ro)
  installer_argument='Z:\\installer\\msime_setup.iss'
fi

# CTest passes these checked-in dictionaries to the Stroke and Zhuyin suites.
# Their executables assert that argv[1] exists, so the Wine runner must supply
# the same inputs rather than counting an argument error as a product failure.
fixtures="$root/platforms/windows/tests/input/fixtures"
stroke_argument='Z:\\fixtures\\msime-stroke.db'
zhuyin_argument='Z:\\fixtures\\msime-zhuyin.db'
# CMake 在子目录里构建 TSF 的测试，其中的接线检查要读源文件。
tsf_source="$root/platforms/windows/tsf"
tsf_source_argument='Z:\\tsf-source'
# 日语转换的接线检查还要读 Server 的 ReplyComposer.cpp，按 TSF 源码目录的 ../src 找，所以 Server 源码挂在 /src，与 /tsf-source 同级。
server_source="$root/platforms/windows/src"

# The Rust host carries the Windows-only code the C++ suite never touches:
# clipboard reads and writes, synthetic key strokes, the extended-key set. Its
# tests build for the same target and run under the same Wine, but this runner
# only ever globbed C++ executables, so none of them ran here. Build them into a
# staging directory and let the loop below pick them up with the rest.
rust_triple=x86_64-pc-windows-gnu
rust_compiler=x86_64-w64-mingw32-gcc
linker_var=CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER
if [ "$arch" = x86 ]; then
  rust_triple=i686-pc-windows-gnu
  rust_compiler=i686-w64-mingw32-gcc
  linker_var=CARGO_TARGET_I686_PC_WINDOWS_GNU_LINKER
fi
rust_stage="$root/target/wine-rust-tests/$arch"
rm -rf "$rust_stage"
mkdir -p "$rust_stage"
rust_build_log="$rust_stage/cargo.log"
rust_command=()
if command -v cargo >/dev/null 2>&1 && command -v "$rust_compiler" >/dev/null 2>&1; then
  rust_command=(cargo)
else
  # The native tests and their runtime can come from build-cross-container.sh,
  # while this host has no MinGW compiler. Use that same image and Cargo cache
  # so the Rust tests are built for the Windows target as well.
  daemon_platform=$(docker info --format '{{.OSType}}/{{.Architecture}}' 2>>"$rust_build_log")
  case "$daemon_platform" in
    linux/aarch64|linux/arm64) cross_platform=linux/arm64; cross_image=msime-cross:local-arm64 ;;
    linux/*) cross_platform=linux/amd64; cross_image=msime-cross:local ;;
    *) echo "Unsupported cross-build Docker platform: $daemon_platform" >>"$rust_build_log" ;;
  esac
  if [ -n "${cross_image:-}" ] &&
     docker build --platform "$cross_platform" -t "$cross_image" "$root/platforms/windows/cross" >>"$rust_build_log" 2>&1; then
    mkdir -p "$root/target/windows-cross/cargo-home"
    rust_command=(docker run --rm --platform "$cross_platform" -v "$root":/repo -w /repo
      -e CARGO_HOME=/repo/target/windows-cross/cargo-home
      -e "$linker_var=$rust_compiler" "$cross_image" cargo)
  fi
fi
if [ "${#rust_command[@]}" -gt 0 ]; then
  # --no-run builds the test binaries and prints where they landed. Report a
  # build failure as a test failure while still running the C++ suite below.
  # host-api is the DLL the Server links against, so its FFI boundary is worth exercising on the target it ships for; it needs no vcpkg prefix, since its C parts build with the same MinGW toolchain.
  # client-core carries the shared logic plus a few #[cfg(windows)] paths - the
  # file-replacement retry in the gloss store among them - that the host run can
  # never reach, because on macOS and Linux the other branch is compiled.
  rust_packages="-p msime-host-windows -p msime-client-core -p msime-engine -p msime-host-api"
  # Filter on profile.test: --no-run also reports examples, which are ordinary
  # programs that expect arguments and would be counted as failures here.
  # --tests excludes examples, which are not tests and need not build for this
  # target.
  # The workspace cargo suite runs the general library and golden tests. Under
  # emulated Wine they exceed the per-program time limit and golden also needs
  # source fixtures; keep the Windows host and bounded integration targets here.
  echo "note: Rust library and golden suites are built for Windows; Wine runs the host and integration suites"
  rust_build_ok=1
  "${rust_command[@]}" test $rust_packages --target "$rust_triple" --no-run --tests \
    --message-format=json 2>>"$rust_build_log" \
    | python3 -c 'import sys, json
for line in sys.stdin:
    try:
        message = json.loads(line)
    except ValueError:
        continue
    executable = message.get("executable")
    target = message.get("target", {}).get("name")
    if target in {"msime_client_core", "msime_engine", "msime_host_api", "golden"}:
        continue
    if executable and message.get("profile", {}).get("test"):
        print(executable)' \
    | while IFS= read -r exe; do
        case "$exe" in /repo/*) exe="$root/${exe#/repo/}" ;; esac
        [ -f "$exe" ] || continue
        cp "$exe" "$rust_stage/rust-$(basename "$exe" .exe | sed 's/-[0-9a-f]\{16\}$//').exe"
      done || rust_build_ok=0
  # Silence here would mean the Rust suites vanish without a word, which is how
  # the C++ side lost msimeui-tests for so long. Say so, and keep the log.
  if [ -z "$(ls -A "$rust_stage" 2>/dev/null | grep -v '^cargo\.log$')" ]; then
    echo "note: no Rust test binaries were staged; see $rust_build_log"
    rust_build_ok=0
  fi
  if [ "$rust_build_ok" -eq 0 ]; then
    grep -E '^error' "$rust_build_log" | head -3
    echo "FAIL rust-test-build"
  fi
else
  echo "note: Rust test builder unavailable; see $rust_build_log"
  echo "FAIL rust-test-build"
fi

docker run --rm --platform linux/amd64 \
  -v "$build":/bin-win:ro -v "$runtime":/rt:ro -v "$rust_stage":/bin-rust:ro ${resources_mount[@]+"${resources_mount[@]}"} \
  ${installer_mount[@]+"${installer_mount[@]}"} -v "$fixtures":/fixtures:ro -v "$tsf_source":/tsf-source:ro -v "$server_source":/src:ro \
  -e "MSIME_RESOURCES=$resources_argument" -e "MSIME_INSTALLER=$installer_argument" \
  -e "MSIME_STROKE_FIXTURE=$stroke_argument" -e "MSIME_ZHUYIN_FIXTURE=$zhuyin_argument" \
  -e "MSIME_TSF_SOURCE=$tsf_source_argument" \
  -e LANG=C.utf8 -e LC_ALL=C.utf8 "$image" sh -c '
mkdir -p /run/t && cp /rt/*.dll /run/t/ && cp /bin-win/*.dll /run/t/ 2>/dev/null
cp /bin-win/tsf/*MetasequoiaImeTsf.dll /run/t/ 2>/dev/null
cd /run/t
# CMake puts TSF tests in tsf/, its registration tests below tsf/tests/,
# and msimeui tests in bin/.
for exe in /bin-win/windows-*.exe /bin-win/tsf/msime-tsf-*.exe /bin-win/msimeui-tests.exe \
           /bin-win/tsf/tests/registration_categories/msime-tsf-*.exe \
           /bin-win/tsf/tests/registration_profiles/msime-tsf-*.exe \
           /bin-win/bin/msimeui-tests.exe /bin-rust/rust-*.exe; do
  [ -f "$exe" ] || continue
  name=$(basename "$exe" .exe)
  cp "$exe" /run/t/ 2>/dev/null || continue
  argument=""
  [ "$name" = windows-session-smoke ] && argument="$MSIME_RESOURCES"
  [ "$name" = windows-installer-launch ] && argument="$MSIME_INSTALLER"
  [ "$name" = windows-stroke-keys ] && argument="$MSIME_STROKE_FIXTURE"
  [ "$name" = windows-zhuyin-keys ] && argument="$MSIME_ZHUYIN_FIXTURE"
  [ "$name" = msime-tsf-paired-punctuation-wiring-test ] && argument="$MSIME_TSF_SOURCE"
  [ "$name" = msime-tsf-smart-punctuation-focus-wiring-test ] && argument="$MSIME_TSF_SOURCE"
  [ "$name" = msime-tsf-japanese-conversion-wiring-test ] && argument="$MSIME_TSF_SOURCE"
  [ "$name" = msime-tsf-gloss-column-wiring-test ] && argument="$MSIME_TSF_SOURCE"
  if timeout 120 xvfb-run -a wine "/run/t/$name.exe" $argument >/dev/null 2>&1; then
    echo "PASS $name"
  else
    echo "FAIL $name"
  fi
done'
