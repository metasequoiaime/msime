# Agent Note: 社区资源卡片与鸿蒙资源行独立复用

Status: implemented

## Problem

资源图库的状态、详情和发布已独立，但桌面资源卡片与鸿蒙资源行仍是页面私有函数。其他共享界面无法复用这两种资源展示，也不能直接验证打开详情和添加按钮的事件边界。

## Decision

`community-resource-card.tsx` 持有桌面卡片，`community-harmony-resource-row.tsx` 持有鸿蒙资源行及胶囊状态类型；包入口导出组件和 props。页面只提供资源、打开详情与添加回调，以及已有的添加状态。原有 DOM、样式、文案、字素截取、作者和下架标记保持；卡片继续复用公共外壳与作者行，资源行继续复用模型副标题方法。

## 旧笔记归属

按 `ResourceCard`、`CommunityHarmonyResourceRow`、资源卡片、资源行、胶囊按钮、`CommunityCard`、`community-resources` 和 `communityResourceSubtitle` 检索活跃笔记：

- [资源图库状态](../../implemented/simplification/2026-10-16-community-resource-gallery.md)部分重叠，继续持有请求、历史和会话添加状态，互链。
- [资源模型与操作](../../implemented/simplification/2026-10-16-community-resource-actions.md)部分重叠，继续持有数据、副标题及安装操作，互链。
- [图库控件外观](../../implemented/simplification/2026-10-16-community-gallery-control-variants.md)持有搜索与分页，本片保持其归属。
- 触屏社区融合、皮肤库与发布、触屏预览及全屏修复持有导航、皮肤操作或预览约束；Windows、Android 的原生社区与资源行属于各自宿主，均无关并保留。没有完全吸收或失效提案。

## Alternatives considered

- 让同一个卡片组件接收 `look`：页面入口更少；但桌面整张是按钮，鸿蒙是含两个按钮的行，操作与信息层次也不同。分别导出两个组件保留可读契约，不引入无效属性组合。
- 只保留页面私有函数：接口最少，现有整页测试也覆盖使用路径；但其他界面仍须依赖整页，独立展示的边界无法复用，因此把原实现移到公共模块。

## Verification

迁移前 3 文件 34 项基线通过；新增 9 项直接组件测试先因公共组件不存在而失败，迁移后 4 文件 42 项通过。直接使用包入口，覆盖展示、类型文案、说明回退、作者与下架标记、评分收藏数、字素首字、胶囊三态、打开与添加事件。禁用按钮使用 DOM 原生 `click()`，避免合成事件绕过浏览器禁用规则。桌面类型检查与生产构建、HarmonyOS 类型检查通过；机械比对确认两种展示的实现与页面正文保持，只改组件引用。完整社区套件 87 文件 431 项通过。独立审查通过，无待修问题；格式、lint、笔记与差异检查通过。`bash scripts/verify-local.sh --quick` 退出 0，契约检查、Rust workspace、Android Rust target、Harmony 设置打包与共享 Apple bridge 通过。桌面 Rust 外壳缺 macOS app bundle，Harmony ArkTS 真编译、Linux 桌面外壳和 Windows 原生宿主缺本机环境而跳过，其余未受影响平台按范围跳过。

## CI 中发现的既有测试时序

前两次完整 CI 均只有 `Linux restart copy covers both input method frameworks` 失败：宿主调用已发生，成功提示未被观察到。本片社区测试全部通过，该用例单独运行及整个设置页 386 项在本机均通过。测试在设置初始异步加载完成前点击导航，违背同文件 `settingsReady()` 和公共 `settingsFormReady()` 已记录的交互起点。用例现复用该就绪条件后再进入维护页，保留原服务文案、宿主调用与成功提示断言，不扩大超时或改变产品生命周期。修正后目标用例、格式、lint、独立审查与 quick 复查通过。

## Consequences

其他共享界面可独立使用两种资源展示，页面负责资源及动作接线；代价是新增公开组件与 props 契约。组件只发出原回调，不接管异步添加、提示、历史或请求代次；不改变原生资源、文件读写、权限或协议。页面与 hook 保留原有 API，桌面和鸿蒙的 DOM、样式及可访问名称不变。
