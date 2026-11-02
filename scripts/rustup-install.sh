#!/usr/bin/env bash
# 安装 rust-toolchain.toml 钉住的工具链，再加上参数里的编译目标；联网失败时有限重试。
# rustup 只对组件下载重试，取 channel 清单和它的 .sha256 失败就直接退出：2026-11-01 夜间 Windows nightly 的 official-plugins 就是在 `rustup toolchain install` 上遇到一次 DNS 解析失败（Temporary failure in name resolution），整个 nightly 因此变红。
# 用法：bash scripts/rustup-install.sh [目标三元组...]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
# 每次重试前等待的秒数，一共四次尝试；够覆盖一两分钟内的解析或连接抖动，真正的配置错误（目标名写错等）最多多花一分多钟才失败。
delays=(5 15 45)
attempts=$((${#delays[@]} + 1))
for ((attempt = 1; attempt <= attempts; attempt++)); do
  if rustup toolchain install && { [[ $# -eq 0 ]] || rustup target add "$@"; }; then
    exit 0
  fi
  if ((attempt == attempts)); then break; fi
  delay=${delays[attempt - 1]}
  echo "rustup failed (attempt $attempt/$attempts); retrying in ${delay}s" >&2
  sleep "$delay"
done
echo "rustup could not install the pinned toolchain${1:+ and targets $*} after $attempts attempts" >&2
exit 1
