# Agent Note: 候选窗口皮肤卡片独立复用

Status: implemented

## Problem

候选窗口皮肤图库把预览图、分类作者、下载评分和素材授权直接写在页面私有的 `CommunityCandidateSkinCard` 中。它只接收皮肤数据、预览加载器和打开详情回调，却无法被其他共享宿主直接复用；预览图实现同时被卡片和详情使用，放在详情模块里也让卡片依赖一个更大的页面展示模块。

## Decision

`community-candidate-skin-card.tsx` 公开 `CommunityCandidateSkinCard` 和 `CommunityCandidateSkinCardProps`。卡片只负责候选皮肤的展示和打开回调，不持有客户端、请求或图库状态。

候选皮肤预览图及 `CandidateSkinPreviewLoader` 移到无详情依赖的 `community-candidate-skin-preview.tsx`；详情模块继续从原路径转导出它们，保持已有公共导入兼容。卡片和详情仍共享调用方提供的缓存加载器、代次防护和原有文案、样式及元数据规则；图库状态与预览 LRU 由[候选皮肤图库 hook](2026-10-17-community-candidate-skin-gallery.md)持有。

## Alternatives considered

- **继续保留页面私有卡片**：改动最小，但其他共享界面仍需依赖完整候选图库，无法独立复用和测试卡片的事件边界。
- **让卡片直接接收客户端并自行读取预览**：能隐藏加载接线，但会把请求、缓存和代次生命周期塞进展示组件，破坏页面拥有状态的边界，也会让卡片和详情重复请求。
- **让卡片从详情模块导入预览图**：能少一个文件，但卡片会依赖包含详情业务控件的模块；独立预览模块保留更窄的依赖方向，同时兼容原详情出口。

## Consequences

候选皮肤卡片可以从 `@msime/ui` 独立导入，其他调用方仍负责加载器、客户端代次和打开详情。卡片新增一个公开 props 契约，预览图移动到独立模块；原有详情导入路径继续有效。没有改变请求、缓存、安装、同步、文件读写、权限或平台资源行为。

组件测试覆盖预览加载、元数据、素材授权、打开回调以及私有和下架作品文案；详情已有测试继续覆盖预览状态和详情操作。
