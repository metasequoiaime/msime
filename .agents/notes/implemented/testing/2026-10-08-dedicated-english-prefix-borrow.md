# Agent Note: 专用英文前缀借用

Status: implemented

## Problem

专用英文模式每次刷新候选都无条件复制小写查询前缀；小写输入在按键路径上重复分配。

## Decision

复用临时英文模式的共享小写前缀策略：小写 ASCII 输入直接借用，包含大写时才创建小写副本。候选查询结果和显示文本保持不变。

## Alternatives considered

继续单独调用 `to_ascii_lowercase()` 会保留重复分配；改变专用英文预编辑的存储规范会影响大小写显示，因此只优化查询参数的所有权。

## Verification

共享小写前缀零分配测试、完整 `msime-engine` 单测与 golden、Clippy、格式检查、笔记检查和 quick 门禁覆盖提交。

## Consequences

专用英文模式处理常见小写输入时不再为查询前缀分配堆内存，大写输入仍按原规则用小写副本查询。
