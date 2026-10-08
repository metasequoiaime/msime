# Agent Note: 社区资源库绑定父目录句柄

Status: implemented

## Problem

社区资源库在锁文件后通过路径检查、读取和临时文件持久化资源列表。文件父目录或祖先被替换时，路径操作可能跨出原本的应用数据目录。

## Decision

资源库读取统一使用共享私有文件入口；Unix 写入时打开父目录句柄，并使用 `write_private_file_at` 原子发布。非 Unix 保留 `NamedTempFile` 发布。

## Alternatives considered

- 保留 `symlink_metadata` 加路径读取：检查与打开之间仍有竞态。
- 只保护资源文件叶节点：父目录替换仍能重定向路径。
- 在社区模块单独实现 `openat` 逻辑：会重复 storage 的统一策略。

## Consequences

社区回复资源的读写绑定到锁住的父目录，祖先目录替换不会把数据导向外部文件；容量、校验和原子发布顺序不变。

## Verification

`cargo test -p msime-client-core --lib community::resource_library --locked --quiet`：4 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
