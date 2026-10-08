# Agent Note: 快照队列状态绑定目录句柄

Status: implemented

## Problem

快照队列状态读取使用路径型私有文件打开，状态写回在 Unix 上通过临时文件路径发布。状态目录在检查和 I/O 之间被替换时，路径竞态可能导致读取或原子写回偏离已确认的队列目录。

## Decision

状态读取统一使用 `open_private_file_in`。Unix 状态写回先打开队列目录句柄，再用 `write_private_file_at` 原子发布；非 Unix 保留现有临时文件实现。

## Alternatives considered

- 只增加重复的符号链接检查：检查与后续路径 I/O 之间仍有竞态窗口。
- 只修读取而保留路径型 `persist`：状态更新仍可能被目录替换重定向。
- 在快照队列内复制平台相关的 `openat` 逻辑：会偏离共享存储策略并增加维护分叉。

## Consequences

Unix 状态文件的读取和发布都绑定已打开的父目录，保留原子更新语义；非 Unix 行为与原实现一致。

## Verification

`cargo test -p msime-client-core --lib cloud::snapshot_queue --locked --quiet`：12 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
