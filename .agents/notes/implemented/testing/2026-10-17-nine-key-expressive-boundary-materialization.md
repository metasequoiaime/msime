# Agent Note: 九键双拼表情边界延后物化

Status: implemented

## Problem

九键双拼表情读法会先为每条完整切分路径计算并拥有数字边界到字母边界的转换结果，再按拼音候选支持位置排序并截取前 8 条。排序后丢弃的读法不会查询表情资源，却已经承担了双拼边界向量的分配和释放。

## Decision

`expressive_readings` 先验证路径是否完整，记录去重读法对应的路径索引，完成支持位置排序并截取 `EXPRESSIVE_READING_LIMIT` 后，再为保留的路径计算 `reading_boundaries`。普通九键和十四键的边界仍借用调用方切片；九键双拼仍为最终读法拥有转换后的字母边界。读法去重、稳定排序、路径顺序、支持位置和无效路径过滤保持不变。

## Historical audit

已检索九键表情读法、边界借用、双拼边界扫描、资源按需解析和表情候选契约相关的活跃笔记。现有[普通边界借用](2026-10-17-nine-key-expressive-boundary-borrow.md)与[双拼边界直接扫描](2026-10-17-nine-key-shuangpin-boundary-scan.md)分别约束边界所有权和单条路径的转换成本；本片只延后被截掉路径的双拼转换，不改变两者决定。已有表情候选测试继续覆盖资源查询、去重、候选锚定和边界切分。

## Alternatives considered

- 继续为全部读法物化边界：实现直接，但会为排序后丢弃的双拼读法支付短命向量分配。
- 先用哈希集合收集全部读法，再按索引选前 8 条：可减少重复字符串比较，但会新增集合和键存储，改变这条小列表路径的分配权衡。
- 只在 `reading_boundaries` 内截断：无法知道支持排序后的最终 8 条，仍会为未保留路径计算边界。

## Verification

新增双拼表情读法分配回归：固定 `94264` 的旧实现分配 138 次，新实现 99 次，门槛不超过 100；测试先在旧实现上按门槛失败，再在新实现上通过。九键模块 139 项、完整 engine 2257 项（40 项既有忽略）、Golden 31 项、Runtime 318 项通过；Clippy、fmt、笔记校验和 `git diff --check` 通过。表情读法行为测试覆盖普通九键、双拼、候选支持排序、重复读法和不完整路径。`bash scripts/verify-local.sh --quick` 已运行至结束，Rust workspace、Android/Wasm、Android Java、macOS 原生宿主和 Windows 本机测试（168 通过、2 项宿主差异排除）通过；Linux/Docker、MinGW、HarmonyOS 和 pipe-only 按环境跳过，共享 Apple bridge 构建失败，不能将 quick 记为全绿。

## Consequences

双拼表情路径只有最终保留的最多 8 个读法才建立字母边界向量，减少被截掉路径的分配和遍历。为此每条候选路径增加一次无分配的完整性扫描，最终保留路径再扫描一次以生成边界；普通网格边界仍直接借用。该局部收益只适用于启用表情混排的双拼查询，不能外推为九键整体延迟预算达标。
