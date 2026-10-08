# Agent Note: 背词单词本库绑定目录句柄

Status: implemented

## Problem

单词本库在锁住目录后仍通过路径读取、原子发布和删除索引及单词本文件。目录或祖先在检查后被替换时，路径解析可能跟随符号链接，把读取或删除导向库目录之外。

## Decision

Unix 上单词本库的索引和单词本读写统一使用已打开的库目录句柄：文件读取用 `openat`，原子发布用临时文件和 `renameat`，删除用 `unlinkat`。列表检查也通过句柄打开文件，避免先检查路径再重新解析。非 Unix 保留原有临时文件发布路径。

## Alternatives considered

- 继续在 `symlink_metadata` 后调用路径型文件操作：检查与实际操作之间仍有目录替换竞态。
- 只给最终文件加 `O_NOFOLLOW`：不能绑定祖先目录，目录替换仍可把路径导向外部位置。
- 为单词本库复制一套目录遍历：会让它和 client-core 其它私有存储使用不同的链接策略。

## Consequences

单词本库的读取、发布和删除都绑定到当前库目录；并发替换目录时不会跟随外部符号链接。旧有索引、崩溃恢复和非 Unix 行为保持不变，公共句柄辅助函数可供其它私有库复用。

## Verification

`cargo test -p msime-client-core --lib vocabulary::library --locked --quiet`：13 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
