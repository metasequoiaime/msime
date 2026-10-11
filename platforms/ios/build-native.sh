#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
variant=${1:-device}
case "$variant" in
  device) rust_target=aarch64-apple-ios; sdk=iphoneos ;;
  simulator) rust_target=aarch64-apple-ios-sim; sdk=iphonesimulator ;;
  *) echo "Usage: $0 [device|simulator]" >&2; exit 2 ;;
esac

if ! grep -qx "$rust_target" <<< "$(rustup target list --installed)"; then
  echo "Install Rust target first: rustup target add $rust_target" >&2
  exit 1
fi
linker=$(xcrun --sdk "$sdk" --find clang)
deployment_target=${IPHONEOS_DEPLOYMENT_TARGET:-16.0}
target_dir="$repo_root/target/ios-cargo/$variant"
output_dir="$repo_root/target/ios/$variant"
mkdir -p "$output_dir"
linker_var="CARGO_TARGET_$(printf '%s' "$rust_target" | tr '[:lower:]-' '[:upper:]_')_LINKER"

env "$linker_var=$linker" \
  IPHONEOS_DEPLOYMENT_TARGET="$deployment_target" \
  CARGO_TARGET_DIR="$target_dir" \
  cargo build -p msime-host-api --target "$rust_target" --release --locked
cp "$target_dir/$rust_target/release/libmsime_host_api.a" "$output_dir/libmsime_host_api.a"
echo "iOS host library built: $output_dir/libmsime_host_api.a"
