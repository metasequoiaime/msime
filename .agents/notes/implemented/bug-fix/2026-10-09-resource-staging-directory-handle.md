# Agent Note: 资源安装 staging 创建绑定目录句柄

Status: implemented

## Problem

资源安装已经打开资源根目录句柄并用它清理、发布，但 staging 仍通过 `tempdir_in(&self.root)` 按路径创建。根路径在打开句柄后被替换成符号链接时，临时目录会被创建到外部目录。

## Decision

Unix 平台用已打开的根目录句柄和 `mkdirat` 创建随机 `incoming-*` 目录，并由带句柄的 stage 守卫负责清理；非 Unix 保留 `tempdir_in` 实现。

## Alternatives considered

- 创建前再次调用根路径检查：检查与 `tempdir_in` 之间仍有竞态。
- 只检查生成后的 staging 路径：外部目录已经被创建，无法撤销竞态窗口。

## Consequences

资源 staging 根目录创建固定在安装开始时打开的资源根对象内。后续文件下载仍使用 staging 路径，继续保持现有文件级 `O_NOFOLLOW` 防护。

## Verification

- `cargo test -p msime-client-core resources::tests:: --lib`
- `cargo fmt --all`
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`
- `cargo check -p msime-client-core --all-targets --locked`
- `git diff --check`
- `npm run verify-notes`
