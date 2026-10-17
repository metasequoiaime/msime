# Agent Note: 触屏 AI 对话二级页不带底部标签栏

Status: implemented

## Problem

[AI 对话改为二级页](2026-10-16-mobile-chat-secondary-page.md)之后，Android、iOS 和鸿蒙共享设置页的对话页仍保留底部标签栏。这条行为来自[对话键盘弹出时保留底部 Tab](../bug-fix/2026-10-13-android-chat-keyboard-keep-tabs.md)：那时对话是一级标签，打字时要能直接切到别的标签。用户在 Pixel 3 XL 上看到对话二级页底部还挂着四个标签，明确要求去掉。Android 上的情形最明显：对话页把标签栏高度算进输入法上方的空间，键盘弹起后标签栏夹在输入栏和键盘之间。

## Decision

对话是二级页，离开它走页面顶部的返回，三端的对话页都不带底部标签栏，返回到来源页后标签栏回来。

- Android：`HomeActivity.updateTabsVisibility` 在回退栈变化和创建时检查栈顶，`ChatFragment` 在栈顶时收起 `home_tabs`。`ChatFragment` 的底部只让开系统导航栏和输入法（`WindowInsetsPolicy.bottomContentInset`），不再预留标签栏高度。独立承载的试用 Activity 本来就没有标签栏，行为不变。
- iOS：`KeyboardTryoutView` 用 `.toolbar(.hidden, for: .tabBar)` 收起标签栏，返回走导航栏。
- 共享设置页（鸿蒙手机）：`contentFills` 的两种对话页（试用键盘、AI 对话）都不渲染 `MobileSettingsTabs`，输入栏自己的 `touchComposer` 让出手势区。

## Alternatives considered

- **保留标签栏，只在键盘弹起时收起。** 打字之外仍能一键切标签。但标签栏随焦点出没正是 10-16 那篇去掉的行为（用户当时看不到底栏），而且没有解决用户这次说的「二级页面不该有底部标签」。
- **保留标签栏，把 Android 的标签栏挪到键盘下面。** 不改信息架构，但 `adjustResize` 缩短窗口后标签栏只能落在键盘上方，10-13 那篇试过按 IME inset 平移，标签栏跑到顶部；要放到键盘下面就得关掉 `adjustResize`，输入栏又会被键盘盖住。

## Consequences

对话页多出一个标签栏高度的聊天空间，键盘弹起时输入栏紧贴键盘。代价是从对话页去别的标签要先返回一次，打字时不能直接切标签；草稿和请求状态在返回后保留（共享设置页的对话保持挂载，Android 的对话页在回退栈里）。iOS 从「试用键盘」进来的对话页同样没有标签栏。

## Verification

- 共享设置页：`apps/desktop` 的 `tests/core/mobile-tab-bar.test.tsx` 与 `tests/settings/mobile-app-chrome.test.tsx` 改为断言对话页没有「主要功能」导航、返回后导航回来、返回再进入草稿仍在，51 项通过。
- Android：`check-host.sh` 通过；`ChatNavigationDeviceSmoke` 改为断言从 AI 设置和「试用键盘」进入的对话页都看不到 `tab_settings`，返回 AI 设置后标签栏回来且仍选中「设置」。
- iOS：`OnboardingUITests` 里在对话页上点标签栏的三处改为用导航栏返回，并断言标签栏不可点。
