# Agent Note: Android 符号面板在不建会话的输入框里也能打开

Status: implemented

## Problem

#10517：Android 0.6.0 上，Termius 这类终端应用的输入框里点「符」打不开符号面板，长按 123 也没反应。

终端应用常把输入框声明成 `TYPE_TEXT_VARIATION_VISIBLE_PASSWORD | TYPE_TEXT_FLAG_NO_SUGGESTIONS` 来关掉联想和纠错。`EditorPolicy.useEngine` 对三种文字密码 variation 返回 false，这类输入框不建引擎会话（`startInputRuntime` 只读外观，不调 `scheduleEngineStartup`），`session` 一直是 0。而符号面板的所有入口都按会话拦：`ImePanels.showSymbolPanel` 开头是 `if (s.session == 0 || ...) { notice("符号面板尚未就绪"); return; }`，底行和工具栏两个「符」按钮按 `session != 0 && connection != null` 画灰，长按 123（`bindSymbolPanelHold`）和文本编辑面板长按回车也先看 `session == 0`。

符号面板本身不需要会话：点按经 `commitText` 直接上屏，颜文字和各符号分类经 `NativeClient.emojiCatalog` 读资源目录里的随包目录，会话只用于打开前 `command(2)` 结束组字，而没有会话的输入框里本来就没有组字。iOS（`KeyboardViewController.showSymbolPanel`）和鸿蒙（`KeyboardView.openSymbolSurface`）的入口都不看会话，只有 Android 有这个限制。

## Decision

- **入口不再看会话**：新增 `SymbolPanelEntryPolicy.opens(connection, panelBuilt)`，`showSymbolPanel`、长按 123、文本编辑面板长按回车和两个「符」按钮的可用性都经它。`showSymbolPanel` 只在有会话时先 `command(2)`，命令失败、会话被停掉时照旧不打开。会话还在建（`STARTING`）的输入框也随之能打开，与表情面板在 #10086 之后的行为一致。
- **资源目录在不建会话的输入框里也填上**：`emojiResources` 原先只在要建会话的输入框里由 `prefillSessionIndependentDirectories` 填，密码框里打开面板后颜文字和各符号分类会显示「暂时不可用」。`startInputRuntime` 的 `!engineWanted` 分支按同一套校验（绝对路径）只填 `emojiResources`。偏好目录不在这里填：`loadAppearanceWithoutSession` 读到外观时已经填上，插件符号集（`loadPluginSymbolGroups`）读的就是它。表情面板的可用性仍由 `ToolbarReadinessPolicy.emojiEnabled` 决定，`UNAVAILABLE` 时返回 false，填上资源目录不会让密码框里的表情变得可点；功能面板的表情磁贴另外要求 `session != 0`。
- **密码框里按字面输入**：`SymbolPanelEntryPolicy.pairsClosingMark(inputType, pairedPunctuation)` 在 `EditorPolicy.password` 为真时返回 false，轻点「（」不补「）」。密码要逐字符对得上，多出来的后半个用户看不见（密码框里是圆点），登录会莫名失败。跨过后半个的 `stepOverPairedSymbol` 不用改：密码框里从不补，配对栈一直是空的。
- **密码框里不记「常用」**：`SymbolPanelEntryPolicy.recordsRecent(inputType, learningSuppressed)` 在密码框里返回 false。「常用」是全局的，在密码里用过的符号会出现在聊天框面板第一页，等于把密码的一部分摆出来。这与剪贴板、语音、按键统计和邮箱后缀在密码框里的口径一致。打开面板时仍显示已有的「常用」，读不涉及这个输入框里输入过什么。

## Alternatives considered

- **让密码框也建引擎会话**：入口自然就不拦了。不做：密码框不走引擎是有意的（不学习、不组字、不读上文），改它牵动整个隐私口径，远超这个问题。
- **在 `!engineWanted` 分支整个复用 `prefillSessionIndependentDirectories`**：triage 计划给出的选项之一。不做：它同时写 `preferencesDirectory`，而这个字段在不建会话的输入框里已由 `loadAppearanceWithoutSession` 在读到外观后填上，提前同步写一次会让语音配置、世界时钟、按键音这些读偏好目录的地方在外观还没读到前就拿到目录，时序与现在不同；本次只需要资源目录。
- **密码框里照样补后半个**：与普通输入框行为一致，代码最少。不做，理由见上：密码里看不见的多余字符比少一个便利更糟。
- **密码框里干脆不给打开符号面板**：等于维持原状，而终端里打 `|`、`~`、`{}` 正是这个面板的主要用途。

## Consequences

- **收益**：终端、密码框、数字框和启动失败退回直接输入的输入框里，「符」按钮不再是灰的，点按和长按 123 都能打开面板，内置分类、颜文字目录和插件符号集都可用。
- **代价**：数字和电话输入框里也能打开面板，选了编辑器不接受的符号时由编辑器自己过滤，与 123 层直接点符号键一样。
- **未覆盖**：Termius 实际声明的 inputType 无法从仓库确认；修复覆盖所有密码类 variation，若它报的是 `TYPE_NULL` 或普通文本则根因不同（那两种本来就建会话）。设备冒烟在 `NumberPadDeviceSmoke` 的文字密码类型里加了「长按 123 打开面板、点（只多一个字符、关闭后回到键盘」，需要模拟器跑；「常用」不记录只由 JVM 冒烟覆盖。

## Verification

`platforms/android/tests/keyboard/SymbolPanelEntryPolicySmoke.java` 覆盖可见密码加不要建议（终端）、文字密码、网页密码、数字密码、普通文本、只关建议的文本和 `TYPE_NULL` 下的三条规则，并确认前四种确实不建会话、`TYPE_NULL` 仍建。`check-host.sh` 钉住接线：所有入口经 `SymbolPanelEntryPolicy.opens`、`showSymbolPanel` 不再按 `session == 0` 拦且只在有会话时 `command(2)`、补后半个和记常用经策略按输入框类型判断、不建会话的输入框填上资源目录。`tests/device/keyboard/NumberPadDeviceSmoke.java` 的密码类型循环加了符号面板一段。
