# Agent Note: iOS 26 键上滑、下滑各一个可配动作

Status: implemented

## Problem

iOS 26 键的字母键只有一条纵向手势：沿设置里的「滑动方向」滑过 14pt 输入右上角角标（[三端 26 键默认上滑输入角标](2026-10-25-mobile-qwerty-swipe-symbols.md)），反方向什么也不做。用户要的是上下各一个动作，并且能逐键改：例如 V 上滑输入 `?`、下滑粘贴剪贴板。角标固定在其中一个方向，另一个方向就永远空着，而粘贴、复制、撤销这类动作在键盘上又没有一步可达的入口。

两处相邻的顺手问题：⌫ 已经能横滑逐格删，删一段长文仍要一格一格滑到底；`#+=` 页只能从数字九宫格的左列或符号层第三排进，123 键的上滑是空的。

## Decision

- 每个字母键（a–z）有上滑、下滑两条独立配置。模型是 `LetterSwipeAction`（无、角标符号、粘贴、复制选中文字、剪切选中文字、撤销、重做、全选、自定义文本、剪贴板历史、切换中英文、光标左移、光标右移）与 `LetterSwipeBinding`（动作 + 可选的自定义文本），都放在 `SharedUI/preferences/KeyboardLayoutPreference.swift`，App 与键盘扩展共用。
- 存本机 App Group 的 JSON `keyboard.gesture.letterSwipeBindings`，不写共享文档、不随账号同步，与「滑动输入符号」「滑动方向」一致：这些是手感偏好，跟着设备走。
- 没配置过的方向按 `KeyboardLayoutPreference.swipeSymbolsDirection` 回落成该键的角标，另一方向不接。所以升级后不动设置的人看到的仍是原来的单向行为。显式选「无」存成一条 `action=none` 而不是删键：删掉就又落回角标，用户关不掉单个方向。
- 角标表从键盘扩展的 `LetterHintTable` 移到 SharedUI 的 `LetterHintSymbols`（`LetterHintTable.hint` 转发），设置页才能把它显示成「角标符号 ?」。表本身仍与 Android 的 `LetterHintTable` 逐条相同。
- 手势在 `KeyboardKeyButton`：`swipeActions` 由控制器按方向给出动作，`onSwipeAction` 回传。判定用 `LetterSwipePolicy.direction`，阈值 14pt 与角标相同，先过阈值的方向锁住这次触摸，之后手指再动也不改判；预览气泡显示该方向的符号或动作名，松手才执行。长按仍是角标，不受这套配置影响。总开关沿用「滑动输入符号」：关掉后两个方向都不接。
- 原有的单方向路径保留给「长空格」逗号（上下都出句号）和 14/18 键字母键，它们不进 `LetterSwipePreference`。
- 动作执行在 `KeyboardViewController.performLetterSwipeAction`：粘贴读 `UIPasteboard` 后结束组字再上屏；复制、剪切写剪贴板；剪切在组字中拒绝（组码和正文会错位）；撤销、重做走宿主的 `undoManager`；全选走运行时探测的 `selectAll:`；光标移动按一个字符调 `adjustTextPosition`。做不到时 `showDiagnostic` 提示，不静默失败。
- 设置入口在「键盘 → 布局」页的预览图：点预览图里的字母键，底部弹出该键的上滑、下滑两个动作选择器（`LetterSwipeKeyEditorSheet`），选「自定义文本」时就地输入，支持单键恢复默认。选完立刻写 App Group，没有「保存」。预览图默认只给字母键画右上角真实角标符号，被单独改过动作的键再在下方标动作徽标（如「下滑·粘贴」），不画双拼键位提示。原来的独立「字母键上下滑」列表页已删除。
- 123 键（手机底行 `layoutToggle` 与 iPad 底行 `tabletLayer`）加上向上 `UISwipeGestureRecognizer`：上滑直接置 `showsSymbols`、`showsMoreSymbols` 进 `#+=` 页，跳过数字九宫格。点击仍进数字九宫格，长按仍打开符号面板。
- 双拼键位提示开启时，字母键的上、下各让出一条 11pt 提示行：底部是声母韵母提示，顶部是右上角角标。只让下边距时字母整体上移 5.5pt，22pt 字母的字顶会钻进 10pt 角标里（角标距上边 3pt、字底约 12.5pt）。上下各让 11pt 后字母仍在按键中心。手机横屏键高只有 34pt，让出两条只剩 12pt，那里仍只让下边距（`letterKeyFitsTwoHintBands`）。方向进了 `LetterCaseInputs`，`viewDidLayoutSubviews` 里转屏会补一次 `updateLetterCaseControls`。
- 工具栏品牌键菜单新增「双拼键位提示」图块，与设置页那一行改的是同一个 `touch_shuangpin_key_hints`：写共享文档成功才重画键面，写不进去时提示并保持原值。只在中文模式的 26 键双拼（含粤语双拼）上可点。

## ⌫ 上滑清空

- 与横滑共用删除键上同一个 pan，按起手方向分派：竖直向上是清空，向左是逐格删除，中途不切换。状态机在 `BackspaceClearSwipe`（40pt 待命、滑回 16pt 取消），执行在 `KeyboardViewController+BackspaceSwipe.swift`。
- 有组字时只清组码和候选（`session.cancel()`），没有组字时逐字清空光标前的正文。逐字删受 `BackspaceClear` 限制：每轮 4096 字、最多 64 轮，编辑器不配合也会停。
- 手写板不接（⌫ 撤的是笔迹）；有选区不接，理由与横滑相同：一次 `deleteBackward` 删的是整段选区。手势被取消或滑回取消区时不清。

## Alternatives considered

- **全局两档 + 单键覆盖**：配置项少，但想改多个键时要逐个覆盖；每键独立配置只是设置页多几行，模型更简单。
- **只有全局一组动作**：最省事，但用户要的正是「V 下滑粘贴、其他键下滑做别的」，全局做不到。
- **反方向改成打开该键的字母选项面板**（Android 现在的行为）：不用新模型，但用户要的是可自定义的动作，而且 iOS 没有这套面板。
- **反方向留空，只做上滑的单一动作**：改动最小，但没有解决「一手势一动作」的根本诉求。
- **配置写进共享文档、随账号同步**：换设备能带走，但手势是逐设备的习惯（键高、拇指位置都不同），也与现有的「滑动输入符号」「滑动方向」本机存储不一致；且共享文档要新增跨端字段，其他客户端收到未知动作会校验失败。
- **用长按代替下滑**：长按已经给了角标，再叠一层要重新设计长按时序，且用户明确要的是滑动。
- **把设置页放进键盘的品牌键菜单**：26 键 × 2 方向的选择器放不进键盘内的面板，放设置页更合适；品牌键菜单只加了「双拼键位提示」这一个开关。
- **⌫ 上滑清空做成独立手势识别器**：与横滑的 pan 抢触摸，删除键上要维护两套仲裁；按起手方向分派同一个 pan 更稳。
- **清空做成一次 `selectAll` + `deleteBackward`**：一次就干净，但会移动光标、改选区，宿主的 `selectAll:` 也未必实现，且清空后光标位置不可控；逐字删虽然慢，但只动光标前的内容。

## Consequences

- 只覆盖 26 键的 a–z。九键、14/18 键、微软双拼的 `;` 仍走原来的单方向角标，没有上下双动作。
- 「滑动输入符号」关掉后所有字母键上下滑都不接，包括粘贴这类与符号无关的动作；要用只能先把开关打开。这条沿用了原有关闭语义，没有另加总开关。
- 粘贴、复制、剪切需要键盘开启完全访问；iOS 读取剪贴板可能弹一次系统「允许粘贴」。撤销、重做、全选依赖宿主输入框实现，部分 App 会提示「不支持」。
- 配置不随账号同步。换设备后要重配。
- 三端行为进一步分叉：Android 反方向打开字母选项面板，鸿蒙只有单方向，iOS 现在是可自定义的双向动作。共享 Rust 层没有新增字段。

## Testing

`KeyboardTests/input/LetterCornerHintTests.swift` 覆盖：上下两个方向各自过 14pt 阈值、方向互斥与取位移大者、默认回落到角标、显式「无」不被回落覆盖、控制器把 V 的上下滑动作用到按键上、总开关关掉后两个方向都为空、`LetterSwipeBinding.summary(symbol:)` 把「角标符号」写成真实符号（无符号回落、带符号带上、其它动作不受影响）。`BackspaceSwipeDeletionTests.swift` 覆盖 `BackspaceClearSwipe` 的待命与取消阈值、非有限距离、`BackspaceClear` 的分轮与轮次上限。`MorePanelFoldTests.swift` 覆盖品牌键菜单里「双拼键位提示」的顺序、读写共享文档、键面 `contentInsets` 与提示标签随之切换、全拼方案下置灰。契约脚本 `test-ios-key-layout-settings.py` 新增 `LetterSwipePreviewContract`：独立列表页已删、预览图字母键点击弹 `LetterSwipeKeyEditorSheet`、可点只在「滑动输入符号」开着且 26 键时、徽标只标单独改过的键、预览图不画双拼提示。

本次改动没有编译也没有跑测试：开发机是 Windows，没有 Swift/Xcode 工具链。跑通的只有 `git diff --check`、括号平衡粗检和 `platforms/ios/tests/settings/ProjectConfigurationTests.py`（35 项中 34 项，唯一失败是 release date 脚本需要 `wsl` 的既有环境问题）。需要在 macOS 上 `xcodegen generate` 后跑 `MSIMEKeyboardTests`，并在真机验收：V 下滑粘贴、Q 下滑自定义文本、有选区时下滑复制、⌫ 上滑清空组字与正文、123 上滑进 `#+=`、开双拼提示后字母与角标/提示不重叠、横屏只让下边距；以及本次 UI 重构的点预览图字母键弹设置、改完徽标当场刷新、点击与改键距/行距的拖拽手势不冲突。

## Related notes

- [三端 26 键默认上滑输入角标](2026-10-25-mobile-qwerty-swipe-symbols.md)：部分取代。iOS 的单方向角标仍在，成为每键未配置时的默认；三端默认上滑的决定不变。
- [字母键角标可以隐藏](2026-11-02-letter-corner-hints-switch.md)：不冲突。「显示字母键角标」只控制画不画，角标照旧交给按键；本笔记的上下边距只在角标真的画着时让位。
- [iOS 手机键帽统一 44pt、第三排等宽、数字九宫格与 ⌫ 横滑删除](2026-11-22-ios-compact-phone-keys-and-backspace-swipe.md)：部分重叠。⌫ 上滑清空与横滑共用删除键的 pan，横滑的步长、选区与联想边界不变；上滑清空新增，见上文。
- [双拼键位提示](2026-10-09-touch-shuangpin-key-hints.md)：承接其提示标签的位置，本次补上让位与工具栏开关。
- [Android 字母滑动选项](../../implemented/bug-fix/2026-10-21-android-letter-option-gesture-settings.md)：无关但有对照。Android 的反方向是打开选项面板，本次刻意没跟，见上面被否的方案。
