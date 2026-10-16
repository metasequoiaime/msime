# Agent Note: 社区插件卡片独立复用

Status: implemented

## Problem

`community-plugins.tsx` 同时持有插件图库的请求、安装和详情状态，以及列表卡片的标题、作者、下架标记、描述和评分展示。卡片没有独立的公共入口，其他共享设置宿主若只需要展示插件列表就必须依赖整页图库。

## Decision

将 `CommunityPluginCard` 与 `CommunityPluginCardProps` 放入独立模块并从 `packages/ui` 包入口公开。组件只接收完整的 `CommunityPlugin` 和打开详情的回调，继续复用公共卡片外壳、作者行和指标行；[插件图库 hook](2026-10-17-community-plugin-gallery.md) 持有图库请求、安装预检查、发布刷新和详情动作，插件页面只把列表项交给卡片渲染。

## Alternatives considered

- **继续把卡片留在页面中**：改动最少，但其他共享设置宿主无法独立复用同一套插件卡片展示，页面仍同时承担状态和展示两种边界。
- **把安装或图库状态一起移入卡片**：可以让页面 JSX 更短，但卡片会取得客户端和异步生命周期，无法安全用于只读列表，也会混淆列表展示与详情操作的所有权，因此不采用。

## Consequences

- 插件列表卡片可由其他共享页面直接复用，输入和动作边界与皮肤、资源卡片保持一致。
- 插件图库 hook 拥有请求代次、安装冲突确认、发布刷新和举报等状态，运行时行为和 DOM 文案保持不变。
- 包入口增加一个公开组件和 props 类型；卡片的展示契约由独立组件测试直接覆盖，页面集成测试继续覆盖图库接线。

## Verification

- `community-plugin-card.test.tsx` 覆盖卡片元数据、打开回调、表单提交边界、描述可选性和 owner 下架标记。
- 相关社区测试、桌面端与 HarmonyOS 类型检查、构建和仓库 quick 门禁在提交前运行。

相关决定：[社区皮肤卡片](2026-10-16-community-skin-cards.md)、[社区资源卡片](2026-10-16-community-resource-cards.md)。
