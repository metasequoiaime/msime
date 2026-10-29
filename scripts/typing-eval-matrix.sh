#!/usr/bin/env bash
# 打字流评测的整组运行：两份打字流评测集 × 三种键盘 × 学习开关 × 键盘模型开关，九键另加三档简拼比例。每一臂单独一个进程，最多同时跑 JOBS 个（默认 4，不要超过编译机留给评测的核数）。
#
# 用法：[JOBS=4] [KEYBOARDS="full nine fourteen"] [SETS="tatoeba chat"] scripts/typing-eval-matrix.sh <已校验的资源目录> <资源锁> <输出目录> [typing_sim 二进制]
# 每一臂写 <输出目录>/<名字>.json（报告）、.stderr（含耗时），每跑完一臂打印一行「名字 ok|FAILED 秒数」，全部跑完后打印一张汇总表（每臂 test 部分的 cfa、cfa_lenient、cca）。资源目录必须是绝对路径。
set -euo pipefail
resources=$1
lock=$2
out=$3
binary=${4:-target/release/examples/typing_sim}
jobs=${JOBS:-4}
# 只跑这几种键盘，例如 PR 只改了九宫格解码时 KEYBOARDS="nine fourteen"。
keyboards=${KEYBOARDS:-full nine fourteen}
sets=${SETS:-tatoeba chat}
mkdir -p "$out"

arms=()
for set in $sets; do
  for keyboard in $keyboards; do
    for learning in off on; do
      for model in off keyboard; do
        arms+=("$set $keyboard $learning $model 0.3")
        if [ "$keyboard" = nine ]; then
          arms+=("$set $keyboard $learning $model 0" "$set $keyboard $learning $model 0.6")
        fi
      done
    done
  done
done

run_arm() {
  local set=$1 keyboard=$2 learning=$3 model=$4 share=$5
  local name="$set-$keyboard-learn$learning-model$model-abbr$share"
  local started
  started=$(date +%s)
  if "$binary" --resources "$resources" --resource-lock "$lock" --set "resources/eval/typing-$set-v1.tsv" \
      --keyboard "$keyboard" --learning "$learning" --model "$model" --abbreviate "$share" \
      --report "$out/$name.json" > /dev/null 2> "$out/$name.stderr"; then
    echo "$name ok $(( $(date +%s) - started ))s" | tee -a "$out/$name.stderr"
  else
    echo "$name FAILED $(( $(date +%s) - started ))s" | tee -a "$out/$name.stderr"
  fi
}
export -f run_arm
export binary resources lock out
printf '%s\n' "${arms[@]}" | xargs -P "$jobs" -L 1 bash -c 'run_arm "$@"' _

# 汇总表：每臂 test 部分的严口径 cfa、宽口径 cfa_lenient 和整块首选率 cca；没写出报告的臂记为 FAILED。
printf '%-50s %-7s %-7s %-7s\n' arm cfa lenient cca
for arm in "${arms[@]}"; do
  read -r set keyboard learning model share <<< "$arm"
  name="$set-$keyboard-learn$learning-model$model-abbr$share"
  if [ -s "$out/$name.json" ]; then
    python3 -I -c 'import json, sys
test = json.load(open(sys.argv[1]))["overall"]["test"]
print("{:<50} {:<7} {:<7} {:<7}".format(sys.argv[2], test["cfa"], test["cfa_lenient"], test["cca"]))' "$out/$name.json" "$name"
  else
    printf '%-50s FAILED\n' "$name"
  fi
done
