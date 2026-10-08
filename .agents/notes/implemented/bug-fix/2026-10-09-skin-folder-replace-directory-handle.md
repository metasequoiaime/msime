# Agent Note: 皮肤目录原子替换拒绝被替换的根目录

Status: implemented

## Problem

文件夹导入和社区候选皮肤安装共用目录替换入口。入口在路径上检查目标、改名备份、发布 staging 和回滚；根目录在调用间隙被替换时，路径操作可能跟随符号链接访问外部目录。

## Decision

Unix 实现要求 staging、目标和备份位于同一父目录，并先以不跟随符号链接方式打开父目录句柄；目标改名、staging 发布、失败回滚和备份清理全部使用该句柄。根目录被替换为符号链接时操作失败，不会写入或删除外部路径。非 Unix 保留原有流程。

## Alternatives considered

- 只在每次改名前重复检查路径：检查和使用之间仍存在竞态。
- 只拒绝目标符号链接：父目录替换仍会把整个替换流程导向外部目录。

## Consequences

原子替换和失败回滚语义保持不变；目录父项发生替换时安全失败，调用方可以按现有 storage 错误处理。

## Verification

`cargo test -p msime-client-core --lib skin::folder_import::tests:: --locked --quiet`：13 passed；新增根目录替换回归测试。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 通过。
