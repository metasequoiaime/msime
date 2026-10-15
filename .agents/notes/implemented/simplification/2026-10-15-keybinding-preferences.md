# Agent Note: 快捷键偏好共用类型契约

Status: implemented

## Problem

公共 API 的 `KeybindingPreferences` 与设置控件的 `InputModeShortcutPreferences` 分别列出同样的八个快捷键字段，默认值模块又从 UI 总入口导入前者。新增按键时，公共偏好和控件支持的补丁类型可能各自增删字段而漂移。

## Decision

由无运行时依赖的 `settings/keybinding-preferences.ts` 持有 `KeybindingPreferences`。总入口继续导入及重导出原名，`Preferences.keybindings` 仍使用它；默认值模块直接导入叶子类型。`InputModeShortcutPreferences` 扩展这份契约，只保留原有的 `input_mode_hud?: boolean` 组件兼容字段。八项快捷键的必填/可选性和默认值保持不变。

## Historical audit

按 `KeybindingPreferences`、`InputModeShortcutPreferences`、`keybinding-defaults` 和快捷键类型检索活跃笔记，没有已有类型归属决定。[Caps Lock 切换](../feature/2026-10-11-caps-lock-language-switch.md)与[中日切换](../feature/2026-10-12-desktop-japanese-toggle-shortcut.md)持有按键行为和宿主能力，两者仅消费此类型，决定独立，不吸收。插件指令、原生快捷键和云同步映射与此类型迁移无关。

## Alternatives considered

- **组件从总入口导入 `KeybindingPreferences`。** 能直接消除重复声明，改动最少，但类型定义仍埋在整份 UI 聚合入口，默认值和组件继续反向依赖它；叶子模块明确契约归属。
- **把 `input_mode_hud` 也并入公共快捷键偏好。** 两个类型可以完全合并，但持久化偏好把它存为顶层字段，移动会扩大契约；组件兼容字段只留在派生接口。
- **把类型放在 `keybinding-defaults.ts`。** 不增加模块，但会让两个类型消费者的契约归属挂在默认值模块；独立类型文件更直接，默认值继续保持原路径。

## Consequences

八个共有快捷键字段只声明一次，默认值和组件直接依赖叶子契约，公共类型名与控件接口名保留。代价是新增一个仅含类型的内部文件；总入口用 `type` 导入和重导出，不增加运行时模块边。`input_mode_hud` 兼容字段仍只在控件派生接口内，不进入持久化的快捷键对象。

改动前后既有快捷键控件、Harmony 候选快捷键和设置偏好测试 5 文件、45 项均通过；桌面构建与 Harmony 类型检查通过。不新增重复验证类型声明的运行时测试。默认值、可选性、快捷键行为和存储格式保持原样。

接口扩展语义参见 [TypeScript 对象类型文档](https://www.typescriptlang.org/docs/handbook/2/objects.html#extending-types)。
