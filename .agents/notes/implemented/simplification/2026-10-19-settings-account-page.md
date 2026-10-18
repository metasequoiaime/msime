# Agent Note: 共享设置账号页接线

Status: implemented

## Problem

`SettingsShell` 内联账号页的平台投影、宿主能力筛选、登录返回和云端动作。公共 `useAccountPageActions` 已经提供其中九个动作，但只有测试使用，实际设置页继续维护一份同形实现。

## Decision

`settings/settings-account-page.tsx` 的 `SettingsAccountPage` 接收现有 `SettingsPageModel` 的账号相关字段与可选新手引导回调，集中组装既有 `AccountPage`。[功能页组合](2026-10-19-settings-feature-pages.md)保留 `(client.account || client.appIcon) && page === "account"`，外壳继续持有导航和持久化状态；新组件不调用模型 hook。

复用现有 `useAccountPageActions`，不增加它的输入或返回字段。桌面云词库优先进入词库页的 `cloud` 标签，以及触屏反馈页能力筛选，仍由账号接线组件持有。宿主方法以闭包调用 `client` 的属性，保留原方法接收者；macOS 的原生云端入口限制、社区和本地设计能力条件、登录返回、新手引导与外链策略不变。公共入口转导出组件与属性类型。

## Historical audit

按 `SettingsAccountPage`、`useAccountPageActions`、`AccountPage` 和账号页接线检索活跃笔记，没有既有接线组件提案。

- [设置页组件模块](../../implemented/simplification/2026-10-19-settings-page-component-module.md)部分重叠，外壳继续持有页面条件和状态，本片只迁出账号子树与动作接线。
- [账号模型](../../implemented/simplification/2026-10-16-account-model.md)部分重叠，继续持有账号数据与宿主契约；本片不移动字段或认证逻辑。
- [移动端登录返回清理](../../implemented/bug-fix/2026-10-15-mobile-login-return-navigation.md)部分重叠，继续由导航层管理返回目标，接线组件仅消费已有结果与回调。
- [工具与支持设置精简](../../implemented/simplification/2026-10-16-macos-tools-support-settings-prune.md)部分重叠，继续持有桌面云词库入口归属和 macOS 云剪贴板入口限制，本片保留原筛选。
- 平台会话存储、原生账号、头像读取与账号子页历史笔记只涉及本片使用的下层能力，不被本片取代。

## Alternatives considered

- **只把账号 JSX 原样搬到组件，保留全部内联动作**：可以进行完整语法树直接对照，验证最简单。但已存在的公共动作 helper 仍无实际消费者，同形实现继续漂移，因此复用它并逐项核对展开后的属性。
- **把桌面词库和反馈导航也加入公共动作 helper**：所有动作可通过一次展开传入。但 helper 目前不认识设置页面或词库标签，增加这些依赖只为减少组件中两条条件；保持通用动作与设置导航的既有边界。
- **让新组件自行创建设置模型**：调用处最短，但产生第二份草稿和导航生命周期，因此接收已有模型的字段投影。

## Consequences

账号页的宿主接线有独立公共组件，既有九个公共账号动作获得实际消费者；代价是新增一层无状态组件和包含十七个字段的输入投影。桌面词库和反馈导航继续留在设置层，公共动作 helper 的接口与实现不变。认证、登录返回、草稿和导航的状态归属不变。

三个新增组件用例先因组件尚未导出而失败，落地后连同两个公共动作用例通过，覆盖宿主方法接收者、云端失败回传、反馈入口随能力变化和 macOS 动作限制。抽取前账号与公共动作基线共八十一项通过。自动化前端验证不代表原生宿主装机验收。

临时结构验证展开公共动作，确认二十二个账号属性、两个组件属性、十七个模型字段以及外壳其余逻辑与原挂载条件一致。组件及九十六个传递模块没有运行时入口循环；原公共导出和公共动作 helper 保持不变，只新增组件及属性类型导出。

构建、类型检查和 lint 通过，lint 只有既有未使用声明警告。独立审查没有阻塞项或重要问题，并独立复跑两个文件的五项组件与动作测试。

quick 门禁通过，覆盖 Rust 工作区、Android 目标、Linux 桌面外壳、共享 Apple 桥和 HarmonyOS 设置包检查；缺少完整工程的 HarmonyOS ArkTS 编译按脚本跳过。

完整设置页、账号、账号动作和设置导航回归共七个文件、496 项通过。改动文件格式、笔记树与格式、归档封印和 `git diff --check` 通过。

## CI 中暴露的维护页测试时序

首次完整 CI 为 740 个文件、4478 项，只有 macOS 输入源修复用例未观察到成功提示，其安装与注册调用断言已经通过。该用例在初始设置未就绪时点击侧栏，缺少同文件 `settingsReady()` 约定的交互起点；[社区资源卡片](2026-10-16-community-resource-cards.md)记录过 Linux 服务用例的同类缺口。现等待设置就绪，并分别以可控 Promise 完成安装和注册：安装未完成不得注册，注册未完成不得显示成功，完成后仍核对成功提示及调用顺序。目标用例通过，不扩大超时，不修改产品服务或异步动作生命周期。
