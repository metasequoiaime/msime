# Agent Note: 通知缓存绑定目录句柄

Status: implemented

## Problem

通知缓存文件在目录锁内读取和写入，但读取前的符号链接检查及原子发布仍使用路径解析。缓存目录被替换时，操作可能读写外部文件。

## Decision

通知缓存读取复用共享私有文件入口，Unix 写入使用已打开目录句柄和 `write_private_file_at` 原子发布，非 Unix 保留临时文件发布。缓存损坏或不存在时仍按原逻辑回退为空缓存。

## Alternatives considered

- 保留路径检查后打开：检查和使用之间仍有竞态。
- 只保护缓存叶文件：父目录替换仍可改变目标。
- 为通知缓存复制目录安全实现：会和其它本地状态库不一致。

## Consequences

通知读取和 dismiss/update 写入绑定到通知目录，缓存回退、容量限制和锁行为不变。

## Verification

`cargo test -p msime-client-core --lib notices --locked --quiet`：13 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
