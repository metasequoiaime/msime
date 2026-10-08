# Agent Note: 剪贴板历史绑定目录句柄

Status: implemented

## Problem

剪贴板历史在锁住旁路锁文件后，读取使用路径型私有文件打开，持久化使用 `NamedTempFile::persist`。历史文件父目录被替换时，路径竞态可能把用户剪贴板数据读写到外部位置。

## Decision

读取复用 `open_private_file_in`，Unix 持久化使用已打开父目录句柄和 `write_private_file_at` 原子发布，非 Unix 保留临时文件实现。已有符号链接检查继续保留以维持稳定错误码。

## Alternatives considered

- 只保留 `reject_symlink` 后的路径操作：检查与实际访问之间仍有窗口。
- 仅保护历史文件叶节点：父目录替换仍可重定向临时发布。
- 为剪贴板模块独立实现目录遍历：会重复共享存储安全逻辑。

## Consequences

剪贴板历史加载、追加、删除、置顶和清空都绑定到历史文件父目录，锁文件和内存状态回滚行为不变。

## Verification

`cargo test -p msime-client-core --lib clipboard --locked --quiet`：19 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
