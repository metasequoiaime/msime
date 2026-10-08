# Agent Note: 私有目录树删除绑定目录句柄

Status: implemented

## Problem

插件包和本地语音模型删除时，会先把目标目录重命名到 aside，再用路径型 `remove_dir_all` 删除。父目录或 aside 项在这段时间被替换时，递归删除可能跟随路径进入外部目录；符号链接叶子也不应被递归跟随。

## Decision

共享 storage 新增 Unix `remove_private_tree_at`：从已打开的父目录句柄按 `openat(..., NOFOLLOW|DIRECTORY)` 进入子目录，使用目录 fd 枚举条目，普通文件和符号链接用 `unlinkat` 删除，目录完成递归后用 `unlinkat(..., REMOVEDIR)` 删除。插件包和本地语音模型的 Unix aside 重命名与清理都改为目录句柄操作；非 Unix 保留原有实现。

## Alternatives considered

- 在 `remove_dir_all` 前重新检查路径：检查和递归之间仍有目录替换竞态。
- 只拒绝 aside 符号链接：内部子目录或父目录替换仍可能重定向删除。
- 先 canonicalize 再删除：无法固定目录 inode，且会扩大对符号链接的信任范围。

## Consequences

Unix 删除始终相对于已打开的受信任父目录，外部目录不会因路径替换被误删；符号链接项作为叶子被移除而不跟随。非 Unix 行为保持不变。

## Verification

新增 storage 回归测试覆盖父目录替换后目录树仍在原目录删除。插件测试：49 passed；本地语音模型测试：53 passed，1 ignored；storage 回归测试通过。`cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`cargo fmt --all` 和 `git diff --check` 通过。
