# Agent Note: 资源校验标记绑定目录句柄

Status: implemented

## Problem

资源校验标记读取和写回使用路径型文件操作。资源目录在检查后被替换时，缓存标记可能从其他位置读取或发布。

## Decision

标记读取改用 `open_private_file_in`；Unix 写回打开标记父目录并通过 `write_private_file_at` 原子发布；非 Unix 保留临时文件实现。

## Alternatives considered

- 只保留父目录符号链接检查：检查与路径 I/O 之间仍有替换窗口。
- 只修写入：读取缓存仍可能被重定向。
- 让资源模块自带系统调用：会重复共享 storage 的平台适配。

## Consequences

Unix 标记快路径的读写绑定已确认的父目录，缓存未命中和原子发布语义保持不变。

## Verification

`cargo test -p msime-client-core --lib resources --locked --quiet`：27 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
