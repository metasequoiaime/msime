# Agent Note: Android 删除键先删选区

Status: implemented

## Problem

在 Android 15 上选中「你好」再按键盘的删除键，没有任何反应。Android 宿主的删除键在没有组字时最终都调用 `InputConnection.deleteSurroundingTextInCodePoints(1, 0)`，而 `deleteSurroundingText` 系列只删选区以外的字：选区在文本开头时，选区前面没有字可删，于是毫无反应；选区在中间时，删掉的是选区前一个字，选中的文字原样留着。笔画布局的删除键还绕过了 `deleteCodePointBeforeCursor`，直接调用 InputConnection，也不更新选区回声预期。

实体键盘的退格交给 `super.onKeyDown`，由编辑器自己处理，不受影响。

## Decision

删除键的公共入口 `deleteCodePointBeforeCursor` 先用 `getSelectedText(0)` 询问编辑器有没有选区；有就用 `commitText("", 1)` 以空串替换选区，并作废选区回声预期，不再删光标前的字；没有选区时照旧按码位删一个字。笔画布局改为调用这个入口。`check-host.sh` 加了守卫：入口必须先删选区，`MSIMEInputService` 以外不许直接调用 `deleteSurroundingTextInCodePoints`。

日文九键轮切（`stepJapaneseToggle`）也经过这个入口，它删的是刚上屏的上一个假名，此时光标总是收拢的，不会走到删选区的分支。

## Alternatives considered

- 在 `onUpdateSelection` 里缓存选区，按删除时读缓存：缓存可能落后于编辑器（应用自己改选区、回报迟到），按下时直接问编辑器更可靠，`selectedEditorText()` 已经这样用。
- 有选区时发送 `KEYCODE_DEL` 按键事件：各应用对合成按键事件的处理不一致（网页、终端类编辑器尤其如此），而 `commitText` 替换选区是 InputConnection 明确规定的行为。
- 在每个删除键的调用处各自判断选区：调用处有七八个，散开写容易再漏掉一个，笔画布局就是这样漏掉的。

## Consequences

选中文字后按删除会删掉选区，长按连删时第一下删选区、之后照常逐字删除。每次在无组字时按删除多一次 `getSelectedText` 的 IPC 调用。鸿蒙端用的是 `deleteBackwardSync`，它在有选区时的行为尚未在真机上确认，这次没有改。

## Verification

- `bash platforms/android/check-host.sh`：通过，171 个 Android JVM 冒烟。新守卫在修复后通过；把笔画布局改回直接调用 InputConnection、或去掉入口里的删选区调用，守卫都会失败。
- CI 同款 Gradle 命令编译六个版本的 Java 并跑 `lintFullRelease`：BUILD SUCCESSFUL。
- 未在真机或模拟器上验证。
