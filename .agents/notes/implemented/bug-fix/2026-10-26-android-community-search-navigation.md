# Agent Note: 输入法弹出时隐藏社区分类导航

Status: implemented

## Problem

Android 社区页进入词库搜索后，系统输入法弹出会压缩可用窗口高度，但页面自己的分类导航仍固定显示在键盘上方，遮挡搜索内容并与输入法争夺底部空间。

## Decision

`CommunityTabPage` 根据窗口 inset 中的 IME 可见状态控制页面分类导航：输入法可见时隐藏导航并取消其 48dp 的布局占位，输入法收起时恢复导航和占位。判断逻辑集中在 `WindowInsetsPolicy.shouldShowFixedPageNavigation`，并用纯 JVM smoke test 覆盖显示与隐藏两种状态。

## Alternatives considered

- 让分类导航继续显示并随窗口高度滚动：这仍会把导航固定在输入法上方，无法解决搜索结果可用空间被占用的问题。
- 只调整导航的透明度或层级：不会释放布局占位，也不能避免导航覆盖搜索内容，因此不满足问题场景。

## Consequences

社区页搜索时底部可用空间交给输入法和搜索内容，输入法收起后原有分类导航行为恢复。改动只作用于 `CommunityTabPage`，其他 Android 页面不受影响。当前分支已通过 `git diff --check` 和 `pnpm run verify-notes`；由于本机磁盘空间不足，尚未在 Android 模拟器上验收，设备验证需由 CI 或维护者补做。
