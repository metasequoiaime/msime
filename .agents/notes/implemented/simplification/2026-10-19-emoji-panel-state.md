# Agent Note: 表情与符号面板集中管理状态和动作

Status: implemented

## Problem

`EmojiPanel` 把目录加载、分类分页、最近使用、剪贴板读取与修改、通知和焦点恢复混在 JSX 前后，视图事件还直接组合多个 setter。状态归属与订阅、操作代次的关系难以单独验证和复用。

## Decision

由公共 `useEmojiPanel` 管理这些状态和动作，返回显示数据、导航绑定和语义动作。剪贴板记录由受控的 [EmojiClipboardEntries](2026-10-19-emoji-clipboard-entries.md) 消费条目和动作，最近符号、符号目录和通用目录共用受控的 [EmojiItemButton](2026-10-19-emoji-item-button.md)，面板统一把原文和关键词交给既有选择动作；`EmojiPanel` 保留其他 JSX、宿主能力控制和按钮文案，搜索框、记录与目录按钮使用内部样式模块；搜索、重载、激活模式、更多入口和分页使用 hook 动作。保留现有 effect 顺序、请求代次、400 ms 可见轮询、宿主订阅补读、剪贴板修改守卫和删除后的焦点恢复。

## Existing notes audit

[剪贴板订阅](../../implemented/simplification/2026-10-14-emoji-clipboard-subscription.md)部分重叠，原有句柄释放与刷新时序仍归公共工具和面板状态，本片把组件归属同步为 hook；当时放弃整体抽取是为了限制那一片范围，本片集中迁移状态与动作，不翻转订阅决定。[临时通知](../../implemented/simplification/2026-10-14-ui-timed-feedback.md)和[桌面符号目录](../../implemented/feature/2026-10-14-desktop-symbol-directory.md)约束提示时长、分类、搜索和最近使用，不改变它们的行为。桌面宿主面板与客户端笔记只管理桥接与主题，原生平台笔记只管理各自宿主，与本片状态归属无关。

## Alternatives considered

- 只抽剪贴板 effect：可明显缩短组件，但它与删除、同步、清空共用请求代次和修改守卫，拆开后需要传递多个可变引用，状态边界不完整。
- 只搬 JSX：能减少单文件长度，但组件仍持有所有状态，调用方无法单独使用或验证面板动作。采用完整面板状态 hook，与已抽取的手写、语音和云词库面板一致。

## Verification

- 抽取前 12 个文件、50 项相关测试通过；新增 4 项 hook 行为测试先确认因缺少公开 hook 失败，再完成实现。
- 抽取后 13 个文件、54 项测试通过，覆盖目录与符号、最近使用、通知生命周期、剪贴板订阅、激活模式和代次工具复用；hook 测试补充分页搜索、成功后记录、StrictMode 客户端切换、目录重试与剪贴板操作互斥。
- 独立审查逐段比较原组件与 hook，除注释、空白外原状态与 effect 语句相同，9 个 JSX 动作保持原 setter 顺序；独立定向回归同为 54 项通过。
- 桌面生产构建、HarmonyOS 外壳类型检查、改动文件 lint 与格式、`git diff --check`、笔记校验和 `verify-local.sh --quick` 通过。ArkTS HAP 环境未准备，quick 跳过该阶段；此片仅验证共享前端，不代表各原生宿主的设备验收。

## Consequences

状态与动作可以独立复用和验证，视图不再组合 setter；代价是排查目录或剪贴板问题需从 JSX 跳到 hook。保留已有语句与 effect 次序，避免改变旧快照的接受条件。hook 内的 DOM 导航仍依赖面板标记，返回绑定必须挂在同一根节点。
