#!/usr/bin/env bash
# 在受控的单核容器里验证输入线程的负载降级。
#
# 普通测试检查慢按键会暂缓可选模型；第二步再用一个持续占满 CPU 的进程，
# 让 Linux cgroup CPU pressure 探针走真实的系统负载分支。基础候选不需要网络或
# 目标桌面，因此使用 Linux 构建门禁的固定镜像即可。
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo_root"

command -v docker >/dev/null 2>&1 || {
  echo "docker is required" >&2
  exit 2
}

build_root="$repo_root/target/linux-build-gate"
mkdir -p "$build_root"
image_tag="msime-input-load:$(printf %s "$repo_root" | shasum | cut -c1-12)"

docker build -q -t "$image_tag" \
  -f platforms/linux/tests/tools/Dockerfile.build-gate platforms/linux/tests >/dev/null

docker run --rm --init --cpus=1 \
  -v "$repo_root":/source \
  -v "$build_root":/build \
  -w /source \
  -e CARGO_TARGET_DIR=/build/input-load-cargo \
  -e CARGO_HOME=/build/input-load-cargo-home \
  "$image_tag" bash -euo pipefail -c '
    cargo test --locked -p msime-input-runtime load_shedding -- --nocapture

    load_pids=()
    for _ in $(seq 1 8); do
      yes >/dev/null &
      load_pids+=("$!")
    done
    cleanup_load() { kill "${load_pids[@]}" 2>/dev/null || true; }
    trap cleanup_load EXIT
    cargo test --locked -p msime-input-runtime linux_external_pressure_probe -- --ignored --nocapture
  '
