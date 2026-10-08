# Agent Note: 皮肤目录原子替换拒绝被替换的根目录

Status: implemented

## Problem

文件夹导入、社区候选皮肤安装、皮肤同步和插件导入共用目录替换入口。入口在路径上检查目标、改名备份、发布 staging 和回滚；根目录在调用间隙被替换时，路径操作可能跟随符号链接访问外部目录。

## Decision

Unix 实现要求目标和备份位于同一父目录（备份是目标原地改名得来的），staging 可以在另一个目录：插件导入在 `plugins/.staging-*` 暂存、装到 `plugins/<kind>/<id>`。两个父目录各以不跟随符号链接的方式打开一个句柄；目标改名、失败回滚和备份清理用目标父目录的句柄，staging 发布用跨句柄的 `renameat`，失败时的 staging 清理用它自己父目录的句柄。最初要求三者同一父目录，插件导入、社区皮肤安装和皮肤同步因此全部失败（20 条测试）。根目录被替换为符号链接时操作失败，不会写入或删除外部路径。非 Unix 保留原有流程。

## Alternatives considered

- 只在每次改名前重复检查路径：检查和使用之间仍存在竞态。
- 只拒绝目标符号链接：父目录替换仍会把整个替换流程导向外部目录。

## Consequences

原子替换和失败回滚语义保持不变；目录父项发生替换时安全失败，调用方可以按现有 storage 错误处理。

## Verification

`cargo test -p msime-client-core --lib skin::folder_import::tests:: --locked --quiet`：13 passed；新增根目录替换回归测试。放开 staging 父目录后，macOS 和 Linux（非 root）上 client-core 1102 条全部通过。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 通过。
