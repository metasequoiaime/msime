# Agent Note: macOS 候选窗向读屏报告并朗读高亮的候选

Status: implemented

## Problem

macOS 候选窗的每一行是 `MSIMECandidateButton`（`platforms/macos/src/candidate/CandidateChrome.h`），`InputController.mm` 给它设了「序号  候选」形式的 accessibilityLabel，但高亮的那一行和别的行在无障碍树里一模一样：没有 AXSelected，高亮移动时也不发任何通知。候选窗是从不成为 key window 的非激活面板，VoiceOver 的焦点一直留在客户程序里，不会自己去读它。结果是读屏用户打字、翻页、用方向键移动高亮时听不到当前是哪个候选，只能盲按数字键。AI 座位上的占位行在读屏里是「3  …」或「3  —」，念出来是标点。

## Decision

**高亮的行报告自己被选中。** `MSIMECandidateButton` 覆写 `isAccessibilitySelected`，返回 `tag >= 0 && candidateHighlighted`：普通候选窗的候选行和展开网格里的格子（标签都不小于 0）按自己的高亮回答，首行的翻页、展开按钮（标签 -1、-2、-3）恒为 NO。按钮的无障碍元素是它的 cell，但 AppKit 读 AXSelected 时会问到按钮上的这个覆写：用 `AXUIElementCopyAttributeValue` 从外部读，高亮行 AXSelected 为 1，其余为 0，没有覆写的普通 NSButton 不支持这个属性。值直接由 `candidateHighlighted` 推出，不在各个设高亮的地方另设一份，所以普通页、网格和原地重画都不会让两者对不上。

**每画一页就请求朗读高亮的那一行。** `renderCandidates` 整页重建之后、`renderExpandedCandidates` 画完网格之后调用 `announceHighlightedCandidate:`：找到报告被选中的那个按钮，把它的 accessibilityLabel 用 `NSAccessibilityAnnouncementRequestedNotification` 交给读屏，元素是候选面板本身（与 `InputModeHUDPanel` 朗读中英切换的做法相同）。朗读请求不移动任何焦点，客户程序保持 key。和上一次读的是同一段话时不再读（`_announcedCandidate`），比如多打一个字母而首选没变；面板在这次显示之前不在屏幕上时先清掉记录，候选窗每次出现都读一次。只换释义的原地重画要求高亮不变，所以不调用它。

**占位行读它的意思。** AI 座位上的占位行（Engine 候选来源 12）的标签是「序号  AI 候选等待中」（文字为「…」时）或「序号  AI 候选没有结果」，说法与 iOS `CandidateMarker.swift`、Android `AiPendingCandidatePolicy` 一致；它高亮时同样报告被选中、同样朗读。

## Alternatives considered

- **把候选区做成 AXList，用 AXSelectedChildren 和 `NSAccessibilitySelectedChildrenChangedNotification`。** 最强的理由是这是 AppKit 表达「一组里选中了哪一个」的标准模型，读屏用户进到候选窗里浏览时会听到「已选择」和位置。没有采用：读屏只对它正在看的那个元素的选中变化作出反应，而它的焦点在客户程序里，从不进候选面板，通知发出去没有人念；面板的内容视图里还混着预编辑、页码、翻页按钮和 logo，要当列表就得重组视图层级。AXSelected 已经让进到候选窗里浏览的人能分辨高亮行，主动朗读才是打字时真正听得到的那一半。
- **在每个设 `candidateHighlighted` 的地方同时设 `button.accessibilitySelected`。** 同样能让 AXSelected 生效（实测普通 NSButton 设了它也会报告），但设高亮的地方有两处（`candidateButtonFor:` 和网格循环），原地重画路径还沿用旧按钮，任何一处漏设读屏就和画面对不上；覆写读取方法只有一个来源。
- **每次渲染都朗读，不去重。** 实现最简单，高亮哪怕没动也会再念一遍。没有采用：打拼音时每个字母都会重画，首选常常不变，同一个词会被连念好几遍，盖过读屏对按键本身的回显。
- **让候选面板或候选行成为读屏焦点。** 读屏会自动念出焦点所在，不需要朗读请求。没有采用：那就是把焦点从客户程序抢走，输入法正是要避免这件事。

## Consequences

- **收益**：VoiceOver 用户打字、翻页、方向键移动高亮和在展开网格里移动时，都能听到当前高亮的候选（带序号），AI 占位行念成文字；进到候选窗里浏览时能分辨哪一行被选中。
- **代价与已知上限**：朗读按读屏自己的排队规则播放，快速连按方向键时可能排起几条；朗读请求没有设优先级，与 `InputModeHUDPanel` 一致。去重只看标签文字：换页后同一序号上恰好是同一个词时不会再念。翻页、展开按钮也带上了恒为 0 的 AXSelected。真机 VoiceOver 的实际朗读效果没有在自动化里覆盖，改这一块后要开着 VoiceOver 打几个字确认。

## Verification

- `platforms/macos/tests/candidate/CandidateRowLayoutTest.mm`（ctest `candidate-row-layout`）经控制器渲染真实页面：高亮在第 1、2、3 行时只有那一行 `isAccessibilitySelected`，翻页按钮不报告；`announcedCandidate` 依次是「1  测试」「2  侧室」「3  AI 候选等待中」「3  AI 候选没有结果」。去掉 `MSIMECandidateButton` 的覆写，这个测试在第一条选中断言上失败。
- `platforms/macos/tests/input/ShortcutTest.mm` 的 `TestExpandedCandidateGridGeneration`：网格里按右方向键后光标那一格报告被选中、刚离开的那一格不报告，朗读的是新那一格的标签。
