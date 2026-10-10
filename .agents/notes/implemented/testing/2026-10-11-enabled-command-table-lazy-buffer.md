# Agent Note: 命令表空启用列表延后缓冲

Status: implemented

## Problem

`enabled_commands` 按 `MAX_COMMANDS` 创建输出向量和触发词去重集合。启用列表为空、插件缺失，或所有插件载入失败时，结果为空但仍保留未使用的固定容量。

## Decision

输出向量和去重集合从空容器开始，首条未重复的有效命令加入前再按原上限执行容量预留。有效命令表继续一次预留 `MAX_COMMANDS`，命令顺序、首个触发词优先、上限和加载失败语义不变。

## Verification

空启用列表的容量断言先在基线因固定预留而失败，修复后通过；相关插件测试、client-core 全量测试、Clippy、Rustfmt、差异检查和 quick 门禁覆盖提交。

## Alternatives considered

- 继续按 `MAX_COMMANDS` 预留：实现简单，但空结果会保留未使用容量。
- 让两个容器逐行自然增长：空结果成本最低，但有效命令表会增加扩容次数。

## Consequences

空命令表不再分配行和去重集合缓冲；首条有效命令出现时仍一次预留原容量，合并结果和限制保持不变。
