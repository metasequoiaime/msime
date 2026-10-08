# Agent Note: 私有文件路径入口统一跨平台实现

Status: implemented

## Problem

前几批目录句柄加固分别在调用方展开 Unix 与非 Unix 分支，导致相同的“根据路径打开私有文件”语义重复，且容易让跨平台代码在后续修改中出现编译分叉。

## Decision

在 `storage` 增加 `open_private_file_in`：Unix 通过父目录句柄和 `openat` 打开，其他平台复用既有私有文件打开实现。单词本库、命名词库集合和背词进度统一调用该入口，删除也复用共享的私有文件删除入口。

## Alternatives considered

- 每个模块继续保留自己的条件编译：重复逻辑多，Windows 分支容易被 Unix 修复遗漏。
- 为 Windows 模拟 `openat`：超出本批目标，且现有 Windows 私有打开策略已经满足叶节点不跟随要求。

## Consequences

三个存储模块的调用代码更短，Unix 仍绑定父目录，非 Unix 仍走原有实现；跨平台编译边界集中在 `storage`。

## Verification

相关模块测试全部通过：单词本库 13、背词进度 20、命名词库集合 18。`cargo check -p msime-client-core --locked`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`cargo fmt --all` 和 `git diff --check` 均通过。
