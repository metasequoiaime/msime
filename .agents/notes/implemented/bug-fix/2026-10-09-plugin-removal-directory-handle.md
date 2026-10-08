# Agent Note: 插件卸载重命名绑定目录句柄

Status: implemented

## Problem

插件卸载在锁住插件根后检查目标，再通过路径执行删除或把已安装目录改名到临时名称。目录组件在检查和操作之间被替换时，路径型操作可能落到错误位置。

## Decision

插件目标为普通文件时复用私有文件删除入口；Unix 上目录卸载的关键重命名使用已打开的类型目录句柄和 `renameat`，把目标条目绑定到受信任的插件目录。非 Unix 保留原有路径调用。

## Alternatives considered

- 继续使用 `fs::rename`：祖先或类型目录替换后会重新解析路径。
- 只重复一次符号链接检查：检查与重命名之间仍有竞态。
- 为插件实现完整独立的目录遍历器：会复制共享存储逻辑，改动面大于此次目标条目绑定。

## Consequences

普通文件删除和目录重命名不再因最终路径竞态而导向外部条目。目录内容的后续清理仍沿用既有清理策略，插件锁和临时旁路命名保持不变。

## Verification

`cargo test -p msime-client-core --lib plugins --locked --quiet`：79 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
