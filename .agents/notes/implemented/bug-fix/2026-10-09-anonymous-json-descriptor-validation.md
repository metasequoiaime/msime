# Agent Note: 匿名账户 JSON 绑定打开句柄校验

Status: implemented

## Problem

匿名账户和会话 JSON 读取先通过路径元数据检查普通文件、权限和属主，再打开文件读取。目录项在两步之间被替换时，安全检查可能对应旧文件，而内容读取对应新文件。

## Decision

Unix 读取先打开父目录句柄，再用 `open_private_file_at` 打开叶文件；权限、属主和普通文件校验全部从这个已打开的文件句柄读取。非 Unix 保留原有路径实现并统一使用私有文件入口。

## Alternatives considered

- 只重复 `symlink_metadata`：仍无法消除检查和打开之间的替换窗口。
- 取消权限/属主校验：会扩大匿名凭据泄露面，不能接受。
- 复制 `FileAccountSessionStorage` 的逻辑：抽取目录句柄校验 helper 会更复杂，当前实现只需在匿名存储内保持明确边界。

## Consequences

Unix 匿名账户读取的安全属性绑定到同一个父目录和文件描述符，缺失文件、符号链接、权限错误及大小限制的既有错误语义保持不变。

## Verification

先运行新增测试确认辅助函数不存在时编译失败；实现后 `cargo test -p msime-client-core --lib account::anonymous --locked --quiet`：10 passed。`cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`cargo fmt --all` 和 `git diff --check` 均通过。
