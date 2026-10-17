# Agent Note: 语音凭据输入使用公共模型

Status: implemented

## Problem

`VoiceCredentialSection` 和 `useProviderCredentials` 分别声明相同的 `{ token, appKey, endpoint? }` 输入结构。hook 通过状态绑定把它交给组件，重复结构可能在新增字段时漂移。

## Decision

将无 React 依赖的 `ProviderCredentialInput` 放入设置模型文件。语音领域别名归 `voice-credential-model.ts`，旧组件路径继续转导出；组件和 hook 复用同一输入契约，以类型测试覆盖深路径和包入口兼容性。完整编辑状态与保存组装的决定见[语音凭据编辑模型与状态](2026-10-17-voice-credential-editor.md)。

## Existing notes audit

活跃笔记中未发现拥有 `VoiceCredentialInput` 类型契约的记录；语音服务协议、凭据保存和平台设置笔记与本次纯 TypeScript 输入模型归属调整独立。

## Alternatives considered

- 保留两份内联结构：改动更小，但重复契约会继续存在。
- 把输入类型放到组件文件：hook 需要依赖展示组件；因此选择无 React 的设置模型文件，并保留旧路径转导出。

## Consequences

- 语音凭据组件继续从旧路径转导出 `VoiceCredentialInput`，包入口保持原导出；hook 直接使用模型类型，删除重复结构。
- 输入字段保持原契约；`provider-credential-input.ts` 只放无 React 的输入结构，完整编辑行为由上述编辑模型与状态笔记记录。
- 类型兼容测试、语音凭据草稿测试、类型检查、构建、lint、笔记检查和 quick 门禁覆盖迁移。
