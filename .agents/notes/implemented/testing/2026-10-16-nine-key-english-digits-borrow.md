# Agent Note: 九键英文候选借用已输入数字

Status: implemented

## Problem

`NineKeySession::english_candidates` 在每次英文候选刷新时先复制 `self.digits`，随后只把这份副本用于前缀展开、数字匹配和排序长度。词库已经打开且英文候选缓冲可复用时，这个复制仍落在九键热刷新路径中。

## Decision

先确保英文词典已经打开，再从会话借用 `digits` 字符串切片，整个查询期间只读使用这份切片。英文词库查询、前缀展开、候选过滤、去重、排序和容量规则保持不变；词典缺失时仍返回空列表。

## Historical audit

已检索[九键英文惰性候选缓冲](2026-10-10-nine-key-english-lazy-buffer.md)和当前英文候选测试。前者处理候选结果容量只在首个命中行出现时预留，本项只移除输入数字的临时拥有副本。`letter_prefixes` 仍按预算生成拥有型前缀，词库查询返回的 `WordItem` 仍由调用方拥有。

## Alternatives considered

- **继续复制 `digits`**：借用边界简单，但每次刷新都会为只读键码付出一次字符串分配。
- **把 `digits` 传入 `open_english`**：会扩大词典打开接口并混合初始化与查询职责；先打开后借用切片即可保持现有接口。
- **把英文查询改成会话级前缀缓存**：需要处理数字、网格和词典版本失效，范围超出本次已确认的单次复制成本。

## Verification

新增热刷新分配回归先在旧实现失败（157 次分配），生产改动后通过（少一次数字副本分配）；九键模块 132 项测试、引擎 2221 项单测和 31 项 golden 通过，格式、Clippy、笔记检查与差异检查通过。quick 门禁的 Windows、Rust、WASM、Android Java 和 shared Apple bridge 阶段通过；macOS CMake 在磁盘只剩 225 MiB 时首次链接失败，清理本工作树 `target/macos-isolated` 后按同一目录手动完整链接至 100% 成功。随后 quick 重新运行时 macOS 产物占满磁盘，Windows 测试链接器报告主机磁盘不足，未将该环境失败归因于本改动。

## Consequences

英文候选热刷新少一次输入数字字符串分配；词库查询、候选顺序、容量、缺失词典处理和公开接口不变。前缀展开及每条查询返回行仍有原有分配，九键端到端延迟需以完整固定资源评测确认，不能由本地分配计数推断 16 ms 预算达标。

完整组码与查询限额的先后顺序由[分组键盘英文先过滤再限额](../bug-fix/2026-10-21-keypad-english-filter-before-limit.md)约束，保留本篇的预算与分配边界。
