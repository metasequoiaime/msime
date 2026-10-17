# Agent Note: 设置外壳 toast provider 边界

Status: implemented

## Problem

设置入口 `packages/ui/src/index.tsx` 同时负责页面模型、外壳布局和 HarmonyOS toast provider 的挂载。`ShellToasts` 只有一条平台边界，却与整个入口共享 `ReactNode` 和 `ToastProvider` 导入；外壳继续拆分时会把这段 provider 包装逻辑重复搬运。

## Decision

`packages/ui/src/settings/settings-shell-toasts.tsx` 提供 `SettingsShellToasts`，只接收 `enabled` 和子内容：启用时挂载现有 `ToastProvider`，禁用时原样返回子内容。设置入口继续仅在 HarmonyOS 外观下启用它，`useToast` 和 toast 的计时生命周期保持在 `core/toast`。

## Alternatives considered

- **继续在入口内联条件 JSX**：代码最短，但 provider 边界仍属于页面入口，后续外壳拆分要重复移动。
- **让 `ToastProvider` 自己判断平台**：能省掉 wrapper，但 core 层会依赖设置平台和页面外壳，破坏 provider 的通用边界。
- **把 toast 状态和 provider 一起搬到新的设置 hook**：可以集中更多代码，但会把已有计时状态决定与外壳布局耦合；本切片只移动 provider 包装。

## Consequences

设置入口不再直接依赖 `ReactNode` 或 provider 实现，toast 的启用边界有独立渲染测试。非 HarmonyOS 仍不产生 `role="status"` 区域，HarmonyOS 仍使用同一个 live region 和计时实现。计时复用与 `ToastProvider` 本身的决定见[共享临时反馈计时](../../implemented/simplification/2026-10-14-ui-timed-feedback.md)。
