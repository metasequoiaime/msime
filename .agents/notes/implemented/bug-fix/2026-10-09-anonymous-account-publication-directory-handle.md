# Agent Note: 匿名账户发布绑定目录句柄

Status: implemented

## Problem

匿名账户身份和会话原先先确认目录可用，再用 `NamedTempFile::persist` 或 `persist_noclobber` 按路径发布。目录项在检查后被替换时，临时文件创建和最终发布可能跟随替换后的目录，导致凭据写入攻击者控制的位置；匿名身份的 no-clobber 竞争语义也需要继续保留。

## Decision

Unix 路径先打开私有目录句柄，再通过 `openat` 在该目录中创建 0600 临时文件。普通发布使用已有的目录句柄 `renameat`；no-clobber 发布使用同目录的原子 `linkat`，遇到已有目标返回 `false`，成功后删除临时目录项。非 Unix 平台保留原有 `NamedTempFile` 实现和错误映射。

共享 storage 模块新增 no-clobber 目录句柄入口，并覆盖两点回归：已有目标不会被覆盖；目录路径替换成符号链接后，写入仍留在最初打开的目录中。

## Alternatives considered

- 只在 `persist` 前重新检查目录：检查与临时文件创建、发布之间仍有竞态。
- 把快照或所有流式文件发布一并改造：这些调用需要独立的流式目录句柄接口，本修复只处理匿名账户的凭据发布。
- 用 `renameat` 后再检查目标：检查无法恢复 no-clobber 的原子性，竞争进程仍可能先覆盖目标。

## Consequences

匿名账户的身份和会话文件在 Unix 上始终相对于已经打开的私有目录写入，目录替换不会把内容导向外部路径；两个进程同时初始化时仍由首个成功发布的身份获胜。非 Unix 行为保持不变。

## Verification

- 先运行 `cargo test -p msime-client-core --lib storage::tests::private_file_no_clobber --locked --quiet`，确认新增测试因入口不存在而失败。
- 实现后运行 `cargo test -p msime-client-core --lib storage::tests::private_file_no_clobber --locked --quiet`：2 passed。
- 运行 `cargo test -p msime-client-core --lib account::anonymous --locked --quiet`：10 passed。
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`cargo fmt --all`、`git diff --check` 通过。
