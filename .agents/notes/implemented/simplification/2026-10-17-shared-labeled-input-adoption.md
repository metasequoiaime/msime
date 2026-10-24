# Agent Note: 社区与词库表单复用带标签输入基础组件

Status: implemented

## Problem

账号与设置输入框已经通过 `LabeledInputField` 复用标签、无障碍名称和受控字符串输入。`CommunityInputField` 仍把 `CommunityField` 与 `ControlledInput` 单独组装，词库编辑的 `DialogField` 又直接组装标签和原生输入框，相同的结构与值事件转换存在多份实现。

## Decision

社区与词库输入框复用现有 `packages/ui/src/core/labeled-input-field.tsx`。两者继续持有自身的属性接口和样式，分别传入原有标签样式、输入框样式、无障碍名称和值回调，不改变最终 DOM、默认输入类型或可选属性。

词库 `DialogField` 作为 `dictionary/dictionary-edit-dialogs.tsx` 内部的样式适配保留，避免三个调用点重复传样式和无障碍名称；新建与词条编辑的公共边界见[词库编辑弹窗](../bug-fix/2026-10-14-dictionary-dialog-ime-keys.md)。社区其他选择框、文本域继续使用 `CommunityField`。

## Alternatives considered

- 两处保留现状：组件短、阅读集中，但基础组件已有完全相同的标签与输入职责，继续手写只会多留两处维护点。
- 删除样式适配组件，让页面直接使用 `LabeledInputField`：可以减少组件层次，但社区多个表单和词库三个字段将重复样式参数；保留适配层有助于维持不同区域的视觉约定。

## Verification

- 两处适配组件复用 `LabeledInputField`，对照迁移前 DOM、原有样式、标签、可选属性和值回调。
- 公共输入框、账号和设置适配层、社区发布与词库编辑的既有行为回归通过。
- 桌面构建、HarmonyOS 类型检查、lint、格式、笔记和 quick 门禁覆盖公共组件与宿主编译。

## Consequences

社区字段支持非字符串标签，必须继续显式传入 `ariaLabel`；词库字段的手机样式与社区样式不同，不能用社区样式覆盖。

账号、设置、社区和词库编辑共用标签输入结构与字符串事件转换，适配层继续持有视觉约定。额外组件层复用已有实现，不新增通用选项，也不统一不相同的样式。使用既有字段和业务表单测试验证行为，避免新增只检查组件委托关系的源码测试。

## Existing notes audit

按 `LabeledInputField`、`CommunityInputField`、`ControlledInput`、`DialogField` 与词库编辑弹窗检索活跃笔记。既有[词库组词按键笔记](../../implemented/bug-fix/2026-10-14-dictionary-dialog-ime-keys.md)持有外层对话框事件契约，与本片部分关联。本片不改变事件或状态归属，没有其他笔记持有这两处基础输入结构的复用边界。
