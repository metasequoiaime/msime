# Agent Note: 账户会话文件绑定父目录句柄

Status: implemented

## Problem

账户会话的读取、发布和清理先检查路径，再通过路径操作文件。账户目录的祖先在检查后被替换时，后续路径解析可能跟随符号链接，把令牌读写或删除导向外部目录。

## Decision

Unix 上账户会话读写统一先用共享 `storage` 辅助函数逐级打开账户目录，再用 `openat`、`renameat` 和相对目录句柄完成文件访问。清理复用私有文件删除入口；非 Unix 保留原有的临时文件原子发布实现。

## Alternatives considered

- 继续在 `reject_symlink` 后使用路径型打开：检查和打开之间仍存在祖先替换竞态。
- 只对会话文件增加 `O_NOFOLLOW`：只能保护最后一个路径组件，不能绑定祖先目录。
- 在账户模块单独实现目录遍历：会复制共享存储的符号链接和系统别名策略。

## Consequences

账户会话的读、写、删操作绑定到已打开的目录；并发替换账户目录时不会把令牌导向外部目录。正常保存仍通过临时文件和原子重命名发布，目录句柄逻辑集中在 `storage`，后续私有存储入口可复用同一套约束。

## Verification

`cargo test -p msime-client-core --lib --locked --quiet`：1071 passed, 1 ignored。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
