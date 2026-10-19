# Agent Note: 抽取手写模型下载提示组件

Status: implemented

## Problem

手写模型缺失、下载进度、失败重试和下载镜像编辑都挤在 `keyboard/panels.tsx`，面板主体因此同时承担手写画板和资源包提示的视图职责。资源包提示已经有独立的设置页资源包契约，继续内嵌在面板文件里会让后续复用和单独验证变得困难。

## Decision

把 `HandwritingModelNotice` 及其内部的 `HandwritingMirrorEditor` 移到 `keyboard/handwriting-model-notice.tsx`，公开导出前者，继续使用现有 `ResourcePacks`、`ModelMirrorClient` 和模型格式化/校验函数。`panels.tsx` 只保留资源包提示的调用，下载、取消、失败重试和镜像保存行为保持不变。

## Alternatives considered

- **继续保留在 `panels.tsx`**：不引入文件变化，但手写画板与资源包提示仍强耦合，无法单独验证组件契约，因此不采用。
- **复用设置页的 `ResourcePackRow`**：设置页行包含资源包标题、方案说明和通用镜像表单，和手写面板的紧凑状态行视觉与交互不同，直接复用会改变面板布局，因此不采用。

## Verification

- 缺失、下载中、失败和已安装四种状态的显示与现有文本、按钮动作一致。
- 失败时的镜像读取、`https://` 校验、保存并重试以及卸载保护保持一致。
- `HandwritingModelNotice` 从 `@msime/ui` 导出，`HandwritingPanel` 的行为测试继续通过。
- 组件测试、桌面构建、HarmonyOS 类型检查、quick 门禁、格式检查和笔记校验通过。

## Consequences

移动组件时最容易遗漏样式、类型或异步卸载保护；组件测试覆盖状态分支和镜像保存，面板仍通过同一个公共组件调用，完整本地门禁用于确认迁移没有改变宿主行为。

本笔记承接[手写面板状态抽取](./2026-10-19-handwriting-panel-state.md)的后续 JSX 简化，不改变其状态和异步生命周期边界。
