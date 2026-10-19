# Agent Note: 语音面板状态独立复用

Status: implemented

## Problem

`VoicePanel` 已是公共组件，但录音、异步事件、语言加载、提交与复制状态仍与 JSX 混在 `panels.tsx`，其他共享入口无法单独使用这套会话逻辑。四种代次、同步互斥与文本修订必须保持一致，分散迁移容易让旧宿主结果影响新会话。

## Decision

公开 `useVoicePanel` 与状态类型，接收既有 `VoicePanelClient`，集中持有原状态、effect、拖动与动作。面板只消费状态并渲染，结果编辑与提交、复制、清空控件由[公共结果视图](2026-10-19-voice-results.md)承载；清空按钮的文本与提示更新合为 hook 动作。客户端、识别、提交、文本代次继续复用 `useAsyncGeneration`，事件继续使用 `trackAsyncSubscription`，不新增请求协议或状态框架。

## Historical audit

- 按 `VoicePanel`、语音面板、提交、四种代次及拟定 hook 名检索活跃笔记，没有同范围的状态模块或提案。
- [窗口异步订阅清理](../../implemented/simplification/2026-10-14-window-state-subscription.md)部分重叠于公共订阅工具，本片继续消费既有工具，不改变窗口规则。
- [AI 抽卡状态](../../implemented/simplification/2026-10-17-ai-skin-generation-state.md)是已落地的同层 hook 拆分参考，属于不同领域，不迁移其会话重置或保存规则。
- [组词按键判定](../../implemented/simplification/2026-10-17-shared-composing-key-event.md)保留语音 Escape 的原生事件条件；各宿主语音服务、配置与原生面板笔记持有平台实现，不受本片影响。没有完全吸收的笔记或失效提案。

## Alternatives considered

- 只把整个 `VoicePanel` 搬到独立模块：可以保持组件语法树且降低大文件长度；但调用方仍只能使用完整界面，无法独立复用录音和提交状态，因此抽取领域 hook。
- 分成语言、录音和提交三个 hook：每块更短；但录音与提交共用同步互斥，事件和手动编辑共用文本修订，分拆需要额外双向协调。先保持完整会话边界，避免重建状态机。

## Verification

- 聚焦回归 11 个文件、42 项通过，覆盖新 hook、原语音面板行为、四种代次复用与拖动。独立审查另行运行新 hook 的 4 项测试，全部通过。
- 固定基线 `43a63806b22e4f128b156f844eed79efb73a2946` 的语法树对照确认所有状态、effect 和动作顺序保持，清空动作等同原回调，替换回调后的完整 JSX 一致，返回字段与消费字段相同，传递依赖不回到包入口。
- 桌面构建、HarmonyOS 类型检查与 lint 通过；lint 保留原有 4 条未使用声明警告。本地 quick 通过，包含 Rust workspace、Android 交叉编译、Linux 桌面容器和 Apple bridge 检查。完整设置页回归与 PR CI 由合并门禁继续核验。
- StrictMode 清理用例通过 `reactStrictMode: true` 在根层启用重放；函数 wrapper 内的嵌套边界不会在首次挂载重放 effect，不能据此验证两次订阅。

## Consequences

语音会话可被公共组件之外的调用方复用，代价是新增公开 hook 与返回状态契约。

effect 和代次初始化顺序必须保持；取消立即更新本地状态，不等待宿主确认。面板与结果视图的禁用条件、Escape 条件、能力可见性和主题仍属于展示，hook 不额外修改操作许可。不改变文件读写、权限或原生资源，前端测试不代表原生录音设备验收。
