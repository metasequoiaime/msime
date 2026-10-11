# Agent Note: 注音列表按去重后行数预留容量

Status: implemented

## Problem

注音候选列表打开时，词库会先返回同一字在多个允许读音下的行。列表在去重前按原始行数预留容量，重复行较多时会让候选向量保留一块不会使用的尾部空间。

## Decision

先使用现有的 `deduplicate_reading_entries` 保留首次出现的词条，再按去重后的行数预留 `self.list`。候选顺序、首次行、列表上限和大列表的哈希去重路径保持不变。

## Verification

回归测试先在基线容量按 8 条原始行预留时失败；修复后确认 8 个相同文字只留下 1 行且列表容量小于原始行数。注音 scheme 测试 52 项、引擎全量测试 1781 项（13 项忽略）、Clippy、Rustfmt、差异检查均通过；quick 门禁中的 agent notes、Rust/Android/Linux/macOS 编译和 Linux native 79 项测试通过，但整体因既有 Android 标签工厂、键盘试用 watcher、live status、ripple、sheet header 和 UI label factories 契约检查失败而返回 1。

## Alternatives considered

- 继续按去重前行数预留：实现最简单，但重复读音会保留未使用的尾部容量。
- 让列表自然增长：能避免过度预留，但有效条目较多时会增加扩容次数。

## Consequences

重复读音较多的注音列表减少候选向量的预留容量；去重本身的临时集合和大列表路径不变。
