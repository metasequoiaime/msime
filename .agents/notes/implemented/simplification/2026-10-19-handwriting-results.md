# Agent Note: 抽取手写识别结果视图

Status: implemented

## Problem

手写面板已经把状态和笔迹编辑区拆开，但候选模式、候选网格、复制与输入辅助按钮、空状态和状态提示仍内联在 `keyboard/panels.tsx`。已有候选按钮组件只能复用单个按钮，不能直接承载一块完整的识别结果视图。

## Decision

`HandwritingResults` 是受控的公共结果视图，其 props 通过包入口导出，组件组合现有 `PanelActivationButtons`、`HandwritingCandidateButton` 和 `StatusMessage`。调用方提供候选、有效模式、能力标志、禁用状态、状态文案和模式/选择/导航回调。模型提示经 `children` 插槽留在状态提示之前，视图不依赖资源下载或宿主客户端。保留原有 section、分组、按钮、辅助操作与空状态 DOM，不新增状态或能力回退逻辑。

## Alternatives considered

- **只抽候选网格**：props 更少，也能复用候选列表；但调用方仍需重复模式、空状态、提示和结果区布局，因此不采用。
- **把客户端和完整手写 hook 状态交给结果组件**：调用接线最短，却使展示层依赖笔迹、下载、关闭和异步代次等无关状态；采用显式视图 props 与插槽保留这些边界。

## Verification

- 新增 8 项组件用例，先确认公共组件缺失时失败，迁移后通过；覆盖模式与能力分支、主次操作、复制快捷键、禁用、空状态、模型插槽和网格事件目标。
- 连同候选按钮、手写面板、指针、模式按钮、窗口订阅和模型资源回归，共 7 个文件、73 项通过。
- 设置整页中的手写相关用例另有 12 项通过，无关用例跳过。
- 桌面生产构建、HarmonyOS 类型检查、lint 和修改文件格式检查覆盖公共入口及消费者。

## Consequences

调用方可以独立复用完整结果区，代价是必须明确传入有效模式和能力标志；能力回退继续由原 hook 持有。模式与能力分支移动时容易把候选的主动作与辅助动作调换；网格包装变化也会改变导航回调的 `currentTarget`。保持原 DOM 和回调参数，通过独立组件测试及原面板回归验证。

## Existing notes audit

[手写面板状态](./2026-10-19-handwriting-panel-state.md)与本片部分重叠，继续持有生命周期，本片接管结果区 JSX；[候选按钮](./2026-10-19-handwriting-candidate-button.md)保留尺寸和快捷键契约，同步其调用方路径。[笔迹编辑区](./2026-10-19-handwriting-ink-editor.md)、[模型提示](./2026-10-19-handwriting-model-notice.md)、[模式按钮](./2026-10-17-shared-panel-activation-buttons.md)和[聚焦显示](./2026-10-17-focus-reveal-nearest.md)分别持有并列或底层机制，保留原决定；平台候选网格笔记与本次共享 React 视图无关。没有需吸收或废弃的旧提案。
