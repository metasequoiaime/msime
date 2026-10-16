# Agent Note: 社区资源图库状态独立复用

Status: implemented

## Problem

资源图库已经共享搜索、分页、详情和发布控件，但列表加载、查询、详情历史、发布刷新、逐项添加和会话标记仍写在 JSX 页面中。其他共享界面无法独立复用图库状态，异步验证必须渲染整页。

## Decision

`use-community-resource-gallery.ts` 持有原有状态与动作，客户端只要求 `list`、`detail`、`apply`、`save` 和可选 `storeReply`。页面持有桌面与 Harmony 的显示分支并委托 hook，资源卡片与鸿蒙资源行由[公共展示组件](2026-10-16-community-resource-cards.md)持有，公开 `CommunityResourcesPageProps`。皮肤与资源的首页组合及分类控件由[社区首页](2026-10-16-community-home.md)持有，不属于资源图库状态。保留搜索草稿和已提交查询分离、分页去重、失败后的分页处理、列表代次、移动历史记录、添加同步锁和会话标记及原提示；发布成功先关闭编辑器再刷新。

逐项添加使用独立的 `useAsyncGeneration(client)`：详情或导入回包迟到时不能在新客户端上写入 `added` 状态、弹出旧提示或保留旧的忙标记。添加操作在异步回包后检查代次；客户端替换时清空会话内添加标记，并用条目到代次的映射避免旧操作的收尾清掉新操作。搜索、分页与范围切换只刷新列表，仍允许同一客户端已经开始的添加完成。

## 旧笔记归属

按 `CommunityResourcesPage`、资源图库、资源列表、添加状态、`activeSearch` 与失败分页检索 proposed、implemented、rejected：

- [资源模型与添加](../../implemented/simplification/2026-10-16-community-resource-actions.md)部分重叠：继续持有实际安装和添加操作，本篇持有图库状态，互链。
- [触屏社区融合](../../implemented/feature/2026-10-11-touch-community-merged-into-feature-pages.md)部分重叠：持有 `historyPage` 与功能页历史约定；这里只移动既有调用，互链。
- [图库控件变体](../../implemented/simplification/2026-10-16-community-gallery-control-variants.md)部分重叠：控件继续持有显示与事件，本篇只迁状态，互链。
- [资源发布](../../implemented/simplification/2026-10-16-community-resource-publication.md)与[资源详情](../../implemented/simplification/2026-10-16-community-resource-detail.md)保留各自状态，图库只拥有打开、关闭和返回回调。
- 原生 Windows、Android 请求代次、iOS 可见性、资源目录句柄、输入源启动、后端分页、夜间发布与 MCP 笔记均持有不同运行时或流程，无关，保留。没有完全吸收或失效提案。

## Alternatives considered

- 继续只共用页面：宿主调用方便，既有端到端组件测试充分；但无界面消费者仍不能复用状态，所以提供独立 hook 并保留页面入口。
- 接入皮肤与插件的通用图库状态机：可以集中更多分页代码，但资源自带云端导入、回复收藏和移动历史，两类现有接口与安装状态不同；本片复用既有代次、历史和资源操作工具，保持业务边界。
- 添加与列表共用请求代次：少维护一份生命周期状态，客户端切换也会使旧回包失效；但每次搜索、分页或范围切换都会推进列表代次，导致同一客户端正在添加的条目停止导入或丢失反馈，因此添加单独按客户端分代。

## Verification

迁移前 5 文件 48 项基线通过；迁移后原页面与独立状态 6 文件 67 项通过，另补充不同资源并行添加的状态隔离测试。源码门禁改读真实 hook 并保留原断言。后续补充客户端替换期间的详情、导入成功与失败回包，以及旧操作不能清除新操作忙标记的回归；添加期间搜索、分页和切换范围的 3 项测试在共用代次时均失败，分离后通过。独立图库 26 项、资源详情 20 项合计 46 项通过。完整社区套件 86 文件 422 项是状态迁移时的验证；当时桌面类型检查与生产构建、HarmonyOS 类型检查、格式、lint、notes 与 `git diff --check` 通过。图库 JSX 的迁移仅 setter、编辑和发布成功回调委托：首页正文不变，经机械比对确认。`bash scripts/verify-local.sh --quick` 退出 0，Rust workspace、Android Rust target 与共享 Apple bridge 编译通过；桌面 Rust 外壳因未生成 macOS app bundle 跳过，Harmony ArkTS 真编译、Linux 桌面外壳和 Windows 原生宿主因本机环境缺失跳过，其余未受影响平台按范围跳过。

## Consequences

图库状态可脱离完整页面独立使用与验证，页面只负责显示分支和控件接线；多出一层状态接口，需要既有页面测试与机械比对确认语义。`useToast` 继续依赖既有 provider，没有 provider 时仍静默；不新增另一套通知状态。初始范围、移动参数默认值集中在 hook，公开页面 props 继承其契约。客户端替换会清空本次会话的已添加与添加中标记，旧客户端已经发出的实际导入请求不回滚，但其结果不再更新新图库界面。

添加与列表请求各自维护代次、请求和状态，多维护一份 ref 与客户端 effect，保证列表筛选不会取消用户已发起的添加；不改变实际宿主读写、权限、Android 资源或协议，也不声明设备验收。
