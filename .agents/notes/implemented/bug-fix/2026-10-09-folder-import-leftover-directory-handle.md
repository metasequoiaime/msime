# Agent Note: 文件夹皮肤导入残留清理绑定目录句柄

Status: implemented

## Problem

文件夹皮肤导入开始时会删除旧 `.import-*` staging 和 `.replaced-*` 备份。路径型递归删除在皮肤根目录或父目录被替换后可能跟随符号链接删除外部内容。

## Decision

Unix 残留删除先打开父目录句柄，再调用 `remove_private_tree_at`；非 Unix 保留原有实现。目录替换入口已经对父目录句柄做同样约束。

## Alternatives considered

- 只再次检查根目录：检查和删除之间仍有竞态。
- 只检查残留项类型：父目录替换仍可能重定向路径操作。

## Consequences

导入会在根目录不再可信时安全失败，不会清理根目录外的 staging 或备份；正常导入和恢复语义保持不变。

## Verification

`cargo test -p msime-client-core --lib skin::folder_import::tests::leftover --locked --quiet`：1 passed；新增符号链接父目录回归测试。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 通过。
