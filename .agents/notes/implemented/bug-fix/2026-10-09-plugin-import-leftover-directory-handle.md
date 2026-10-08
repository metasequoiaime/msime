# Agent Note: 插件导入残留清理绑定目录句柄

Status: implemented

## Problem

插件导入会在失败或下次导入时扫描并递归删除 `.staging-*`、`.replaced-*` 和 `.old-*` 残留；staging 析构也会清理目录。路径型删除在插件根或 kind 子目录被替换后可能跟随符号链接访问外部树。

## Decision

Unix staging 保存父目录句柄，析构时按句柄删除；残留扫描仍保留年龄和命名规则，但删除通过扫描目录对应的已打开父目录句柄完成。非 Unix 保留原有路径实现。

## Alternatives considered

- 只在删除前重做符号链接检查：检查和递归删除之间仍有竞态。
- 只拒绝残留项本身是符号链接：父目录替换仍可把路径操作导向外部位置。

## Consequences

插件残留清理在目录替换期间最多跳过清理，不会删除根目录外的树；年龄阈值和导入失败恢复语义保持不变。

## Verification

`cargo test -p msime-client-core --lib plugins::tests::leftover --locked --quiet`：2 passed；新增 kind 根目录替换回归测试。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 通过。
