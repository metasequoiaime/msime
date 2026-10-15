#!/usr/bin/env bash
# Manual portable test. Does not exercise Windows authentication or audio.
#
# Rust 那一半以前在这里用裸 `rustc --test` 编译一个不属于任何 Cargo 包的文件，经五层 `#[path]` 伸进 client-core 的私有源码。`cargo check --workspace` 和 clippy 门禁都看不到它，它也是树里唯一一个偏离了 rustfmt 的文件。现在它是 client-providers 的普通集成测试（语音控制器已从 client-core 搬到那里），这个脚本只需编出 C++ 对端，再让 cargo 指向它。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
output="$repo_root/target/voice-wire-interop"
mkdir -p "$output"
"${CXX:-c++}" -std=c++17 -Wall -Wextra -Werror -pthread \
  -fsanitize=address,undefined -fno-omit-frame-pointer \
  -I"$repo_root/shared/contracts" \
  $(find "$repo_root/platforms/windows/src" -type d -exec printf -- '-I%s ' {} +) \
  "$repo_root/platforms/windows/tests/voice/voice_wire_peer.cpp" -o "$output/peer"
MSIME_VOICE_WIRE_PEER="$output/peer" \
  cargo test --manifest-path "$repo_root/Cargo.toml" \
  -p msime-client-providers --test voice_wire_interop -- --nocapture
