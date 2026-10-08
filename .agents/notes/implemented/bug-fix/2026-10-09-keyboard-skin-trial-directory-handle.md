# Agent Note: 键盘皮肤试用记录绑定目录句柄

Status: implemented

## Problem

键盘皮肤试用记录在锁住状态目录后仍通过路径检查、读取、写入和删除记录。状态目录被替换时，这些操作可能访问外部路径。

## Decision

试用记录读取使用共享私有文件入口，Unix 写入通过父目录句柄和原子 `renameat` 发布，删除复用目录句柄删除入口；非 Unix 保留临时文件发布。

## Alternatives considered

- 继续用路径元数据检查再打开：检查与使用之间仍有竞态。
- 只拒绝记录文件符号链接：父目录替换仍可重定向。
- 在皮肤模块单独维护句柄代码：会与其它本地状态库产生分叉。

## Consequences

试用恢复、保留和清理都绑定到受信任状态目录，崩溃恢复和偏好回滚逻辑不变。

## Verification

`cargo test -p msime-client-core --lib skin::keyboard_trial --locked --quiet`：6 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
