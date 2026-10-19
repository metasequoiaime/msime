# Agent Note: 抽取手写候选按钮组件

Status: implemented

## Problem

手写面板把候选按钮的尺寸监听、自适应字号、复制快捷键和按钮语义写在大型 `keyboard/panels.tsx` 中。候选网格本身继续由面板状态控制，但按钮的可复用视图契约无法独立测试或供其他手写候选列表使用。

## Decision

把 `HandwritingCandidateButton` 移到 `keyboard/handwriting-candidate-button.tsx`，通过 `@msime/ui` 导出其 props 和组件。保留现有 `ResizeObserver`/窗口缩放回退、Unicode 码点长度计算、字号边界、`Control+C` 复制快捷键、组合输入保护和 `disabled` 行为；[手写识别结果组件](./2026-10-19-handwriting-results.md)负责传入候选和动作回调，面板继续提供状态与宿主能力。

## Alternatives considered

- **继续内联按钮**：改动最小，但尺寸生命周期和快捷键行为只能通过整个手写面板间接验证，后续候选列表无法复用，因此不采用。
- **把字号计算单独抽成纯函数**：能复用数学计算，却仍把尺寸监听和复制快捷键留在面板文件，无法形成完整按钮契约，因此不采用。

## Verification

- 现有手写候选按钮 DOM、样式、标题、ARIA 快捷键和点击动作保持不变。
- 元素初次测量、`ResizeObserver` 通知、旧宿主窗口缩放回退和卸载清理保持不变。
- `Control+C` 只在非组合、非重复、未禁用且提供复制动作时触发一次。
- 公共组件有独立测试，手写面板和现有候选回归继续通过。
- 窗口事件调用方契约同步检查抽取后的按钮文件；旧路径在全套测试中失败，迁移路径后定向验证通过。

## Consequences

移动时最容易遗漏旧宿主窗口回退或把组合输入误当作复制快捷键；独立测试覆盖这些分支，并用原手写面板回归确认接线没有变化。尺寸观察的公共订阅边界继续由[元素尺寸订阅](./2026-10-15-element-resize-subscription.md)持有，本片只移动按钮的组合逻辑。
