# Agent Note: 皮肤页搜索时隐藏分类导航

Status: implemented

## Problem

Android 设置的皮肤页进入「社区」并点击搜索框后，输入法弹出会压缩窗口，但皮肤页自己的「内置 / 我的 / 社区」分类导航仍固定显示在键盘上方，遮挡社区搜索列表。

## Decision

`SkinsPage` 根据窗口 inset 中的 IME 可见状态控制固定分类导航：输入法可见时隐藏导航，并把社区列表容器的顶部位置从「标题栏 + 导航高度」改为只保留标题栏高度；输入法收起时恢复导航和原有占位。判断逻辑复用 `WindowInsetsPolicy.shouldShowFixedPageNavigation`，并用 `WindowInsetsPolicySmoke` 覆盖两种状态。

## Alternatives considered

- 只让社区列表滚动到导航下方：导航仍然会固定在输入法上方，搜索结果可见区域仍被占用。
- 让导航留在页面上但调整透明度：不会释放顶部占位，也不能解决导航覆盖搜索内容的问题。

## Consequences

皮肤页社区搜索时，输入法和列表不再被分类导航夹在同一块底部空间里；输入法收起后分类切换和列表位置恢复。改动只作用于 `SkinsPage`。当前分支通过 `git diff --check`；本机磁盘空间不足，未启动 Android 模拟器，设备验收需由 CI 或维护者补做。

