# Agent Note: 抽取 MCP 连接状态

Status: implemented

## Problem

命令模型已经独立，`mcp-connect.tsx` 仍同时持有状态读取、安装授权与替换、复制反馈、客户端选择和权限草稿。其他展示入口无法复用这些交互规则，状态回归必须依赖完整设置组件。

## Decision

`use-mcp-connect-state.ts`，集中上述状态与操作，接收明确的 `status`、`install`、`copyText` 和 `confirm` 回调。组件保留确认框渲染和已有 JSX，只消费 hook 的状态与动作。客户端名称和权限元数据在同一模块定义一次，供状态与展示共用；不扩大包入口。

复用 `useAsyncActionRunner`、`useAsyncGeneration` 和 `useTemporaryFlag(1600)`。读取 effect 仍跟随三项宿主能力；安装仍在授权、冲突和完成边界检查当前归属；复制仍用独立代次，只反馈最近一次请求。权限顺序、存储键、草稿和 tab 回退保持原样。

## Existing notes audit

- [命令模型](../../implemented/simplification/2026-10-24-mcp-command-model.md)部分重叠，更新组件与 hook 的事实归属，纯命令规则保留。
- [加载归属](../../implemented/bug-fix/2026-10-14-mcp-load-owner-refresh.md)、[旧安装确认](../../implemented/bug-fix/2026-10-14-mcp-stale-entry-confirmation.md)、[复制失败反馈](../../implemented/bug-fix/2026-10-14-mcp-copy-failure-feedback.md)和[复制计时](../../implemented/bug-fix/2026-10-12-mcp-copy-feedback-lifetime.md)各持有不同的时序约束，全部保留，抽取遵循这些约束。
- [独立设置页](../../implemented/architecture/2026-10-18-mcp-independent-settings-page.md)拥有导航与宿主能力注入，本片不改变入口或原生 MCP 实现。

## Alternatives considered

- **仅抽取读取和复制**：移动更少，但权限草稿、客户端选择和安装仍留在展示组件，复用连接流程仍需依赖该组件。
- **在 hook 中调用 `useConfirm` 并返回对话框**：调用方接入更短，但状态层会绑定确认组件；注入确认回调可独立测试延迟回答和失效操作，页面继续拥有对话框。
- **重建统一 MCP 异步状态机**：能显式列出所有阶段，但会重做已经验证的公共 runner 和代次逻辑，增加回归范围。本片直接组合已有基础 hook。

## Verification

独立 hook 回归先因模块缺失失败；实现后 6 项通过，覆盖两个实例的状态隔离、权限草稿与保存、安装或复制能力更换后的旧读取失败、迟到授权与新操作忙碌隔离、冲突替换和成功后清草稿、复制顺序与计时器清理。与既有 MCP 展示、命令模型和生命周期回归合跑，6 个文件 54 项通过。桌面生产构建、桌面与 HarmonyOS TypeScript 类型检查、lint、变更文件格式、笔记校验和差异检查通过。提交前 quick 通过，Rust 工作区、Android 目标、Linux 设置外壳和共享 Apple bridge 编译成功；HarmonyOS 真打包缺少预备资源、Windows 缺 MinGW 而跳过，其余原生宿主按范围跳过。

## Consequences

MCP 连接流程可以在不挂载设置表单或确认组件时复用，展示组件缩减为状态消费与 JSX。新 hook 仍拥有与原组件相同的每实例状态；权限偏好保持既有可选本地存储，不改变宿主协议或配置写入实现。三个宿主能力的归属边界、1600 毫秒复制提示和确认时序均保留。

源契约回归迁到实际持有 runner 的 hook，并检查组件接入 hook，避免未来在展示层重新复制生命周期。注入的确认回调不纳入宿主代次，与原有 `useConfirm` 的稳定身份契约一致。验证使用合成宿主与 jsdom，不代表真实助手配置写入或原生设备验收。
