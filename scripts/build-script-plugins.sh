#!/usr/bin/env bash
# 构建 plugins/script/ 下的脚本插件，组装成可以导入的插件包。
#
# 用法：scripts/build-script-plugins.sh [<id>...]   不写 id 时构建全部插件
#
# 每个插件是 plugins/script/<id>/ 下一个独立的 no_std crate（自带 [workspace]，不属于仓库的 Cargo 工作区）。cargo 在插件目录里运行：它从当前目录向上找 .cargo/config.toml，共用的目标（wasm32-unknown-unknown）、栈大小、release 配置和产物目录（target/script-plugins/cargo）都写在 plugins/script/.cargo/config.toml 里，从仓库根用 --manifest-path 调用时这些设置不生效。
#
# 产物：target/script-plugins/<id>/（插件包文件夹，在设置里「导入文件夹」选它）和 target/script-plugins/<id>-<version>.zip。组装和检查（模块大小、内存页数、许可证副本）在 plugins/script/tools/assemble.py。
#
# 工具链用 rust-toolchain.toml 钉住的版本，需要给它加上 wasm32-unknown-unknown 目标；没有时退出码 2。

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# 产物路径写死在 target/ 下（插件在 plugins/script/.cargo/config.toml，msime-pack 在仓库默认位置）；环境里的 CARGO_TARGET_DIR 会让 cargo 写到别处，组装时就会静默用上旧模块。
unset CARGO_TARGET_DIR
cd "$root"

toolchain="$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)"
if [ -z "$toolchain" ]; then
  echo "error: rust-toolchain.toml 里没有 channel" >&2
  exit 2
fi
installed="$(rustup target list --installed --toolchain "$toolchain")"
case $'\n'"$installed"$'\n' in
  *$'\n'wasm32-unknown-unknown$'\n'*) ;;
  *)
    echo "error: $toolchain 没有安装 wasm32-unknown-unknown 目标，先运行 rustup target add wasm32-unknown-unknown --toolchain $toolchain" >&2
    exit 2
    ;;
esac

if [ "$#" -gt 0 ]; then
  ids=("$@")
else
  ids=()
  for manifest in plugins/script/*/Cargo.toml; do
    [ -e "$manifest" ] || continue
    dir="${manifest%/Cargo.toml}"
    ids+=("${dir##*/}")
  done
fi
if [ "${#ids[@]}" -eq 0 ]; then
  echo "plugins/script/ 下没有插件"
  exit 0
fi

wasm_dir="$root/target/script-plugins/cargo/wasm32-unknown-unknown/release"
for id in "${ids[@]}"; do
  dir="plugins/script/$id"
  if [ ! -f "$dir/Cargo.toml" ] || [ ! -f "$dir/pack/plugin.toml" ]; then
    echo "error: $dir 不是插件目录（要有 Cargo.toml 和 pack/plugin.toml）" >&2
    exit 2
  fi
  echo "== $id"
  (cd "$dir" && cargo +"$toolchain" build --release --offline)
  # cdylib 的文件名是包名把 - 换成 _。
  python3 plugins/script/tools/assemble.py "$dir" "$wasm_dir/${id//-/_}.wasm"
done
