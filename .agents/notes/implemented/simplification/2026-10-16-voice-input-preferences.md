# Agent Note: 语音输入偏好使用公共模型

Status: implemented

## Problem

`VoiceInputPreferences` 仍内联在 `packages/ui/src/index.tsx`，语音设置组件、默认值和凭据测试配置都要从总入口反向引用它。语音偏好契约没有独立的无 React 来源，后续只消费语音字段的公共逻辑会继续依赖整个设置入口。

## Decision

在 `packages/ui/src/settings/voice-input-preferences.ts` 定义 `VoiceInputPreferences` 和 `defaultVoiceInput`。共享入口的 `Preferences.voice_input` 和原有包导出引用并转导出该模型；`voice-input-defaults.ts` 保留旧路径并转导出默认对象。字段、默认值和运行时行为保持不变。

## Existing notes audit

检索语音凭据输入、语音凭据测试、语音默认值和语音服务设置笔记后，已有笔记覆盖输入投影与服务行为，但没有一篇拥有完整 `VoiceInputPreferences` 偏好模型的归属。

## Alternatives considered

- 保留入口内联类型：改动最少，但所有语音消费者仍依赖总入口，公共契约无法独立复用。
- 只移动类型、把默认对象留在旧文件：可以降低入口体积，但模型与其共享默认值仍分散，新增消费者会继续选择错误的归属。
- 让新模型从 `Preferences["voice_input"]` 派生：会反向依赖总偏好，不能作为持久化契约的独立来源。

## Consequences

- 语音偏好有了无 React 的公共来源，旧的 `voice-input-defaults.ts` 路径和 `@msime/ui` 类型导出继续可用。
- 字段可选性、索引签名、默认值、设置更新和宿主持久化行为不变。
- 增加一个叶子模块和兼容性测试；以后若 `Preferences.voice_input` 新增字段，需同步更新该模型，类型检查会在总入口和测试处暴露不一致。
