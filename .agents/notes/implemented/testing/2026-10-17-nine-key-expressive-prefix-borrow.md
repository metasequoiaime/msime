# Agent Note: 九键表情读法直接写入锁定前缀

Status: implemented

## Problem

`NineKeySession::expressive_readings` 每次调用先用 `self.locked.concat()` 创建完整锁定读音前缀，再把它复制进每条候选读法。锁定拼写存在时，这个前缀字符串只被读取一次，独立分配会落在九键表情混排刷新热路径上。

## Decision

读法字符串仍按原容量创建，但改为逐项把 `self.locked` 中的拼写直接追加到当前读法，再追加路径片段。锁定拼写顺序、读法去重、支持位置排序、表情边界和最终拥有型结果保持不变；没有锁定拼写时不增加工作。

## Historical audit

已检索九键表情读法、边界借用、双拼边界延后物化和表情资源按需解析笔记。[九键表情读法边界借用](2026-10-17-nine-key-expressive-boundary-borrow.md)负责边界所有权，[双拼表情边界延后物化](2026-10-17-nine-key-expressive-boundary-materialization.md)负责截断前的双拼边界计算，[表情资源按需解析](2026-10-17-nine-key-expressive-resource-lazy.md)负责资源路径生命周期；本片只移除锁定读音前缀的中间字符串，没有重复实现它们的决定。

## Alternatives considered

- **继续使用 `concat`**：代码最短，但每次存在锁定拼写时都会创建只消费一次的前缀字符串。
- **在会话中长期保存完整前缀**：需要维护锁定拼写变化后的失效关系，并扩大会话状态；本片只在已有读法字符串中追加。
- **把锁定前缀作为借用片段返回**：会改变读法结果的拥有边界，且最终仍需把片段写入每条读法；直接追加保持现有接口。

## Verification

新增分配回归测试，固定 `mei` 锁定前缀和剩余 `486` 路径。未修改实现时测试报告 16 次分配并按 15 次上限失败；改为直接追加后为 15 次并通过。九键模块 141 项、完整引擎库 2262 项（40 项既有忽略）、golden 31 项和 runtime 318 项通过；Clippy、格式、差异、笔记校验和 `TMPDIR=/Volumes/Workspace/tmp/msime CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过。quick 完成 Rust workspace、Android target、WASM、Android Java、macOS、共享 Apple bridge 和 Windows 本机测试（157 项通过、2 项宿主差异排除）；Linux Docker、MinGW、HarmonyOS 真工程和 pipe-only 按环境跳过。

## Consequences

锁定拼写参与表情读法时少一次短命前缀字符串分配；每条最终读法仍拥有自己的字符串，读法排序、边界和查询资源不变。收益只适用于启用表情混排且已有锁定拼写的九键刷新，不能外推为整体九键延迟达标。
