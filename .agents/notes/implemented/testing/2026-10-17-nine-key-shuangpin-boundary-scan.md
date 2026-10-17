# Agent Note: 九键双拼边界转换直接扫描

Status: implemented

## Problem

九键双拼的表情读法边界转换先为每条路径建立 `(数字位置, 字母位置)` 的 `ends` 向量，再扫描边界查找匹配项。路径完成后只需要边界对应的字母位置，中间表在每条读法上都是临时存储。

## Decision

按已排序的数字边界维护一个游标，遍历锁定拼写和路径的音节末尾时直接写入命中的字母位置。普通九键和 14 键继续借用调用方边界；双拼仍返回拥有型转换结果，未完成路径和无匹配边界的行为保持不变。

## Historical audit

已检索九键表情读法、双拼边界、路径分配和普通网格边界借用相关笔记。本片接续[普通网格边界借用](2026-10-17-nine-key-expressive-boundary-borrow.md)，只处理双拼分支的中间 `ends` 表，不改变表情查询、路径限制或双拼编码契约。

## Alternatives considered

- 保留 `ends` 表：查找逻辑直观，但每条路径都申请并填充只消费一次的中间向量。
- 对每个边界重新遍历路径：可以省掉向量，但会重复计算音节编码长度，边界较多时复杂度更差。
- 让双拼直接借用数字边界：数字位置与字母位置不同，无法表达双拼边界转换；仍需拥有转换后的字母位置。

## Verification

固定 Migala3、数字 `6447` 和两个边界的双拼读法路径分配从旧实现 101 次降为 77 次；新增测试门槛为不超过 100。九键模块 138 项测试通过，覆盖普通九键、14 键、九键双拼和表情读法；完整 engine 2256 项通过（40 项既有忽略），Golden 31 项通过，Runtime 318 项通过，Clippy、fmt、笔记检查和 `git diff --check` 通过。

`bash scripts/verify-local.sh --quick` 已运行至结束。Rust、Android、Wasm、macOS、Android host、notes 等阶段通过；HarmonyOS、Linux Docker、Windows 交叉编译和 pipe-only 阶段按宿主条件跳过。Windows 测试在当前 macOS 宿主出现 `clang++` 链接失败，并因 `/var/folders` 无空间导致若干临时目录创建失败；共享 Apple bridge 构建失败。这些是宿主资源和工具链差异，不能作为本改动的产品回归结论。

## Consequences

双拼每条路径不再建立中间边界表，只保留最终字母位置向量；边界按升序游标扫描，输出顺序与旧实现一致。收益只发生在启用双拼表情读法的路径，不能据此宣称九键整体 16 ms 目标完成。
