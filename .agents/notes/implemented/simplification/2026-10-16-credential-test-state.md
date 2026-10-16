# Agent Note: 凭据测试状态使用公共模型

Status: implemented

## Problem

`CredentialTestSection` 定义了凭据测试结果状态，而 `useProviderCredentials` 在状态表里再次内联同样的字段，并在渲染时用类型断言接回组件。两个消费者的字段契约可能漂移。

## Decision

把无 React 依赖的 `CredentialTestState` 放到设置模型文件，组件继续从原路径转导出，hook 的状态表直接引用该类型。通过类型测试核对旧导出路径和公共类型一致，保持运行时字段和状态更新不变。

## Existing notes audit

检索活跃笔记中的 `CredentialTestState`、凭据测试和状态字段，没有发现拥有此 TypeScript 展示状态契约的笔记；已有凭据测试笔记描述服务协议和探测行为，与本次纯类型归属调整独立。

## Alternatives considered

- 保留 hook 内联结构：改动最少，但当前类型断言继续隐藏契约重复。
- 把状态放在 React 组件文件：旧路径可用，但 hook 的纯状态模型依赖展示组件；因此选用设置目录下的无 React 模型文件。

## Consequences

- `CredentialTestSection` 继续从旧路径转导出 `CredentialTestState`，包入口仍公开同名类型；hook 的状态表直接使用公共模型，渲染不再需要类型断言。
- 凭据测试的字段、签名可见性和异步代次行为保持不变；公共文件只放无 React 的状态契约，后续纯逻辑消费者可直接复用。
- 增加一个类型兼容测试，相关凭据测试、类型检查、构建、lint、笔记检查和 quick 门禁覆盖迁移。
