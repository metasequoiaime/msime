# Agent Note: 插件说明与操作块独立复用

Status: implemented

## Problem

`plugin-detail-view.tsx` 的私有 `ActionBlock` 被已安装插件的使用、删除和缺失插件的恢复操作反复使用。它只组合说明和按钮容器，却归详情页持有，其他共享界面无法独立复用相同的说明与操作布局。

## Decision

`settings/plugin-action-block.tsx` 持有 `PluginActionBlock` 与 `PluginActionBlockProps`，通过 `packages/ui` 包入口公开。它继续组合 `SettingsManagerBlock`、`SettingsManagerNote` 和 `SettingsManagerActions`，保留原有说明及子节点的条件渲染。详情页和缺失插件视图只传说明与操作节点，插件能力判断、按钮状态和动作回调仍由调用方持有。

## Alternatives considered

- **保留详情页私有组件**：不增加公共 API，阅读详情页时可以就近找到实现；但插件说明与操作布局仍依附完整页面，无法由其他共享界面独立使用。
- **把插件能力和按钮选择一起放进操作块**：调用方参数可以更少，但操作块会绑定插件类型和偏好规则，无法承载缺失插件恢复等不同操作，因此只提取已有展示边界。

## Consequences

公共 UI 新增一个组件及属性类型，调用方需要维护说明和操作内容；组件不接管偏好、异步请求、禁用条件或导航。原有 DOM、说明文案、子节点顺序及空操作区的处理保持不变。

## Verification

独立测试覆盖说明和按钮组合、以及只有说明时不创建操作容器；既有 `plugins-section` 和脚本插件测试覆盖使用、删除、授权与恢复接线。提交前运行桌面与 Harmony 类型检查、桌面构建、lint、格式、笔记校验和 quick 门禁。本片不涉及原生资源、文件读写或权限，不声明设备验收。

## Related decisions

检索 `ActionBlock`、插件操作块和 `PluginAction` 未找到持有该组件边界的旧笔记。[插件导航行](2026-10-17-plugin-nav-row.md)持有入口行和焦点恢复契约，与本片的详情操作布局无重叠，保留原决定。
