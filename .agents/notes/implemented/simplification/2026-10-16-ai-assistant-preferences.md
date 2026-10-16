# Agent Note: AI 助手偏好使用公共模型

Status: implemented

## Problem

`Preferences` 在共享 UI 入口内联声明 `AiAssistantPreferences`，而 AI 设置、默认值、凭据测试和服务商切换逻辑都从入口反向引用它。公共偏好契约没有独立的叶子模块，新增只使用该字段的设置代码会被迫依赖总入口。

## Decision

在 `packages/ui/src/settings/ai-assistant-preferences.ts` 定义 `AiAssistantPreferences`，共享入口的 `Preferences.ai_assistant` 和原有 `@msime/ui` 导出都引用并转导出这个模型。字段、可选性和运行时行为保持不变。

## Existing notes audit

检索 AI 助手设置、凭据投影、服务商预设和重叠操作笔记后，已有笔记记录运行时行为与生命周期，但没有一篇拥有这份 TypeScript 偏好模型的归属。

## Alternatives considered

- 保留入口内联类型：改动最少，但公共偏好仍和总入口绑定，叶子消费者无法直接依赖契约。
- 让独立模块从 `Preferences` 派生：会让总偏好反向依赖自身投影，不能作为持久化契约的独立来源。
- 同时重排所有 AI 设置模块的类型导入：会扩大本片范围；本片只移动公共模型，保留既有导入路径和运行时边界。

## Consequences

- `AiAssistantPreferences` 有了无依赖的公共来源，`@msime/ui` 的旧导出路径保持兼容。
- AI 偏好的字段形状、默认值、更新逻辑、凭据测试和持久化行为没有改变。
- 类型兼容测试锁定独立模型与 `Preferences.ai_assistant` 的一致性。
