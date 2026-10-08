# Agent Note: 社区候选皮肤 staging 写入绑定目录句柄

Status: implemented

## Problem

社区候选皮肤安装清理 staging 后，Unix 仍通过路径创建 staging 根、包目录、父目录和图片文件。`root` 在清理与写入之间被替换时，路径操作可能跟随符号链接，把下载内容写到外部目录。

## Decision

Unix 打开 root 目录句柄，用 `mkdirat` 创建 staging 层级，用 `open_private_directory_at` 绑定目录，并用 `write_private_file_at` 写入 manifest 和图片；包内父目录也逐级绑定句柄。非 Unix 保留原路径实现。

## Alternatives considered

- 只在每次写入前重新检查 root：检查与创建之间仍有竞态。
- 只给最终图片加 `O_NOFOLLOW`：staging 目录和父目录仍可能在外部树创建。

## Consequences

Unix staging 内容始终创建在安装开始时打开的 root 对象内；目录替换期间发布会失败或清理旧对象，不会把包文件写到外部目录。正常安装、替换和残留恢复语义保持不变。

## Verification

- `cargo test -p msime-client-core skin::candidate_community::tests:: --lib`
- `cargo fmt --all`
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`
- `git diff --check`
- `npm run verify-notes`
- `bash scripts/verify-local.sh --quick`
