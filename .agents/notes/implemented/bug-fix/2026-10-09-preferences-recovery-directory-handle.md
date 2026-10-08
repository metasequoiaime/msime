# Agent Note: 偏好恢复读取绑定目录句柄

Status: implemented

## Problem

偏好正常加载已使用私有文件入口，但损坏文档恢复分支重新读取原文件时仍使用路径型打开，目录项替换可能让备份和修复依据偏离锁内检查的文档。

## Decision

恢复分支改用 `open_private_file_in`，保留锁、大小限制、JSON 解析、备份和版本恢复逻辑。

## Alternatives considered

- 只依赖正常 `read_locked`：恢复流程必须读取损坏文档原始字节，不能跳过。
- 重写整个恢复事务：问题只在文件读取入口，避免影响备份和版本号语义。

## Consequences

偏好恢复在锁内通过私有文件读取入口获取损坏文档，现有恢复结果和错误映射保持不变。

## Verification

`cargo test -p msime-client-core --lib preferences::tests --locked --quiet`：115 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
