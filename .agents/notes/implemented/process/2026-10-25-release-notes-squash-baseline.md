# Agent Note: squash 发布按等价源码树确定更新说明基线

Status: implemented

## Problem

Linux 0.13.0 的发布提交由 main 上的 squash 产生，不在 develop 的祖先中。直接计算 `linux-v0.13.0..develop` 会再次收录正式版已经包含的功能；人工对比时能找到与该 tag 完全相同的 develop 源码树，发布说明生成器却没有使用这份基线。

## Decision

`scripts/generate-release-notes.py` 在起始 tag 不是目标提交祖先时，从目标历史中查找与该 tag 完全相同的根 tree，使用找到的提交计算新增改动与 PR 归属。起始 tag 已经是祖先时保留原范围；找不到等价 tree 时同样保留原范围，避免 main 独有热修被误认为 develop 已经包含。

## Related notes

检索现有活跃笔记后，[空模型配置回退](../bug-fix/2026-10-15-release-notes-empty-model.md)处理请求模型选择，[桌面更新通道](../architecture/2026-10-25-update-channels-core.md)处理发布元数据和通道，两者与提交基线无重叠。此处不改变发布 tag、版本号、分支关系或生成说明的格式。

## Alternatives considered

- 为这次 Linux beta 手工选择起点或重写说明：可以立即得到正确文字，但后续从 develop 发布的版本仍会重复收录旧内容；在共享生成入口识别等价源码树，使所有平台的同类发布得到相同修复。
- 仅按发布时间截断历史：无需查找源码树，但延迟合入的提交可能早于发布时间，时间不能证明它已随旧版发布，因此不用它排除提交。

## Verification

`scripts/test-release-notes.py` 创建独立的合成 Git 仓库，重现正式版 squash 后从 develop 发新版的关系：修复前重复收录旧功能，修复后只保留新修复；main 有独有热修而树不相同时保持原范围。原有祖先 tag 与 merge／squash PR 归属测试继续覆盖正常路径。

## Consequences

生成说明会多读取一次目标提交的历史和根 tree，开销随提交数增长，不需要额外网络请求。没有等价源码树的分叉仍可能包含重复条目，脚本不猜测部分补丁是否已发布；此修复不改写既有公开说明。
