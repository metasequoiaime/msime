# Agent Note: 共享界面防抖回调生命周期

Status: implemented

## Problem

自定义双拼校验、手写滑块保存、鸿蒙词库搜索和触屏聊天发送后的输入框调整都在延迟执行最后一次操作，并分别注册、替换和清理 `setTimeout`。它们的业务状态不同，但局部计时器生命周期重复；现有 `useTimeout` 只按延迟值变化重启，直接复用会漏掉这些依赖变化。

## Decision

在 `packages/ui/src/core/use-debounced-callback.ts` 提供 `useDebouncedCallback`。Hook 返回稳定的 `schedule` 和 `cancel`：每次 `schedule` 先取消旧计时器，延迟后通过 `useLatestRef` 调用最新回调；组件卸载或延迟变化时清理待执行回调。自定义双拼校验、手写滑块保存、鸿蒙词库搜索和聊天发送后的输入框调整复用这个生命周期，继续由各自调用方决定依赖、代次、请求参数和业务状态。

手写页两个滑块共用的预览与保存协调收在 `packages/ui/src/core/use-settled-slider.ts`，通过公共出口提供 `useSettledSlider`。它立即展示本地值，静止 300ms 后仅在值不同于外部值时保存，并清除本地预览；保存回调需要同步更新外部值，异步持久化由调用方负责。外部值变化会重新等待，回调身份变化不会重置倒计时，卸载会取消未执行保存。手写默认值、设置字段和写入接口仍由手写页拥有。

## Alternatives considered

- 三处继续各自维护 `setTimeout`：业务代码直观，但会继续复制取消、卸载清理和最新回调处理。
- 给 `useTimeout` 增加不透明的重置键：可以少一个 Hook，但调用方需要把多个依赖压成键，容易误把业务身份和值变化绑定在一起，也让一次性定时器和可重复调度的防抖语义混在同一接口中。
- 抽出带动态依赖数组的 `useDebouncedEffect`：调用点看起来更短，但依赖数组需要在 Hook 内展开或依赖调用方稳定化，React 的依赖检查边界不清晰；返回显式 `schedule` 更容易保持现有 effect 的启用条件。
- 把 `useSettledSlider` 留在手写页：源码集中，两个现有调用也能复用；但本地预览、合并保存和外部值接替不依赖手写业务，独立公共 Hook 能让其他滑块直接复用相同语义，并单独验证这些时序。

## Verification

公共 Hook 测试覆盖重复调用只执行最后一次、手动取消、最新回调和卸载清理；自定义双拼、鸿蒙词库、聊天页和相关共享 UI 回归测试通过。桌面类型检查、构建、lint、格式、笔记校验和 `verify-local.sh --quick` 覆盖共享 UI 及宿主边界。

`use-settled-slider.test.ts` 直接覆盖拖动立即预览、最后一次变化后等待 300ms、保存时无旧值闪回、回到原值不保存、最新回调、外部值变化与卸载取消。手写页回归继续覆盖两个调用的宿主展示。

## Consequences

四处 UI 共享同一套防抖计时器替换与清理规则，旧回调不会在组件卸载后执行。滑块额外共享本地预览与保存协调，但不扩展为任意类型表单状态，也不替调用方管理异步写入。异步代次、请求取消边界、搜索分页顺序和聊天输入框布局仍由调用方拥有；新的延迟执行 UI 应优先复用 `useDebouncedCallback`，不要把业务状态塞入公共 Hook。

## Existing notes audit

按 `deboun`、`setTimeout`、`useTimeout` 和延迟查询检索活跃笔记。此前的[共享延迟定时器笔记](./2026-10-15-shared-timeout-hook.md)明确把依赖变化的三个调用方留给需要扩展 API 的独立改动，本篇提供该扩展并接管三处生命周期；手写预览仍由原笔记记录一次性阶段定时器。词库详情的异步代次与防抖业务边界仍由[词库发现代次笔记](./2026-10-12-dictionary-discovery-generation.md)持有。
