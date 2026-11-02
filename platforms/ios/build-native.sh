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

# 整句联想「增强」的逐键重排跑在 ONNX Runtime 上（host-api 的 `onnx-rerank` 特性，见 .agents/notes/implemented/architecture/2026-11-01-onnx-rerank-backend.md）：精简构建的静态库按 resources/onnxruntime-minimal.lock.json 下载核对，链进 libmsime_host_api.a。模型不在这里，由 stage-resources.sh 放到 EngineResources 旁边。
ort_location=$(python3 "$repo_root/scripts/fetch_onnxruntime.py" --target "$rust_target")
# ort-sys 把静态库复制进自己的输出目录，库换了也不会重新复制；按锁里库的 SHA-256 记一个戳，变了就让它重新复制。
ort_stamp="$target_dir/onnxruntime-library.sha256"
ort_library_sha=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["targets"][sys.argv[2]]["library_sha256"])' "$repo_root/resources/onnxruntime-minimal.lock.json" "$rust_target")
if [ -d "$target_dir" ] && [ "$(cat "$ort_stamp" 2>/dev/null)" != "$ort_library_sha" ]; then
  CARGO_TARGET_DIR="$target_dir" cargo clean -p ort-sys --target "$rust_target" --release
fi

env "$linker_var=$linker" \
  IPHONEOS_DEPLOYMENT_TARGET="$deployment_target" \
  CARGO_TARGET_DIR="$target_dir" \
  ORT_LIB_LOCATION="$ort_location" \
  cargo build -p msime-host-api --features onnx-rerank --target "$rust_target" --release --locked
mkdir -p "$target_dir"
printf '%s\n' "$ort_library_sha" > "$ort_stamp"
cp "$target_dir/$rust_target/release/libmsime_host_api.a" "$output_dir/libmsime_host_api.a"
echo "iOS host library built: $output_dir/libmsime_host_api.a"
