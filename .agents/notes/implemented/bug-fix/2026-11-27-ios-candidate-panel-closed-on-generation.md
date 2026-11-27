# Agent Note: iOS 展开候选面板不再因换代被收掉

Status: implemented

## Problem

真机反馈：键盘刚起来时点「展开候选」，候选网格有时装上就没了——键区先被面板盖住、随即又露回来，看起来像「闪了一下」「点了没展开」；删掉几个字或收起键盘重来，同一个面板就能正常停住。

面板本身不改键盘高度：`showCandidatePanel` 把 `KeyboardCandidatePanelView` 铺在候选条下沿到键盘底之间，`installPanel` 里 `setKeyRowsCovered(true)` 把键区各行 `alpha` 置 0。所以用户看到的「键区闪一下」等于面板装上之后又被收掉。

能立刻收掉面板的代码路径只有四条：`viewWillDisappear`（收起键盘）、用户再点一次箭头、`didReceiveMemoryWarning`（有意为之，展开网格最占内存）、以及 `refreshCandidatePanelAnnotations` 的换代分支。前三条都不是「刚启动时偶发」，问题在第四条。

`refreshCandidatePanelAnnotations` 在候选 generation 变化时刷新面板，原来的规则是：九键三栏面板、或刚在面板里选过候选（`candidatePanelFollowsSelection`）时留着面板按新一代重建，**其余一律 `closeKeyboardPicker()`**。而会换代的不只是「上屏、退格、换方案」这些组字真变了的情况，还有云候选与 AI 行到达、引擎放出暂扣的尾部、词库同步——都在刚起第一段组字时最容易赶上。用户刚展开列表就赶上一次异步换代，面板就被收掉，于是表现为「有时展开不了、等一会儿或重新打一次就好」。

## Decision

- 换代时先看组字：`snapshot.preedit == current.preedit` 且新一代列表非空（`generationOnly`）时，普通候选网格也留在原地换新一代，不再收起。组字真的变了（上屏、退格、换方案、清空）照旧收起——那是另一份候选，留着反而看到过期内容。
- 这种情况重画时保留滚动位置（`reload(keepingScrollPosition: true)`）：用户可能已经翻到第二屏，换代不该把他弹回开头。九键面板与「刚选过候选」那条路径的行为不变（仍回到开头），它们的换代本来就跟选词、退格绑在一起。
- 与 `#6888` 给 AI 行开的先例合成同一条规则：换代只换了列表内容、没换组字，就不该把整个面板收掉；区别只是这次不再限于 AI 行。
- 内存警告仍然收面板（这是对的：展开网格是键盘里最占内存的东西），但面板开着时补一条 `memory_warning_closed_picker` 日志，并在候选条上显示 4 秒「内存紧张，已收起展开的候选」。诊断日志写在 App Group 里，未签名的测试包读不到，这个现象否则永远分不清是换代还是内存。

## Alternatives considered

- **只把云候选到达那条路径改成不收**：云候选只是换代原因之一，引擎放出尾部、词库同步走的是同一条分支，按原因打补丁会反复漏。
- **面板开着时干脆不换代、等用户关掉再刷**：列表会停在旧一代，用户翻到一半看到的是过期候选，比收回面板更糟。
- **换代后收起再重新展开（保持展开状态但换一次）**：仍然会有一次 alpha 归零再复原的闪烁，正是要修的现象。

## Consequences

- 收益：首段组字里展开候选不会再被打断；云候选、AI 行、暂扣尾部到达都只是列表内容变了，滚动位置保留。
- 代价：面板开着时若有组字内容不变的换代，会多一次 `reload`（重建全部 chip），频率取决于候选更新，实测只在刚起字时密集；`closeKeyboardPicker()` 的那次高度/alpha 复原反而省了。

## Verification

- 契约脚本 `scripts/test-ios-key-layout-settings.py`、`scripts/test-ios-diagnostic-log-binding.py`、`scripts/test-ios-project-config.py`、`scripts/test-ios-developer-options-lifecycle.py` 通过。
- 本机没有 Xcode，Swift 未在本地编译；靠 `ios-*` 分支的 fork CI 编译并跑 XCTest。
- 真机验证待做：iPhone 13 上连打几字后立刻展开候选，观察面板是否稳定停留；若再出现「收起」并伴随「内存紧张，已收起展开的候选」，则是内存而不是换代。
