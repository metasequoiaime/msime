# Agent Note: 社区图库客户端适配器共用入口

Status: implemented

## Problem

社区键盘皮肤、候选皮肤和插件三个页面都重复组装 `CommunityGalleryClient`：详情、评分、下架和举报逐项转发到页面 client，只有列表分页参数和返回字段各自不同。

## Decision

在 `packages/ui/src/community/community-gallery.ts` 增加 `createCommunityGalleryClient`，集中转发详情、评分、下架和可选举报，并把结果值适配为图库 hook 使用的 `Promise<void>`。三个页面继续显式处理分类或插件筛选、列表映射和下架命令选择。

## Alternatives considered

- 让图库 hook 直接接收各页面的完整 client：会把页面特有的列表参数耦合进通用 hook。
- 抽取一个覆盖详情页 JSX 的大组件：三个页面的安装、试用和可见性流程不同，抽取会引入大量条件分支。

## Consequences

图库操作适配只有一份组装逻辑，页面仍控制分页和筛选语义，外部 client 与 UI 行为不变。UI 格式和 lint 检查、TypeScript 类型检查、相关测试（9 项）、前端构建及本地 quick 门禁均通过；quick 中 HarmonyOS 原生编译因缺少已暂存的库和资源按脚本规则跳过。
