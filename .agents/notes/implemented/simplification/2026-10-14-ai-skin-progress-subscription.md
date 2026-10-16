# Agent Note: AI 皮肤进度监听复用公共异步订阅清理

Status: implemented

## Problem

`AiSkinGeneration` 手写异步注册后的取消句柄保存、迟到注册释放和卸载清理。`onProgress` 返回的 Promise 只有成功续接，注册失败产生未处理拒绝；首次挂载和请求开始时合成宿主拒绝注册的组件测试稳定复现，Vitest 报未处理拒绝并以 1 退出。

## Decision

进度监听交给 `trackAsyncSubscription` 持有句柄，沿用其正常卸载、迟到注册释放和拒绝处理。[AI 抽卡状态](2026-10-17-ai-skin-generation-state.md)的领域 hook 直接调用宿主注册，事件由服务会话与请求 ID 的代次和 `requestRef` 一起判断归属，取消先使本地请求失效。公共工具只管理订阅句柄，不接管抽卡状态。

## Existing notes audit

已检索 `AiSkinGeneration`、`onProgress`、异步订阅、订阅清理与 `trackAsyncSubscription`。组件与样式边界归 [AI 皮肤公共模块](../../implemented/simplification/2026-10-14-ai-skin-generation-module.md)；公共工具类型与窗口错误提示归 [窗口状态监听](../../implemented/simplification/2026-10-14-window-state-subscription.md)。它们仍持有独立决定，本篇记录进度事件的请求归属和注册失败边界。

## Alternatives considered

- 只在原链尾加 `catch`。最强理由是修复缺陷的改动最少，生命周期不动；但句柄保存与迟到释放仍复制公共工具的完整实现，后续维护要继续同步。
- 新建 AI 专用进度订阅 hook。最强理由是同时封装事件过滤与清理；但请求标识和取消由抽卡流程管理，其他进度客户端用资源 ID 而非请求 ID。现有公共清理工具覆盖真正相同的句柄生命周期，无需再造抽象。

## Verification

修复前四项正常生命周期测试与整页四项生成、取消、保存、发布测试通过。加入首次挂载和请求开始时拒绝注册的回归后，旧实现稳定报告两项未处理拒绝并以 1 退出；同一组五项组件测试在修复后无未处理错误，连同公共工具与外壳契约合计四文件十一项通过。设置整页同样四项通过，desktop 构建和 HarmonyOS 外壳类型检查通过。

## Consequences

组件删除一份手写句柄保存和释放逻辑，首次挂载与请求开始时的注册拒绝都由公共工具处理，生成能够正常完成。代价是清理规则需跨到公共工具查看。公共工具只负责句柄，事件仍须保留代次守卫；注册调用时机、方法接收者和本地请求即时失效不变。此次不增加宿主错误提示，不声称原生设备验收。
