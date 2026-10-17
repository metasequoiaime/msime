# Agent Note: CI 只在共享界面变更时运行界面测试

Status: implemented

## Problem

`ci-platforms.yml` 用 `apps/*` 判断是否需要运行共享设置界面的 Vitest。桌面 Tauri 原生宿主也在 `apps/desktop/src-tauri` 下，所以只改 Rust 宿主命令的 PR 也会启动包含四千多个用例的共享界面测试。打字统计云端竞态修复只改了 `apps/desktop/src-tauri` 和 client-services，却因此触发界面测试；该检查在 CI 上运行超过十一分钟后被杀掉，阻塞了与界面无关的 PR。

## Decision

把界面范围收窄到 `packages/ui`、`apps/desktop/src`、`apps/desktop/tests`、`apps/harmony/src` 及各自的前端配置和依赖清单。`apps/desktop/src-tauri` 保持在共享 Rust 路径的其它平台门禁中，但不再触发共享界面 Vitest。

## Alternatives considered

- **继续用 `apps/*`，提高 Vitest 超时。** 会让原生宿主改动继续等待不相关的界面测试，也掩盖路径分类错误。
- **在每个 PR 上手动跳过界面测试。** 会失去共享设置代码的必需检查，且容易漏跑。
- **把整个 Tauri 目录移出共享路径。** 会误跳过原生宿主自己的 Android、Linux、Windows 和其它平台门禁；只调整 `ui` 标记更符合现有职责边界。

## Consequences

- 共享设置代码和其配置变更仍运行 UI lint、类型检查和 Vitest。
- Tauri 原生宿主变更不再等待共享 UI 测试，但仍由 `shared` 路径触发 Rust 与平台编译门禁。
- 新增前端根文件时需要同步更新路径清单；这与现有工作流路径清单的维护方式一致。
