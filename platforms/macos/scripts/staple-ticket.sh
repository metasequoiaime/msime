#!/usr/bin/env bash
set -euo pipefail

# 用法：staple-ticket.sh [--validate] <pkg 路径>
# 默认把公证票据装订进安装包（xcrun stapler staple，签过名的 flat pkg 和 DMG 都支持）。--validate 只核对安装包里已装订的票据（xcrun stapler validate），release-macos.yml 更新 Homebrew cask 之前用它判断安装包是否公证过。两者都要向 CloudKit 查票据，断线时按同一个窗口重试。
action=staple
label=装订
if [ "${1:-}" = --validate ]; then
  action=validate
  label=核对
  shift
fi
if [ "$#" -ne 1 ]; then
  echo "用法：staple-ticket.sh [--validate] <pkg 路径>" >&2
  exit 64
fi

# 票据下载在 CloudKit 侧断线时，同一个安装包依次等 15、30、60、90、120 秒再试，最多 6 次、合计约 5 分钟。Release macOS 0.54.0（run 39267138324）的五笔版在 15、30 秒后的三次装订全部断线，整轮构建因此作废，所以窗口放宽到分钟级；同一次发布的第 2 次尝试里，更新 Homebrew cask 前的 stapler validate 也因为同样的断线把 full 版当成未公证，msime cask 没有更新。
delays=(15 30 60 90 120)
attempts=$((${#delays[@]} + 1))
for ((attempt = 1; attempt <= attempts; attempt++)); do
  if output="$(xcrun stapler "$action" "$1" 2>&1)"; then
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
    # 核对时用完重试返回 75（EX_TEMPFAIL），调用方据此把「连不上票据服务」和「安装包没有公证」分开；装订保留 stapler 的退出码。
    if [ "$action" = validate ]; then
      exit 75
    fi
    exit "$status"
  fi
  delay="${delays[$((attempt - 1))]}"
  echo "${label}公证票据时连接中断，${delay} 秒后重试（${attempt}/${attempts}）" >&2
  sleep "$delay"
done
