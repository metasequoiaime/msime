# Agent Note: 账号资料页面独立于账号主页

Status: implemented

## Problem

`account-page.tsx` 内嵌触屏通用和鸿蒙两套完整资料页面。它们已经复用昵称、会话、复制和确认等领域组件，但其他入口仍无法直接组合资料编辑页，行为测试必须先加载账号主页再进入历史子页。

## Decision

`MobileAccountProfilePage` 和 `HarmonyProfilePage` 位于 `account/account-profile-pages.tsx`，显式导出页面与 props，共用客户端、用户、资料、返回与更新回调的基础类型。保留两种退出回调契约：触屏通用页只通知退出，鸿蒙页同时通知反馈文案。资料页从 `account-model` 取得类型，不依赖账号主页或设置表单，不扩大包入口。

账号主页继续选择宿主布局、持有已登录资料和历史导航，并在原挂载位置传入相同 props。两页继续调用已有 `useAccountAction`、`useAccountRename`、`useAccountSessionActions`、复制 helper 和展示组件；局部草稿、忙碌、确认与反馈的生命周期不变。

## Existing notes audit

- [账号模型](../../implemented/simplification/2026-10-16-account-model.md)持有资料和客户端契约，部分重叠，继续直接引用。
- [账号子视图历史](../../implemented/simplification/2026-10-17-account-subview-history.md)持有历史条目和系统返回，部分重叠，保留在账号主页，不移进资料页。
- [设置同步卡片](../../implemented/simplification/2026-10-23-account-settings-sync-card.md)是同一账号主页的其他独立边界，本片不改变它。
- [面板焦点恢复](../../implemented/bug-fix/2026-10-14-account-sheet-restore-focus.md)、[Tab 边界](../../implemented/bug-fix/2026-10-14-account-sheet-tab-focus.md)和[组词期间 Esc](../../implemented/bug-fix/2026-10-14-account-sheet-ime-escape.md)继续由 `BottomSheet` 持有，本片保留调用关系。
- 原生 Android/iOS 资料生命周期与账号 Rust 迁移笔记拥有不同宿主实现，不归并到 React 页面抽取。

## Alternatives considered

- **保持两个私有组件**：改动最少，当前账号入口也能使用全部功能；但独立资料入口和测试仍须经过账号主页，完整页面边界继续隐藏。
- **合成一个带宿主条件的大组件**：可以合并少量状态声明，但行内表单与底部昵称面板、反馈形式、重新登录入口和退出回调均不同。继续复用现有底层逻辑，保留两套显式布局，避免将展示差异压进同一流程。
- **每种布局各一个文件**：能单独浏览每种宿主，但两页属于同一个资料功能，客户端与回调边界也相同；在一个模块共享 props 基础类型即可，不增设跨页状态。

## Verification

迁移前账号页、代次、会话分派、复制、昵称与动作 7 个文件 88 项通过；独立资料页测试先因模块不存在失败，迁移后 6 项通过。含底部面板、昵称与会话 hook 的相关回归 9 个文件 109 项通过，覆盖独立渲染、保存昵称、返回、取消与确认注销、两种退出回调参数和旧客户端迟到结果隔离。

桌面构建（含 TypeScript）、HarmonyOS 类型检查、变更文件格式检查与笔记校验通过。全仓 lint 通过，仅保留未改动文件的两条既有未使用导入警告。独立审查确认两页完整函数体与基线一致、挂载条件和回调不变，独立复跑资料页和账号页两个测试文件 85 项通过，未发现阻断问题。

提交前 `bash scripts/verify-local.sh --quick` 通过，涵盖契约、Rust 工作区、Android 目标、Linux 桌面容器检查与共享 Apple 桥。原生宿主阶段按范围或未配置环境跳过；HarmonyOS 未配置完整 HAP 打包所需的暂存产物，未将类型检查视为设备验收。

## Consequences

两套完整资料编辑页可直接组合，不依赖账号主页初始化、设置表单或历史路由。账号主页只在原位置选择页面并注入资料与导航动作。新增一个内部模块和共用 props 基础类型；两个退出回调保留各自参数，不强行统一宿主反馈。页面局部草稿、忙碌、确认与复制提示，以及头像、昵称和会话 hook 的调用顺序均保持原样。

页面仍依赖既有共享展示组件与 toast context；鸿蒙需要显示 toast 的调用方继续提供原有 `ToastProvider`。没有修改账号协议、持久化或原生资源。验证使用合成客户端与 jsdom，不代表原生设备或真实账号验收。
