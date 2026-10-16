# Agent Note: 凭据状态消息使用公共模型文件

Status: implemented

## Problem

`credential-status-message.tsx` 定义了 `{ ok, text }` 状态消息，而凭据 hook 再次声明 `ProviderCredentialMessage` 同样的结构。AI、语音和翻译组件共用前者，hook 却保留重复契约。

## Decision

把无 React 依赖的 `CredentialStatusMessageValue` 放入独立模型文件 `credential-status-message-model.ts`，展示组件从旧路径转导出，hook 将 `ProviderCredentialMessage` 改为该公共类型别名。保留现有包入口和深路径类型，运行时消息行为不变。

## Existing notes audit

检索凭据状态、消息和测试状态相关活跃笔记，没有发现拥有此消息模型契约的记录；凭据协议笔记不涉及 TypeScript 展示状态。

## Alternatives considered

- 保留 hook 的内联 `{ ok, text }`：改动最少，但状态消息契约继续重复。
- 将公共类型放进 React 消息组件：纯 hook 依赖展示层；因此选择无 React 的模型文件并保留旧转导出。

## Consequences

- `CredentialStatusMessageValue` 从无 React 模型文件导出；展示组件继续从旧路径转导出，hook 的 `ProviderCredentialMessage` 变为公共类型别名，包入口兼容不变。
- 消息字段、角色渲染和凭据异步行为保持不变；类型测试验证模型、hook 和包入口一致。
- 相关凭据测试、类型检查、构建、lint、笔记检查和 quick 门禁覆盖迁移。
