# Agent Note: 凭据操作完成时保留请求期间的新编辑

Status: implemented

## Problem

`useProviderCredentials` 在 AI、腾讯、语音识别和润色操作成功后无条件清空对应输入。操作期间输入框仍可编辑，后输入的 Token、地域或接口地址因此被旧回执抹掉。四种延迟回归在旧实现失败，未编辑和失败控制用例通过。

## Decision

操作开始时捕获对应草稿。成功后用 React 函数式更新逐字段比较当前值和捕获值，只清空仍相同的字段，保留请求期间改变的字段。客户端代次、操作互斥和状态回执仍沿用现有机制；失败不清空，保存与清除共用此规则。

## Alternatives considered

- **操作期间禁用输入框**：实现简单，可完全避免并发编辑；但会阻止用户在慢请求期间准备下一次修改，也改变各凭据字段的交互。
- **整个草稿只要改变就全部保留**：只需一次对象比较；但仅改地域也会留下已保存的密钥。逐字段清理让已提交且没有继续编辑的秘密及时离开输入框。

## Verification

新增 12 项回归，连同既有客户端代次、操作互斥和初始状态用例共 32 项通过。延迟成功回执保留新输入，包括新输入与回执同一批 React 更新；未改字段仍清空，失败保持草稿，其他凭据类型不受影响。桌面前端类型检查及构建、鸿蒙设置壳类型检查、lint、`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 通过。lint 的四条未使用导入警告属于既有文件；构建保留既有大包提示。没有调用真实凭据服务或进行原生窗口验收。

## Consequences

比较以值为准，改动后又改回原值视为同一草稿，可以清空。不自动提交新输入。这里只处理共享设置页的内存草稿。

## Existing notes audit

检索活跃笔记的 `useProviderCredentials`、凭据草稿和清空规则。[凭据初始状态](../../implemented/bug-fix/2026-10-15-provider-credential-initial-status.md)与[AI 请求归属](../../implemented/bug-fix/2026-10-15-ai-assistant-preferences-owner.md)部分重叠，分别管理状态快照与网络请求失效，本篇管理用户输入；保留并互链。Android Google 登录笔记仅涉及系统登录入口，无关。
