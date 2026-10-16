# Agent Note: 悬浮工具栏偏好模型归属共享叶子模块

Status: implemented

## Problem

`floating-toolbar-preferences.ts` 已经持有悬浮工具栏偏好的缺省投影，却反向从 `packages/ui/src/index.tsx` 导入 `FloatingToolbarPreferences`。默认值、选项和预览因此不能脱离共享 UI 入口使用，设置页之外的宿主若复用模型还会把类型依赖带回入口。

## Decision

由无 React 依赖的 `settings/floating-toolbar-preferences.ts` 持有 `FloatingToolbarPreferences`、来源类型和有效偏好投影。`index.tsx` 继续重导出原来的公开类型，并用叶子类型组合总 `Preferences`；默认值、按钮与尺寸选项、设置动作和预览直接引用叶子模块。字段、枚举范围、缺省对象和嵌套保存格式保持不变。

## Historical audit

检索 `FloatingToolbarPreferences`、`floating_toolbar`、工具栏按钮和尺寸选项的活跃笔记：`floating-toolbar-options` 与 `toolbar-keyboard-handwriting-settings-prune` 记录按钮能力门控、尺寸档位和页面行为，本片只改变类型归属并保留这些规则；Windows/macOS 工具栏宿主笔记约束原生读取格式，与 TypeScript 依赖方向无关，均保留不吸收。

## Alternatives considered

- **继续从 `index.tsx` 导入类型。** 不改公开 API 且改动最小，但叶子模块仍依赖 React 设置入口，复用缺省投影时会形成反向依赖；把契约和投影放在同一模块能让依赖方向清楚。
- **新建第二个类型文件，保留现有投影模块只做函数。** 可以避免改现有文件，但会把同一偏好契约拆散，默认值和类型又可能被不同调用方引用；沿用已有叶子模块作为唯一归属更直接。
- **删除 `index.tsx` 的兼容导出并迁移所有调用方。** 能立即收窄入口，但外部 `@msime/ui` 使用者和现有总偏好类型会失去稳定导入路径；保留转导出，内部调用逐步改用叶子模块。

## Consequences

悬浮工具栏偏好类型与有效投影现在可以被设置页、预览、选项模型和宿主独立复用；旧的 `@msime/ui` 导入路径、缺省行为、字段范围和存储格式不变。代价是内部文件多一处叶子类型导入，后续新增字段需同步 `client-core` 与该模块。

叶子模型与总偏好类型契约测试、既有缺省投影测试共同覆盖兼容性；桌面类型检查、构建、共享 UI 测试、lint、格式、`verify-notes` 和 quick 门禁作为合并验证。

同步后的共享 UI 全量测试定位到 `settings.test.tsx` 的三项旧断言：手机单译文改动已隐藏第二语言，它们仍查找该控件。用例按现有契约断言手机隐藏第二语言、编辑首选语言仍保留共享第二语言、离线释义控制首选语言的可编辑状态，以及旧俄语选项不会污染新页面；保留 macOS 双语言保存用例。此处只校准测试，不改变产品行为。
