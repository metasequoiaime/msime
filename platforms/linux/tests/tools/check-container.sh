#!/usr/bin/env bash
set -euo pipefail
# Four levels: this script sits in platforms/linux/tests/tools. It said three
# when it lived one directory up, and nothing has been able to run it since that
# move - docker build was handed platforms/platforms/linux/tests as its context.
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
[[ ( $# == 1 || ( $# == 2 && ( $2 == --ibus-1.5.32 || $2 == --fcitx5 ) ) ) && -d "$1" ]] || {
  echo "usage: check-container.sh <locked-resource-directory> [--ibus-1.5.32|--fcitx5]" >&2
  exit 2
}
resource_dir=$(cd "$1" && pwd)
mkdir -p "$repo_root/target/linux"
# Tag per checkout, as build-container.sh does: with a fixed tag, concurrent worktrees overwrite each other's image and a run can silently test another checkout's Dockerfile.
tag=$(printf %s "$repo_root" | shasum | cut -c1-12)
base_image=msime-linux-test:$tag
docker build -t "$base_image" -f "$repo_root/platforms/linux/tests/tools/Dockerfile" "$repo_root/platforms/linux/tests"
test_image=$base_image
if [[ ${2:-} == --ibus-1.5.32 ]]; then
  test_image=msime-linux-ibus132-test:$tag
  docker build -t "$test_image" --build-arg BASE_IMAGE="$base_image" -f "$repo_root/platforms/linux/tests/tools/Dockerfile.ibus-1.5.32" "$repo_root/platforms/linux/tests"
fi
if [[ ${2:-} == --fcitx5 ]]; then
  test_image=msime-linux-fcitx5-test:$tag
  docker build -t "$test_image" --build-arg BASE_IMAGE="$base_image" -f "$repo_root/platforms/linux/tests/tools/Dockerfile.fcitx5" "$repo_root/platforms/linux/tests"
fi
docker run --rm --init \
  -v "$repo_root:/source:ro" -v "$repo_root/target/linux:/build" -v "$resource_dir:/resources:ro" \
  -e CARGO_TARGET_DIR=/build/cargo -e CARGO_HOME=/build/cargo-home -e CARGO_BUILD_JOBS=4 \
  -e MSIME_ISOLATED_LINUX_TEST=1 \
  -e MSIME_SKIP_RUST_TESTS="${MSIME_SKIP_RUST_TESTS:-0}" \
  -e MSIME_TEST_FCITX5=$( [[ ${2:-} == --fcitx5 ]] && echo 1 || echo 0 ) \
  "$test_image" bash platforms/linux/tests/tools/in-container.sh
