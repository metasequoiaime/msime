# Agent Note: 候选字号、字体和窗口样式共用叶子契约

Status: implemented

## Problem

总偏好重复声明候选字号、字体族和窗口样式的九个字段。字号 helper、预览类型和设置组件又各写一份字号字段，其他组件从总入口挑选窗口字段。修改可选性或空值约定时，多处声明可能漂移。

## Decision

由纯类型模块 `candidate/candidate-preferences.ts` 持有字号、字体族和窗口样式三份对象契约。总偏好组合三份契约；helper、预览、字体控件与外部皮肤直接消费它们；旧模块继续重导出已有类型。字号组件保留原接口并继承叶子对象，保持原模块的声明合并能力，不让组件扩展传播到持久化偏好。控件补丁回调保持原签名。

## Historical audit

按三份类型名、`candidate-font-family`、`candidate-font-size` 和 `candidate-window-style` 检索活跃笔记，没有已有类型归属决定。[macOS 预览 token](../../implemented/bug-fix/2026-10-11-macos-candidate-preview-native-tokens.md)、[内边距下限](../../implemented/bug-fix/2026-10-15-macos-candidate-padding-below-2pt.md)、[macOS 候选控件](../../implemented/feature/2026-10-12-macos-candidate-page-controls-toggle.md)持有运行时几何和控件行为，独立保留；[Windows 候选字体](../../implemented/feature/2026-10-11-windows-settings-candidate-fonts.md)持有原生字体实现，独立保留。[预编辑类型](../../implemented/simplification/2026-10-15-preedit-preferences.md)提供叶子对象与原位置兼容接口的既有做法，契约独立。

## Alternatives considered

- **总偏好直接组合现有 helper 的类型。** 改动少，但字号类型仍依附 TSX 控件，字体和窗口契约仍依附带运行时实现的模块；纯类型叶子使所有消费者直接依赖契约。
- **用一份总外观类型同时接管预览颜色。** 一次覆盖更多外观字段，但预览故意接受 `unknown` 颜色以校验异常输入，总偏好有更窄的存储约束；本片只统一九个完全同义的字段。
- **总偏好组合字号组件接口。** 可以少一个兼容层，但接口声明合并将影响整份持久化类型，也会失去对象类型与 `Record<string, unknown>` 的既有兼容性。

## Consequences

九个字段各只有一份声明，旧导出路径、字段可选性、可空值和组件接口扩展语义保持。代价是新增一个纯类型文件和字号组件的兼容接口。总偏好、helper 和控件不再重复列字段；预览的颜色字段仍接受 `unknown`，运行时代码、字号与字体校验、窗口边界、默认值、控件顺序、补丁内容和 CSS 输出保持。

抽取前后既有字号、字体族、外部皮肤预览、字号控件、窗口样式和原生预览 token 测试均为六文件、60 项通过；桌面构建与 Harmony 类型检查通过，偏好字段门禁仍核对 35 组，四项组合解析回归通过。独立 TypeScript 编译探针核对九字段投影、旧导出、可空值、空偏好、预览宽输入、原接口声明合并及其不传播到总偏好和 helper、`Record<string, unknown>` 兼容性。不新增重复类型声明的运行时测试。

类型组合语义参见 [TypeScript 对象类型文档](https://www.typescriptlang.org/docs/handbook/2/objects.html#intersection-types)。纯类型抽取不改变平台资源、文件读写或原生宿主行为。
