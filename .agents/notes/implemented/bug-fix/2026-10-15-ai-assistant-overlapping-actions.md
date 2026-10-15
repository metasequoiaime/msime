# Agent Note: AI 模型目录与润色请求独立完成

Status: implemented

## Problem

`useAiAssistant.test()` 启动时递增共用的 `requestGeneration`，使正在读取的模型目录过期。目录请求的 `finally` 只能在当前代次清理忙碌状态，因此成功或失败都会留下 `modelsBusy = true`，按钮无法再次点击。

## Decision

共用代次只标识配置生命周期，两种操作启动时都读取它，不修改它。每种操作既有的 owner 和 busy ref 继续负责自己的互斥与完成归属。同配置的模型目录和润色可独立完成，目录自动选模仍通过 `updateAi` 推进配置代次并丢弃旧模型的润色结果。

## Alternatives considered

- **发送润色时取消目录并立即清除目录忙碌状态**：保留旧代次语义，按钮不会卡住；但同配置下读取目录与润色没有冲突，丢弃已经发出的目录请求没有必要。
- **只在模型请求结束后无条件清除忙碌状态**：改动局部；但旧请求可能解除新配置请求的互斥，破坏已有 owner 守卫。
- **两种操作共用一把锁**：最容易消除交错；但界面本来提供独立按钮，没有理由让慢目录阻止测试或让慢测试阻止读取目录。

## Verification

合成延迟回归在旧实现 3 项失败、1 项反向交错控制通过；修复后连同现有 AI 动作与重复请求守卫共 7 项通过。用例覆盖目录成功和失败后的独立清理、两种启动顺序、各自反馈，以及目录自动选模使旧模型润色失效。测试不访问真实服务，数据全部合成。

## Consequences

同配置下两种网络请求可同时存在，与现有独立按钮和独立互斥一致。不增加重试或底层取消接口；真实配置变化继续失效两种旧操作。

## Existing notes audit

[AI 配置与凭据请求归属](2026-10-15-ai-assistant-preferences-owner.md)负责配置和凭据变化时使旧请求失效；本篇负责同配置下两类操作独立完成，两者共用配置代次但不共用操作 owner。

[Windows 原生 AI 页](../../implemented/feature/2026-10-11-windows-settings-ai-page.md)记录原生宿主移植共享操作的行为，功能边界不同，本次只改共享 hook。[AI 回复模板代次](../../implemented/simplification/2026-10-12-ai-reply-generation.md)涉及另一组件的加载生命周期，无相同决定。
