#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "用法：staple-ticket.sh <DMG 路径>" >&2
  exit 64
fi

# 票据下载在 CloudKit 侧断线时，同一个 DMG 依次等 15、30、60、90、120 秒再装订，最多 6 次、合计约 5 分钟。Release macOS 0.54.0（run 39267138324）的五笔版在 15、30 秒后的三次尝试全部断线，整轮构建因此作废，所以窗口放宽到分钟级。
delays=(15 30 60 90 120)
attempts=$((${#delays[@]} + 1))
for ((attempt = 1; attempt <= attempts; attempt++)); do
  if output="$(xcrun stapler staple "$1" 2>&1)"; then
    [ -z "$output" ] || printf '%s\n' "$output"
    exit 0
  else
    status=$?
  fi
  printf '%s\n' "$output" >&2
  # 只重试已观察到的票据下载断线（退出码 68，同时报告 NSURLErrorDomain -1005 或 CloudKit 应答异常）；签名、票据和其他失败保留原始退出码。
  if [ "$status" -ne 68 ]; then
    exit "$status"
  fi
  case "$output" in
    *"NSURLErrorDomain Code=-1005"* | *"CloudKit's response is inconsistent"*) ;;
    *) exit "$status" ;;
  esac
  if [ "$attempt" -eq "$attempts" ]; then
    exit "$status"
  fi
  delay="${delays[$((attempt - 1))]}"
  echo "装订公证票据时连接中断，${delay} 秒后重试（${attempt}/${attempts}）" >&2
  sleep "$delay"
done
