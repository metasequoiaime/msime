# Agent Note: 候选窗口皮肤图库状态独立复用

Status: implemented

## Problem

候选窗口皮肤已有独立卡片、详情和发布组件，`CommunityCandidateSkinsPage` 仍同时持有客户端适配、分类分页、每客户端预览 LRU、安装预检、替换确认、作者操作、同步和发布后刷新。其他共享宿主只能通过整页复用这套业务生命周期。

## Decision

`use-community-candidate-skin-gallery.ts` 提供受客户端驱动的 `useCommunityCandidateSkinGallery`，接收 `client` 和可选的 `localSkins`，持有原页面的业务状态与动作。页面接收 hook 返回的状态和动作，只负责展示与发布对话框接线；hook、选项与返回类型从 `@msime/ui` 导出。

保留每客户端容量 16 的预览 Promise LRU、读取失败后移除并可重试、安装预检的迟到结果守卫，以及安装已完成时总是通知目录重扫的顺序。同步仍只在明确调用时运行，不能因为打开、搜索、筛选或安装而自动同步。发布草稿与宿主文件操作留在已有组件和接口。

## Alternatives considered

- 只移动列表适配器：改动较小，也能进一步共用分类分页，但安装、预览缓存与作者操作仍依赖页面闭包，其他宿主仍需复制完整生命周期。
- 合并键盘皮肤与候选皮肤 hook：两者共享分类和基础图库，但键盘皮肤下载后可试用并恢复，候选皮肤需要安装替换及目录同步；统一会把两套动作和清理语义塞进条件分支。
- 继续保留页面状态：已有页面回归覆盖充分，迁移风险最低；但共享详情与卡片仍无法搭配一个独立业务入口复用。

## Verification

既有页面、卡片、详情和预览代次回归共 53 项在抽取前后通过。新 hook 的 5 项测试先在缺少公共入口时失败，随后与基础图库、反馈和分类筛选测试共 8 文件 71 项通过，覆盖请求与发布刷新、预览重试与客户端隔离、安装预检迟到结果，以及已完成安装的目录通知。新测试的列表客户端每次返回新的数组快照，与宿主序列化响应一致，避免复用同一 mock 数组导致 React 合并更新后不重画。

逐字比较确认页面 JSX 与动作接线保持原样，迁移业务代码除中文注释外逐字一致。桌面类型检查与生产构建、HarmonyOS 类型检查通过。

## Consequences

其他共享宿主可以独立接入完整候选皮肤图库状态，页面只持有展示接线。代价是新增公共 hook 的状态与动作契约，调用方仍需注入候选皮肤客户端及可选本机目录读取器。预览 `useMemo` 继续按客户端隔离；真实安装完成后先通知目录重扫，再检查界面代次，不丢弃真实目录变更。没有改变宿主文件读写、权限、协议或原生资源，本片验证共享 UI 行为与构建，不声明设备验收。

## Existing notes audit

检索 `community-candidate-skins.tsx`、`CandidateSkin`、图库和预览缓存：

- [候选皮肤详情](../../implemented/simplification/2026-10-17-candidate-skin-detail.md)及[卡片](../../implemented/simplification/2026-10-17-community-candidate-skin-card.md)持有展示边界；本片同步它们的状态归属事实，保留展示契约。
- [键盘皮肤图库](../../implemented/simplification/2026-10-17-community-skin-gallery.md)提供同类分层，试用状态不与候选皮肤安装混合。
- [图库适配器](../../implemented/simplification/2026-10-12-community-gallery-adapter.md)、[安装展示](../../implemented/simplification/2026-10-12-community-install-actions.md)、[候选皮肤发布](../../implemented/simplification/2026-10-16-candidate-skin-publication.md)继续承担各自公共契约。
- [缓存回归等待](../../implemented/testing/2026-10-16-candidate-preview-cache-search-wait.md)保留原有页面回归等待策略；候选皮肤原生渲染、Windows 设置接口和移动宿主笔记属于其他层，不受影响。

没有同机制的旧提案需要吸收或拒绝。
