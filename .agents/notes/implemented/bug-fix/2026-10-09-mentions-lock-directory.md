# Agent Note: @ 名单锁绑定目录句柄

Status: implemented

## Problem

插件 @ 名单保存时按路径获取 `mentions.lock`，随后重新按路径打开目录并写入 `mentions.json`。目录被替换时，锁与文档可能落入不同目录，跨进程互斥失效，名单更新也可能写到错误目录。

## Decision

保存操作先打开并锁定名单目录，保留目录句柄和锁文件，再相对于该句柄原子发布 `mentions.json`。符号链接、格式校验和大小限制保持不变。

## Alternatives considered

- 每次写入前重复检查路径：仍无法消除目录替换窗口。
- 继续按路径写入：锁与文档仍可能脱钩。

## Consequences

其他共享存储的作用域释放由[共享文件锁守卫](2026-10-15-scoped-file-lock-release.md)统一处理；与本篇部分重叠，各自保留目录身份或租约边界的决定。

名单保存的锁、临时文件和最终文档始终绑定同一目录对象；目录替换不会把已取得锁的写入重定向到新目录。

## Verification

新增目录替换回归测试，旧实现失败、修复后通过。`cargo test -p msime-client-core plugins::mentions --locked` 和 `cargo clippy -p msime-client-core --all-targets --locked -- -D warnings` 均通过。
