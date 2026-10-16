# Agent Note: 候选窗口皮肤详情独立复用

Status: implemented

## Problem

候选窗口皮肤页把预览、摘要、安装替换确认、作者可见性与分类、评分、下架和举报全部写在图库页面函数里。页面同时负责列表缓存和客户端动作，其他宿主无法复用详情布局，也容易在迁移预览时把异步代次或缓存边界带错。

## Decision

`packages/ui/src/community/community-candidate-skin-detail.tsx` 持有候选窗口皮肤详情的展示与回调契约，公开 `CommunityCandidateSkinDetail`、`CommunityCandidateSkinDetailProps`、`CandidateSkinPreviewImage` 和 `CandidateSkinPreviewLoader`。组件接收已选皮肤、预览加载器、详情状态和动作回调，继续复用公共详情框架、摘要、安装、分类、评分、下架和举报控件；预览组件保留 `useAsyncGeneration` 的迟到结果守卫。

[`useCommunityCandidateSkinGallery`](2026-10-17-community-candidate-skin-gallery.md) 持有客户端、列表和详情状态、每客户端容量 16 的预览 Promise LRU、安装预检、同步通知、评分和发布动作。`CommunityCandidateSkinsPage` 只把这些状态接线到详情组件；卡片与详情继续共享同一个图库缓存。此边界延续[社区皮肤详情独立复用](2026-10-17-community-skin-detail.md)的展示与图库分层，并与[社区皮肤图库状态](2026-10-17-community-skin-gallery.md)保持状态归属一致。

## 旧笔记归属

按候选皮肤详情、预览缓存、发布和图库状态检索活跃笔记：

- [候选窗口皮肤发布状态](2026-10-16-candidate-skin-publication.md)继续持有发布对话框及其草稿状态，本片不改变发布流程。
- [候选皮肤缓存回归等待搜索列表替换](../testing/2026-10-16-candidate-preview-cache-search-wait.md)继续记录缓存淘汰测试的异步等待；本片只迁移加载器和组件，不改变容量或等待策略。
- [社区皮肤详情独立复用](2026-10-17-community-skin-detail.md)提供同类皮肤详情的公共组件边界；候选皮肤保留自己的安装、预览和审核字段。

未发现完全相同的候选详情提案或需要拒绝的旧决定，无需归档。

## Alternatives considered

- 继续把详情 JSX 留在候选图库页面：改动最小，但共享宿主仍需复制候选皮肤的整段条件分支。
- 让详情组件直接接收 `CandidateSkinCommunityClient` 并操作客户端：调用更短，但会把请求、错误、代次、列表刷新和预览缓存重新塞回展示组件。
- 把预览缓存也移入详情组件：详情可以自包含，但卡片和详情无法共享同一客户端的 Promise，列表重绘会重复读取预览图。

## Verification

新增详情模块测试先在模块缺失时失败，随后覆盖摘要与一键安装、作者私有转公开和分类控件。源码契约测试改查新的详情 owner，并保留共享详情框架、标题、审核控件和预览代次断言。候选详情、候选图库和相关契约测试通过，桌面类型检查通过；提交前继续运行完整社区测试、桌面构建、Harmony 类型检查、笔记门禁和 `git diff --check`。

## Consequences

其他共享宿主可以直接复用候选皮肤详情布局并注入平台动作；详情组件不会自行读取客户端、修改皮肤或持有确认状态。图库 hook 是预览 LRU 和业务生命周期的唯一 owner，调用方必须提供预览加载器与动作回调。此片不改文件读写、权限、网络协议或原生资源，不声明设备验收。
