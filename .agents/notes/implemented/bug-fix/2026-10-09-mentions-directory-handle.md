# Agent Note: 插件 mentions 文件绑定目录句柄

Status: implemented

## Problem

插件 mentions 文档读取使用路径型打开，保存通过临时文件路径重命名。插件目录被替换时，检查与 I/O 之间的竞态可能让名单读写偏离原目录。

## Decision

读取改用 `open_private_file_in`；Unix 保存先打开插件目录句柄，再用 `write_private_file_at` 原子发布；非 Unix 保留临时文件实现。

## Alternatives considered

- 只重复符号链接检查：无法覆盖检查后的目录替换窗口。
- 只改读取：保存时的路径型 `persist` 仍可能重定向文档。
- 在 mentions 模块自行实现 openat：会绕过 client-core 共享 storage 封装。

## Consequences

Unix mentions 文档读写均绑定确认过的插件目录，同时保持旧文档/新文档原子切换语义和非 Unix 行为。

## Verification

`cargo test -p msime-client-core --lib plugins::mentions --locked --quiet`：4 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
