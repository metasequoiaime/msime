# Agent Note: 背词进度文件绑定目录句柄

Status: implemented

## Problem

背词进度在目录锁内检查文件后，仍通过路径读取和临时文件发布。目录祖先被替换时，检查与使用之间的窗口可能把学习记录读写到外部目录。

## Decision

Unix 上进度文件读取使用已打开目录句柄的 `openat`，原子发布使用共享 `write_private_file_at` 的 `renameat`；缺失文件仍表示新用户，损坏文件仍报告错误。非 Unix 保留原有临时文件发布。

## Alternatives considered

- 继续在 `symlink_metadata` 后使用路径型打开：无法覆盖目录替换竞态。
- 仅对叶文件加 `O_NOFOLLOW`：祖先替换仍可重定向路径。
- 为背词模块复制目录遍历：会偏离其它私有存储的统一链接策略。

## Consequences

进度读写绑定到锁住的目录，原子发布和损坏文件处理语义保持不变；非 Unix 行为不变。

## Verification

`cargo test -p msime-client-core --lib vocabulary::progress --locked --quiet`：20 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
