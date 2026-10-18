#!/bin/bash
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/../../.." && pwd)
# With an output path the harness is only built there, which is how CMake prebuilds it for ctest; without one it is built to a temporary file and run.
if [[ $# -gt 0 ]]; then
  out=$1
else
  out=$(mktemp "${TMPDIR:-/tmp}/msime-backend-account-tests.XXXXXX")
  trap 'rm -f "$out"' EXIT
fi
sources=()
# find rather than globstar: /bin/bash on macOS is 3.2, which has no globstar, so the recursive pattern silently matched nothing there.
while IFS= read -r source; do
  if [[ ${source##*/} != Package.swift ]]; then sources+=("$source"); fi
done < <(find "$root/shared/backend/account" "$root/shared/backend/clients" "$root/shared/backend/content" "$root/shared/backend/storage" "$root/shared/backend-ui" "$root/platforms/macos/src/backend" -type f -name '*.swift' -print | sort)
swift_target=${MSIME_SWIFT_TARGET:-$(uname -m)-apple-macosx${MACOSX_DEPLOYMENT_TARGET:-13.0}}
# Weak like the backend dylib (build_backend_swift.sh): Translation.framework first ships with macOS 15, and a strong link would stop the harness from loading on an older runner.
# `-I shared/backend/rust` 与后端 dylib 相同（build_backend_swift.sh），只为 C 头文件的类型，不链接 Rust；Rust 的入口是 FakeRustTable.swift 里的替身函数表。
xcrun swiftc -parse-as-library -emit-executable -target "$swift_target" -Xlinker -weak_framework -Xlinker Translation \
  -I "$root/shared/backend/rust" \
  -o "$out" "${sources[@]}" "$root/platforms/macos/tests/core/FakeRustTable.swift" "$root/platforms/macos/tests/core/BackendAccountTests.swift"
if [[ $# -eq 0 ]]; then "$out"; fi
