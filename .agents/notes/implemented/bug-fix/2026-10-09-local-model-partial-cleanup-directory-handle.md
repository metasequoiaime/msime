# Agent Note: 本地语音模型部分下载清理绑定目录句柄

Status: implemented

## Problem

本地语音模型的 partial 文件和目录在下载失败、校验失败或安装完成后通过路径型删除清理。根目录或 `.partial-*` 父目录被替换时，清理可能跟随符号链接访问外部文件。

## Decision

Unix `remove_leftover` 先打开目标父目录，再使用 `remove_private_tree_at` 按目录句柄删除；非 Unix 保留路径实现。所有 partial 文件、partial 目录和发布备份的调用点因此共享同一保护。

## Alternatives considered

- 只重复 `symlink_metadata`：检查和删除之间仍有竞态。
- 只限制 partial 文件名：父目录替换仍能重定向路径型删除。

## Consequences

下载恢复和清理语义不变；目录不再可信时清理会跳过，不会删除根目录外的文件。

## Verification

`cargo test -p msime-client-core --lib voice::local_models::tests::leftover --locked --quiet`：2 passed；新增 partial 文件父目录替换回归测试。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 通过。
