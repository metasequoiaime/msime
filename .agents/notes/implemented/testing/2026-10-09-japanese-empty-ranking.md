# Agent Note: 日文未命中查询延迟分配排名堆

Status: implemented

## Problem

`best_ids_from_iter` 和 continuing 查询在读取任何候选之前申请 `limit` 个排名槽位。合成词库的 exact 未命中和空 suffix continuing 未命中各分配一次；双假名未知读音矩阵搜索分配 14 次，其中五次来自空排名堆。

## Decision

普通排名先检查迭代器首条，空迭代器直接返回；非空时按原容量建堆，用 `once(first).chain(ids)` 保持原处理顺序。零限额仍在创建迭代器前返回。

continuing 跨多个 suffix 扫描，使用无存储的 `BinaryHeap::new()`，首次实际候选在填堆分支中 `reserve_exact(limit)`。每条候选仍按 `(cost, id)` 排名；首次命中时容量足以容纳 `limit` 条且不主动进行推测性扩容，命中分配次数保持。短前缀缓存、重叠 suffix 的重复 ID 和最终排序保持，不增加缓存。非空 suffix 的查询字符串由[查询内单缓冲](2026-10-09-japanese-continuing-query-buffer.md) 复用，本篇只约束排名存储。

## 既有笔记审计

[词条文本借用](../../implemented/testing/2026-10-09-japanese-lemma-borrow.md) 部分重叠，继续约束查询视图、生命周期与命中分配预算，本篇只约束空排名存储，两篇互链。对活跃笔记检索 `BinaryHeap`、空查询、空排名和 `best_ids_from_iter`，命中的拼音纠错单段笔记处理其他输入方案，无关且保留。日文 provider 转换缓冲与罗马字位置笔记分别约束转换和候选位置，无关且保留。没有同一决定的旧提案。

## Alternatives considered

- 两处都使用 `new` 加首次填堆时 `reserve`：实现统一且不拉取首项，但普通排名每次填堆都会检查是否为空；普通查询可直接取得首项，只在 continuing 多范围扫描使用该分支。
- 任由堆按 `push` 自动增长：空查询自然不分配，代码更少；非空大结果会多次扩容，破坏既有两次分配预算。

## Verification

真实红测试：exact 和 prefix 未命中各分配一次；空 suffix continuing 分配一次，非空 suffix 未命中分配两次；空迭代器直接排名分配一次；未知双假名矩阵搜索分配 14 次。

修复后日文 70 项测试通过：exact、prefix 和空迭代器零分配，continuing 只保留非空 suffix 的一份字符串分配，矩阵完整结果不变且为 9 次。零限额不读取迭代器或成本，空迭代器不读取成本；首条迭代一次，负成本、平局、限额与后续 suffix 命中正确。既有 70 条合成词库命中分配预算和重叠 suffix 结果通过；有词库单假名 matrix 和充分预热 provider 均保持 13 次。独立设计审查的容量措辞建议通过 `reserve_exact` 和精确文档解决。

完整引擎 1457 项、golden 31 项、Clippy、Rustfmt、差异及 341 篇笔记检查通过。独立代码审查没有遗留问题。完整 quick 门禁退出为 0 并报告 `quick check passed`：共享 Rust、Android 目标和宿主 Java、Linux 桌面共享层、Linux 原生宿主 73 项测试、macOS 原生宿主和 Apple bridge 构建通过。Windows、WASM、HarmonyOS 的工具链或前置缺失由脚本明确跳过；没有设备验收。

## Consequences

未命中查询不申请排名存储，未知双假名矩阵搜索减少五次分配。命中时仍预留整个限额，保留原有排名、输出排序、重复 ID 和命中分配次数；continuing 首次填堆检查增加一个分支。首条预读不缓存候选，不增加常驻状态或中间向量。

查询内单缓冲字符串和结果向量仍会分配；[后缀键复用](2026-10-09-japanese-continuing-query-buffer.md) 限制为每次查询至多一份键，此决定只约束空排名堆，不声称整个词库查询零分配。分配计数依赖当前标准库实现，断言针对已测具体路径。加载、映射、校验和模型生命周期没有变化。

[精确词条流式消费](2026-10-09-japanese-exact-lemma-stream.md) 进一步去掉命中时的精确视图结果容器，不更改本篇的排名堆和未命中零分配边界；本篇历史命中预算保留，当前预算由新片独立验证。

[排名堆成本键复用](2026-10-10-japanese-ranked-heap-output.md) 将项存储由 `((cost, id), id)` 收为 `(cost, id)`，最终对保存键不稳定排序，避免重读成本；继续保留本篇的空查询、流式消费、重复项与词库借用边界。历史分配次数和单项计时保留，当前排名请求字节与独立选型证据由新篇核算。
