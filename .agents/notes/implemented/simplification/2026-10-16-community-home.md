# Agent Note: 社区首页与分类切换独立复用

Status: implemented

## Problem

`CommunityHomePage` 同时组合皮肤和资源图库，却留在资源图库模块中；分类按钮也只能随完整首页使用。社区入口为了取得首页反向依赖资源页面，分类、宿主能力与样式的边界不清楚。

## Decision

首页与公开 props 归入 `community-home-page.tsx`，分类按钮归入受控的 `CommunityCategoryTabs`。分类沿用 `CommunityDestinationCategory`，首页持有原分类状态，控件只接收当前分类、皮肤可用性、外观和变更回调。公共包入口保留原首页导出，内部入口直接引用新模块。资源模块不再导出首页；包清单只公开根入口与样式，没有受支持的资源模块子路径。

保留无皮肤宿主的词库回退、初始参数仅初始化状态、资源分类间保留搜索和范围、皮肤分支重新挂载、原有 props 转交、DOM、样式和文案。这里是原有三分类首页；共享触屏壳底部社区根页的两分类与导航状态不属于本片。

## 旧笔记归属

按 `CommunityHomePage`、`community-resources`、社区首页与社区分类检索活跃笔记：

- [资源图库状态](../../implemented/simplification/2026-10-16-community-resource-gallery.md)部分重叠：继续持有资源列表与动作，本篇持有首页组合，互链。
- [Windows 原生社区](../../implemented/feature/2026-10-11-windows-settings-community-resources.md)部分重叠：继续持有原生界面的规则对齐约定，更新共享首页来源位置；本片不改变规则，不改原生实现。
- [触屏社区融合](../../implemented/feature/2026-10-11-touch-community-merged-into-feature-pages.md)和[底部社区恢复](../../implemented/feature/2026-10-16-mobile-community-bottom-tab.md)持有不同导航边界，保留；新的两分类根页不使用这里的三分类控件。
- Windows 云账号、Android 原生社区页、资源公共卡片等记录持有宿主能力或各自控件，无关，保留。没有完全吸收或失效提案。

## Alternatives considered

- 只迁首页：最少新增文件，立即解除入口与资源模块的归属耦合；但分类控件仍被完整图库包住，不能独立组合，因此同时迁出完整的受控控件。
- 将分类也迁为独立状态 hook：能让无界面消费者取得状态；但这里只有一次初始化与 setter，没有可共享的异步或业务操作，额外 hook 会增加接口而没有新的复用边界，因此状态留在首页。

- 资源模块额外重导出首页：可让未检索到的源码子路径消费者继续使用旧位置；但包只公开根入口，仓库没有这种调用，重导出还会让首页和资源页循环引用，因此只保留受支持的公共入口。

## Verification

迁移前原有 3 文件 37 项测试通过；新增首页生命周期、资源搜索与范围保留、皮肤图库重新挂载的 3 项用例在原实现上通过；公共分类控件的 6 项用例先因组件未导出而失败，迁移后相关 5 文件 46 项通过。机械比对确认资源页正文、首页参数默认值和图库分支原文一致，分类 DOM 仅回调委托更名。完整社区套件 88 文件 428 项通过，桌面类型检查与生产构建、HarmonyOS 类型检查、修改文件格式和 lint、笔记门禁及 `git diff --check` 通过；全库 lint 退出 0，仍有 3 条未修改文件的未使用导入警告。独立审查通过，无待修问题。`bash scripts/verify-local.sh --quick` 退出 0，契约检查、Rust workspace、Android Rust target、Harmony 设置打包和共享 Apple bridge 通过；桌面 Rust 外壳缺 macOS app bundle，Harmony ArkTS 真编译、Linux 桌面外壳和 Windows 原生宿主缺本机环境而跳过，其余未受影响平台按范围跳过。

## Consequences

首页与分类控件可以独立导入，资源页只持有自己的展示分支；多出两个明确模块，需要保持分类控件受控，避免重复保存首页分类。公共包入口仍提供同一个首页组件，内部直接引用首页模块，不为无人使用的资源模块首页导出引入循环依赖。

资源分类切换继续复用同一个图库实例；从资源切到皮肤再返回仍重新挂载。初始参数只用于原来的初始化时机，`CommunityPage` 的目的地 key 继续负责重新挂载。共享触屏壳底部根页的两分类不接入此三分类控件。本片不改原生资源、文件读写、权限或协议，不声明设备验收。

共享设置的桌面与触屏根页接线由[设置社区根页](2026-10-19-settings-community-page.md)消费；本组件继续持有原三分类内容，不接管触屏两分类状态。
