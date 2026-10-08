# Agent Note: 社区候选皮肤残留清理绑定目录句柄

Status: implemented

## Problem

社区候选皮肤安装结束时会清理 staging 目录，安装前还会删除旧 staging 和替换备份。Unix 路径型递归删除在根目录被替换后可能跟随符号链接清理外部目录。

## Decision

Unix 残留删除统一先打开父目录，再用 `remove_private_tree_at` 按目录句柄删除；安装结束的 staging 清理也复用该入口。非 Unix 保留原有路径实现。

## Alternatives considered

- 只重复检查根目录不是符号链接：检查与删除之间仍有竞态。
- 只拒绝 staging 叶子符号链接：父目录替换仍可把递归删除导向外部路径。

## Consequences

社区皮肤安装的 staging、替换备份和失败清理不会跟随被替换的根目录访问外部树；安装验证和原子替换语义保持不变。

## Verification

`cargo test -p msime-client-core --lib skin::candidate_community::tests:: --locked --quiet`：53 passed；新增符号链接父目录回归测试。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`git diff --check` 和 `npm run verify-notes` 通过。
