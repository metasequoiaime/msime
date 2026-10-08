# Agent Note: 版本记录绑定目录句柄

Status: implemented

## Problem

版本记录文件的读取、删除和原子写回都曾通过路径执行。状态目录在检查后被替换时，路径竞态可能让版本记录操作落到其他目录。

## Decision

记录读取统一使用 `open_private_file_in`；Unix 删除使用 `remove_private_file`，写回使用打开的状态目录句柄和 `write_private_file_at`，非 Unix 保留原有临时文件实现。

## Alternatives considered

- 只增加 `reject_symlink`：无法消除检查与后续路径操作之间的窗口。
- 只保护写回：读取和 full 版本清理仍可能访问被替换的目录。
- 在 edition 模块复制平台相关系统调用：会绕开共享 storage 封装。

## Consequences

Unix 版本记录的读、删、原子写回均相对于确认过的目录句柄执行，同时保持已有缺失记录和 full 版本行为。

## Verification

`cargo test -p msime-client-core --lib preferences::tests --locked --quiet`：114 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
