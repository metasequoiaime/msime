# Agent Note: iOS 整段上屏改回「改标记再取消标记」，部分选词仍直接插入

Status: implemented

## Problem

#10873：iOS 0.53.0 在闲鱼 App 的搜索框和聊天框里用全拼选词，拼音和汉字一起留在输入框里；#10843（B 站评论区拼音一起上屏、语序错乱）关闭时并到这里跟进。

0.53.0 带上了 [iOS 部分上屏直接替换标记区](2026-10-31-ios-cantonese-partial-commit.md)（#10374）。它把所有替换行内预编辑的上屏都改成 `textDocumentProxy.insertText`，依据是系统文本框的 `insertText` 会替换标记文本。自绘输入框不一定这样：Flutter 的 `FlutterTextInputView` 一类宿主的 `insertText` 只替换当前选区，而键盘写标记时把选区放在标记末尾、长度为 0，于是拼音留在原处，汉字插在后面，正好是报告里的样子。闲鱼是不是 Flutter 输入框没有核实，真实闲鱼和 B 站也没有验证过。

## Decision

`KeyboardViewController.insertOwnText` 替换行内预编辑时，按同一次渲染之后还有没有组字分两条路：

- 组字到此结束（快照的 `preedit` 为空，包括整段选词、空格上屏、数字键先原样上屏拼音）：`setMarkedText(正文, 选区在末尾)` 接 `unmarkText()`。替换标记文本是 `setMarkedText` 的本义，只要宿主支持标记文本就会替换掉拼音，不依赖宿主 `insertText` 怎么处理标记。这是 #10374 之前的做法，带 `${cursor}` 的快捷短语也一直这样提交。
- 部分选词（快照里既有 `commitText` 又有剩下的 `preedit`）且没有光标后缀：照 #10374 直接 `insertText`，再标记剩余组字。#10374 在真实跨进程代理上复现过「改标记、取消标记、再标记剩余组字」会让剩余组字盖掉已选汉字。

`render` 用 `continuesComposition: !snapshot.preedit.isEmpty` 告诉 `insertOwnText` 是哪一种。不替换组字的插入（`replacingComposition` 为假）照旧先清标记再 `insertText`。

## 测试

- `InlineCompositionCommitTests`（`KeyboardTests/input/InlineCompositionTests.swift`）用按自绘输入框规则建模的文档（`insertText` 只替换选区）驱动正式渲染器：整段上屏后只剩汉字、调用序列是改标记加取消标记；真实引擎逐键打 `nihao` 点首选后没有残留字母；部分选词的调用序列仍是先插入再标记。改动前前两项失败，文档里是 `甲ni'hao你好丙`。
- `KeyboardExtensionEditorUITests.testQuanpinCommitReplacesPinyinInAnEditorWhoseInsertTextOnlyReplacesTheSelection` 在测试宿主里放一个同样规则的 `UITextInput` 编辑器（`--selection-only-insert-editor`），经系统加载的键盘扩展和真实跨进程代理打字选词。改动前编辑器里是 `ni'hao你好`，改动后是 `你好`。
- `testQuanpinPartialSelectionKeepsCommittedPrefixInSystemEditor` 在系统 `UITextField` 里打 `nihaov` 选「你好」，文档是「你好」接剩余字母，守住 #10374 的部分选词路径；它不依赖粤拼词库，粤语逐段用例在没暂存粤拼词库的机器上进不了入口。
- `GlideTypingTests` 与 `KeyLayoutStyleTests` 的假文档原来只把 `insertText` 算作上屏，现在取消标记时留下的文字也算上屏，与系统文本框一致。
- `QuickPhraseCursorKeyboardTests` 的编辑器等输入会话接好再开始（50ms 改 500ms）。整套跑时排在 `PluginKeyboardTests`、`PunctuationSettingsTests` 之后，等得不够时「改标记再取消标记」之后光标会被挪到文末或者改动没写进去：改动前光标后缀那一项就会这样失败，整段上屏也走这条调用之后，没有后缀的那一项也会。

## Alternatives considered

- **所有替换组字的上屏都改回改标记加取消标记**（triage 的方案）：一条路径最简单，自绘宿主里部分选词也能对。但 #10374 在真实系统扩展里复现过这个序列在部分选词时丢掉已选汉字，进程内测试证明不了它没事，不能拿 #10284 的修复去换。
- **先清标记再 `insertText`**：`setMarkedText("")`、`unmarkText()` 再插入，在两类宿主里看上去都对。但 `render` 里已经记着跨进程时先撤标记再插入会读到旧选区、数字跑到拼音前面，部分选词同样会撞上，无法在这里验证。
- **按宿主 bundle ID 列例外**：键盘扩展拿不到可靠的宿主身份，Flutter 应用也不止闲鱼一个。

## Consequences

- 收益：整段上屏不再依赖宿主 `insertText` 替换标记文本，自绘输入框里选词后只剩汉字。
- 代价：自绘输入框里凡是同一个快照「上屏一段、还剩组字」的仍走 `insertText`，那里可能还会留下标记文本。不只是部分选词：韩语每打完一个音节就上屏它、接着组下一个（`crates/engine/src/korean/scheme.rs` 的 `finished_syllables_leave_the_composition`，就地组字方案总是标记组字），五笔顶字（出厂开）在行内预编辑开着时也是上屏首选接着标记第五码，都走这条路。要改它得先在真实跨进程代理上弄清剩余组字为什么会盖掉已选汉字。
- 代价：整段上屏多了一对 `textWillChange`/`textDidChange` 回声（`setMarkedText` 接 `unmarkText` 会回调，`insertText` 不会），由 `OwnEditEchoWindow` 的时间窗认出，与快捷短语光标路径相同。
- 带光标后缀的部分选词（快捷短语后面接着组字）本来就走改标记加取消标记，这次不动。

## 未验证

真实闲鱼、B 站和真机都没有验证，需要报告人更新后确认。
