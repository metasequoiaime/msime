# Agent Note: 移动设置历史状态共用构造函数

Status: implemented

## Problem

移动设置子页面、设置导航和桌面宿主承载的手机面板重复构造同一类 WebView 历史状态：保留当前对象的自有字段，写入 `msimeSettings` 标记，再覆盖当前路由字段。重复代码让字段覆盖顺序与宿主状态保留规则容易漂移。

## Decision

新增无 React 和浏览器依赖的 `settings/mobile-history-state.ts`，提供 `createMobileSettingsHistoryState(current, patch)`。三个消费者的四处构造复用它，保留原有浅拷贝与字段覆盖顺序；非对象旧状态不扩散为对象字段。调用方仍控制浏览器写入、SSR 判断、推入或替换，以及设置页选择时删除旧 `panel` 字段。

模块只作为共享源码工具导出，桌面宿主沿用已有纯工具的深路径导入方式，不扩大 `@msime/ui` 聚合入口。

## Historical audit

按 `msimeSettings`、`pushMobileSettingsState`、移动历史与 `useSettingsNavigation` 检索活跃笔记：

- [设置导航返回监听](../../implemented/simplification/2026-10-15-settings-navigation-popstate.md)持有订阅生命周期。本篇只统一构造值，监听、历史初始化及写入时机仍由调用方控制。
- [登录返回导航](../../implemented/bug-fix/2026-10-15-mobile-login-return-navigation.md)持有待返回页的清理边界，本片不修改登录、返回或标签记忆。
- [移动对话二级页](../../implemented/feature/2026-10-16-mobile-chat-secondary-page.md)、标签重选和触屏社区笔记持有页面层级与浏览器历史格式，均保留，不改路由字段含义。

## Alternatives considered

- **保留四处内联构造。** 每处很短，阅读时能直接看到全部字段；但合并旧状态和写入标记的相同规则需要同时维护，抽出纯函数可在一个位置验证覆盖顺序和不修改原状态。
- **连同 `pushState`、`replaceState` 一起统一。** 可以进一步减小调用方代码，但不同调用方对初始化、替换、浏览器可用性和面板字段有不同约束，本篇只统一共同的状态构造。
- **同时收紧路由校验并固定标记不可覆盖。** 可以让类型更窄，但现有 `pushMobileSettingsState` 允许调用方字段最后覆盖标记；本片保持现有行为，不借提取改变历史格式或校验规则。

## Verification

纯函数空实现下 8 项断言全部失败，补齐状态合并后全部通过。纯函数测试覆盖旧宿主字段、路由字段覆盖、标记顺序、非对象旧值、数组自有字段和浅拷贝不修改原状态。初始化保留面板状态、替换页面清理面板而不改旧状态、嵌套路由推入历史三项集成用例通过，连同既有导航、移动标签、社区详情历史与桌面路由共 7 个文件 45 项测试通过。类型检查、生产构建、lint、格式与笔记检查通过，`bash scripts/verify-local.sh --quick` 通过，原生平台阶段按改动范围或本机工具条件跳过，不计为设备验收。

## Consequences

收益是四处状态构造共用同一个无浏览器依赖的规则；代价是调用方多一处工具导入，领域字段依旧由调用方负责。面板和页面共用一条浏览器历史链，但字段清理规则不同，不能让公共构造函数统一删除 `panel`。本片不替代原生宿主或设备返回手势验收。
