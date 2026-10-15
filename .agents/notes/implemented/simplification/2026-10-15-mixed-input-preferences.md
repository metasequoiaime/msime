# Agent Note: 混输偏好与控件状态独立于 React 展示层

Status: implemented

## Problem

`MixedInputPreferences` 与 `defaultMixedInput` 定义在 React 组件中，总偏好和纯解析 `settingsInputPreferences` 需要从展示层取类型及默认对象。符号、日文的缺键语义和英文触发字符数的置灰条件也在 JSX 中，脱离页面后没有统一的控件状态入口。

按类型、默认对象、组件名和混输偏好检索旧笔记：[日文混输](../../implemented/feature/2026-10-12-mixed-japanese-candidates.md) 与[符号混输](../../implemented/feature/2026-10-13-symbol-candidates.md) 分别持有缺键读作关、开的决定，沿用这些规则。[Android 符号与 emoji](../../implemented/feature/2026-10-08-android-symbol-catalog-and-emoji-candidates.md)、[表达候选跟随词](../../implemented/feature/2026-10-08-expressive-rows-follow-their-word.md)、[九宫格表达候选](../../implemented/feature/2026-10-08-nine-key-expressive-candidates.md) 处理宿主默认或候选排位，没有持有本片的 React 依赖归属决定。

## Decision

纯模块 `mixed-input-preferences.ts`，持有原类型、原默认对象及 `mixedInputControlState`。总偏好、输入偏好解析和组合组件直接依赖它；旧组件重导出原类型和默认对象。控件状态包含符号与日文是否勾选，以及英文触发字符数是否置灰，不修改偏好对象。

## Alternatives considered

- **解析时补齐所有缺省字段，组件只读取完整偏好**：调用点简单，能共用所有缺省值。但 `japanese` 和 `symbols` 特意保持可选，旧文档与默认对象不应仅因显示控件就多出键。独立投影只提供展示状态，写回继续从原偏好展开。
- **只移动类型和默认对象**：改动最少，模型不再直接导入组件。但可选字段的缺省规则仍藏在 JSX，纯模块集中控件状态能在不加载 React 的情况下验证它们，因此一起抽取。

## Consequences

- 总偏好和纯解析直接依赖无 React 的模块，旧组件路径与包入口保持类型兼容并引用同一个默认对象。
- 原默认对象仍只有四项，不新增 `japanese`、`symbols`。读取旧文档时符号开、日文关，显式布尔优先，控件状态计算不补写字段。
- 英文关闭只置灰触发字符数，不改变数值或其他开关；不带临时日文的版本仍隐藏日文行。组件继续从原偏好展开写回，文案、顺序和选项保持不变。
- 多一个小型纯模块；展示状态的字段用 `Checked` 和 `Disabled` 后缀区分偏好键，不能当作完整偏好写回。

## Verification

纯模块测试覆盖默认对象引用一致、旧文档可选键不补写、缺省符号开与日文关、显式布尔优先和英文关闭保留数值。组件回归与真实设置页回归覆盖独立保存、功能门控和触发字符数。类型探针核对旧路径与包入口的类型形状、可选字段与 `Record` 兼容；另跑桌面构建、Harmony 类型检查、字段契约、笔记和本地 quick 门禁。
