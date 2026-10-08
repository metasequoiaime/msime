# Agent Note: 资源残留阶段清理绑定目录句柄

Status: implemented

## Problem

资源安装在持有 `resources.lock` 时清理 `incoming-*` 残留目录，但 Unix 通过 `read_dir` 得到路径后调用 `remove_dir_all`。root 被替换后，清理可能跟随新路径进入外部目录。

## Decision

Unix 阶段清理直接从已打开的 root 目录句柄枚举条目，并调用共享 `remove_private_tree_at` 递归删除；非 Unix 保留原有实现。清理仍只处理真实目录且失败继续安装。

## Consequences

残留阶段清理固定在锁住的 root 目录 inode 内，不会因路径替换删除外部树；资源安装的锁、年龄和异常容忍语义不变。

## Alternatives considered

- 在 `remove_dir_all` 前重新检查路径：递归过程仍可被目录替换重定向。
- 只拒绝残留项符号链接：父目录替换仍可能改变删除目标。

## Verification

新增 root 替换为符号链接后的清理回归测试先在路径实现上失败，改为目录句柄枚举后通过。资源测试：26 passed；clippy、fmt、diff 检查通过。
