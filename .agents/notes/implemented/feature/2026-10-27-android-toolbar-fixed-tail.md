# Agent Note: Android 功能面板固定尾部入口

Status: implemented

## Problem

Android 功能面板加入新入口后，设置、反馈、宣传、官网和关于可能被挤到不同页或改变相对位置。用户希望这五个常用入口始终固定在面板尾部，新增功能只占用它们之前的位置。

## Decision

调整 `FunctionPanelModel` 的静态顺序，把五个入口按 `设置`、`反馈`、`宣传`、`打开官网`、`关于` 放在全部可扩展工具之后。页面和分页逻辑不变，模型测试锁定固定尾部的顺序。

## Existing note audit

- 无直接归属旧笔记；保留现有四列三行分页和条目身份，只调整固定尾部顺序。

## Alternatives considered

- 在渲染层按页强行移动入口：不用，模型和无障碍顺序会与视觉顺序分离，分页边界也会重复实现。
- 把五项从分页模型中单独绘制：不用，会破坏统一的条目状态、点击和无障碍契约。

## Consequences

固定尾部占据模型最后九个槽位中的五项，当前已有工具的分页位置会随尾部整理而变化；新增工具应插入固定尾部之前。

## Verification

- `FunctionPanelModelSmoke` 锁定第三页和全量列表的固定尾部顺序。
- Android host 编译和设备面板验收待磁盘空间恢复到仓库规定的 50 GiB 启动线后补跑。
- `MoreToolsDeviceSmoke` 当时没跟着改，设置不在第一页后它一直失败，已按固定尾部改成跨页全序检查，见[夜间设备验收归因](../bug-fix/2026-10-28-android-nightly-device-failures.md)。
