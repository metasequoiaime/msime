# Agent Note: 候选皮肤同步状态绑定目录句柄

Status: implemented

## Problem

候选皮肤同步状态文件参与本地删除决策，原实现通过路径检查和路径型打开读取，并用临时文件路径重命名保存。状态文件父目录被替换时，路径竞态可能影响同步决策或把状态写到外部位置。

## Decision

状态读取复用共享私有文件入口；Unix 保存时打开父目录句柄并使用 `write_private_file_at` 原子发布，非 Unix 保留临时文件实现。已有符号链接拒绝保留，以维持同步遇到外部状态时返回存储错误的行为。

## Alternatives considered

- 只重复 `reject_symlink` 检查：检查与读写之间仍有窗口。
- 只修读取而保留路径 `persist`：恶意状态仍可把写入重定向。
- 为同步模块单独复制 `openat` 逻辑：会分叉 client-core 的统一存储策略。

## Consequences

同步状态读取、发布和后续删除决策绑定到状态文件父目录；状态损坏和符号链接错误仍按既有策略处理。

## Verification

`cargo test -p msime-client-core --lib skin::candidate_sync --locked --quiet`：41 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
