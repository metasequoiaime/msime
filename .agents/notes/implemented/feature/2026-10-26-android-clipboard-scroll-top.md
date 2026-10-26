# Agent Note: Android 剪贴板历史提供回到顶部按钮

Status: implemented

## Problem

剪贴板历史页只有滚动列表，用户向下浏览后必须手动连续滚动才能回到最早记录（#9787）。

## Decision

在详情页共享滚动回调上增加页面级 `onScrollChanged` 扩展点。剪贴板历史页在滚动位置大于零时显示右下角「返回顶部」按钮，点击后平滑滚动到零；回到顶部时隐藏按钮。显示规则独立放在 `ScrollTopButtonPolicy`，避免把滚动状态判断复制进页面。

## Alternatives considered

- **只在剪贴板页自己替换详情页滚动监听**：会覆盖详情页原有的大标题收起逻辑，页面状态会失真。
- **把按钮放在列表末尾**：不能在长列表中快速触达，也不符合浮动按钮的交互目标。

## Consequences

长列表下滑后可一键回到顶部，按钮不占用列表行；详情页增加了一个通用滚动通知点，其他页面默认不改变行为。按钮按详情页底部导航预留高度放置，避免遮住最后一行。

## Verification

- 先加入 `ScrollTopButtonPolicySmoke`，确认修改前因缺少策略类而编译失败。
- 修改后运行 `bash platforms/android/check-host.sh`：277 个 Android JVM 冒烟、245 个设备套件源码编译、JNI/API、资源和契约检查通过。
- 尚未在 Android 模拟器或真机上进行实际触摸验收。
