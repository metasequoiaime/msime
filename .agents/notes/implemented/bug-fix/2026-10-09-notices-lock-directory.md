# Agent Note: 通知缓存锁绑定目录句柄

Status: implemented

## Problem

通知缓存原先按路径获取 `notices.lock`，随后按路径读取和写入 `notices.json`。缓存目录被替换时，锁与缓存文件可能落入不同目录，跨进程互斥失效，通知状态也可能写到错误目录。

## Decision

通知缓存的刷新和关闭操作先打开并锁定目录，再相对于同一目录句柄读取和原子发布缓存文件。只读测试辅助路径保留原有容错行为。

## Alternatives considered

- 每次读写前重新检查目录：仍无法消除目录替换窗口。
- 继续按路径读写：锁文件与通知缓存仍可能脱钩。

## Consequences

其他共享存储的作用域释放由[共享文件锁守卫](2026-10-15-scoped-file-lock-release.md)统一处理；与本篇部分重叠，各自保留目录身份或租约边界的决定。

通知缓存的锁、读取和写入绑定到同一目录对象；目录替换不会重定向已取得锁的状态操作，缓存损坏时的容错语义保持不变。

## Verification

新增目录替换回归测试，修复后通过。`cargo test -p msime-client-core notices --locked` 和 `cargo clippy -p msime-client-core --all-targets --locked -- -D warnings` 均通过。
