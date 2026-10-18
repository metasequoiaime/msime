# Agent Note: 表情面板剪贴板监听复用公共订阅清理

Status: implemented

## Problem

`EmojiPanel` 的剪贴板 effect 原先手写异步监听句柄保存、过期注册释放和退出时释放，重复 `trackAsyncSubscription` 已维护的生命周期。此监听还与首次读取、可见轮询和注册后补读相连，不能把句柄抽走时改变刷新顺序。

## Decision

`start` 负责注册、注册结果后的补读和关闭轮询，返回宿主句柄；公共工具持有该异步结果并负责释放。面板状态 hook 继续管理请求代次、合并后续读取、可见性和修改期间的轮询守卫；400 ms 临时轮询的计时器注册与清理复用 `subscribeToInterval`。首次读取不等注册；注册期间剪贴板页按 400 ms 可见轮询；成功取得句柄才停轮询，注册失败或没有能力时继续轮询。

## Existing notes audit

已检索剪贴板、`clipboardLifecycle`、监听、轮询和 `trackAsyncSubscription`。Android 历史保留和 Linux Wayland 不抢焦点的笔记持有原生采集决定，仍然成立，不归此 UI 句柄生命周期；[窗口监听清理](../../implemented/simplification/2026-10-14-window-state-subscription.md) 持有公共工具可接收空注册结果的契约；[AI 皮肤进度监听](../../implemented/simplification/2026-10-14-ai-skin-progress-subscription.md) 持有请求 ID 过滤决定。本项没有同机制的既有归属。

## Alternatives considered

- 把整个读取与订阅 effect 抽成新的剪贴板 hook。最强理由是面板会短很多，也能独立测试刷新调度；但请求代次同时服务删除、清空和同步，修改守卫来自 `usePanelAction`。整体抽走要增加多项跨模块引用契约，超过本次重复句柄清理的范围。
- 把注册完全交给公共工具，先等它结束再读列表。最强理由是所有订阅顺序统一、流程更短；但宿主注册可能慢或不返回，面板必须在此期间可用，首次读取和临时轮询不能受它阻塞。

## Verification

九项真实面板生命周期测试在迁移前后通过，覆盖立即读取、400 ms 边界、成功注册补读与停止轮询、能力缺失、同步和异步失败、可见性恢复、StrictMode、迟到句柄、离开与重新打开页面、客户端更换、过期快照、通知合并及修改期间暂停轮询。计时行为用虚拟时钟推进契约中的轮询周期，异步结果用可控 Promise 和 React `act` 等待，不以真实延时猜测完成。另有源码测试确认临时轮询复用 `subscribeToInterval`。迁移后连同现有表情、符号、提示生命周期、代次复用和公共工具共八文件二十九项通过；desktop 构建、HarmonyOS 外壳类型检查、lint、改动文件格式检查通过。

## Consequences

面板减少一份句柄保存、过期释放和退出释放实现，公共工具继续用于空注册结果。代价是刷新调度仍由面板状态 hook 持有、句柄释放需跨到公共工具查看。公共工具接收异步结果时多一次 Promise 续接，释放可在注册结果后的微任务完成；事件与读取继续检查生命周期代次，这段间隔不会采纳旧结果。此项不改变原生剪贴板采集或做文件操作，验证覆盖共享 UI。

完整状态与动作归属见[表情面板状态](2026-10-19-emoji-panel-state.md)；订阅工具与刷新时序保持此处约定。
