# Agent Note: 插件导航行独立复用

Status: implemented

## Problem

`plugin-list-view.tsx` 同时负责插件列表编排和导航行的标题、描述、尾部标记、无障碍关联及焦点恢复标识。导航行只依赖展示值和打开回调，却被页面私有实现包住，其他共享设置页面无法复用同一套行为。

## Decision

将 `PluginNavRow` 与 `PluginNavRowProps` 放入独立模块并从 `packages/ui` 包入口公开。组件继续负责 `aria-describedby` 的动态关联、`data-plugin-row` 焦点恢复标识和 `type="button"` 的表单边界；插件列表页只负责计算标题、描述、标记和打开回调。

## Alternatives considered

- **继续保留页面私有实现**：改动最少，但导航行无法被其他共享设置宿主复用，页面仍混合列表编排和行控件契约。
- **把整段插件列表一起抽出**：可以减少页面文件中的 JSX，但会把目录分组、缺失插件和导入状态也绑定到公共组件，复用边界过大，因此不采用。

## Consequences

- 共享 UI 可以直接复用插件导航行，并通过独立测试钉住描述关联、焦点标识和打开动作。
- 插件列表的运行时行为和 DOM 文案保持不变；页面仍拥有分组、目录状态和回调所有权。
- `packages/ui` 增加一个公开组件和 props 类型，后续使用方只传值和动作，不依赖插件目录内部状态。

## Verification

- `plugin-nav-row.test.tsx` 覆盖标题、描述、标记、可选 `aria-describedby`、`data-plugin-row`、点击回调和外层表单提交边界。
- 通过定向桌面测试、相关插件设置回归测试、类型检查、构建、lint、笔记校验和 quick 门禁确认抽取未改变现有页面接线。
