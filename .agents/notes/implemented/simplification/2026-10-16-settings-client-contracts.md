# Agent Note: 设置宿主客户端脱离页面入口

Status: implemented

## Problem

`SettingsClient`、`PreferencesRecovery`、AI 助手调用及凭据测试类型仍在 React 设置入口定义。设置环境、持久化、恢复、窗口状态和服务 hook 为了取得宿主接口而依赖页面聚合入口，不能直接消费公共契约。

## Decision

`core/settings-client.ts` 组合现有领域客户端并持有设置宿主接口与恢复结果。AI 调用和凭据测试分别归入 `core/ai-assistant-client.ts`、`core/api-credential-test.ts`，让服务 hook 直接依赖自己的契约。三个模块只有类型，包入口保留全部旧导出，内部消费者直接引用公共模块。字段、方法形式、可选性、空值、参数和返回值保持原样。

按钮能力门禁从 `core/settings-client.ts` 和原入口一起取得可选回调，保留迁移前完全相同的 42 个名称。按钮扫描仍沿用原入口范围与既有守卫规则，不把这次声明迁移变成新的全 UI 检查器。

## Historical audit

检索 `SettingsClient`、`PreferencesRecovery`、AI 调用、凭据测试及宿主契约的活跃笔记：

- [偏好文档模型](../../implemented/simplification/2026-10-16-preferences-model.md)、[服务商凭据契约](../../implemented/simplification/2026-10-16-provider-credential-contracts.md)和[面板宿主契约](../../implemented/architecture/2026-10-15-panel-client-contracts.md)拥有各自领域，保留并组合；本篇拥有设置宿主聚合契约的归属。
- [恢复 owner](../../implemented/bug-fix/2026-10-15-preference-recovery-owner.md)、[加载 owner](../../implemented/bug-fix/2026-10-15-settings-reload-owner-generation.md)和[保存 owner](../../implemented/bug-fix/2026-10-15-settings-save-owner-generation.md)持有异步代次与取消边界；本片不改变这些规则。
- AI 偏好、凭据状态与状态消息笔记分别持有配置和展示类型，没有拥有本片五个声明的笔记。没有完全吸收或过时的决定。

## Alternatives considered

- **把五个类型放在同一个宿主契约文件。** 文件更少，但 AI 与凭据 hook 仍依赖包含全部设置能力的聚合模块；独立叶子类型能让领域消费者直接引用。
- **同时把接口拆成多个能力接口并继承。** 能缩短聚合声明，但改变接口结构和后续扩展位置，验证面也更大；本片原样迁移，已有领域客户端仍由原模块持有。
- **消费者各自声明所需方法。** 依赖可更窄，但会复制宿主接口并让签名漂移；现有 `Pick<SettingsClient, ...>` 保留，直接投影公共契约。

## Verification

五个声明与旧入口去除注释及空白后逐字一致；临时 TypeScript 探针先因三个新模块不存在而失败，迁移后实际编译器确认独立旧基线、新模块及旧公开导出完全一致，覆盖必需方法、可选能力、恢复空值、凭据配置的 `unknown` 和服务名联合。14 个内部消费者除了类型导入外完全一致；入口除迁出声明与类型导入外完全一致，运行时导入符号及顺序保持不变。20 个相关测试文件、127 项测试通过；桌面生产构建、鸿蒙类型检查、lint、格式、笔记与差异检查通过。lint 仅有三条既有未使用导入警告。

接口迁出后，`test-settings-action-guard.py` 原先只读入口，导致识别的回调从 42 降到 1；修正声明读取源后恢复同一集合。新增四项回归：迁出接口的未受保护调用必须失败，三种既有保护形式通过，没有回调时失败，原入口的回调仍被检查。负向用例在修正前错误通过，修正后准确失败；四项测试全部通过，修正后的 `bash scripts/verify-local.sh --quick` 通过。

## Consequences

公共设置消费者直接依赖宿主契约，不再为它们反向引用页面入口。代价是三个类型模块以及聚合接口对现有领域类型的组合导入。纯类型迁移会被运行时测试擦除，需要实际编译器核对；组合接口仍引用少数由组件持有的领域类型，但所有依赖必须为 `import type`，不引入 React 运行时代码。没有文件 I/O、原生资源或设备行为变化。
