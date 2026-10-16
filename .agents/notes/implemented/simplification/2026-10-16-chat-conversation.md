# Agent Note: 对话会话逻辑独立于页面布局

Status: implemented

## Problem

`ChatPage` 在桌面与触屏两套布局之前持有模型目录、消息、草稿、登录提示和请求生命周期。纯上下文函数与宿主契约又为取得四个类型反向依赖页面，其他组件不能独立复用这份会话逻辑。

## Decision

四个公共类型原样迁入无依赖的 `chat/chat-model.ts`；旧页面与包入口保留既有类型导出。`chat/use-chat-conversation.ts` 接管目录加载、活动状态刷新、发送、重试、取消、清空和两组客户端代次。页面只组合这个 hook 与焦点、输入框尺寸、确认框、输入法引导及布局。保留 `defaultModel`、消息边界、提示词、错误文案、草稿清空时机、取消只丢弃回复及切换客户端的现有语义。

## Historical audit

按 `ChatPage`、`ChatClient`、`chatRequest`、会话代次与公共聊天逻辑检索活跃笔记，没有已有记录持有会话模块归属。

- [服务端选择模型](../../implemented/feature/2026-10-11-chat-server-picks-model.md)持有默认模型与错误重试规则，本片只同步其实现位置。
- [试用键盘子页](../../implemented/feature/2026-10-08-touch-try-keyboard-page.md)明确保留问候、目录失败处理、气泡上限与清空差异；本片只让它直引公共类型，不强行共用会话状态机。
- [共享防抖回调](../../implemented/simplification/2026-10-15-shared-debounced-callback.md)持有输入框调整计时器；[账号模型](../../implemented/simplification/2026-10-16-account-model.md)持有账号契约，均保留。

## Alternatives considered

- **只迁四个类型。** 类型依赖调整简单、兼容风险小，但目录与请求的公共逻辑继续困在布局文件里；本片连同这一完整会话边界迁出。
- **同时统一 `TryKeyboardPage` 的状态机。** 两处发送、取消和模型目录访问相似，能删除更多重复代码；但试用页允许等待目录后发送、把错误画成不进入上下文的气泡、保留固定问候，直接套用会改变产品行为。保留独立策略，只共用既有提示词、历史边界和类型。

## Verification

三项源码约束先在原页面上失败，迁移后通过；独立 hook 测试先因模块不存在而失败。新会话生命周期、既有聊天页、试用页和首页共 9 个文件、86 项测试通过。逐段去注释与空白比对确认四个类型、会话状态、目录 effect 和请求逻辑与原页面一致；从输入框调整函数开始的确认与 JSX 原文相同，聚焦 effect 原文保留。桌面生产构建（包含真实 TypeScript）与 HarmonyOS 类型检查通过。

提示词门禁从 6 个调用位置扩为 7 个：页面委托和公共会话请求各自受约束，3 份提示词常量仍逐字一致。隔离副本里分别移除页面委托或会话提示词请求，门禁均失败，恢复后通过。三项生命周期检查跟随到真实持有者，页面委托另受检查。独立审查没有发现阻止提交的问题。lint 没有新增告警，仍有三条既有未使用导入告警。

`bash scripts/verify-local.sh --quick` 通过，设置按钮门禁仍识别 42 个可选回调，偏好字段门禁仍覆盖 35 组共享类型。quick 编译 Rust 工作区、Android 目标和 Apple 桥接；Linux 容器、Windows 交叉编译与 HarmonyOS 原生打包因本机工具链条件跳过，其余原生阶段按范围跳过。格式、笔记和差异检查通过。

## Consequences

页面的两套布局消费同一份可独立使用的会话逻辑；纯消息、历史截断和宿主契约直接引用无依赖模型，不再反向依赖页面。每次调用 hook 的状态相互隔离，符合 [React 自定义 Hook 的规则](https://react.dev/learn/reusing-logic-with-custom-hooks)。两组异步代次、挂载守卫和 effect 顺序保留，旧导入路径多一层类型重导出。试用页继续拥有独立策略，输入框尺寸、防抖、焦点和确认语义仍由页面持有。本片没有原生宿主或文件 I/O 改动，前端自动化结果不计为设备验收。
