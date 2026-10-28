#!/usr/bin/env bash
# 固定工具链在 Docker 验证云候选和上屏后预测；联网探针只发送合成拼音 nihao。
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
image="msime-commercial-prediction:$(printf %s "$root" | shasum | cut -c1-12)"
docker build -q -f "$root/platforms/linux/tests/tools/Dockerfile.build-gate" -t "$image" "$root/platforms/linux/tests" >/dev/null
mkdir -p "$root/target/linux-build-gate"
docker run --rm --init --cpus=3 --memory=6g \
  -v "$root":/source -w /source \
  -e CARGO_TARGET_DIR=/source/target/linux-build-gate/cargo \
  -e CARGO_HOME=/source/target/linux-build-gate/cargo-home \
  -e CARGO_BUILD_JOBS=2 -e MSIME_COMMERCIAL_LIVE="${MSIME_COMMERCIAL_LIVE:-0}" \
  "$image" bash -euo pipefail -c '
    cargo test --locked -p msime-client-core cloud::candidates
    cargo test --locked -p msime-engine -p msime-input-runtime prediction
    cargo test --locked -p msime-host-api cloud_prediction
    cargo run --locked -q -p msime-input-runtime --example commercial_prediction_probe -- transaction
    if [ "$MSIME_COMMERCIAL_LIVE" = 1 ]; then
      cargo run --locked -q -p msime-input-runtime --example commercial_prediction_probe -- describe-baidu > /tmp/cloud-request.json
      python3 scripts/probe-commercial-cloud.py /tmp/cloud-request.json /tmp/cloud-response.json
      cargo run --locked -q -p msime-input-runtime --example commercial_prediction_probe -- parse-baidu < /tmp/cloud-response.json
    fi
  '
