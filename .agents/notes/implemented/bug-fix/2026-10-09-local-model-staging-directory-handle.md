# Agent Note: 本地语音模型 staging 创建绑定父目录句柄

Status: implemented

## Problem

本地语音模型安装先打开模型根目录作为清理和发布的边界，随后却用根路径创建随机 staging 目录。根目录在这两个操作之间被替换成符号链接时，`create_dir` 会把 staging 建到外部目录，后续下载流程就可能把模型数据写出受信任的模型根。

## Decision

Unix 平台让 `Staging` 通过已打开的父目录句柄调用 `mkdirat` 创建 staging 目录；所有安装入口统一使用这个方法。非 Unix 保留原有路径实现。

## Alternatives considered

- 创建前再次检查根路径：检查与创建之间仍存在替换竞态。
- 只给 staging 路径加 `O_NOFOLLOW`：目录创建本身仍可能跟随被替换的祖先路径。

## Consequences

staging 根目录创建绑定到安装开始时确认的父目录对象，根路径被替换时不会在外部目录创建同名 staging。staging 内部文件和子目录仍由后续安装步骤管理。

## Verification

- `cargo test -p msime-client-core voice::local_models::tests:: --lib`
- `cargo fmt --all`
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`
- `cargo check -p msime-client-core --all-targets --locked`
- `git diff --check`
- `npm run verify-notes`
