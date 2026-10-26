# Agent Note: AI 设计皮肤输入时隐藏分类导航

Status: implemented

## Problem

Android AI 设计皮肤页有「设计 / 音效 / 动画」固定分类导航。点击描述输入框后，输入法弹出，导航仍显示在输入法上方；固定导航还占据页面顶部空间，使描述框必须额外滚动才能看见。

## Decision

在 `DetailPage` 提供受保护的固定导航 IME 策略开关，`AiSkinPage` 启用它。窗口 inset 报告 IME 可见时隐藏导航并重算详情滚动区的顶部占位，IME 收起时恢复导航；页面已有的 `PageInsetsPolicy` 继续负责滚动内容对输入法底部 inset 的避让。

## Alternatives considered

- 只修改 AI 页的滚动监听：滚动监听无法移除固定导航本身的顶部占位，且输入法显示时导航仍会覆盖可视区域。
- 在所有详情页无条件隐藏固定导航：会改变没有输入框或仍需要分类切换的页面行为，超出该 issue 的范围，因此保留子类显式开关。

## Consequences

AI 设计皮肤输入时，分类导航不再浮在输入法上方，描述框可以由现有滚动和 inset 逻辑移到键盘上方；输入法收起后分类导航恢复。其他详情页保持原行为。当前分支通过 `git diff --check`；本机磁盘空间不足，未启动 Android 模拟器，设备验收需由 CI 或维护者补做。
