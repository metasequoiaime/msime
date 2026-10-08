# Agent Note: 候选皮肤同步删除绑定目录句柄

Status: implemented

## Problem

候选皮肤同步删除本地包时，先按路径改名到 `.removed-*`，再按路径递归删除。皮肤根目录或其父目录被替换后，清理可能跟随符号链接访问根目录外的树；失败回滚也可能把目录改回错误路径。

## Decision

Unix 删除流程打开皮肤根目录句柄后，用 `renameat` 将包移到旁边，再通过同一目录句柄递归删除和失败回滚。通用的残留删除入口在 Unix 上也改用父目录句柄；非 Unix 保留原有实现。

## Alternatives considered

- 只在删除前重复检查根目录：检查和改名之间仍可发生目录替换。
- 只拒绝根目录符号链接：已打开的根目录父项仍可能被替换，路径回滚仍不安全。

## Consequences

目录替换期间不会把候选皮肤删除或恢复到外部路径，原有先移出目录可见范围再删除的语义保持不变。

## Verification

`cargo test -p msime-client-core --lib skin::candidate_sync::tests:: --locked`：42 passed；新增符号链接父目录回归测试。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 通过。
