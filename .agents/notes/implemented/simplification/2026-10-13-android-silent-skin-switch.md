# Agent Note: Android 皮肤切换成功后不再显示提示

Status: implemented

## Problem

Android 键盘选择皮肤后已经立即刷新外观，保存成功又显示「皮肤已切换」，重复反馈打断正常切换。

## Decision

`MSIMEInputService.finishKeyboardSkinSave` 应用成功响应后不再显示「皮肤已切换」。保存、偏好快照应用、皮肤刷新和面板收尾沿用现有流程；保存失败和更新竞争的提示保留。

历史检索：触屏皮肤网格请求代次、社区皮肤库请求代次两篇笔记处理共享 React 加载生命周期，与 Android 原生保存完成后的提示无关；未找到该成功提示的既有决定。

## Alternatives considered

缩短成功提示的显示时间能保留显式确认，但外观变化已经提供反馈，仍不符合正常切换时不显示提示的要求。

## Consequences

成功保存不再另行确认，用户通过皮肤实际外观确认切换结果；本次仅删除提示调用，不修改 Android 文件访问或资源。

`bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 和 `git diff --check` 均通过；quick 包含 Android 目标检查和宿主 Java 冒烟。未做模拟器界面验收。
