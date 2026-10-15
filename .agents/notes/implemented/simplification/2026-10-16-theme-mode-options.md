# Agent Note: 主题模式选项模型

Status: implemented

## Problem

主题页把“跟随系统、浅色、深色”三个选项直接写在页面模块中。需要复用主题模式的宿主只能依赖页面实现，选项顺序和文案也没有独立的契约测试。主题解析、模式选项和设置页 hook 使用的类型分散在 `theme-settings-section.tsx` 与 `surface-theme-select.tsx`；总偏好 `Preferences` 还逐项重复声明了 `ThemePreferences` 已持有的八个界面主题字段。

## Decision

`theme-preferences.ts` 持有 `ThemeMode`、`SurfaceTheme`、`ThemePreferenceKey` 和 `ThemePreferences`，主题解析、模式选项、偏好投影和控件直接引用这份无 React 依赖的契约。总偏好 `Preferences` 通过交叉类型组合 `ThemePreferences`，八个界面主题字段仍然可选，颜色模式 `theme` 仍单独声明。原来的两个组件模块保留类型转导出，已有深路径消费者继续兼容。

`ThemePreferences` 显式声明八个可选字段，`ThemePreferenceKey` 由 `keyof ThemePreferences` 派生。这个形状与原来的 `Partial<Record<ThemePreferenceKey, SurfaceTheme>>` 等价，也在偏好字段检查器支持的对象声明范围内：检查器可以展开交叉类型并继续核对每个字段，无需为一个类型迁移引入新的语法解析。

`theme-mode-options.ts` 公开 `themeModeOptions` 常量，并复用模型中的 `ThemeMode` 类型。主题页继续把同一常量传给分段控件，包入口公开该模型供其他宿主使用。三个选项的顺序、中文文案和取值保持不变。

`surfaceThemeOptions` 仍是直接返回 `<option>` 的普通函数，保持 [HarmonyOS 选择行](../bug-fix/2026-10-10-harmony-phone-settings-input-rows.md)需要直接子元素的边界；不把类型抽取扩成控件重写。

## Alternatives considered

- **继续保留页面内数组**：改动最小，但共享宿主无法脱离页面模块复用和验证主题模式选项。
- **重新定义一份主题模式类型**：可以让模型完全独立，但会造成与组件原有公共类型重复，增加类型漂移风险。移动原有定义并保留转导出，使模型和组件使用同一份契约。
- **继续在组件里持有类型，并逐项声明总偏好字段**：少一个文件，深路径也无需迁移；但解析函数和纯模型仍依赖 UI 模块，总偏好的八个字段仍需与 `ThemePreferences` 人工保持一致。

## Consequences

主题模式选项和主题偏好类型现在可以在无 React 页面依赖的情况下导入和测试，页面行为保持不变。模型通过包入口公开；修改主题模式文案或顺序时需要同步更新模型测试。总偏好和主题控件共用同一组可选字段，没有改变主题解析、持久化或平台能力判断。代价是保留两个组件模块的兼容类型出口。

验证包括模型测试、主题页回归测试、前端类型检查、构建、lint、笔记门禁和仓库 quick 门禁。
