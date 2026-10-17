# Agent Note: Android 对话键盘弹出时保留底部 Tab

Status: implemented

「对话页保留底部 Tab」已被 [触屏 AI 对话二级页不带底部标签栏](../feature/2026-10-17-mobile-chat-hides-tabs.md) 取代：对话改为二级页后，离开它走顶部返回，三端的对话页都收起标签栏。本篇关于 `adjustResize` 与输入法避让的设备证据仍成立。

## Problem

[输入框让开底部导航](2026-10-13-android-chat-input-covered-by-tabs.md)通过隐藏导航修复遮挡，但用户打字期间无法切换 Tab，必须先关闭键盘。输入框可见和导航可点按都需要满足。

## Decision

移除对话页通知宿主隐藏导航的入口。`HomeActivity` 使用 `adjustResize`，系统缩短根窗口后底部导航自然位于键盘上方，不额外按输入法高度平移。对话页底部预留 `max(systemBottom, imeBottom) + tabs`，同时避让导航和输入法；独立的试用 Activity 没有 Tab，继续只避让系统栏和输入法。切换 Tab 仍由 `onHiddenChanged` 清焦点并隐藏键盘。

检索审计：旧的输入框避让笔记被本篇取代并互链，保留其原决定与验证理由；对话 Tab 功能笔记与本篇部分重叠，功能和会话边界继续有效；社区入口重组笔记只描述菜单结构，与本次避让无关。

## Alternatives considered

- **继续隐藏导航**：输入区获得最多空间，但直接导致本次不能切换 Tab 的问题。
- **按 IME inset 额外抬升导航**：全屏未缩短窗口时可以让导航避开键盘，但 Android 12 真机的 `adjustResize` 已缩短根窗口，再平移会让导航错误地跑到顶部。设备截图确认后去掉额外平移。
- **只恢复导航，输入区仍只让开 IME**：代码更少，但导航再次覆盖输入栏；输入区必须继续预留 Tab 高度。

入口层级由[移动端 AI 对话二级页](../feature/2026-10-16-mobile-chat-secondary-page.md)调整，输入框避让、试用行为和会话边界继续保留。

## Consequences

键盘打开时输入框和全部四个 Tab 均可见，直接切换其他 Tab 后键盘关闭。聊天区因此少一个导航栏高度的可用空间。没有更改聊天请求、草稿或账号状态。

## Verification

- `check-host.sh` 通过 230 项 JVM 冒烟、JNI 语法检查和设备源码编译，新增导航与输入法同时避让的数值覆盖。
- 发布签名 APK 构建通过并覆盖安装 Pixel 3 XL（Android 12）；实际打开对话输入框后，输入框、导航和键盘均可见；直接点统计页成功切换且键盘关闭。没有发送聊天内容。
- Android 15（API 35）专用模拟器安装同一 APK，确认输入框、四个 Tab 和键盘的实际位置，直接点设置页成功且键盘收起。`scripts/verify-local.sh --quick` 和 `pnpm run verify-notes` 通过。
