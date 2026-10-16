# Agent Note: 社区键盘皮肤卡片独立复用

Status: implemented

## Problem

普通社区皮肤卡片和 Harmony 皮肤卡片仍是 `community-skins.tsx` 的私有组件。其他共享界面无法独立使用原有预览、作者指标与胶囊操作，事件边界只能通过整页图库验证。

## Decision

两种原实现分别归入 `community-skin-card.tsx` 和 `community-harmony-skin-card.tsx`，公开组件及 props，Harmony props 继承普通卡片的皮肤、主题和打开回调契约。`CommunitySkinPillState` 继续由计算它的库 hook 持有，通过包入口公开；卡片只按类型引用，不引入 hook 的运行时依赖。

页面继续提供皮肤、外观、胶囊状态、详情和操作回调；库读取、获取恢复、应用偏好、试用、提示与历史保持原归属。保留原 DOM、样式、普通紧凑预览、Harmony 手机缩略图、分类作者拼接、使用量、评分、按钮禁用与事件传播。两种卡片继续复用现有预览、卡片外壳、作者行和指标组件。

## 旧笔记归属

按组件名、皮肤卡片、胶囊、公共作者和指标、手机缩略图检索活跃笔记：

- [社区皮肤库代次](../../implemented/simplification/2026-10-12-community-skin-library-generation.md)部分重叠：继续持有库状态、胶囊状态和获取/应用操作，本篇持有展示，互链。
- [资源公共卡片](../../implemented/simplification/2026-10-16-community-resource-cards.md)是另一种资源的展示边界，保留，沿用独立组件模式，不合并不同操作。
- [触屏键盘几何](../../implemented/simplification/2026-10-14-touch-keyboard-geometry-preferences.md)与触屏预览约束持有预览本身，本片原样传递 `compact` 与 `thumbnail`，不改几何或宿主上下文。
- Linux 模糊阴影与 WebKit、皮肤编辑器、原生皮肤图库、手机底部社区及候选皮肤装饰带属于其他渲染器或导航边界，无关，保留。没有完全吸收或失效提案。

## Alternatives considered

- 单个组件接收 `look` 和可选胶囊回调：导出更少，也可集中预览字段；但普通整张卡片是按钮，Harmony 卡片包含详情与操作两个按钮，状态与必填回调不同，单接口会允许无效组合，因此分别提供明确契约。
- 将胶囊类型与文案另立一个模型文件：展示和库状态的类型归属更中立；但类型已经由计算状态的 hook 持有，类型引用不形成运行时依赖，本片只移展示，不为五个状态再增加一层模型。

## Verification

迁移前 3 文件 42 项基线通过；新增 10 项直接组件测试先因公共组件未导出而失败，迁移后相关 4 文件 52 项通过。测试使用真实预览，覆盖主题、设计背景、紧凑与手机缩略图、装饰性可访问属性、分类作者与下架标记、指标、胶囊五态、详情和操作事件、回调更新与外层表单边界。禁用按钮使用 DOM 原生 `click()`，避免合成事件绕过浏览器规则。机械比对确认两张卡片函数体、胶囊文案与皮肤页正文保持原文。完整社区套件 89 文件 438 项通过；桌面类型检查与生产构建、HarmonyOS 类型检查、修改文件格式与 lint、笔记门禁及 `git diff --check` 通过。独立审查通过，无待修问题。全库 lint 退出 0，仍有 3 条未修改文件的未使用导入警告。`bash scripts/verify-local.sh --quick` 退出 0，契约检查、Rust workspace、Android Rust target、Harmony 设置打包与共享 Apple bridge 通过；桌面 Rust 外壳缺 macOS app bundle，Harmony ArkTS 真编译、Linux 桌面外壳和 Windows 原生宿主缺本机环境而跳过，其余未受影响平台按范围跳过。

## Consequences

其他共享界面可以独立使用两种卡片，页面只负责数据和动作接线；新增公开组件及 props 契约需要保持与原图库一致。组件只发出原回调，不接管库状态、获取恢复、偏好应用、试用、提示或历史。胶囊状态类型只通过类型引用复用，不增加运行时依赖。本片不改原生资源、文件读写、权限或协议，不声明设备验收。
