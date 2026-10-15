#!/usr/bin/env bash
# Manual portable test. Does not exercise Windows authentication or audio.
#
# The Rust half used to be compiled here with a bare `rustc --test` on a file that belonged to no Cargo package, reaching into client-core's private source with a five-level `#[path]`. Nothing in `cargo check --workspace` or the clippy gate could see it, and it was the one file in the tree that had drifted out of rustfmt. It is now an ordinary integration test of client-providers (the voice controller moved there from client-core), so this script only has to build the C++ peer and point cargo at it.
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
