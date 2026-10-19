# Agent Note: 云词库本机应用流程的公共受控视图

Status: implemented

## Problem

`CloudDictionaryApplyPanel` 已将轮询、预览、确认和队列操作放入公共 hook，但准备本机版本、确认预览和处理结果三段视图仍内联。另一种窗口外壳无法直接复用同一组可见性、禁用条件和面板内应用提示。

## Decision

提供 `CloudDictionaryApplyContent` 与 props 类型，以 Fragment 返回三段原有 section，保持同级 DOM。显式传入本机版本、快照元数据、请求状态、忙碌状态、宿主是否在面板内应用，以及下载、刷新、入列、丢弃与取消回调。预览令牌由父面板转换为 `canApply` 布尔值，组件不读取令牌或客户端；hook 继续负责轮询、确认和宿主协议。窗口导航、确认框和状态通知留在面板。

## Existing notes audit

按 `CloudDictionaryApplyPanel`、`panelSnapshotNotice`、`snapshotAppliesInPanel`、快照预览与处理结果检索活跃笔记。

- [应用面板状态](../../implemented/simplification/2026-10-19-cloud-dictionary-apply-panel-state.md)部分重叠，继续持有轮询和代次，原地同步视图归属。
- [macOS 本机版本](../../implemented/bug-fix/2026-10-09-macos-snapshot-status-local-version.md)与 [Windows 云账号](../../implemented/feature/2026-10-10-windows-desktop-cloud-account.md)部分重叠，分别保留本机版本前置与面板内处理队列的提示约束，不改变宿主行为。
- [文件面板状态](../../implemented/simplification/2026-10-19-cloud-dictionary-files-panel-state.md)属于导入导出和云端恢复，继续独立；[共享间隔订阅](../../implemented/simplification/2026-10-15-shared-interval-hook.md)属于未改动的轮询机制。
- 其他命中为原生宿主、资源生命周期和无关功能，无完全吸收或过时提案。

## Alternatives considered

- 将三段各自导出可单独复用，但当前三段共同构成同一个预览与应用流程，拆开会让调用方重复连接状态和宿主提示，先保持一个完整受控视图。
- 直接传入整个 hook 返回值可减少接线，但会把预览令牌、确认框和状态管理契约带入纯呈现组件，因此使用显式展示数据与回调。

## Verification

新增组件回归先因缺少公开组件出现 12 项失败；抽取前真实面板、旧消费者与 hook 共 26 项通过。抽取后相关 7 文件、54 项通过，覆盖本机版本前置、queued/preparing 下载限制、预览与应用许可、忙碌禁用、五个动作、终态按钮与面板内应用提示。真实面板新增丢弃后重新下载和空令牌禁止替换回归；旧消费者继续覆盖预览、确认、入列、取消与状态轮询。最终新增测试 2 文件、14 项重跑通过；独立审查也跑相同 14 项、桌面类型检查与差异检查，无阻塞问题。桌面构建、HarmonyOS 类型检查、lint（原有 4 项警告）、格式和笔记检查通过。CSS SHA256 与抽取前一致：`d5f8b485896f48db99d4883b4cc93221613f3ed3aba7f1a6d589a3d0d9b3377d`。

## Consequences

完整本机应用呈现可被不同窗口外壳复用，代价是显式连接展示数据与回调。`canApply` 只表示已有可用令牌，忙碌与预览可见性仍分别决定按钮状态；该布尔值不是权限，也不替代 hook 的令牌校验。组件与编译验证不等于原生宿主应用词库验收。
