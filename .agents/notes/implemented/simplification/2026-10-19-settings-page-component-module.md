# Agent Note: 设置页组件独立模块

Status: implemented

## Problem

设置页模型已有独立模块，`packages/ui/src/index.tsx` 仍同时承担约 800 行页面与外壳渲染及公共导出。外部宿主经入口使用设置页，内部页面又可能仅为类型引用入口；继续在入口维护组件会让导出契约与外壳实现交织。

## Decision

`packages/ui/src/settings/settings-page.tsx` 持有公开的 `SettingsPage` 和模块内的 `SettingsShell`，入口仅转导出同名组件。两段函数保持原样，包含布局上下文、hook 顺序、社区图库位于表单之外、触屏聊天首次打开后持续挂载、表单上下文与恢复动作。导入直接指向既有功能模块，模型和属性类型仍归 `use-settings-page-model.ts`；不新增宿主实现或平台分支。

## Historical audit

- [模型独立模块](../../implemented/simplification/2026-10-18-settings-page-model-module.md)部分重叠，继续持有模型组装；本次明确它由组件模块调用，公共入口不再调用 hook。
- [外壳临时状态](../../implemented/simplification/2026-10-17-settings-shell-state-hook.md)、[导航投影](../../implemented/simplification/2026-10-17-settings-shell-navigation.md)、[平台展示](../../implemented/simplification/2026-10-17-settings-shell-platform-presentation.md)和[社区视图](../../implemented/simplification/2026-10-17-settings-community-views.md)部分重叠，各自规则与边界保留；它们此前限定只拆状态或投影，本次移动调用层而不合回实现。
- [侧栏分组](../../implemented/simplification/2026-10-18-settings-sidebar-groups.md)部分重叠，其渲染与折叠回调随外壳整体搬移，分组来源、搜索展开和当前页展开规则不变。
- [设置界面平台归属](../../implemented/architecture/2026-10-15-settings-ui-ownership-by-platform.md)无关，本次只整理既有共享 React 设置页。
- `proposed` 与 `rejected` 中按 `SettingsPage`、`SettingsShell` 和设置外壳检索，没有重复提案。

## Alternatives considered

- **继续只拆外壳子逻辑**：每片较小，也能减少组件中的细节。但状态、导航、社区和平台投影已有专属模块，当前剩余职责是调用它们与渲染页面；移出入口让公共导出与具体组件各有归属，后续子组件抽取仍可在组件模块进行。
- **同时把外壳拆成多层布局组件**：可进一步缩短渲染函数。但会同时引入新属性契约和 React 挂载层级变化，本次先保持完整组件语法树，单独验证模块边界。

## Consequences

公共入口只保留转导出，外部宿主继续经同名的 `SettingsPage` 使用设置页。组件仍有约 800 行，后续子组件可以在该模块内继续抽取；本次不增加组件层级和属性契约，也不改变功能 hook 的归属。

两段组件的完整语法树与搬移前一致，70 个搬移的导入绑定保持来源与类型属性，原有未使用导入不再保留。既有公共导出保持兼容，组件及 725 个传递模块没有到入口的运行时依赖。桌面类型检查、构建和 lint 已通过，lint 只剩既有未使用声明警告。quick 门禁通过，覆盖 Rust 工作区、Android 目标、Linux 桌面外壳、共享 Apple 桥和 HarmonyOS 设置包检查；缺少完整工程的 HarmonyOS ArkTS 编译按脚本跳过。独立审查无阻塞项。完整设置页与外壳回归共 13 个文件、467 项通过，抽取前的外壳聚焦基线为 4 个文件、18 项通过。自动化回归不代表原生宿主装机验收。

侧栏渲染由[设置侧栏组件](2026-10-18-settings-sidebar-groups.md)负责，设置页继续提供同一组平台展示、导航投影和窗口拖动处理器。

完整偏好表单由[偏好设置表单组件](2026-10-19-settings-preferences-form.md)负责；外壳保留原挂载条件，向组件提供已加载模型与同一组动作，社区图库继续位于表单之外。

账号页由[设置账号接线组件](2026-10-19-settings-account-page.md)组装，复用既有公共账号动作；外壳保留原页面挂载条件与导航模型。

社区根页由[设置社区根页组件](2026-10-19-settings-community-page.md)组装；外壳保留根页挂载条件，分类状态仍归既有模型，图库仍在设置表单之外。
