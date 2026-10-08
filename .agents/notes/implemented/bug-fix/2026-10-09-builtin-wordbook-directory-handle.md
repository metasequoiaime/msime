# Agent Note: 内置词库读取绑定目录句柄

Status: implemented

## Problem

内置词库加载在检查资源文件类型和大小后仍按路径打开文件。资源目录项被替换时，校验对象与反序列化内容可能不一致。

## Decision

内置词库 JSON 读取改用 `open_private_file_in`，保留文件名、大小、词库 id 和内容合法性检查。

## Alternatives considered

- 只重复文件元数据检查：无法消除检查到打开之间的竞态。
- 改动内置词库的打包布局：问题只涉及读取入口，不需要改变发布契约。

## Consequences

内置词库加载相对于已确认的父目录读取普通文件，缺失资源仍返回空列表，损坏资源仍返回原错误。

## Verification

`cargo test -p msime-client-core --lib vocabulary::builtin --locked --quiet`：7 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
