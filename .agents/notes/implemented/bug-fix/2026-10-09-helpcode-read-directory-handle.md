# Agent Note: 自定义辅助码读取绑定目录句柄

Status: implemented

## Problem

自定义辅助码表读取已经检查资源目录和普通文件类型，但仍通过路径打开文件。资源目录项被替换时，检查对象与读取对象可能不一致。

## Decision

辅助码显示元数据读取改用 `open_private_file_in`，保留有界读取、UTF-8 和头部注释解析逻辑。

## Alternatives considered

- 只增加一次符号链接检查：无法消除检查到打开之间的竞态。
- 修改辅助码发现和排序逻辑：与文件打开安全无关，扩大范围没有收益。

## Consequences

自定义辅助码元数据相对于已确认的父目录读取普通文件，资源缺失和损坏仍按原规则忽略。

## Verification

`cargo test -p msime-client-core --lib helpcode --locked --quiet`：12 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。

目录与加载的扩展名解析、同主干选择规则见[扩展名一致性修复](2026-10-16-custom-helpcode-extension.md)；显示元数据继续使用本篇的目录句柄读取。
