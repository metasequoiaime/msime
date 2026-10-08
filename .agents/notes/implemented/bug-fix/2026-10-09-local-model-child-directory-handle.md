# Agent Note: 本地模型子目录创建绑定 staging 句柄

Status: implemented

## Problem

模型 staging 根目录已通过父目录句柄创建，但紧接着的 `model` 子目录仍通过路径创建。根目录在这两个操作之间被替换为符号链接时，安装流程会在外部目录创建子目录。

## Decision

Unix 平台通过 staging 父目录句柄重新打开 staging，再用 `mkdirat` 创建 `model` 子目录。四个安装入口统一调用此方法；非 Unix 保留原路径实现。

## Alternatives considered

- 再次检查根路径：检查和创建之间仍有竞态。
- 只在根目录创建时使用句柄：后续子目录仍可能被重定向。

## Consequences

`model` 子目录创建绑定到实际 staging 对象。后续文件下载、解包和发布仍各有独立路径操作，需要继续审计和句柄化。

## Verification

- `cargo test -p msime-client-core voice::local_models::tests:: --lib`
- `cargo fmt --all`
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`
- `cargo check -p msime-client-core --all-targets --locked`
- `git diff --check`
- `npm run verify-notes`
