# Agent Note: 遥测本地队列绑定目录句柄

Status: implemented

## Problem

遥测队列、状态和会话标记在目录锁内读取和写入，但通用文件入口仍先检查路径再打开，原子写入也通过路径持久化。目录替换可能让本地遥测状态访问错误位置。

## Decision

遥测普通文件读取统一经 `open_private_file_in`，删除统一经共享私有文件删除入口；Unix 原子 JSON 写入使用目录句柄和 `write_private_file_at`，非 Unix 保留临时文件实现。崩溃记录的专用创建路径保持其 `create_new` 和权限语义。

## Alternatives considered

- 仅重复符号链接检查：检查和实际访问之间仍有竞态。
- 只修队列而不修状态和会话标记：同一目录仍保留未加固入口。
- 用遥测模块自己的 `openat` 辅助函数：会重复 client-core 的存储策略。

## Consequences

遥测队列、重试状态、会话标记和清理操作绑定到遥测目录，网络发送、事件清洗和崩溃记录格式保持不变。

## Verification

`cargo test -p msime-client-core --lib telemetry --locked --quiet`：26 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
