# Agent Note: 候选字体设置行组合

Status: implemented

## Problem

候选窗口桌面字体标签与 HarmonyOS 手机「更多选项」分别拼接候选字体预设、字体输入和字号控件。两处需要保持字体预设、平台能力、字体目录读取和字号可见性一致，重复的组合容易让桌面自定义字体意图或手机固定字体框发生漂移。

## Decision

`CandidateFontSettingsRows` 统一承载候选字体预设、主字体/英文字体输入和主字体/预编辑字号行。它继续复用 `CandidateFontPresetRow`、`CandidateFontControls` 和 `CandidateSizingSection`，只接收受控偏好、能力开关和回调，不复制字体规则、目录读取或字号边界。桌面传入 `custom` 与 `onCustomChange`，所以预设字体只有用户选择「自定义…」或当前字体不是预设时显示主字体框；HarmonyOS 手机不传自定义回调，主字体框始终显示且没有「自定义…」。字体控件保持一次组合调用，让主字体和英文字体沿用现有目录生命周期与刷新行为。Fragment 保留各行同级 DOM。

## 历史审计

- [候选窗口标签恢复](../feature/2026-10-18-candidate-window-top-tabs.md)描述桌面标签和手机直列边界，本次沿用，不改变分区排版。
- [候选窗口精简](2026-10-16-macos-appearance-settings-prune.md)已记录「自定义…」、字体目录状态并入字体框以及 HarmonyOS 手机不提供自定义选项，本次把这些现有决定收进组合组件。
- `candidate-sizing-section`、`candidate-window-style-section` 和 `candidate-font-controls` 的现有测试继续负责各自底层契约；新增组合测试只验证模式切换、接线、懒读取和同级 DOM。

## Alternatives considered

- **在两个页面继续各自拼接控件**：改动最小，但预设与字体框的显示条件和手机模式容易再次分叉，因此不采用。
- **把自定义状态移进组合组件**：可以减少父页面状态，但会改变桌面切换标签和路由时的生命周期，且手机不需要该状态；保留父级受控状态。
- **分别挂载主字体和英文字体控件**：看起来更直观，但会产生两份字体目录状态和刷新请求；沿用单次 `CandidateFontControls` 调用。

## Consequences

桌面和 HarmonyOS 手机共享同一组行组合，平台差异只通过受控参数表达；父页面仍拥有自定义字体意图，切换标签不会丢失。组合组件新增一个公共 UI 契约，未来扩展字体行需要同步两种宿主模式。字体目录读取行为保持原状。

## Verification

新增组合组件测试覆盖自定义意图、非预设字体、手机固定字体框、英文字体与两种字号、Windows 回退链、惰性字体目录和 Fragment 同级 DOM。另跑候选设置回归、desktop typecheck/build、HarmonyOS 类型检查、格式、笔记校验、diff check 与 quick 门禁。
