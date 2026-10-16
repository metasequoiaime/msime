# Agent Note: 服务商凭据契约归属公共模型

Status: implemented

## Problem

`ProviderCredentialClient`、状态与语音凭据类型定义在 React 设置入口。AI 默认投影、翻译设置和凭据 hook 因而反向依赖入口；语音表单又分别声明同样的条目、种类和保存参数，接口字段变化时可能漂移。

## Decision

在 `core/provider-credential-contracts.ts` 集中现有五个服务商凭据类型，入口保留兼容转导出，内部消费者直接引用模型。语音表单的条目和种类使用领域别名，保存输入投影自客户端方法参数。表单状态仍接受只读数组，客户端快照仍返回可变数组；方法、字段、可选性、空值和运行时保存逻辑保持原样。

## Historical audit

检索活跃笔记的 `ProviderCredentialStatus`、`provider-credential`、凭据契约与语音输入模型：

- [语音凭据输入](../../implemented/simplification/2026-10-16-voice-credential-input.md)持有编辑草稿模型，本篇管理已存条目和宿主操作契约，保留两者。
- [服务商预设模型](../../implemented/simplification/2026-10-16-provider-preset-model.md)持有展示元数据，与凭据契约独立。
- [凭据草稿保留](../../implemented/bug-fix/2026-10-16-provider-credential-draft.md)和[初始状态顺序](../../implemented/bug-fix/2026-10-15-provider-credential-initial-status.md)持有异步行为，本片不改变这些规则，复用其回归测试。
- 翻译凭据偏好、AI 偏好、凭据测试状态与消息笔记分别拥有偏好或展示契约，没有拥有本篇五个类型的记录。

## Alternatives considered

- **保留入口声明与表单的重复结构。** 改动最少，也不增加文件；但纯模型消费者仍依赖 React 入口，语音表单和客户端参数需要两处维护，因此采用叶子模型和类型投影。
- **把所有契约放进语音表单。** 可直接复用已有语音类型，但 AI、翻译和 hook 会反向依赖展示组件，且服务商状态不只包含语音，采用无 React 的公共模块。
- **统一将状态数组改为只读。** 界面只消费快照，能减少误改；但会收窄公开客户端类型，本片保留原有数组可变性，只在表单边界接受只读视图。

## Verification

类型测试先因叶子模块不存在而失败，迁移后 `tsc --noEmit` 通过。用真实 TypeScript 检查确认新叶子模块和公开路径兼容，表单条目、种类、保存参数与客户端一致，数组可变性保持原样。现有凭据初始化、并发操作、草稿保留、AI/翻译投影及语音表单等 11 个文件 46 项测试通过；类型检查、生产构建、lint、格式及笔记检查通过。迁出前后的五个声明去除注释和空白后完全一致。`bash scripts/verify-local.sh --quick` 通过；原生 macOS、Windows 和 Linux 阶段按改动范围或本机工具条件跳过，没有把这些跳过计为设备验收。

## Consequences

公共模型减少入口反向依赖与重复字段。代价是凭据契约多一个独立文件，后续变更应同时考虑宿主实现；表单只读视图与客户端可变快照仍分别保留。纯类型迁移容易被 Vitest 擦除类型后掩盖，因此类型兼容以实际 `tsc --noEmit` 为准。没有原生窗口或真实凭据服务验收；本片不改落盘、网络协议或运行时逻辑。
