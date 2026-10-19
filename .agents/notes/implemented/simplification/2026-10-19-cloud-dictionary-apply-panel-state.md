# Agent Note: 云词库应用面板状态集中到公共 hook

Status: implemented

## Problem

`CloudDictionaryApplyPanel` 在呈现函数中持有状态轮询、快照预览、替换确认和撤销操作。状态与 JSX 绑定，无法单独复用完整交互；轮询与操作代次的关系也缺少直接 hook 层的验证。

## Decision

提供 `useCloudDictionaryApplyPanel(client)` 与派生的 `CloudDictionaryApplyPanelState`。迁移现有状态、状态文案函数及动作，面板消费返回值并连接[公共应用内容视图](./2026-10-19-cloud-dictionary-apply-content.md)，由该受控组件持有 DOM、禁用条件与宿主提示；面板保留确认框、状态通知和窗口导航。

保持原 hook 调用顺序、客户端 effect、先作废后读取、每 2000 ms 轮询及先作废再停止的 cleanup 顺序。`statusRevision` 与 `lifecycleRevision` 继续分别保护并发状态请求和客户端生命周期，确认仍在调用宿主前核对生命周期。丢弃预览的内联更新命名为 `discardPreview`，不增加守卫。

## Existing notes audit

检索 `CloudDictionaryApplyPanel`、`panelSnapshotNotice`、云词库快照应用及轮询。

- [共享间隔订阅](../../implemented/simplification/2026-10-15-shared-interval-hook.md)部分重叠；继续持有为何在业务 effect 内注册轮询的决定，只同步调用方路径。
- [macOS 本机版本状态](../../implemented/bug-fix/2026-10-09-macos-snapshot-status-local-version.md)部分重叠；本项保留 `localVersion` 展示与下载前置，不改变宿主读取。
- [移动预览账户代次](../../implemented/bug-fix/2026-10-09-mobile-snapshot-preview-generation.md)与[移动入列账户代次](../../implemented/bug-fix/2026-10-09-mobile-snapshot-enqueue-generation.md)属于宿主写入和账号锁，不改变。
- 前一片[云词条面板状态](../../implemented/simplification/2026-10-19-cloud-dictionary-panel-state.md)属于单词条 CRUD，与完整快照应用分别持有状态。
- 移动退出清理、Windows 云账号、候选皮肤生命周期及其他平台轮询命中属于其他层，不合并决定。

## Alternatives considered

- 只迁移提示函数最小且无需新 hook，但状态和异步交互仍绑定 JSX，无法直接复用。
- 改用 `useInterval` 可以缩短 effect，但客户端切换时轮询相位会改变，既有间隔笔记明确要求保留同一 effect 内重建，所以继续用 `subscribeToInterval`。

## Verification

抽取前相关消费者和共享守卫 34 项、设置页应用面板 1 项通过。新 hook 首次运行 7 项均因缺少公开导出失败；迁移后直接 hook 与原相关套件合计 41 项通过。覆盖 StrictMode 下初始化及轮询清理、客户端切换后重建轮询相位、并发状态响应先后隔离、旧预览成功与失败隔离、旧完成不释放新忙碌、同步防重、忙碌时暂停状态读取、清理预览和令牌、缺少本机版本和无效预览错误。真实桌面消费者覆盖预览、确认、撤销、面板内重试提示及状态请求不能覆盖操作结果。

设置页真实应用面板另 1 项通过，合计 42 项；最终新 hook 7 项重跑通过，独立审查也重跑 7 项通过。状态抽取时 AST 对照确认全部原逻辑、helper、JSX 与丢弃预览回调等价，56 个传递模块没有运行时入口循环。桌面类型检查和生产构建、HarmonyOS 类型检查、改动文件 lint/格式、笔记与差异检查通过。

`bash scripts/verify-local.sh --quick` 输出 `quick check passed`，覆盖仓库契约、Rust workspace、Android 目标、Linux 设置外壳和共享 Apple bridge 编译；HarmonyOS 原生构建缺少 hvigorw 与 staged 依赖、Windows 缺 MinGW，其余原生宿主按改动范围跳过。

## Consequences

状态和呈现分离，其他共享视图能直接使用同一套快照交互；代价是增加公共 hook 与派生类型。原有代次、状态文案和轮询次序保持，已发出的宿主请求不会取消。验证范围是共享 React/jsdom 与编译，不代表原生宿主装机或真实云账号验收。
