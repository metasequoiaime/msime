#!/usr/bin/env bash
# Run the cross-built Windows test executables under Wine, in a container.
#
# These suites were built by build-cross.sh and then never run: executing a
# Windows binary needs Windows, so every report about them said "linked".
# Linking does not catch an assertion. Wine runs 72 of them as they are, which
# is the difference between a suite that compiles and a suite that passes.
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

# The MinGW runtime is not bundled beside the executables, so collect it from
# the toolchain that produced them - which is not the same toolchain for both
# architectures. x64 is built by this host. x86 cannot be: the i686 MinGW
# usually installed on macOS uses SJLJ exceptions and Rust's target needs DWARF,
# so build-cross-container.sh builds it inside the cross image. Taking the x86
# runtime from this host would pair DWARF-built executables with an SJLJ
# unwinder, and the unwinder is exactly what differs.
runtime="$(mktemp -d)"
trap 'rm -rf "$runtime"' EXIT

if [ "$arch" = x86 ]; then
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

# The Rust host carries the Windows-only code the C++ suite never touches:
# clipboard reads and writes, synthetic key strokes, the extended-key set. Its
# tests build for the same target and run under the same Wine, but this runner
# only ever globbed C++ executables, so none of them ran here. Build them into a
# staging directory and let the loop below pick them up with the rest.
rust_triple=x86_64-pc-windows-gnu
[ "$arch" = x86 ] && rust_triple=i686-pc-windows-gnu
rust_stage="$root/target/wine-rust-tests/$arch"
rm -rf "$rust_stage"
mkdir -p "$rust_stage"
if command -v cargo >/dev/null 2>&1; then
  # --no-run builds the test binaries and prints where they landed; anything that
  # fails to build is reported and skipped rather than failing the whole run,
  # because the C++ suite below is still worth having.
  # host-api is the DLL the Server links against, so its FFI boundary is worth exercising on the target it ships for; it needs no vcpkg prefix, since its C parts build with the same MinGW toolchain.
  # client-core carries the shared logic plus a few #[cfg(windows)] paths - the
  # file-replacement retry in the gloss store among them - that the host run can
  # never reach, because on macOS and Linux the other branch is compiled.
  rust_packages="-p msime-host-windows -p msime-client-core -p msime-engine -p msime-host-api"
  # Filter on profile.test: --no-run also reports examples, which are ordinary
  # programs that expect arguments and would be counted as failures here.
  # --tests excludes examples, which are not tests and need not build for this
  # target.
  rust_build_log="$rust_stage/cargo.log"
  cargo test $rust_packages --target "$rust_triple" --no-run --tests \
    --message-format=json 2>"$rust_build_log" \
    | python3 -c 'import sys, json
for line in sys.stdin:
    try:
        message = json.loads(line)
    except ValueError:
        continue
    executable = message.get("executable")
    if executable and message.get("profile", {}).get("test"):
        print(executable)' \
    | while IFS= read -r exe; do
        [ -f "$exe" ] || continue
        cp "$exe" "$rust_stage/rust-$(basename "$exe" .exe | sed 's/-[0-9a-f]\{16\}$//').exe"
      done
  # Silence here would mean the Rust suites vanish without a word, which is how
  # the C++ side lost msimeui-tests for so long. Say so, and keep the log.
  if [ -z "$(ls -A "$rust_stage" 2>/dev/null | grep -v '^cargo\.log$')" ]; then
    echo "note: no Rust test binaries were staged; see $rust_build_log"
    grep -E '^error' "$rust_build_log" | head -3
  fi
else
  echo "skipped: cargo unavailable, the Rust host tests will not run under Wine"
fi

docker run --rm --platform linux/amd64 \
  -v "$build":/bin-win:ro -v "$runtime":/rt:ro -v "$rust_stage":/bin-rust:ro ${resources_mount[@]+"${resources_mount[@]}"} \
  ${installer_mount[@]+"${installer_mount[@]}"} \
  -e "MSIME_RESOURCES=$resources_argument" -e "MSIME_INSTALLER=$installer_argument" \
  -e LANG=C.utf8 -e LC_ALL=C.utf8 "$image" sh -c '
mkdir -p /run/t && cp /rt/*.dll /run/t/ && cp /bin-win/*.dll /run/t/ 2>/dev/null
cd /run/t
# msimeui puts its test executable in bin/ rather than beside the others, so a
# top-level pattern silently matched nothing and that suite was never run here.
for exe in /bin-win/windows-*.exe /bin-win/msime-tsf-*.exe /bin-win/msimeui-tests.exe \
           /bin-win/bin/msimeui-tests.exe /bin-rust/rust-*.exe; do
  [ -f "$exe" ] || continue
  name=$(basename "$exe" .exe)
  cp "$exe" /run/t/ 2>/dev/null || continue
  argument=""
  [ "$name" = windows-session-smoke ] && argument="$MSIME_RESOURCES"
  [ "$name" = windows-installer-launch ] && argument="$MSIME_INSTALLER"
  # That one walks the src/ directory of its own crate at run time to compare the
  # C header against the Rust exports. Only the executable is copied in here, so
  # it has nothing to walk, and it is a source-consistency check with no platform
  # dimension that the host run already covers.
  [ "$name" = rust-msime_host_api ] && argument="--skip the_c_header_and_the_rust_exports_agree"
  if timeout 120 xvfb-run -a wine "/run/t/$name.exe" $argument >/dev/null 2>&1; then
    echo "PASS $name"
  else
    echo "FAIL $name"
  fi
done'
