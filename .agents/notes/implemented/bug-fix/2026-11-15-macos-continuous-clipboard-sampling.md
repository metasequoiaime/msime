# Agent Note: macOS 剪贴板采样不等待历史落盘

Status: implemented

## Problem

[#11448](https://github.com/metasequoiaime/msime/issues/11448) 已确认全平台范围。原生 `MacClipboardMonitor` 每次等待设置读取与历史保存完成后再休眠 400 ms，保存稍慢时中间复制的文本被系统覆盖。单纯调短休眠不能消除 `await save` 期间的空窗。

## Decision

主线程独立按 50 ms 检查变化计数，将已过滤并规范化的正文放入最多 128 条的队列。设置读取和保存仍串行执行，每次保存前核对共享历史开关，保存失败重试队头；采样确认与保存确认分开，系统覆盖旧内容后仍能重试原文。

关闭、读取设置失败或取消时停止采样并丢弃待保存正文。保存入口本身继续核对持久化开关。启动、重新启用和目录切换不导入原有剪贴板；敏感类型和长度限制由原有捕获器执行。队列满时保留最新 128 条，迟到保存结果只能确认原来的条目，不能删除新队头。不改变用户设置默认值。

## Existing notes audit

检索剪贴板采集、`MacClipboardCapture`、`MacClipboardMonitor` 和连续复制。Windows/Linux 的连续复制笔记持有相同 issue 的其他平台切片；本项单独记录 macOS 调度与内存边界。既有[隐私标记规则](../../implemented/feature/2026-10-10-windows-clipboard-privacy-panel-statistics-host-diagnostics.md)继续成立；[面板订阅清理](../../implemented/simplification/2026-10-14-emoji-clipboard-subscription.md)仅处理共享界面的列表刷新，保留其 400 ms 回退节奏。

## Alternatives considered

仅将原循环改为 50 ms 能缩短空闲期漏采窗口，改动最小，但仍会在等待文件锁、存储或设置读取时停止采样。采用独立采样与有界队列。系统只暴露当前正文与[变化计数](https://developer.apple.com/documentation/appkit/nspasteboard/changecount)，不能把多次变化的计数还原成已经覆盖的历史；不承诺捕获两个采样点之间已经被覆盖的内容。

## Verification

隔离命名 `NSPasteboard` 中暂停第一条保存，再分次复制两条且确认采样已经读到；恢复后第一条保存先失败一次，再成功按序保存全部三条。相同用例在旧监视循环中等待第二次采样时失败，新循环通过。原有采集器、监视器、服务生命周期与新连续复制四组专项通过（远端 `20261115-000546`）。新增落盘等待期间取消的停采断言后重新验证。

原有保存重试仍要求两次保存尝试及相同状态序列；系统正文读取次数由 4 改为 3，因为重试使用已捕获的原文，不再重读剪贴板。这是采样与保存分离后的预期契约，未关闭历史开关或绕过旧场景。远端 `20261115-001415` 完整 quick 与原生宿主构建通过，随后构建 `macos-clipboard-tests` 并运行 `ctest -L clipboard-local`，11 项全部通过。报告者现场仍需复验。

## Consequences

50 ms 检查提高变化计数读取频率，正文仅在计数变化时读取；队列最多约 1.5 MiB，异常长时间无法保存时只保留较新的 128 条。系统权限限制、进程未运行或两次采样之间发生覆盖仍可能漏采，不能伪称操作系统历史导入。
