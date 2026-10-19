# Agent Note: 云端候选面板状态集中到公共 hook

Status: implemented

## Problem

`CloudCandidatesPanel` 同时持有编码草稿、确认查询、调频参数、候选与固定位置结果，以及四类确认修改动作。这套逻辑绑在 JSX 中，难以独立复用；确认等待和修改后刷新还需要核对客户端切换时的操作代次。

## Decision

提供 `useCloudCandidatesPanel(client)` 与派生的 `CloudCandidatesPanelState`，搬出现有状态、查询、确认和修改后刷新。面板组合[候选结果与固定位置视图](./2026-10-19-cloud-candidate-results.md)，保留窗口导航、查询可见性与参数控件约束，继续复用 `usePanelAction`、`useLatestRef`、`useConfirm` 和既有云词库协议。

保留明确查询才发送编码、规范拼音用于修改、简拼与快捷短语分流、先显示候选再加载固定位置、修改后按已确认查询刷新。先以真实组件回归验证旧确认与旧修改回复的宿主隔离，确认缺陷后在同一片复用操作代次修复。

## Existing notes audit

按 `CloudCandidatesPanel`、候选排序、固定位置、`cloud-candidates-text` 检索活跃笔记。

- [完整目录状态](../../implemented/simplification/2026-10-19-cloud-dictionary-catalog-panel-state.md)与[单词条状态](../../implemented/simplification/2026-10-19-cloud-dictionary-panel-state.md)部分重叠，复用相同确认代次原则；目录与候选协议分别保留。
- [查询栏组合输入](../../implemented/bug-fix/2026-10-14-cloud-dictionary-ime-submit.md)部分重叠，键盘事件和禁用规则仍由公共工具栏处理。
- [桌面客户端装配](../../implemented/simplification/2026-10-18-desktop-cloud-dictionary-client.md)持有宿主会话能力，不改变该层。
- 其余命中属于 Rust 引擎排序、九键缓冲、平台候选窗口和设备测试门禁，与本篇 React 云端候选面板状态无关；没有完全吸收或过时提案。

## Alternatives considered

- 保留在呈现组件内可以减少模块，但状态不能直接用于另一种呈现方式，也无法脱离完整面板验证异步交互。
- 为候选面板另建客户端 generation 可以只隔离客户端替换，但已有 `usePanelAction` 的代次同时覆盖查询取代、客户端替换和卸载，复用它更容易保持各面板一致。

## Verification

新增 hook 测试先因缺少公开导出失败；抽取后 hook、确认生命周期、旧面板消费者、查询栏、动作和确认框相关套件共 60 项通过，设置页真实候选消费者 2 项通过。保留已有类型导出与 JSX 结构；确认与修改回归先证明旧宿主误发，再以代次守卫修复。桌面构建、HarmonyOS 类型检查与改动文件 lint 通过；独立审查新增 22 项通过，55 个传递模块无入口循环。

## Consequences

候选主请求和固定位置次请求的发布顺序不能颠倒；旧客户端结果不得影响新结果和忙碌状态。组件与编译检查不代表真实云账号或原生设备验收。
