#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "用法：staple-ticket.sh <DMG 路径>" >&2
  exit 64
fi

for attempt in 1 2 3; do
  if output="$(xcrun stapler staple "$1" 2>&1)"; then
    [ -z "$output" ] || printf '%s\n' "$output"
    exit 0
  else
    status=$?
  fi
  printf '%s\n' "$output" >&2
  # 只重试已观察到的票据下载断线；签名、票据和其他失败保留原始退出码。
  if [ "$status" -ne 68 ]; then
    exit "$status"
  fi
  case "$output" in
    *"NSURLErrorDomain Code=-1005"*) ;;
    *) exit "$status" ;;
  esac
  if [ "$attempt" -eq 3 ]; then
    exit "$status"
  fi
  delay=$((attempt * 15))
  echo "装订公证票据时连接中断，${delay} 秒后重试（${attempt}/3）" >&2
  sleep "$delay"
done
