# Agent Note: AI 凭据配置共用偏好投影

Status: implemented

## Problem

`ai-credential-save-config.ts` 和 `ai-credential-test-config.ts` 各自声明相同的 `Pick<AiAssistantPreferences, "provider" | "endpoint" | "model">`。保存和测试请求必须使用同一三字段投影，重复声明会让契约漂移。

## Decision

在无 React 依赖的 `ai-credential-preferences.ts` 定义并导出 `AiCredentialPreferences`，两个配置构造模块直接引用它，投影直接依赖独立的 `ai-assistant-preferences.ts`。保留构造结果与请求字段不变，并用类型测试验证投影与原偏好字段一致。

## Existing notes audit

检索活跃笔记中的 `AiCredentialPreferences`、AI 凭据保存和测试配置，没有发现拥有此 TypeScript 类型归属的笔记；端点、凭据协议和原生 AI 设置笔记不涉及该内部投影。

## Alternatives considered

- 保留两个 `Pick`：无需新文件，但重复的跨模块契约仍会漂移。
- 让配置模块共享一个函数返回值类型：会把保存与测试的请求形状耦合；独立纯类型能表达共同输入而不改变函数行为。

## Consequences

- 两个 AI 凭据配置模块共用无 React 的 `AiCredentialPreferences`，保存和测试函数的输出字段与运行逻辑不变；测试禁用条件另共用 `AiCredentialTestPreferences` 的最小投影。
- 原函数路径和调用方式保持，偏好投影与配置函数也从包入口公开；展示模型和宿主保存参数的归属见[AI 凭据编辑公共模型](2026-10-17-ai-credential-editor.md)。类型测试覆盖两个投影与 `AiAssistantPreferences` 的结构一致。
- 配置测试、类型检查、构建、lint、笔记检查和 quick 门禁验证迁移。
