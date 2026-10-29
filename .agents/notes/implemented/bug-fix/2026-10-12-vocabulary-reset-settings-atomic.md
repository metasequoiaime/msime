# Agent Note: 清空背词进度时原子保留设置

Status: implemented

## Problem

背单词的「清空进度」先把整个进度文档写成默认值，再单独写回事先读取的设置。两次写入之间退出会让选中的词书、每日新词数和会话上限丢失；另一进程在此期间更新设置，也会被旧快照覆盖。

## Decision

进度存储增加 `clear_review_progress`：持有同一把进程间文件锁时读取最新设置，构造只有设置的文档，再原子替换一次。会话的 Reset 动作直接调用它；原有 `reset` 仍保留全部清空语义。

## Alternatives considered

- 清空后再写回设置：两次发布之间仍存在默认设置的中间状态。
- 在会话层提前读取设置并传给存储：即使存储只写一次，传入值也可能落后于另一进程刚保存的设置。

## Consequences

清空背词记录和保留设置成为一次受锁保护的读改写。其他宿主对设置的更新要么发生在清空之前并被保留，要么发生在清空之后；崩溃不会留下默认设置的中间文档。

## Verification

新增测试先因缺少 `clear_review_progress` 编译失败，随后通过。`cargo test -p msime-client-core vocabulary:: --locked`：96 passed。其他门禁见 PR 验证记录。

作答时的跨窗口词书归属见[卡片来源词书](../../implemented/bug-fix/2026-10-30-vocabulary-answer-wordbook.md)，宿主从显示快照携带书 id，共享层不从全局选择推断。
