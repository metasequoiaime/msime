# Agent Note: macOS 激活时向客户端覆盖键盘布局

Status: proposed

## Problem

#11151 报告 HBuilderX 等 Qt 应用无法输入。本机 HBuilderX 5.34（Qt 6.5.3）能复现：空组字时先按物理 `n/i` 没有候选，先按物理 `a` 后可以组字。Qt 同时报告 Cocoa 字符与 Carbon 的零值不匹配。

[Qt 6.5.3 的键盘映射](https://github.com/qt/qtbase/blob/v6.5.3/src/gui/platform/darwin/qapplekeymapper.mm)先读输入法布局覆盖，再读当前输入源；水杉输入源没有 Unicode 布局数据。Apple SDK 的 `IMKInputSession.h` 要求输入法每次激活时调用客户端的 `overrideKeyboardWithKeyboardNamed:`，让客户端设置其 TSM 文档的键盘布局。当前宿主没有调用。

## Proposal

在 `activateServer:` 完成父类激活之后，通过客户端的 `overrideKeyboardWithKeyboardNamed:` 指定 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 返回的布局。每次激活都重新读取，不缓存，也不修改用户选择的输入源。没有有效布局或客户端不支持该接口时跳过。

主体放在独立的 `ClientKeyboardLayout.h`，控制器只接入一行。布局仍跟随用户选择的 ASCII 布局，与[字母快捷键跟随布局](../../implemented/bug-fix/2026-10-09-macos-letter-shortcuts-follow-keyboard-layout.md)及[布局键身份](../../implemented/bug-fix/2026-10-23-toggle-hotkey-layout-identity.md)的决定一致；旧笔记只部分重叠，不取代。

## Alternatives considered

- 在输入法进程里调用 `TISSetInputMethodKeyboardLayoutOverride`。接口直接且无需客户端方法，但实际调用成功后 HBuilderX 的首次输入仍失败，不能覆盖另一个进程的客户端 TSM 文档。
- 固定指定美式布局。来源确定、容易验证，但会改变使用 Dvorak、Colemak 等布局的用户手感，因此沿用当前 ASCII 布局。

## Acceptance criteria

行为测试覆盖有效布局转发、空值和无接口客户端、重复激活所需的重复调用；本地 quick 通过。真实 HBuilderX 中先按 `n/i` 即出现候选，取消后再次输入、切换输入源再输入也正常，保留改前改后截图。测试期间关闭内容学习和网络候选，恢复原安装和原始输入源。

## Risks

不能仅凭 SDK 说明或测试替身宣称修复；只有真实 HBuilderX 验证后才把笔记转为 implemented。客户端激活时调用远端接口，需确认不会造成重入或影响既有输入模式同步。
