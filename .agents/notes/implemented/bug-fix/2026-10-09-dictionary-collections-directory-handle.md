# Agent Note: 命名词库集合绑定目录句柄

Status: implemented

## Problem

命名词库集合在目录锁内读取、原子写入和删除集合文件，但实际文件操作仍通过路径解析。目录祖先被并发替换时，路径可能跟随符号链接，把集合数据读写或删除到外部目录。

## Decision

Unix 上集合索引、集合正文、待发送队列和回执文件统一通过已打开的集合目录句柄访问。读取使用 `openat`，发布使用共享的临时文件加 `renameat`，删除使用 `unlinkat`；非 Unix 保留原有实现。

## Alternatives considered

- 继续依赖 `symlink_metadata` 后的路径操作：检查与使用之间仍有竞态。
- 只检查集合文件的最终组件：无法防止祖先目录替换。
- 为命名词库单独复制目录安全逻辑：会使多个私有存储入口的策略分叉。

## Consequences

集合状态的读写删操作绑定到锁住的目录，目录替换不会把数据导向外部位置。集合文件的原子发布、崩溃顺序和错误码保持不变，共享 `storage` 句柄辅助函数继续作为安全边界。

## Verification

`cargo test -p msime-client-core --lib dictionary::collections --locked --quiet`：18 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
