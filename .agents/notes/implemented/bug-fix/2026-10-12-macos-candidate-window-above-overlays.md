# Agent Note: macOS 候选窗开在截图遮罩之上

Status: implemented

## Problem

在 PixPin 的区域截图里用文字工具打字，搜狗的候选窗出现在文本框下方，水杉的候选窗却看不见，只剩内嵌的组字，按空格照样上屏（#7326）。会话和光标位置都正常：搜狗能贴着文本框摆候选窗，说明 PixPin 给出了可用的光标矩形；`renderCandidates` 里因 `invalid_caret` 收起的路径也就排除了。问题在窗口层级：候选窗、输入模式提示和双拼键位图都开在 `NSPopUpMenuWindowLevel`（101），截图工具的全屏遮罩开在屏保层级或更高，候选窗被排在遮罩下面。`orderFrontRegardless` 只在同一层级内往前排，帮不上忙。

## Decision

`platforms/macos/src/candidate/CandidateWindowLevel.h` 的 `MSIMECandidateWindowLevel()` 是贴着光标出现的窗口的唯一层级来源，取 `CGWindowLevelForKey(kCGCursorWindowLevelKey) - 100`，和鼠须管相同：高于应用能开的各种置顶层级（屏保、截图遮罩），仍在光标之下。用它的有：

- `InputController.mm` 现用的候选窗 `MSIMECandidatePanel`
- 只剩测试在编的旧候选窗 `MetasequoiaCandidatePanel`（`CandidatePanel.mm`），保持一致
- 输入模式提示 `InputModeHUDPanel.mm` 和双拼键位图 `ShuangpinKeymapPanel.mm`，它们同样在光标旁出现，否则在同一个遮罩下一样看不见
- 打字特效 `TypingEffectPanel.mm` 取 `MSIMECandidateWindowLevel() - 1`，仍紧贴在候选窗之下，火花不盖住候选

悬浮工具栏、语音波形、表情送达提示和屏幕键盘不跟光标走，仍在 `NSFloatingWindowLevel`，不在这次范围内。

## 候选右键菜单

候选的右键菜单不需要特殊处理。AppKit 用 `popUpContextMenu:withEvent:forView:` 开菜单时，把菜单窗口放在来源窗口层级加一：在本机用一个无边框、不激活的 `NSPanel` 实测，面板在 `kCGCursorWindowLevelKey - 100`（`CGWindowListCopyWindowInfo` 报 layer 2147483530）时，菜单开在 2147483531，仍在候选窗之上。

第一版曾给顶层菜单设代理，`menuWillOpen:` 把候选窗降回 `NSPopUpMenuWindowLevel`，以为菜单固定开在弹出菜单层级、会被候选窗盖住。这个前提不成立：实测降下来以后面板在 101，菜单跟着开在 102，两者一起落到截图遮罩下面，报告者在 PixPin 里右键候选时会看不见候选窗，也看不见已经接管鼠标的菜单。评审指出后撤掉了代理，候选窗全程留在 `MSIMECandidateWindowLevel()`。

## Alternatives considered

- 固定取 `NSScreenSaverWindowLevel`（1000）：能盖过多数截图遮罩，层级也更常见。但遮罩开在屏保层级之上（不少截图、录屏工具就是这样）时照样输，问题只是往上挪了一档。光标层级减 100 是 AppKit 之下能取到的最高稳妥位置，鼠须管长期这样用。
- 按客户端窗口自适应，取 `max(NSPopUpMenuWindowLevel, client.windowLevel + 1)`，用 `IMKTextInput` 的 `windowLevel`：只比当前输入的窗口高一层，对其他应用最克制。但 Qt 应用和自绘遮罩的客户端不一定如实报告它，PixPin 这类正是最可能报错的一类，修不住本问题。
- 右键菜单开着时把候选窗降回弹出菜单层级：基于「菜单固定开在弹出菜单层级」的错误前提，实测反而让候选窗和菜单一起掉到遮罩下面，见上一节。

## Consequences

- **收益**：候选窗、输入模式提示、双拼键位图在截图遮罩、屏保层级的置顶窗口和全屏应用之上都看得见；Spaces 和全屏的 `collectionBehavior` 没变。层级集中在一个头文件里，打字特效的「紧贴候选窗之下」随之变化。
- **代价**：这些窗口现在高过所有应用的置顶窗口，包括别的应用的弹出菜单；它们只在组字或切换模式时短暂出现，与鼠须管的表现一致。
- **测试**：`shortcut` 断言控制器建的候选窗在 `MSIMECandidateWindowLevel()`、高于屏保层级、低于光标层级，并断言右键菜单若有代理，开、关时候选窗都留在 `MSIMECandidateWindowLevel()`；把候选窗层级改回去，或重新加上降层级的菜单代理，断言都会失败。`typing-effect-panel`、`input-mode-hud-panel`、`shuangpin-keymap-input` 各自断言层级。
- **未验证**：本机没装 PixPin，没有在真实截图遮罩里看过候选窗；右键菜单开在面板层级加一是在独立的测试程序里用 `CGWindowListCopyWindowInfo` 核对的，没有在输入法进程里截图核对。修复发布后请报告者确认；若候选窗仍不出现，打开诊断日志看收起原因，`invalid_caret` 说明是另一个光标矩形问题。
