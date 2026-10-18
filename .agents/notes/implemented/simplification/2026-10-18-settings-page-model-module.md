# Agent Note: 设置页模型独立模块

Status: implemented

## Problem

设置页已把路由、持久化、凭据、词库和主题等状态拆成专属 hook，但约 880 行模型组装仍位于 `packages/ui/src/index.tsx`。入口同时承担公共导出、模型组装和外壳渲染；表单上下文还要从入口导入模型类型，继续扩大对入口的依赖。

## Decision

`packages/ui/src/settings/use-settings-page-model.ts` 持有 `useSettingsPageModel`、同形的 `SettingsPageProps` 和由返回值推导的 `SettingsPageModel`。[设置页组件](2026-10-19-settings-page-component-module.md)调用同一个模型 hook，入口继续转导出已有的模型类型；`settings-form-context.ts` 直接从模型模块导入类型。原有 hook 调用顺序、effects、处理器、返回字段和每个设置窗口的状态归属保持原样；模型只通过各功能模块组装，不从入口导入实现。

## Historical audit

[路由状态 hook](../../implemented/simplification/2026-10-17-settings-page-routing-hook.md)与本次部分重叠，继续持有路由栈与历史恢复；它当时先拆路由的范围选择保留，本次只移动其调用所在的组装层。[模型平台可见性](../../implemented/simplification/2026-10-17-settings-page-model-visibility.md)与[社区导航分流](../../implemented/simplification/2026-10-17-settings-community-navigation.md)继续持有各自规则，不合回大模型。[设置界面平台归属](../../implemented/architecture/2026-10-15-settings-ui-ownership-by-platform.md)保持不变，本次只重组共享 React 实现。

## Alternatives considered

- **继续在入口组装模型，只拆更小的功能 hook**：每片改动更小，已拆出的功能也能独立复用。但当前大段已主要是这些 hook 的组装、字段投影和生命周期连接，继续留在入口会使公共导出和窗口模型相互耦合；独立模型模块让现有功能边界保持可见。
- **只移动函数，表单上下文仍从入口取类型**：可少改一个文件，原有导入也能继续工作。但内部上下文因此仍经公共入口回绕模型，故内部类型依赖改为直接指向模型，入口保留既有外部导出。

## Consequences

入口不再定义模型 hook，公共模型类型导出保持兼容，表单上下文直接依赖模型类型。组装层仍有约 880 行，后续独立功能继续在各自模块演进；本次没有改变状态归属，也没有减少现有运行时 hook 数量。

跨模块移动可能遗漏依赖或引入入口循环，也可能误改 hook 顺序与返回字段。搬移前后的完整函数、属性契约和推导类型语法树一致；模型及 133 个传递模块没有回到入口的运行时依赖，独立审查未发现阻塞项。桌面类型检查、构建和 lint 已通过，lint 只有既有未使用声明警告。设置页和生命周期回归共 11 个文件、451 项通过；quick 门禁通过，包含 Rust 工作区、Android 目标、Linux 桌面外壳、共享 Apple 桥和 HarmonyOS 设置包检查；缺少本地完整工程的 HarmonyOS ArkTS 编译按脚本跳过。自动化前端回归不代表原生产品装机或真实账号服务验收。
