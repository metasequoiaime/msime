# Agent Note: 偏好暂存文件清理绑定目录句柄

Status: implemented

## Problem

偏好写入前清理 `.tmp*` 暂存文件时，先用路径扫描目录，再用 `entry.path()` 删除文件。目录在扫描后被替换成符号链接，会把删除操作导向用户数据目录之外。

## Decision

Unix 使用已打开的偏好目录句柄枚举目录项，并通过 `openat` 检查文件和 `unlinkat` 删除过期暂存文件。目录被替换或变成符号链接时无法打开，清理直接跳过；非 Unix 保留原有路径实现。

## Alternatives considered

- 只在删除前重新检查符号链接：检查和删除之间仍有竞态。
- 继续用 `entry.path()` 删除：目录替换后仍可能跟随外部符号链接。

## Consequences

Unix 清理在目录被替换期间最多跳过过期文件，不会删除目录外的文件；正常目录下的年龄和命名筛选保持不变。

## Verification

- `cargo test -p msime-client-core preferences::tests:: --lib`
- `cargo fmt --all`
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`
- `git diff --check`
- `npm run verify-notes`
- `bash scripts/verify-local.sh --quick`
