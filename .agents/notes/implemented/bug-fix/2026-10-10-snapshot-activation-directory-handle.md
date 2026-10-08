# Agent Note: 词库快照激活绑定状态根目录句柄

Status: implemented

## Problem

快照激活取得维护锁后，仍通过可替换的状态根路径 `read_dir` 和 `rename`。同一用户进程可以在锁取得后替换目录，导致激活把文件操作重定向到另一棵目录，或让回滚与实际加锁的目录不一致。

## Decision

维护锁从已打开的状态根目录句柄中创建，并把句柄保存在 `DictionaryAccess` 中。激活取得句柄后，状态根之间的文件移动和回滚统一使用相对目录句柄的 `renameat`；路径被替换时句柄仍绑定原目录。新增的句柄重命名接口在非 Unix 平台保留路径实现。

## Alternatives considered

- 在每次 `rename` 前重新检查路径：检查和移动之间仍有竞态窗口。
- 只把维护锁改成句柄：激活自身的路径型移动仍可能落到替换后的目录。
- 在快照模块复制 Unix `renameat` 逻辑：会绕开 `client-core` 的共享私有存储边界。

## Consequences

激活成功、失败回滚和备份清理的文件移动都绑定到已经打开的目录；替换活动根路径不会把文件写入外部目录。激活仍在路径层读取目录项，但所有实际移动由已绑定句柄执行。

## Verification

- `cargo test -p msime-client-core --lib`：1128 passed。
- `cargo test -p msime-host-api --lib`：388 passed。
- `cargo clippy -p msime-client-core -p msime-host-api --all-targets --locked -- -D warnings`：通过。
- `cargo fmt --all`：通过。
- `bash scripts/verify-local.sh --quick`：通过（缺少工具链的阶段按门禁规则跳过）。
