# Agent Note: iOS 点读音改为打开放大拼写编辑条，三端统一

Status: implemented

## Problem

#10591 要求三端「点读音行编辑拼写」的行为一致。改之前三端各做一套：Android 点读音（拼写写在输入框里时点读音位置的笔）在读音行和候选之间打开一行放大编辑条，点哪个字母光标就到哪；iOS 点读音弹系统菜单「编辑拼写」，只有「光标移到开头」「光标移到末尾」「删除光标后的字母」三项，到不了中间的位置；鸿蒙只把读音和光标画到候选栏上。

iOS 还有两处缺口。26 键和九键的读音并在候选行最左一段，拼写写进输入框后这一段收成零宽、按钮隐藏，刚打完字根本没有入口，要先拖空格把光标移进拼写才出现。读音单独一行（14/18 键）时，读音和候选之间的皮肤行距（`lineGap`）不在命中区里。

维护者 2026-11-02 拍板（「按建议」）：三端统一采用 Android 的「笔图标 / 读音 → 放大编辑条」；iOS 26/9 键常驻笔图标入口；编辑条把键盘撑高一行；iOS 去掉「编辑拼写」菜单，包括「删除光标后的字母」。

## Decision

iOS 键盘点读音打开放大拼写编辑条，界面和规则照 Android（`ImeCompositionEditor`、`CompositionEditorView`、`CompositionCaretPolicy`）。

- **入口**：`preeditButton` 不再挂拼写菜单，轻点走 primary action `openSpellingEditor`，播按键反馈（`playInputClick`，含振动）。只有空闲时的本地输入菜单还在，这时 `showsMenuAsPrimaryAction` 才为真。可编辑时无障碍描述是「编辑拼写」、标为按钮，值仍是正在组的拼写。
- **什么时候能点**：沿用 iOS 原有的 `editableSpelling`：`hasSpellingCaret`（中文方案和粤拼），排除五笔 14 键，拼写全为 ASCII，不在本地模式。它与 Android 的 `editable` 加上「排除五笔 14 键」一致（Android 那一半由 Android 的 PR 收紧）。
- **笔图标**：可编辑、拼写写进了输入框、读音段没有标题可画时，读音段画 SF Symbol `pencil`（读音字号的 1.15 倍，按钮至少 40pt 宽，同 Android `SpellingEditIcon` 和读音 `minWidth` 40dp），不再收成零宽。光标被移进拼写、或有已选的那段词时照旧画文字。不能编辑或空闲时仍收成零宽。
- **命中区**：读音单独一行时，`ReadingRowView` 把行底下的皮肤行距（`updateCandidateCanvasMetrics` 算出的 `gap`）也算进命中区；14 键拼音选择条下面那段缝不归读音，与 Android 只扩到读音那一段（`preeditFrame`）为止相同。并入候选行时仍是整段、上下占满。
- **编辑条**：`CompositionEditorBar` 是键盘根 stack 里候选栏下面的一行，高 52pt（44pt 拼写加上下 4pt），背景透明透出键盘底色，左边 `CompositionSpellingView` 画 22pt 拼写和 2pt 宽的强调色光标（拖动时常亮，平时 1 秒一周、亮 0.6 秒，「减弱动态效果」时不闪），右边 64×40 的「完成」。它进 `gapRoutingExclusions`，键距里的触摸不会交给它，横滑光标也不在它上面起手。
- **画什么**：画读音本身 `shownPreedit`（九键、全拼 14 键和 18 键是 `nine_key_reading` 的拼音，26 键是引擎给的带分隔的拼写），不含已选的那段词；读音对不上按键时退回画 `editing_text`，不猜位置。读音和按键按顺序逐字对齐：九键的字母按电话键位对数字；全拼 14 键、18 键的按键是每组的首字母（打 `nihao` 按的是 BN UI GH AS OP，`editing_text` 是 `bugao`），对齐时字母按它在正在打字的那张网格上的组码比（`letterGroups`，组码表就是键面的 `FourteenKeyLayout` / `EighteenKeyLayout`，与引擎 `KeyGrid::code_of` 同一张），所以编辑条画 `ni'hao`，点 h 前面光标落在 GH 前；选过拼音后按键里有原字母（`ni'gao`），同一个字母照样算。只给全拼 14 键、18 键传网格：26 键逐字比，`n` 不会对上 `b`；九键、14 键、18 键双拼的按键是双拼码，读音是拼音，不按字母一一对应，照旧对不上、退回画原始按键；五笔 14 键不可编辑。网格按控制器此刻实际打字的布局取（`typesLetterGrid` 且方案是全拼 14 键或 18 键，`grid_composing` 为真），不按设置推断。读音行在光标移进拼写后照旧画带光标的组码（`bu|gao`），与九键画 `64|426` 相同，这次不动。光标位置用 `CompositionCaretPolicy.displayOffset`，隔着显示分隔时画在哪一侧看引擎的 `grid_composing`（`MetasequoiaInputSnapshot.gridComposing`，这次新解码）：网格组字画在分隔后，26 键画在分隔前。
- **编辑**：点一下把光标放到最近的字缝；水平拖动超过 8pt 后按手指位移移动，起点是按下时光标的位置；拖到两端 28pt 内时每 90ms 走一个按键并滚动。目标位置经 `CompositionCaretPolicy.tapTarget` 换成按键下标，再由 `moves` 换成引擎命令：到开头 `moveCaretToStart`、到末尾 `moveCaretToEnd`，其余逐格 `moveCaretLeft` / `moveCaretRight`。宿主从不自己改拼写，删除和插入照旧用键盘本身。VoiceOver 下放大拼写是可调整元素，上下轻扫逐个按键移动光标（对应 Android 的 `ACTION_SET_SELECTION`）。
- **键盘高度**：编辑条显示时 `preferredKeyboardHeight` 加 52pt，经 `updatePreferredKeyboardHeight` 生效；顶栏下的间距（`topRowGap`）移到编辑条和按键之间，按键和候选栏的位置与高度不变。编辑条自己钉一条 52pt 的高度约束（优先级 999，隐藏时让给栈视图收起它的约束）：按键行的高度是从键盘剩下的高度里分出来的，编辑条只靠拼写的固有高度撑起时，系统键盘按新高度放大视图后根栈视图把它拉到约 90pt，四排键各矮约 12pt、往下挪。这是模拟器系统键盘里看到的，进程内的测试原来只看高度约束的常量、没按新高度重排视图，所以没发现。
- **收起**：「完成」先关编辑条、再 `moveCaretToEnd`，组字留着；任何一次渲染里没有组字或 `editableSpelling` 为空（上屏、取消、退格删空、进本地模式、换方案、键盘收起时 `finishComposition`）都自动收起；编辑条开着时再点入口保持打开。

光标换算在 `KeyboardExtension/Sources/input/CompositionCaretPolicy.swift`，是 Android 同名类 `alignment`、`tapTarget`、`markIndex`、`displayOffset`、`moves` 的 Swift 移植；`CompositionCaretPolicyTests` 逐条对应 Android 的 `CompositionCaretPolicySmoke`，两端对同一组输入必须给出同一个光标位置。

## Alternatives considered

- **保留 iOS 的系统菜单，让 Android 和鸿蒙改成菜单** — 最强的理由是菜单不改键盘高度，iOS 键盘扩展改高度会让 UIKit 重排，严重时会重新选输入法。不用：菜单到不了拼写中间，九键长句里改中间一个数字仍要拖空格；Android 的编辑条已经上线，三端改成菜单等于把 Android 现有能力退回去。
- **编辑条盖住候选行，不改键盘高度** — 避开上面那条风险。不用：盖住之后看不到候选，改完拼写还得关掉编辑条才能选词；维护者选了撑高，与 Android 一致。
- **保留菜单里的「删除光标后的字母」** — 能点选位置以后它只省一步（点到那个字母后面再按退格），留着就是同一个入口上两套界面。维护者确认去掉。
- **按布局推断网格组字，不解码 `grid_composing`** — iOS 键盘扩展收不到硬件键盘，组字所在的布局基本就是网格与否，少一个字段。不用：契约（`msime_client.h` 的 View 说明）要求宿主画光标时看 `grid_composing` 而不是布局，Android 也这么做，按布局推断在九键双拼的字母键、本地模式等边角上可能和引擎不一致。

## Consequences

- 26 键和九键刚打完字就看得到入口，点一下就能把光标放到拼写中间任意位置；三端（Android、iOS，以及按同一方案重写的鸿蒙）点读音后出现同一个编辑条，开关时机相同。
- 代价：26 键和九键组字时候选从行首右移约 40pt，读音段的笔一直占着；编辑条开关会改键盘高度，组字结束时自动收起也会改一次高度。iPhone 17（iOS 27.0）模拟器的系统键盘里录屏逐帧看过 14 键打开、「完成」、再打开、选候选上屏四次高度变化：每次键盘上沿在相邻两帧之间一步移动 156 px（52pt），中间没有过渡帧、闪白或空白帧，按键行的位置在整段录像里不变，扩展进程号不变，诊断日志里 `keyboard_built` 只出现一次，没有被系统重新选输入法，上屏后下一键照常组字。真机还没看过。
- 「删除光标后的字母」在手机上没有对应操作了，要点到那个字母后面再按退格。共享 C ABI 的 `DeleteForward` 和 bridge 的 `deleteForward()` 留着，桌面和测试仍在用。
- 测试：`CompositionCaretPolicyTests` 覆盖换算；`CompositionCaretTests` 断言 26 键打字后有笔、无菜单、无障碍「编辑拼写」，点入口打开编辑条且键盘高 52pt，再点保持打开，点拼写开头和读屏调整能移光标，「完成」收起并把光标移回末尾，选候选后自动收起，九键点读音字母落到对应数字前，行距命中区不含拼音选择条下面那段。原来断言读音段收成零宽、第一个候选贴在行首的 `InlineCompositionDefaultTests` 两条和 `NineKeyKeyboardTests.testComposingSwapsTheToolbarForTheCandidatesInTheSameRow` 改为断言读音段不画拼写、只画笔（40 pt），候选紧跟在笔后面，拼写变长也不移动。`CompositionCaretPolicyTests` 另有全拼 14 键、18 键读音对组码，26 键和双拼网格不对组码；`FourteenKeyKeyboardTests` 断言 14 键点读音后编辑条画读音（`bu'hao` 或 `ni'hao`，跟着引擎首选）而不是 `bugao`、点 h 前面引擎光标落在 GH 前（读音行 `bu|gao`），选 ni 之后画 `ni'hao`；按新的高度约束放大视图后编辑条 52pt、三排键的高度和到底边的距离不变。这些是进程内构造控制器的测试；系统键盘入口另用临时 XCUITest 在模拟器上走过（见上），没有提交，也没有在真机上看过。
- 相关笔记：[手机键盘放大读音行的点按区域](../bug-fix/2026-11-02-mobile-preedit-tap-target.md)（命中区的原决定，这次把行距补进 iOS），[Android 放大拼写条压成一行](2026-10-25-android-compact-spelling-editor.md)（编辑条的参照实现）。
