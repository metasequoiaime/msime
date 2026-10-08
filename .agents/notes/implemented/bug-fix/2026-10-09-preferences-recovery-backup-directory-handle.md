# Agent Note: 偏好恢复备份绑定目录句柄

Status: implemented

## Problem

恢复损坏偏好时，`write_backup` 用路径型 `create_new` 创建 `preferences.json.corrupt-*`。偏好目录在锁定后被替换成符号链接时，备份可能被写到外部目录。

## Decision

Unix 先打开偏好目录，再用 `openat` 配合 `O_EXCL|O_NOFOLLOW` 创建备份，写入失败用 `unlinkat` 清理；非 Unix 保留现有实现。

## Alternatives considered

- 只在路径写入前重新检查符号链接：检查与创建之间仍有竞态。
- 先创建路径文件再检查父目录：外部文件已经可能被创建。

## Consequences

恢复备份始终落在打开的偏好目录内；目录被替换时操作失败，不会写入外部树。备份命名冲突和损坏文件保留语义不变。

## Verification

- `cargo test -p msime-client-core preferences::tests:: --lib`
- `cargo fmt --all`
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`
- `git diff --check`
- `npm run verify-notes`
- `bash scripts/verify-local.sh --quick`
