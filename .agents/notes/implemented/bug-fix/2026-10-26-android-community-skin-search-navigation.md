# Agent Note: 输入法弹出时隐藏社区皮肤分类导航

Status: implemented

## Problem

Android 从社区根页进入皮肤分类后，点击搜索框会弹出输入法，但社区页自己的「皮肤 / 词库」分类导航仍固定显示在键盘上方，遮挡搜索内容并占用底部空间。

## Decision

`CommunityTabPage` 根据窗口 inset 中的 IME 可见状态控制分类导航：输入法可见时隐藏导航并取消其 48dp 的布局占位，输入法收起时恢复导航和占位。判断逻辑集中在 `WindowInsetsPolicy.shouldShowFixedPageNavigation`，并用 `WindowInsetsPolicySmoke` 覆盖显示与隐藏两种状态。

## Alternatives considered

- 让分类导航继续显示并随窗口高度滚动：导航仍会固定在输入法上方，无法解决搜索结果可用空间被占用的问题。
- 只调整导航透明度或层级：不会释放布局占位，也不能避免导航覆盖搜索内容。

## Consequences

社区皮肤搜索时，输入法和搜索列表不再与分类导航争夺同一块底部空间，输入法收起后原有分类切换恢复。改动只作用于 `CommunityTabPage`。当前分支通过 `git diff --check` 和 `pnpm run verify-notes`；本机磁盘空间不足，未启动 Android 模拟器，设备验收需由 CI 或维护者补做。
