#!/usr/bin/env bash
# Local verification for the shared 水杉输入法 client.
#
# This is the half of verification GitHub Actions does not cover, and the fast feedback before a commit. Nothing here talks to CI; it runs the Rust workspace tests, fmt, clippy and a dependency audit, the UI type check, and the native host builds and tests, then reports the result.
#
# The point of this script is the baseline. Several suites have long-standing
# failures, so a bare pass/fail number says nothing: the only question that
# matters before a merge is "did this change break something that worked?".
# Every phase therefore compares the set of failing test *names* against
# scripts/known-failures.txt and fails only on names that are not in it.
#
# Six compile breaks reached develop in one session because branches were
# merged without building the merge result. `--quick` exists for exactly that:
# it runs only the compile phases, which caught every one of those six.
#
# Usage:
#   scripts/verify-local.sh --quick   compile only, the pre-merge gate
#   scripts/verify-local.sh           everything
#   scripts/verify-local.sh --update-baseline   append newly observed failures to known-failures.txt
#
# --update-baseline appends what one run observed and removes nothing. Several
# desktop tests are flaky, so a single run under-reports: the committed baseline
# is the union of several, and entries are removed by hand as they are fixed.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
baseline="scripts/known-failures.txt"
quick=0
update=0
for argument in "$@"; do
  case "$argument" in
    --quick) quick=1 ;;
    --update-baseline) update=1 ;;
    *) echo "unknown option: $argument" >&2; exit 2 ;;
  esac
done

# vcpkg supplies curl, fmt, nlohmann-json and utfcpp, the native dependencies the Windows host links. Derived from VCPKG_ROOT when that is set; export MSIME_VCPKG_PREFIX directly for an installed tree somewhere else. Without one of the two the Windows CMake build fails in a way that looks like a code error but is not, so the Windows branch below says so out loud rather than leaving it to be guessed from a compiler message.
: "${MSIME_VCPKG_PREFIX:=${VCPKG_ROOT:+$VCPKG_ROOT/installed/x64-windows-static-md}}"
: "${MSIME_NATIVE_BUILD:=target/win-full}"
# The pipe-only configuration builds the protocol tests without the Rust host
# library. It is a separate CMake configuration, so nothing in the ordinary
# build covers it - and a configuration nobody runs is one that rots.
: "${MSIME_PIPE_BUILD:=target/windows-pipe}"
# platforms/macos was covered by nothing. It stopped compiling at some point and nobody found out, and the 103 tests behind that break had never reported at all. Configured directories only: the build needs a pinned Sparkle, so a machine without it skips this the way it already skips the Windows phases.
: "${MSIME_MACOS_BUILD:=target/macos-isolated}"
# The Foundation-only part of shared/apple-bridge. It costs two translation units and no
# dependency at all, so unlike the phase above it configures itself: the bridges shared with
# the Apple client are the ones a broken merge silently takes out of both the iOS keyboard
# and the macOS host at once.
: "${MSIME_APPLE_BRIDGE_BUILD:=target/apple-bridge}"
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'

windows_host=0
case "$(uname -s 2>/dev/null)" in MINGW*|MSYS*|CYGWIN*) windows_host=1 ;; esac

# The Windows compile gate can also run off a Windows host, through the MinGW
# cross build. It needs the compilers and a vcpkg already bootstrapped at the
# manifest baseline; bootstrapping one is a long download, so this only adopts
# a tree that is already there rather than creating one mid-verification.
#
# The main worktree's tree counts too. This repository is developed in many
# short-lived worktrees, each with its own target/, so looking only beside this
# checkout would leave the gate skipping in every one of them - which is the
# failure this gate exists to stop, one directory removed.
cross_vcpkg=""
if [ "$windows_host" -eq 0 ] && command -v x86_64-w64-mingw32-g++ >/dev/null 2>&1; then
  main_worktree="$(dirname "$(git rev-parse --git-common-dir 2>/dev/null || echo .)")"
  for candidate in "${MSIME_VCPKG_ROOT:-}" "$root/target/tooling/vcpkg" \
    "$main_worktree/target/tooling/vcpkg"; do
    [ -n "$candidate" ] && [ -x "$candidate/vcpkg" ] && cross_vcpkg="$candidate" && break
  done
fi
apple_host=0
case "$(uname -s 2>/dev/null)" in Darwin) apple_host=1 ;; esac

# Only the Windows host gets its CMake search path from vcpkg. This used to be exported unconditionally from a hardcoded prefix, which meant a macOS run overwrote the CMAKE_PREFIX_PATH the README asks for - `$(brew --prefix)`, the one thing that lets CMake find the Homebrew libraries there - with a path that does not exist on the machine.
if [ "$windows_host" -eq 1 ]; then
  if [ -n "$MSIME_VCPKG_PREFIX" ]; then
    export CMAKE_PREFIX_PATH="$MSIME_VCPKG_PREFIX"
    export CXXFLAGS="-I$MSIME_VCPKG_PREFIX/include"
  else
    echo "note: neither MSIME_VCPKG_PREFIX nor VCPKG_ROOT is set; the Windows build will not find its vcpkg dependencies"
  fi
fi

failed=0
new_failures=""
observed_wine="$(mktemp)"
collected="$(mktemp)"
# One EXIT trap for every temp file and whichever build lock this run holds; the lock sections set and clear held_lock instead of replacing the trap.
held_lock=""
# shellcheck disable=SC2317  # reached through the trap
cleanup() {
  local file
  for file in "$collected" "$collected".* "$observed_wine" "${harmony_log:-}" "${cross_log:-}" "${wine_log:-}" "${android_host_log:-}"; do
    if [ -n "$file" ]; then rm -f -- "$file"; fi
  done
  if [ -n "$held_lock" ]; then rmdir "$held_lock" 2>/dev/null; fi
  return 0
}
trap cleanup EXIT
note() { printf '\n=== %s ===\n' "$1"; }
fail() { echo "FAIL: $1"; failed=1; }

# Compare this phase's failing test names against the baseline. Anything not
# already known is a regression this change introduced.
compare() {
  local phase="$1" observed="$2"
  local unexpected
  unexpected="$(comm -23 <(grep -v '^ *$' "$observed" | sort -u)                         <(grep -vE '^ *(#|$)' "$baseline" | sort -u) || true)"
  if [ -n "$unexpected" ]; then
    fail "$phase has failures that are not in the baseline:"
    printf '  %s\n' "$unexpected"
    new_failures="$new_failures$unexpected"$'\n'
  else
    echo "$phase: at baseline"
  fi
}

# The special checks below run inline here because they need this script's locks or toolchains; scripts/run-checks.sh names them in special_checks, and its registry phase fails when this file stops invoking them. Everything else under scripts/test-*.py is discovered and run by that script, sourced further down.

# The 32-bit TSF DLL is loaded into every 32-bit host application, and nothing
# built that architecture: build-cross.sh x86 needs a DWARF-unwinding MinGW for
# the Rust side and the common macOS toolchain is SJLJ. This re-checks the same
# sources with the same flags under the i686 compiler, which needs no 32-bit
# libraries because it never links.
note "windows x86 syntax"
# Under the same lock as the cross build below, and for the same reason: this reads the compile
# flags that build produced, and they point into the vcpkg prefix every worktree on this machine
# shares. Another worktree reinstalling the manifest mid-read takes curl and nlohmann out from under
# it, which arrives as "no such file" on a source that compiled a minute earlier and will compile a
# minute later. Seen twice in one afternoon; both times the retry passed, which is exactly the shape
# of failure that teaches people to ignore a gate.
x86_lock=""
if [ -n "$cross_vcpkg" ]; then
  x86_deps="$(dirname "$cross_vcpkg")/windows-native-deps"
  x86_lock="$x86_deps/.verify-cross-build.lock"
  mkdir -p "$x86_deps" 2>/dev/null
  if ! mkdir "$x86_lock" 2>/dev/null; then
    echo "windows x86 syntax: skipped (another run holds $x86_lock)"
    x86_lock="skipped"
  else
    held_lock="$x86_lock"
  fi
fi
if [ "$x86_lock" != "skipped" ]; then
  python3 scripts/test-windows-32bit-compile.py || fail "windows x86 syntax"
  [ -n "$x86_lock" ] && rmdir "$x86_lock" 2>/dev/null
  held_lock=""
fi

# Most of the Windows tests are policy with no Win32 call in the translation
# unit. Without Windows and without Docker they had one level of evidence - the
# cross build linked them - and linking does not catch an assertion. Same
# sources, host compiler, actually executed.
note "windows tests on this host"
python3 scripts/test-windows-native-run.py || fail "windows tests on this host"

# The HarmonyOS settings window is a WebView over a generated bundle that is
# committed to the repository and that nothing rebuilds. It drifted for
# fifty-two commits of shared UI before anyone looked, and a stale bundle is a
# working bundle: the window renders, it simply renders last month's UI.
note "harmony settings bundle"
python3 scripts/test-harmony-settings-bundle.py || fail "harmony settings bundle"

# The contract checks themselves: the registry phase and every other scripts/test-*.py, discovered by file name. Sourced rather than run so the output and this script's failure count stay as they were; the contracts workflow runs the same file on its own. It has to stay after the three inline special phases just above.
# shellcheck source=scripts/run-checks.sh
. scripts/run-checks.sh

note "compile: rust workspace"
# The desktop app's Tauri config lists the platform IME bundle as a packaged
# resource, and Tauri's build script fails when a listed resource is absent. On
# a checkout that has not built the native host yet that is not a code error, so
# it is skipped rather than reported as a broken workspace - the same treatment
# the native phases below already get.
#
# The name has to be the bundle tauri.macos.conf.json lists, not the CMake
# target that produces it: platforms/macos names the target
# MSIMEClientInputMethod and then sets OUTPUT_NAME to 水杉输入法, so the
# guard below matched on no machine and the desktop crate was excluded from
# every run anyone has made. Three compile errors reached develop behind that.
desktop_resource="target/macos/水杉输入法.app"
# The bundle is not the only resource tauri.macos.conf.json points at. A checkout with the input method
# built but the dictionary release not staged has half of them, and the build script fails on the missing
# half rather than skipping - which reads as a broken crate instead of an unprepared checkout.
desktop_companion="target/macos/EngineResources"
[ "$windows_host" -eq 1 ] && { desktop_resource="target/win-full"; desktop_companion="target/win-full"; }
if [ -e "$desktop_resource" ] && [ -e "$desktop_companion" ]; then
  cargo check --workspace --all-targets 2>&1 | tail -3
  [ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo check"
else
  missing="$desktop_resource"
  [ -e "$desktop_resource" ] && missing="$desktop_companion"
  echo "msime-desktop: skipped ($missing not built yet)"
  cargo check --workspace --all-targets --exclude msime-desktop 2>&1 | tail -3
  [ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo check"
fi

note "compile: android target"
# cargo check --workspace above only ever sees the host target, so every
# `#[cfg(target_os = "android")]` branch in msime-desktop is invisible to it.
# Ten compile errors accumulated behind that and only surfaced when someone
# tried to build the APK: a duplicated block, a moved value, a missing match
# arm, three private types the generated command handler could not name, and
# imports gated for the wrong targets. Checking the target here is what makes
# the next one fail in a minute instead of at packaging time.
#
# Skipped rather than required: it needs the pinned NDK and the Rust Android target, the same way the native phases below skip when unconfigured.
# The SDK's default location counts as configuration too. Neither ANDROID_SDK_ROOT nor ANDROID_HOME is set by the Android Studio installer on macOS, so a machine with the pinned NDK and the Rust target still skipped this phase - which reads as "not available here" when everything it needs is sitting in the standard directory.
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
if [ -z "$android_sdk" ] && [ -d "$HOME/Library/Android/sdk" ]; then
  android_sdk="$HOME/Library/Android/sdk"
fi
android_ndk=${MSIME_ANDROID_NDK:-$android_sdk/ndk/28.2.13676358}
case $(uname -s) in
  Darwin) android_host_tag=darwin-x86_64 ;;
  Linux) android_host_tag=linux-x86_64 ;;
  *) android_host_tag="" ;;
esac
android_clang="$android_ndk/toolchains/llvm/prebuilt/$android_host_tag/bin/aarch64-linux-android28-clang"
if [ -n "$android_host_tag" ] && [ -x "$android_clang" ] \
  && rustup target list --installed 2>/dev/null | grep -q '^aarch64-linux-android$'; then
  env "CC_aarch64_linux_android=$android_clang" \
    "CXX_aarch64_linux_android=${android_clang}++" \
    "AR_aarch64_linux_android=$android_ndk/toolchains/llvm/prebuilt/$android_host_tag/bin/llvm-ar" \
    "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$android_clang" \
    "ANDROID_NDK_HOME=$android_ndk" "MSIME_ANDROID_NDK=$android_ndk" \
    cargo check -p msime-desktop --target aarch64-linux-android --lib --locked 2>&1 | tail -3
  [ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo check --target aarch64-linux-android"
else
  echo "skipped: pinned NDK or aarch64-linux-android target not present"
fi

note "compile: wasm target"
# 网页内置输入法的引擎（msime-engine-wasm）只在 wasm32-unknown-unknown 上编译 `bindings.rs`，上面主机目标的 `cargo check --workspace` 看不到它，clippy 也只有在这里才会检查到它。和 android 一样，缺目标或缺工具链时跳过而不是失败：需要 Rust 的 wasm32-unknown-unknown 目标，以及能编 wasm 的 LLVM clang 和 llvm-ar（SQLite 是 C 代码；Apple 的 ar 会产出空的 libwsqlite3.a，Apple clang 不认 wasm32）。
wasm_cc="${CC_wasm32_unknown_unknown:-}"
wasm_ar="${AR_wasm32_unknown_unknown:-}"
if [ -z "$wasm_cc" ] || [ -z "$wasm_ar" ]; then
  for wasm_prefix in /opt/homebrew/opt/llvm/bin /usr/local/opt/llvm/bin; do
    if [ -x "$wasm_prefix/clang" ] && [ -x "$wasm_prefix/llvm-ar" ]; then
      wasm_cc="${wasm_cc:-$wasm_prefix/clang}"
      wasm_ar="${wasm_ar:-$wasm_prefix/llvm-ar}"
      break
    fi
  done
fi
[ -n "$wasm_cc" ] || wasm_cc="$(command -v clang || true)"
[ -n "$wasm_ar" ] || wasm_ar="$(command -v llvm-ar || true)"
if [ -n "$wasm_cc" ] && [ -n "$wasm_ar" ] \
  && printf 'int x;\n' | "$wasm_cc" --target=wasm32-unknown-unknown -x c -c -o /dev/null - >/dev/null 2>&1 \
  && rustup target list --installed 2>/dev/null | grep -q '^wasm32-unknown-unknown$'; then
  env "CC_wasm32_unknown_unknown=$wasm_cc" "AR_wasm32_unknown_unknown=$wasm_ar" \
    cargo check --locked -p msime-engine-wasm --target wasm32-unknown-unknown 2>&1 | tail -3
  [ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo check --target wasm32-unknown-unknown"
  env "CC_wasm32_unknown_unknown=$wasm_cc" "AR_wasm32_unknown_unknown=$wasm_ar" \
    cargo clippy --locked -p msime-engine-wasm --target wasm32-unknown-unknown -- -D warnings 2>&1 | tail -3
  [ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo clippy --target wasm32-unknown-unknown"
else
  echo "skipped: wasm32-unknown-unknown target or an LLVM clang/llvm-ar that can build wasm not present"
fi

# The Java half. `cargo check` above compiles the Rust the service calls into and says nothing about
# the service itself, which is where most of this host lives: the input method, its keyboard, its
# panels and two dozen policy classes with their own smoke tests. check-host.sh compiles all of it
# against the SDK's android.jar and runs those tests, and it needs no device.
note "android host java"
if [ -n "$android_sdk" ] && [ -d "$android_sdk/platforms" ]; then
  android_host_log="$(mktemp)"
  if ANDROID_SDK_ROOT="$android_sdk" bash platforms/android/check-host.sh >"$android_host_log" 2>&1; then
    echo "android host java: service, policies and smoke tests compile and pass"
  else
    tail -20 "$android_host_log"
    fail "android host java"
  fi
  rm -f "$android_host_log"
else
  echo "skipped: no Android SDK platforms directory"
fi

# ArkTS is a strict subset of TypeScript, and the only thing that fully decides what it accepts is
# its own compiler. `test-harmony-arkts-subset.py` catches the two classes a text scan can catch
# without false positives; the third - a nested object literal inside a call it otherwise accepts -
# needs types. develop has sat in a state where no HAP could be built without anyone knowing,
# because nothing here ran that compiler. This runs it when its inputs are present: the native
# libraries and the staged engine resources, both of which are build products, and the DevEco
# command line tools. Seven seconds once they are.
note "harmony arkts compile"
if [ -n "${MSIME_HVIGOR:-}" ]; then
  harmony_hvigor="$MSIME_HVIGOR"
else
  harmony_hvigor=""
  for candidate in \
    "$HOME/command-line-tools/bin/hvigorw" \
    "/Applications/DevEco-Studio.app/Contents/tools/hvigor/bin/hvigorw"; do
    if [ -x "$candidate" ]; then
      harmony_hvigor="$candidate"
      break
    fi
  done
fi
if [ -x "$harmony_hvigor" ] && [ -d "$root/platforms/harmony/entry/libs" ] \
  && [ -d "$root/platforms/harmony/entry/src/main/resources/resfile/engine" ] \
  && [ -d "$root/platforms/harmony/oh_modules" ]; then
  harmony_log="$(mktemp)"
  if (cd "$root/platforms/harmony" && PATH="$(dirname "$harmony_hvigor"):$PATH" \
      "$harmony_hvigor" assembleHap --no-daemon >"$harmony_log" 2>&1); then
    echo "harmony arkts compile: the HAP builds"
  else
    grep -E "ERROR|error:" "$harmony_log" | head -5
    rm -f "$harmony_log"
    fail "harmony arkts compile"
  fi
  rm -f "$harmony_log"
else
  echo "skipped: hvigorw, staged native libraries, staged resources or oh_modules not present"
  echo "  see platforms/harmony/README.md for the one-time setup"
fi

note "compile: linux desktop shell"
# Same hole the android phase above exists to close, for the other target nobody
# here compiles. `cargo check --workspace` sees one target, and the macOS run
# excludes msime-desktop outright because the bundle it lists as a resource is
# not built - so every `#[cfg(target_os = "linux")]` branch in the Tauri shell
# was compiled by nothing at all. Thirty-six errors had collected behind that:
# the refactor that moved panel delivery out of the crate root left the crate
# root calling the moved functions unqualified, clipboard_history lost the
# module and the Mutex it names, four label arguments went through a method
# unstable on this toolchain, and a Vec needed its element type. The Linux
# settings window, every shared panel and the account surface could not be built
# at all, which is a more complete outage than any single feature gap.
#
# In a container, because the Linux shell needs webkit2gtk, gtk3 and libsoup and
# a macOS machine has none of them. Skipped rather than required, like the native
# phases: no Docker means no gate, and the command is printed so the next person
# can run it. The build tree is kept out of target/debug - the container's
# aarch64-unknown-linux-gnu host build would otherwise share that directory with
# the host's own and the two would rebuild each other on every run.
#
# The build dependencies live in an image (platforms/linux/tests/tools/Dockerfile.desktop-check) rather than being installed with apt in a throwaway container: that reinstall of the whole webkit2gtk closure ran on every --quick and so on every push, and it is the part of this phase that does not change. The image is tagged per checkout the same way platforms/linux/build-container.sh tags its gate image, so concurrent worktrees never run each other's Dockerfile; the README says how to prune the tags old worktrees leave behind.
linux_desktop_note="image=msime-linux-desktop-check:\$(printf %s \"\$PWD\" | shasum | cut -c1-12); docker build -t \"\$image\" -f platforms/linux/tests/tools/Dockerfile.desktop-check platforms/linux/tests && docker run --rm -v \"\$PWD\":/source -w /source \"\$image\" cargo check -p msime-desktop --locked --all-targets"
if [ "$(uname -s 2>/dev/null)" = "Linux" ]; then
  cargo check -p msime-desktop --locked --all-targets 2>&1 | tail -3
  [ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo check -p msime-desktop (linux)"
elif command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  mkdir -p "$root/target/linux-desktop-check"
  linux_desktop_image="msime-linux-desktop-check:$(printf %s "$root" | shasum | cut -c1-12)"
  # The build log is kept rather than discarded, so an apt failure shows apt's own message instead of only an exit code; once the image is cached the build is a few lines of CACHED.
  if docker build -t "$linux_desktop_image" \
    -f platforms/linux/tests/tools/Dockerfile.desktop-check platforms/linux/tests \
    > "$root/target/linux-desktop-check/image.log" 2>&1; then
    docker run --rm \
      -v "$root":/source \
      -v "$root/target/linux-desktop-check":/ctarget \
      -w /source \
      -e CARGO_TARGET_DIR=/ctarget \
      "$linux_desktop_image" \
      cargo check -p msime-desktop --locked --all-targets --message-format short \
      > "$root/target/linux-desktop-check/check.log" 2>&1
    status=$?
    grep -E ': error' "$root/target/linux-desktop-check/check.log" | head -5
    [ "$status" -eq 0 ] || fail "cargo check -p msime-desktop (linux container)"
    tail -1 "$root/target/linux-desktop-check/check.log"
  else
    tail -20 "$root/target/linux-desktop-check/image.log"
    fail "docker build $linux_desktop_image (linux desktop shell)"
  fi
else
  echo "skipped: no docker available for the linux desktop shell check"
  echo "  run $linux_desktop_note"
fi

note "compile: linux native host"
# The product on Linux is platforms/linux - the IBus engine, the Fcitx5 addon and
# the provider entry points - and no phase here built any of it. It had stopped
# building: three tests carried relative includes one level short of where the
# sources moved, and one could not name its own fixtures, so the target would not
# even configure. The Windows equivalent of this hole cost six simultaneous
# breakages before anyone looked.
#
# build-container.sh does the work, in a container because a macOS machine has no
# IBus, Fcitx5 or XKB development packages. It compiles and runs the unit tests;
# tests/tools/check-container.sh remains the acceptance run that needs a verified
# dictionary directory and a live IBus daemon.
if [ "$(uname -s 2>/dev/null)" = "Linux" ] && pkg-config --exists ibus-1.0 2>/dev/null; then
  # CMake 链接的是 target/debug 下的 Host API，而上面的 Rust 阶段只做 cargo check，不产出它；
  # 与 build-container.sh 一样先构建，否则配置阶段就报「Build msime-host-api for Linux first」。
  cargo build -p msime-host-api --locked >/dev/null 2>&1 &&
    cmake -S platforms/linux -B "$root/target/linux-gate" -DMSIME_ENABLE_FCITX5=ON >/dev/null 2>&1 &&
    cmake --build "$root/target/linux-gate" >/dev/null 2>&1 &&
    ctest --test-dir "$root/target/linux-gate" --output-on-failure >/dev/null 2>&1 &&
    echo "linux native host: builds and its tests pass" ||
    { cmake --build "$root/target/linux-gate" 2>&1 | grep -Ei "error" | head -5
      fail "linux native host"; }
elif command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  if bash platforms/linux/build-container.sh > "$root/target/linux-native-gate.log" 2>&1; then
    grep -E "tests passed" "$root/target/linux-native-gate.log" | tail -1
  else
    grep -E "error|FAILED|Errors while" "$root/target/linux-native-gate.log" | head -5
    fail "linux native host (container)"
  fi
else
  echo "skipped: no docker available for the linux native host check"
  echo "  run platforms/linux/build-container.sh"
fi

note "compile: native host"
if [ -d "$MSIME_NATIVE_BUILD" ]; then
  # The native tests link the Rust library, so it has to be current or they
  # fail to start with an entry-point error that looks like a test failure.
  cargo build -p msime-host-api 2>&1 | tail -2
  [ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo build -p msime-host-api"
  for built in target/debug/msime_host_api.dll target/debug/libmsime_host_api.so; do
    [ -f "$built" ] && cp -f "$built" "$MSIME_NATIVE_BUILD/Debug/" 2>/dev/null
  done
  cmake --build "$MSIME_NATIVE_BUILD" --config Debug 2>&1 | grep -Ei "error C[0-9]|error LNK" | head -5
  cmake --build "$MSIME_NATIVE_BUILD" --config Debug >/dev/null 2>&1 || fail "native build"
elif [ -n "$cross_vcpkg" ]; then
  # Not a Windows host, but the MinGW toolchain and a bootstrapped vcpkg are
  # both here, so the Windows compile gate can run anyway.
  #
  # It is worth the minutes. This phase said "skipped" on every machine anyone
  # ran it on, and behind that the native build had been broken six separate
  # ways at once - a signature whose callers were never updated, a resource
  # header CMake pointed at a path that does not exist, twelve tests left a
  # directory level short by a move. None of it was subtle; nothing was looking.
  # Once, into a log: unlike the CMake phases above this one costs minutes even
  # incrementally, so it is not run twice to get both the message and the code.
  # Built dependencies live beside the vcpkg tree that produced them, so every worktree on this machine shares one rather than each rebuilding curl before it can compile anything of ours.
  cross_deps="$(dirname "$cross_vcpkg")/windows-native-deps"
  # One cross build per machine at a time. The vcpkg checkout and the built
  # dependencies are both shared, and this repository is worked in several
  # worktrees at once, so two runs otherwise overlap: one reinstalls the
  # manifest into the prefix the other is already compiling against, and the
  # second fails on a header that is present before and after. A gate that goes
  # red for reasons unrelated to the change is a gate people learn to pass with
  # --no-verify, so a run that cannot take the lock reports what it did - which
  # is nothing - rather than a failure.
  #
  # mkdir is the test-and-set: it is atomic on every filesystem this runs on,
  # unlike "test -e then create". The trap covers an interrupted run; a lock
  # left by a killed process is cleared by removing the directory it names,
  # which the message points at.
  cross_lock="$cross_deps/.verify-cross-build.lock"
  mkdir -p "$cross_deps" 2>/dev/null
  if ! mkdir "$cross_lock" 2>/dev/null; then
    echo "windows cross build: skipped (another run holds $cross_lock)"
  else
    cross_build_ok=0
    held_lock="$cross_lock"
    cross_log="$(mktemp)"
    if MSIME_VCPKG_ROOT="$cross_vcpkg" MSIME_WINDOWS_DEPS_ROOT="$cross_deps" \
      bash platforms/windows/build-cross.sh x64 >"$cross_log" 2>&1; then
      echo "windows cross build (x64): links"
      cross_build_ok=1
    elif grep -q "Failed to take the filesystem lock" "$cross_log"; then
      # vcpkg's own lock, taken by something that is not this gate.
      echo "windows cross build: skipped (vcpkg busy in another run)"
    else
      grep -Ei "error:|Error [0-9]|No rule to make target" "$cross_log" | head -5
      fail "windows cross build"
    fi
    rm -f "$cross_log"
    rmdir "$cross_lock" 2>/dev/null
    held_lock=""
  fi

  # The native CMake build does not compile the shared Tauri shell. Its Windows
  # branches use the same host-windows dependency prefix but a different Rust
  # target, so keep this check beside the cross build that prepares that prefix.
  # This catches visibility/import drift in the public settings and panel code
  # before a Windows packaging attempt, while still avoiding a second native
  # host build.
  if [ "${cross_build_ok:-0}" -eq 1 ]; then
    cargo check -p msime-desktop --target x86_64-pc-windows-gnu --lib --locked 2>&1 | tail -3
    [ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo check -p msime-desktop (windows GNU)"
    echo "msime-desktop (windows GNU): checks"
  fi
else
  echo "skipped: $MSIME_NATIVE_BUILD not configured, and no MinGW cross toolchain"
  echo "  run platforms/windows/build-cross.sh x64 once to enable this gate here"
fi

# Not in --quick: it emulates x86_64 on an arm64 host, so it costs minutes.
# It is the only thing here that runs the Windows suites rather than building
# them, which is why it is in the full run rather than nowhere.
if [ "$quick" -eq 0 ]; then
  note "windows suites under wine"
  wine_log="$(mktemp)"
  bash platforms/windows/run-tests-wine.sh x64 >"$wine_log" 2>&1
  if grep -q "^skipped:" "$wine_log"; then
    sed -n '1,2p' "$wine_log"
  else
    grep "^FAIL " "$wine_log" | sed 's/^FAIL /wine /' > "$observed_wine"
    echo "wine: $(grep -c '^PASS' "$wine_log") passed, $(grep -c '^FAIL ' "$wine_log") failed"
    compare "windows suites under wine" "$observed_wine"
  fi
  rm -f "$wine_log"
fi

# Configured, not merely present. A configure that fails part way - this one needs a pinned
# Sparkle, and says so with a FATAL_ERROR - still leaves the directory and its CMakeCache.txt
# behind, so testing for either turns a skip into two failing stages on a machine that never had
# the dependency. The generated build system is the thing that only a finished configure writes.
macos_configured() {
  [ -f "$MSIME_MACOS_BUILD/build.ninja" ] || [ -f "$MSIME_MACOS_BUILD/Makefile" ]
}

# Rebuild the Rust host library the configured macOS build actually links, which is MSIME_HOST_LIBRARY in its cache and not necessarily target/debug: platforms/macos/README.md configures target/macos-isolated against target/macos-cargo. Nothing else in this script builds that copy, so without this a host-api ABI change failed the macOS compile with undefined symbols until someone rebuilt it by hand. A library outside target/ is built the way the README builds it, with the same deployment flags, because the cc build scripts fingerprint CFLAGS and CXXFLAGS and a different set would rebuild the C objects there on every alternation between the two. The README's CMAKE_PREFIX_PATH is left out: nothing in msime-host-api's build graph reads it. One inside target/ is built without them, like every other cargo step here that writes there, for the same reason.
build_macos_host_library() {
  local library profile_dir cargo_dir profile
  library=$(sed -n 's/^MSIME_HOST_LIBRARY:[A-Z]*=//p' "$MSIME_MACOS_BUILD/CMakeCache.txt" 2>/dev/null | head -1)
  if [ -z "$library" ]; then
    echo "macos: $MSIME_MACOS_BUILD/CMakeCache.txt names no MSIME_HOST_LIBRARY"
    return 1
  fi
  profile_dir=$(dirname "$library")
  # CMake's default is platforms/macos/../../target/debug/..., so the directory is resolved rather than compared as text. It is created first because deleting a broken artefact directory and letting it rebuild is the documented repair (AGENTS.md), and that must not turn into a failed stage.
  cargo_dir=$(mkdir -p "$(dirname "$profile_dir")" 2>/dev/null && cd "$(dirname "$profile_dir")" && pwd -P) || {
    echo "macos: cannot create $(dirname "$profile_dir")"
    return 1
  }
  # Cargo writes the dev profile to debug/; every other profile to a directory of its own name.
  profile=$(basename "$profile_dir")
  [ "$profile" = debug ] && profile=dev
  if [ "$cargo_dir" = "$(cd target 2>/dev/null && pwd -P)" ]; then
    cargo build -p msime-host-api --locked --profile "$profile" 2>&1 | tail -2
  else
    CFLAGS="-mmacosx-version-min=13.0" CXXFLAGS="-mmacosx-version-min=13.0" CMAKE_OSX_DEPLOYMENT_TARGET=13.0 \
      CARGO_TARGET_DIR="$cargo_dir" cargo build -p msime-host-api --locked --profile "$profile" 2>&1 | tail -2
  fi
}

# The one dependency that configure refuses without. It was recorded as absent from this machine
# and it was not - the same wrong call as the container runtime, the Android SDK, Xcode and DevEco,
# and this is the target platform, so a skip here costs more than any of them. Look where a copy
# actually tends to be before concluding anything, and configure the build when one is found: the
# host library Cargo already built in this run is the other half, and both are cheap next to the
# platform going unbuilt.
macos_sparkle_root() {
  if [ -n "${MSIME_SPARKLE_ROOT:-}" ]; then
    printf '%s\n' "$MSIME_SPARKLE_ROOT"
    return 0
  fi
  for candidate in "$HOME/deps/Sparkle-2.9.6" "$HOME/msime-shared/sparkle"; do
    [ -d "$candidate/Sparkle.framework" ] && printf '%s\n' "$candidate" && return 0
  done
  return 1
}

if [ "$apple_host" -eq 1 ] && ! macos_configured; then
  if sparkle_root=$(macos_sparkle_root); then
    note "configure: macos"
    # The workspace stage above checks rather than builds, so the static library configure insists on may not exist yet even though everything needed to produce it does. This only has to make it exist; the compile stage below rebuilds whichever library the configured build links.
    [ -f target/debug/libmsime_host_api.a ] ||
      cargo build -p msime-host-api >/dev/null 2>&1 ||
      echo "macos: could not build msime-host-api"
    if cmake -S platforms/macos -B "$MSIME_MACOS_BUILD" \
      -DMSIME_SPARKLE_ROOT="$sparkle_root" -DCMAKE_BUILD_TYPE=Debug >/dev/null 2>&1; then
      echo "macos: configured against Sparkle at $sparkle_root"
    else
      echo "macos: configure failed against Sparkle at $sparkle_root, leaving the stage skipped"
    fi
  fi
fi

note "compile: macos"
if macos_configured; then
  if build_macos_host_library; then
    cmake --build "$MSIME_MACOS_BUILD" --parallel 2>&1 | grep -E "error:|symbol\(s\) not found" | head -5
    cmake --build "$MSIME_MACOS_BUILD" --parallel >/dev/null 2>&1 || fail "macos build"
  else
    # Linking against the stale copy would only report its missing symbols as a macOS break.
    fail "cargo build -p msime-host-api (macos host library)"
  fi
else
  echo "skipped: $MSIME_MACOS_BUILD not configured"
fi

note "compile: shared apple bridge"
if [ "$apple_host" -eq 0 ]; then
  echo "apple bridge: skipped (needs an Apple host)"
elif cmake -S shared/apple-bridge -B "$MSIME_APPLE_BRIDGE_BUILD" >/dev/null 2>&1 &&
  cmake --build "$MSIME_APPLE_BRIDGE_BUILD" --parallel >/dev/null 2>&1; then
  echo "apple bridge: builds"
else
  fail "apple bridge build"
  cmake --build "$MSIME_APPLE_BRIDGE_BUILD" --parallel 2>&1 |
    grep -E "error:|symbol\(s\) not found" | head -5
fi

note "compile: pipe-only configuration"
# Cheap: no Rust library, no vcpkg dependencies, just the protocol tests.
if cmake -S platforms/windows -B "$MSIME_PIPE_BUILD" -DMSIME_WINDOWS_PIPE_ONLY=ON      >/dev/null 2>&1; then
  if cmake --build "$MSIME_PIPE_BUILD" --config Debug >/dev/null 2>&1; then
    echo "pipe-only: builds"
  else
    fail "pipe-only build"
    cmake --build "$MSIME_PIPE_BUILD" --config Debug 2>&1 |
      grep -Ei "error C[0-9]|error LNK" | head -5
  fi
elif [ "$windows_host" -eq 0 ] && command -v x86_64-w64-mingw32-g++ >/dev/null 2>&1; then
  # It cannot configure against the *host* compiler off Windows, but it can
  # cross-configure, and this one needs nothing but the compiler - no Rust
  # library, no vcpkg. So the configuration the comment above calls "one
  # nobody runs" can actually be run nearly everywhere, rather than skipped
  # on every machine that is not Windows.
  # Both architectures the product ships a DLL for. 32-bit is not a formality
  # here: windows_ipc.h pins its frame sizes and field offsets with
  # static_assert, and those are exactly what a pointer-width change moves.
  # The full cross build cannot cover i686 with the MinGW commonly installed on
  # macOS (x86 Rust GNU needs DWARF unwinding and Homebrew's i686 MinGW is
  # SJLJ); platforms/windows/build-cross-container.sh does it in a container
  # whose toolchain has DWARF. This configuration links no Rust at all, so the
  # protocol gets checked here either way.
  for cross_arch in x86_64 i686; do
    command -v "$cross_arch-w64-mingw32-g++" >/dev/null 2>&1 || continue
    cross_dir="${MSIME_PIPE_BUILD}-cross-$cross_arch"
    if cmake -S platforms/windows -B "$cross_dir" -DMSIME_WINDOWS_PIPE_ONLY=ON \
      -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_CXX_COMPILER="$cross_arch-w64-mingw32-g++" \
      -DCMAKE_BUILD_TYPE=Debug >/dev/null 2>&1 &&
      cmake --build "$cross_dir" --parallel >/dev/null 2>&1; then
      echo "pipe-only: cross-builds ($cross_arch)"
    else
      cmake --build "$cross_dir" --parallel 2>&1 |
        grep -Ei "error:|Error [0-9]" | head -5
      fail "pipe-only cross build ($cross_arch)"
    fi
  done
elif [ "$windows_host" -eq 0 ]; then
  # platforms/windows cannot configure off Windows without a cross compiler,
  # and this phase had no guard for that while every other native phase does.
  # --quick is documented as the pre-merge gate, so an unconditional failure
  # here made that gate permanently red on macOS and Linux - which is a good
  # way to teach everyone to skip it.
  echo "pipe-only: skipped (needs a Windows host or a MinGW cross compiler)"
else
  fail "pipe-only configure"
fi

if [ "$quick" -eq 1 ]; then
  echo
  if [ "$failed" -eq 0 ]; then
    echo "quick check passed"
  else
    echo "quick check FAILED"
  fi
  exit "$failed"
fi

note "rust tests"
: > "$collected.rust"
# The neural sentence model tests (msime-engine's lattice::neural and settling tests, msime-input-runtime's settle test) skip without the shipped models, and neither model is in the repository. Fetch the pinned pair so they run here; the path is absolute because cargo runs each test binary from its own crate directory. A failed fetch is not a test failure: the tests skip and say why.
if python3 scripts/fetch_neural_model.py --out "$root/target/neural-model" > "$collected.neural-model" 2>&1; then
  export MSIME_NEURAL_MODEL_DIR="$root/target/neural-model"
else
  echo "neural model tests: skipped ($(tail -1 "$collected.neural-model"))"
fi
# msime-host-macos and msime-desktop were missing from this list, and a crate nobody tests is not the
# worst of it: cargo's exit status is not the verdict here (known failures make it non-zero), so a crate that does not build at
# all collects no failing names and is reported as being at baseline. msime-desktop did not link on macOS
# for that reason, and its 86 tests had never run. msime-engine and msime-tauri-mobile-platform
# were missing too, and nothing else runs their tests on the host target.
for package in msime-client-core msime-engine msime-engine-wasm msime-host-api msime-input-runtime msime-host-windows \
  msime-host-macos msime-mcp-server msime-tauri-mobile-platform msime-desktop; do
  # A package that does not build produces no failing test names, which reads as "at baseline" - which is
  # how msime-desktop went unbuildable on macOS without anything noticing. Say so instead.
  status=0 build_failed=0
  cargo test -p "$package" --no-fail-fast > "$collected.$package" 2>&1 || status=$?
  if grep -qE "^error: (could not compile|linking with)" "$collected.$package"; then
    grep -E "^error: (could not compile|linking with)" "$collected.$package" | head -1
    fail "$package build"
    build_failed=1
  fi
  # Names come only from the list under each binary's second `failures:` header (the first is followed
  # by unindented `---- name stdout ----` blocks), so top-level integration tests and doc-tests count too.
  names="$(awk '/^failures:$/ { list = 1; next } list && /^    / { sub(/^ +/, ""); print; next } { list = 0 }' \
    "$collected.$package")"
  if [ -n "$names" ]; then
    printf '%s\n' "$names" | sed "s#^#$package #" >> "$collected.rust"
  elif [ "$status" -ne 0 ] && [ "$build_failed" -eq 0 ]; then
    # A failure cargo named nowhere (a crashed test binary, a summary this parser missed) is still one.
    echo "$package <unnamed failure>" >> "$collected.rust"
  fi
done
compare "rust tests" "$collected.rust"

note "rust fmt (changed files only)"
# The whole tree is not clean, and reformatting it would collide with every
# other branch in flight. What matters is that this change does not add to it -
# so an unformatted file this change touched is a failure, not a printed note.
changed="$(git diff --name-only origin/develop...HEAD -- '*.rs' 2>/dev/null || true)"
if [ -n "$changed" ]; then
  unformatted=""
  for file in $changed; do
    [ -f "$file" ] || continue
    rustfmt --check --edition 2021 "$file" >/dev/null 2>&1 || unformatted="$unformatted  $file"$'\n'
  done
  if [ -n "$unformatted" ]; then
    fail "these changed files are not rustfmt-clean:"
    printf '%s' "$unformatted"
  else
    echo "changed Rust files: rustfmt-clean"
  fi
else
  echo "no Rust files changed against origin/develop"
fi

note "clippy: first-party crates"
# 文件头很长时间里承诺了 clippy 却哪里都没跑，唯一跑过它的是已经停用的 CI workflow。下面九个 crate 今天在 -D warnings 下都是干净的，所以这是没有基线的硬门槛：一旦失败，就是被测的改动引起的。
#
# The workspace as a whole is not gated: apps/desktop needs a built frontend
# before its Tauri build script will run, which makes "clippy failed" and
# "frontend not built" indistinguishable on a developer machine.
clippy_failed=""
for crate in msime-client-core msime-engine msime-engine-wasm msime-host-api msime-host-macos \
             msime-host-windows msime-input-runtime msime-mcp-server msime-tauri-mobile-platform; do
  cargo clippy -p "$crate" --all-targets -- -D warnings >/dev/null 2>&1 ||
    clippy_failed="$clippy_failed  $crate"$'\n'
done
if [ -n "$clippy_failed" ]; then
  fail "clippy -D warnings failed for:"
  printf '%s' "$clippy_failed"
else
  echo "first-party crates: clippy clean at -D warnings"
fi

note "frontend lint"
# The counterpart to the clippy gate above: until this existed, TypeScript had
# `tsc --noEmit` and nothing else, so an unused import or an unsafe optional
# chain reached develop unremarked. A hard gate with no baseline - the tree is
# clean today, and the handful of rules that do not apply to this codebase are
# turned off in vite.config.ts with the reason written beside each one.
#
# Skipped without node_modules, the same way the typescript phase is: on a
# checkout without them an empty result would otherwise look like a pass.
if [ -d node_modules/vite-plus ]; then
  if pnpm lint; then
    echo "frontend: lint clean"
  else
    fail "pnpm lint"
  fi
  # Oxfmt 0.68.0 is not idempotent: one pass over this tree leaves nine files it
  # would still change, and a second pass settles them. `pnpm format` therefore
  # has to be run twice to reach the state this check wants. Reported rather
  # than fixed here, because a gate that rewrites the tree is not a gate.
  if pnpm format:check; then
    echo "frontend: format clean"
  else
    fail "pnpm format:check (run pnpm format twice)"
  fi
else
  echo "vite-plus not installed; skipping (pnpm install)"
fi

note "dependency advisories"
# Lockfile-only, so it runs without building anything. A vulnerability is fatal;
# unmaintained and unsound advisories are accepted one at a time in
# .cargo/audit.toml, each with the chain that pulls it in and why - the same
# discipline as known-failures.txt, for the same reason.
#
# Not part of --quick: that gate has to work offline, and this fetches the
# advisory database.
if command -v cargo-audit >/dev/null 2>&1; then
  cargo audit 2>&1 | tail -3
  [ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo audit"
else
  echo "cargo-audit not installed; skipping (cargo install cargo-audit)"
fi

note "sentence conversion eval"
# Conversion quality had no number attached to it, so nothing could show that a lattice or ranking
# change helped. This compares against a committed baseline; resources/eval/README.md explains what
# each set can and cannot measure. Skipped where no verified dictionary is present, like the native
# phases - the directory is a 181 MB download this script must not require.
if [ -n "${MSIME_EVAL_RESOURCES:-}" ] && [ -d "${MSIME_EVAL_RESOURCES:-}" ]; then
  # `harvested` is the failure set: cases picked because the product gets them wrong, so its
  # top-1 is near zero by construction and its top-5 is the number that means something. It is
  # gated the same way regardless, because a regression moves it just as visibly.
  #
  # `neutral` is harvested the same way but against the engine alone, so no reranking model is
  # implicated in choosing the cases. That is what makes its top-1 comparable across models —
  # 0.622 for the shipped 4.25M weights against 0.805 for the 24.9M ones, disagreeing on 272 of
  # 1104 cases where the hand-written set produced three disagreements and McNemar p = 0.25.
  for set in sentences harvested neutral words; do
    case "$set" in
      sentences) args="--set resources/eval/sentences-v1.tsv" ;;
      harvested) args="--set resources/eval/sentences-v2.tsv" ;;
      neutral) args="--set resources/eval/sentences-neutral-v1.tsv" ;;
      words) args="--set resources/eval/quanpin-words-v1.tsv --limit 3000" ;;
    esac
    # shellcheck disable=SC2086
    if cargo run --release -q -p msime-input-runtime --example convert_eval --locked -- \
        --resources "$MSIME_EVAL_RESOURCES" $args \
        --baseline "resources/eval/baseline-$set.json" >/dev/null 2>&1; then
      echo "eval $set: at baseline"
    else
      fail "eval $set differs from resources/eval/baseline-$set.json"
      echo "  accept it with --update-baseline once you have read the diff"
    fi
  done
else
  echo "skipped: set MSIME_EVAL_RESOURCES to a verified dictionary directory to run the eval"
fi

note "reranker keystroke latency"
# convert_eval answers whether reranking ranks correctly. This answers what it costs, and the two move independently: a model swap, a wider lattice or a larger candidate page all change the number. The benchmark has existed since 418b4fb78 and nothing ever ran it, so the frame budget it checks was never actually enforced, and it was over it when this stage was added: p95 20.64ms against 16.00ms, 9.8% of keystrokes past a frame. Resuming candidate scoring across keystrokes in chinese-ime-lm brought that under the budget, so this now gates rather than records; with the Rust engine the last run was p95 9.50ms, with 2 of 1320 keystrokes (0.2%) over 16ms. The measurement is machine-dependent, which is why it is compared as a pass/fail name against known-failures.txt rather than as a committed millisecond figure. It needs the sentence model, which the resource lock ships, so the eval's own guard covers it too.
if [ -n "${MSIME_EVAL_RESOURCES:-}" ] && [ -d "${MSIME_EVAL_RESOURCES:-}" ]; then
  if [ -f "$MSIME_EVAL_RESOURCES/sentence-model.safetensors" ]; then
    : > "$collected".latency
    if ! cargo run --release -q -p msime-input-runtime --example rerank_latency --locked -- \
        --resources "$MSIME_EVAL_RESOURCES" --set resources/eval/sentences-v1.tsv \
        > "$collected".latency.log 2>&1; then
      echo "rerank-latency sentences" > "$collected".latency
    fi
    grep -E "with model|over the .*budget:" "$collected".latency.log | sed 's#^ *#  #'
    compare "reranker latency" "$collected".latency
  else
    echo "skipped: no sentence-model.safetensors in $MSIME_EVAL_RESOURCES"
  fi
else
  echo "skipped: set MSIME_EVAL_RESOURCES to a verified dictionary directory to run the latency gate"
fi

note "native tests"
if [ -d "$MSIME_NATIVE_BUILD" ]; then
  (cd "$MSIME_NATIVE_BUILD" && ctest -C Debug 2>&1) |
    grep -E "\*\*\*(Failed|Not Run|Timeout)" |
    sed 's/.*Test *#[0-9]*: *//' | sed 's/[. ]*\*\*\*.*//' | sed 's#^#native #' > "$collected.native" || true
  compare "native tests" "$collected.native"
else
  echo "skipped: $MSIME_NATIVE_BUILD not configured"
fi

note "macos tests"
if macos_configured; then
  # The per-test lines put the reason between the dots and the ***, so this reads
  # the summary block instead: "\t 52 - local-mode-preferences (Subprocess aborted)".
  ctest --test-dir "$MSIME_MACOS_BUILD" 2>&1 |
    sed -n '/The following tests FAILED:/,$p' |
    sed -n 's/^[[:space:]]*[0-9][0-9]* - \([^ ]*\).*/macos \1/p' > "$collected.macos" || true
  compare "macos tests" "$collected.macos"
else
  echo "skipped: $MSIME_MACOS_BUILD not configured"
fi

note "shared apple bridge tests"
if [ -d "$MSIME_APPLE_BRIDGE_BUILD" ]; then
  ctest --test-dir "$MSIME_APPLE_BRIDGE_BUILD" 2>&1 |
    grep -E "\*\*\*(Failed|Not Run|Timeout)" |
    sed 's/.*Test *#[0-9]*: *//' | sed 's/[. ]*\*\*\*.*//' |
    sed 's#^#apple-bridge #' > "$collected.apple_bridge" || true
  compare "shared apple bridge tests" "$collected.apple_bridge"
else
  echo "skipped: $MSIME_APPLE_BRIDGE_BUILD not configured"
fi

note "pipe-only tests"
if [ -d "$MSIME_PIPE_BUILD" ]; then
  ctest --test-dir "$MSIME_PIPE_BUILD" -C Debug 2>&1 |
    grep -E "\*\*\*(Failed|Not Run|Timeout)" |
    sed 's/.*Test *#[0-9]*: *//' | sed 's/[. ]*\*\*\*.*//' |
    sed 's#^#pipe #' > "$collected.pipe" || true
  compare "pipe-only tests" "$collected.pipe"
else
  echo "skipped: $MSIME_PIPE_BUILD not configured"
fi

note "typescript"
if [ -x apps/desktop/node_modules/.bin/tsc ]; then
  (cd apps/desktop && ./node_modules/.bin/tsc --noEmit -p tsconfig.json) || fail "tsc"
  # The project's own vitest, never npx: an npx copy resolves jsdom from its
  # own cache, fails to start every jsdom worker, and still reports success -
  # which hid this suite's real failures for an entire session.
  (cd apps/desktop && ./node_modules/.bin/vitest run --reporter=dot 2>&1) |
    grep -E "^ FAIL" | sed 's/^ FAIL  //;s/ [0-9]*ms$//' | sed 's#^#desktop #' > "$collected.ts" || true
  compare "typescript tests" "$collected.ts"
else
  echo "skipped: apps/desktop dependencies not installed"
fi

if [ "$update" -eq 1 ]; then
  # Append-only: phases skip by host (wine, macos, latency, typescript), so what was not observed here
  # is not evidence of a fix, and the comments beside each entry must stay where they are.
  added="$(cat "$collected".rust "$collected".native "$collected".macos "$collected".apple_bridge \
      "$collected".pipe "$collected".ts "$collected".latency "$observed_wine" 2>/dev/null |
    grep -v '^ *$' | sort -u | comm -23 - <(grep -vE '^ *(#|$)' "$baseline" | sort -u) || true)"
  if [ -n "$added" ]; then
    { printf '\n# Added by --update-baseline on %s\n' "$(date +%F)"; printf '%s\n' "$added"; } >> "$baseline"
  fi
  echo
  echo "baseline: $(printf '%s' "$added" | grep -c . || true) new entries appended"
  exit 0
fi

echo
if [ "$failed" -eq 0 ]; then
  echo "verification passed: no failures outside the baseline"
else
  echo "verification FAILED"
fi
exit "$failed"
