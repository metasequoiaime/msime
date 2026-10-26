#!/usr/bin/env bash
# 构建并测试 plugins/script/ 下的脚本插件。
#
# 用法：scripts/test-script-plugins.sh [--update] [<id>...]   不写 id 时测试全部插件
#
# 对每个插件：
# 1. 插件包文件夹和 .zip 都用 msime-pack validate 校验，输出必须是 `ok <id> script <version>`。
# 2. 用插件目录里的 fixture.jsonl 跑 msime-pack run --grant all，退出码必须为 0（没有任何失败）；输出去掉 fuel、init_fuel 后和 expected.jsonl 逐行相同，每行的 fuel 不超过钩子单次上限的一半。
# 3. 不授予任何能力再跑一遍，每个事件都不能有动作。
# 4. 插件目录里有 check_output.py 时，再用它按插件自己的规则核对第 2 步的输出（uuid 按 RFC 9562 从夹具的随机数独立算一遍）。
# 5. 插件目录里每个 cases/<name>/ 是另一次运行：设置项的值（settings.json，交给 --settings）、存储的初值（storage.bin，交给 --storage）和事件（fixture.jsonl，没有时用插件的），按第 2、3 步核对那里的 expected.jsonl。
# 核对在 plugins/script/tools/check_run.py。--update 把第 2、5 步的结果写回各自的 expected.jsonl（写完逐行看过再提交）。
#
# plugins/official-packs.json 登记的官方脚本插件（随桌面安装包分发）用的是保留 id，validate 和 run 对它们加 --builtin，其余插件不加：保留 id 出现在别的插件上照样报 plugin_reserved。
#
# 最后运行 scripts/test-script-plugin-data.py，确认提交进仓库的生成数据没有漂移。common-chars.bin 的检查要锁定的 Unihan zip：缓存里没有时打印 skip，设 MSIME_FETCH=1 才下载；设了 MSIME_REQUIRE_SCRIPT_PLUGINS=1 时 skip 算失败（CI 两个都设）。
#
# 没有 wasm32-unknown-unknown 目标时打印 skip 并以 0 退出；设了 MSIME_REQUIRE_SCRIPT_PLUGINS=1（CI 这样设）时改为失败。msime-pack 用仓库工作区的默认产物目录（target/debug/msime-pack）。

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# 产物路径写死在 target/ 下（插件在 plugins/script/.cargo/config.toml，msime-pack 在仓库默认位置）；环境里的 CARGO_TARGET_DIR 会让 cargo 写到别处，组装时就会静默用上旧模块。
unset CARGO_TARGET_DIR
cd "$root" || exit 2

update=()
ids=()
for argument in "$@"; do
  case "$argument" in
    --update) update=(--update) ;;
    -*)
      echo "usage: scripts/test-script-plugins.sh [--update] [<id>...]" >&2
      exit 2
      ;;
    *) ids+=("$argument") ;;
  esac
done

toolchain="$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)"
installed="$(rustup target list --installed --toolchain "$toolchain" 2>/dev/null)"
case $'\n'"$installed"$'\n' in
  *$'\n'wasm32-unknown-unknown$'\n'*) ;;
  *)
    message="wasm32-unknown-unknown is not installed for $toolchain (rustup target add wasm32-unknown-unknown --toolchain $toolchain)"
    if [ "${MSIME_REQUIRE_SCRIPT_PLUGINS:-}" = 1 ]; then
      echo "FAIL: $message"
      exit 1
    fi
    echo "skip: $message"
    exit 0
    ;;
esac

if [ "${#ids[@]}" -eq 0 ]; then
  for manifest in plugins/script/*/Cargo.toml; do
    [ -e "$manifest" ] || continue
    dir="${manifest%/Cargo.toml}"
    ids+=("${dir##*/}")
  done
fi

if ! cargo build --locked -p msime-pack-tool --bin msime-pack; then
  echo "FAIL: msime-pack 构建失败"
  exit 1
fi
pack_tool="$root/target/debug/msime-pack"

failed=0
fail() {
  echo "FAIL: $1"
  failed=1
}

if [ "${#ids[@]}" -gt 0 ] && ! bash scripts/build-script-plugins.sh "${ids[@]}"; then
  echo "FAIL: 插件构建失败"
  exit 1
fi

scratch="$root/target/script-plugins/test"
mkdir -p "$scratch"
official="$(python3 -c 'import json,sys; print(" ".join(p["id"] for p in json.load(open(sys.argv[1]))["packs"] if p["kind"] == "script"))' plugins/official-packs.json)" || {
  echo "FAIL: plugins/official-packs.json 读不出"
  exit 1
}
for id in ${ids[@]+"${ids[@]}"}; do
  echo "== $id"
  dir="plugins/script/$id"
  folder="target/script-plugins/$id"
  version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$dir/pack/plugin.toml")"
  archive="target/script-plugins/$id-$version.zip"
  builtin=()
  case " $official " in
    *" $id "*) builtin=(--builtin) ;;
  esac

  if ! "$pack_tool" validate ${builtin[@]+"${builtin[@]}"} "$folder" "$archive" >"$scratch/$id.validate" 2>&1; then
    cat "$scratch/$id.validate"
    fail "$id: msime-pack validate"
    continue
  fi
  expected_line="ok $id script $version"
  if [ "$(cat "$scratch/$id.validate")" != "$expected_line"$'\n'"$expected_line" ]; then
    cat "$scratch/$id.validate"
    fail "$id: validate 的输出应该是两行 $expected_line"
  fi

  if [ ! -f "$dir/fixture.jsonl" ]; then
    fail "$id: 缺少 fixture.jsonl"
    continue
  fi
  if ! "$pack_tool" run "$folder" --events "$dir/fixture.jsonl" --grant all ${builtin[@]+"${builtin[@]}"} >"$scratch/$id.granted.jsonl"; then
    fail "$id: msime-pack run --grant all 报告了失败（输出在 $scratch/$id.granted.jsonl）"
  fi
  if ! "$pack_tool" run "$folder" --events "$dir/fixture.jsonl" ${builtin[@]+"${builtin[@]}"} >"$scratch/$id.ungranted.jsonl"; then
    fail "$id: 不授权的 msime-pack run 报告了失败（输出在 $scratch/$id.ungranted.jsonl）"
  fi
  if ! python3 plugins/script/tools/check_run.py "$dir" "$scratch/$id.granted.jsonl" "$scratch/$id.ungranted.jsonl" ${update[@]+"${update[@]}"}; then
    fail "$id: 输出和 expected.jsonl 不一致或超出 fuel 上限"
  fi
  if [ -f "$dir/check_output.py" ] && ! python3 "$dir/check_output.py" "$dir/fixture.jsonl" "$scratch/$id.granted.jsonl"; then
    fail "$id: check_output.py 的核对没有通过"
  fi

  # 另外的运行：cases/<name>/ 里的 settings.json、storage.bin 交给 --settings、--storage，事件取那里的 fixture.jsonl，没有时用插件的；输出同样和那里的 expected.jsonl 逐行核对、按 fuel 上限断言、再不授权跑一遍。
  for case_dir in "$dir"/cases/*/; do
    [ -d "$case_dir" ] || continue
    case_dir="${case_dir%/}"
    name="$id.${case_dir##*/}"
    events="$case_dir/fixture.jsonl"
    [ -f "$events" ] || events="$dir/fixture.jsonl"
    inputs=()
    [ -f "$case_dir/settings.json" ] && inputs+=(--settings "$case_dir/settings.json")
    [ -f "$case_dir/storage.bin" ] && inputs+=(--storage "$case_dir/storage.bin")
    if ! "$pack_tool" run "$folder" --events "$events" --grant all ${inputs[@]+"${inputs[@]}"} ${builtin[@]+"${builtin[@]}"} >"$scratch/$name.granted.jsonl"; then
      fail "$name: msime-pack run --grant all 报告了失败（输出在 $scratch/$name.granted.jsonl）"
    fi
    if ! "$pack_tool" run "$folder" --events "$events" ${inputs[@]+"${inputs[@]}"} ${builtin[@]+"${builtin[@]}"} >"$scratch/$name.ungranted.jsonl"; then
      fail "$name: 不授权的 msime-pack run 报告了失败（输出在 $scratch/$name.ungranted.jsonl）"
    fi
    if ! python3 plugins/script/tools/check_run.py "$case_dir" "$scratch/$name.granted.jsonl" "$scratch/$name.ungranted.jsonl" ${update[@]+"${update[@]}"}; then
      fail "$name: 输出和 $case_dir/expected.jsonl 不一致或超出 fuel 上限"
    fi
  done
done

echo "== generated data"
if ! python3 scripts/test-script-plugin-data.py; then
  fail "生成数据和生成器不一致"
fi

if [ "$failed" -ne 0 ]; then
  echo "script plugins: at least one check failed"
  exit 1
fi
echo "script plugins: ${#ids[@]} plugin(s) passed"
