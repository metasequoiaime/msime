# Agent Note: telemetry 崩溃记录清理绑定目录句柄

Status: implemented

## Problem

关闭 telemetry 时会递归删除 `telemetry-crashes`。路径型删除与 telemetry 根目录的检查、锁定分离，根目录被替换后可能把清理操作导向外部路径。

## Decision

Unix `clear` 在持有锁后打开 telemetry 根目录，并用 `remove_private_tree_at` 按句柄删除 crash 目录；非 Unix 保留原有实现。

## Alternatives considered

- 只重复检查根目录：检查和删除之间仍有目录替换竞态。
- 只检查 crash 目录不是符号链接：父目录替换仍可能重定向递归删除。

## Consequences

目录替换时清理会安全失败或跳过，不会删除根目录外的内容；队列、会话标记和安装 ID 语义不变。

## Verification

`cargo test -p msime-client-core --lib telemetry::tests::clear_drops_everything_but_the_install_id --locked --quiet`：1 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 通过。
