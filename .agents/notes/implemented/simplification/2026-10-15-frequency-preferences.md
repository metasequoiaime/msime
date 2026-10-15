# Agent Note: 拼音调频偏好独立于设置控件

Status: implemented

## Problem

`FrequencyPreferences` 和 `defaultFrequency` 定义在 `frequency-section.tsx`，总偏好与纯解析函数 `settingsInputPreferences` 因此依赖 React 展示模块。触发次数和线性步长共用的选项规则也藏在 JSX 中：常用范围是 1 到 6，但已保存的更大值必须继续可选，不能因控件默认范围而被改小。

已按 `FrequencySection`、`frequency-section`、`trigger_count`、`linear_step` 和调频选项检索活跃笔记。[九宫格简拼个人排名](../../implemented/feature/2026-10-09-nine-key-initials-personal-ranking.md) 拥有引擎如何应用调频的决定，本片只处理设置偏好的归属，不改引擎规则；没有既有笔记持有这一层的决定。

## Decision

无 React 的 `frequency-preferences.ts` 集中原类型、同一个默认对象和数值选项生成函数。总偏好、输入偏好解析与共享设置组合直接依赖纯模块。组件负责标签、布局和事件，原路径继续重导出类型与默认对象，包入口兼容。

## Alternatives considered

- **类型和默认值留在组件中**：文件少，现有组件测试已覆盖基本交互。但纯偏好解析仍依赖展示层，数值范围与保留已存值的约束也无法独立复用，因此拆出已有规则。

## Consequences

- 总偏好与纯解析不再依赖调频展示组件；旧组件与包入口仍重导出同一个默认对象，不产生两份缺省值。
- 默认仍为 `promote`、触发次数 1、步长 1。选项仍按 1 到 6 排列，只在当前值大于 6 时追加该值；组件保持其他字段、文案、模式顺序与可编辑状态。
- 多一个小型纯模块，数值选项函数只实现现有展示规则，不承担无效偏好校验，也不钳制或归一化数值。

## Verification

纯规则测试覆盖常用值不重复、大于 6 的值保留、默认对象引用一致和显式偏好保留。组件回归覆盖逐字段写回；真实设置页回归覆盖大值可见、独立保存和不受学习开关限制。类型探针对照旧组件路径、包入口与纯模块的类型形状及默认对象可变性。另跑桌面构建、Harmony 类型检查、偏好字段契约、笔记门禁和本地 quick。
