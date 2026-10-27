# Agent Note: Android 世界时钟卡片保留三行文字

Status: implemented

## Problem

世界时钟卡片同时显示城市时间、日期和日出日落说明。卡片复用普通按钮的 48dp 最小高度，在 Android 11 的字体和行距下可能裁掉多行中文文字，表现为系统时间页面的文字超出或显示不完整。

## Decision

只提高世界时钟 `row()` 的最小高度到 76dp，并保留现有自动换行和最多五行设置。世界时钟数据、格式、城市选择和其他工具行不变。

## Existing note audit

- [世界时钟设备验收](../feature/2026-10-23-offline-world-clock.md)：复用现有时钟卡片、格式和重启验收；本次只调整卡片容器高度。

## Alternatives considered

- 缩小字体：不用，会降低长城市名和辅助说明的可读性。
- 截断日出日落说明：不用，会丢失用户需要的参考城市信息。

## Consequences

世界时钟卡片会多占约 28dp 垂直空间，三行文本有足够高度显示；区域列表和其他工具行仍使用原来的最小高度。

## Verification

- `git diff --check`：通过。
- Android host 编译和 API 28/35 世界时钟设备验收待磁盘空间恢复到仓库规定的 50 GiB 启动线后补跑。
