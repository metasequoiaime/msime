# Agent Note: 补充相对日期“后年”

Status: implemented

## Problem

相对日期触发词已有“今年、明年、来年”等候选，但缺少“后年”；Android 反馈中的 `hounian` 无法生成带年份的候选。

## Decision

在共享 Engine 的 `INLINE_KEYWORDS` 中加入 `hounian` → “后年”，年份偏移为 `+2`，并让现有全拼、九键相对日期路径复用该条目。测试夹具加入对应锚定词和 2028 年结果。

## Alternatives considered

无；该行为属于现有相对年份触发词的直接补全。

## Consequences

“后年”与其他相对年份一样受日期时间候选开关和本地时钟影响，生成文本不写入学习词库。

## Verification

- `git diff --check` 通过。
- CI 将运行 Engine 测试和 Android host/设备覆盖；本机磁盘剩余约 6.5 GiB，未启动新的 Rust 构建树。
