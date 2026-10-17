# Agent Note: 复用全屏设置子视图的滚动回顶逻辑

Status: implemented

## Problem

AI 皮肤抽卡和社区图库都覆盖所在设置页的滚动容器。它们各自保存一个 `ref`，在 `useLayoutEffect` 里调用 `scrollIntoView({ block: "start" })`，保证全屏视图的标题和返回按钮在顶部；这段生命周期逻辑容易在新增全屏子视图时漏掉，或出现两个调用的触发条件不一致。

## Decision

新增 `useFullScreenScrollToTop`，返回全屏容器的 ref，并在启用时于布局 effect 中把容器滚到顶部。AI 皮肤抽卡把 `fullScreen` 作为启用条件，社区图库始终启用；全屏样式、标题、内容和滚动容器的所有权仍由调用方决定。

## Alternatives considered

- **继续在每个全屏视图内联 ref 和 effect。** 代码很短，但相同的滚动时机和 `scrollIntoView` 参数会继续复制，后续视图容易只实现一半。
- **让 `fullScreenSurface` 样式组件自动滚动。** 这样可以少传一个 hook，但样式常量不是 React 组件，且会把滚动副作用绑定到所有使用该样式的宿主，无法表达 AI 抽卡的条件启用。

## Consequences

- AI 皮肤抽卡和社区图库继续在打开时回到顶部，现有标题和返回交互不变。
- 全屏滚动逻辑有一个可复用的公共 Hook，并从 `@msime/ui` 导出；调用方仍掌握启用条件和容器结构。
- Hook 测试覆盖启用与停用分支，相关桌面页面测试、类型检查、笔记门禁和 quick 门禁作为提交验证。
