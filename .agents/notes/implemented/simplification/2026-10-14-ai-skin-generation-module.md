# Agent Note: AI 皮肤抽卡与全屏样式独立于皮肤编辑器

Status: implemented

## Problem

`AiSkinGeneration` 同时由皮肤页和编辑器使用，却定义在 `touch-keyboard-skin-editor.tsx` 中。社区图库和语音设置也从编辑器取 `fullScreenSurface` / `fullScreenColumn`，通用呈现因此依赖具体功能组件。

## Decision

抽卡组件位于 `keyboard/ai-skin-generation.tsx`，两个使用者直接导入。全屏样式位于 `core/full-screen-style.ts`，由抽卡、编辑器、图库和语音设置直接引用。模块迁移保持属性、样式字符串和业务结构；进度监听的取消句柄由公共工具持有，见 [AI 皮肤进度监听](2026-10-14-ai-skin-progress-subscription.md)，组件仍维护请求代次。

## 历史约束

已核对 [鸿蒙编辑器全屏](../../implemented/bug-fix/2026-10-11-harmony-skin-editor-full-screen.md)、[社区图库融合](../../implemented/feature/2026-10-11-touch-community-merged-into-feature-pages.md) 和 [编辑器平台控件](../../implemented/feature/2026-10-12-skin-editor-platform-controls.md)。它们分别持有导航、表单边界和控件布局决定，仍然成立；本次模块抽取不替代这些决定。

## Alternatives considered

- 把组件搬出去，继续从编辑器取全屏样式。最强理由是少建一个文件，现有样式导入路径也不用改；但编辑器还要导入抽卡组件，会形成双向依赖，通用图库和语音设置也继续依赖编辑器。
- 抽出统一全屏 React 容器。最强理由是连外壳 DOM 都可以统一；但四个使用者的语义、滚动引用和内容结构不同，本次已有共享样式足以消除功能间依赖，引入容器会扩大迁移范围。

## Consequences

皮肤页可以独立使用抽卡组件，图库和语音设置只依赖公共全屏样式。代价是新增两个模块及对应导入；不保留从编辑器转出的兼容出口，因为包公共入口没有导出它们，仓内引用已全部迁移。模块迁移时函数体和样式字符串逐字一致；进度订阅的句柄维护归上面的监听笔记。

源码生命周期复用检查随组件迁到 `tests/keyboard/ai-skin-generation-mounted-reuse.test.ts`，保留既有断言。本次验证覆盖共享 UI 行为、类型和构建，不声称做过原生设备验收。

## Verification

迁移前，抽卡外壳、编辑器、图库、语音设置和皮肤页窗口样式五个文件的 37 项行为测试，以及设置整页的四项抽卡、取消、保存、发布集成通过。迁移后运行同一批行为测试和编辑器源码契约检查，检查组件函数体、样式字符串与基线一致，并运行 desktop 构建、HarmonyOS 外壳类型检查、lint、格式、quick 和笔记门禁。
