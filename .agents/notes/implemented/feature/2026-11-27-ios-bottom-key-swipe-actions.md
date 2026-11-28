# Agent Note: iOS 底行按键上滑、下滑与「上屏候选译文」

Status: implemented

## Problem

字母键有了上下各一个动作之后，底行还是一片空白：空格只有轻点、左右拖光标、长按语音，中/英、符、表情、回车、常用标点和句号键上下滑什么也不做。用户要的是两件事：

- 空格上滑能直接把候选下的译文上屏，不必长按候选再点释义卡片里的「上屏译文」。
- 底行其余按键也能各配上下滑动作，现在没有功能的默认「无」，不改变现有手感。

## Decision

- `LetterSwipeAction` 新增 `commitTranslation`（标题「上屏候选译文」，短名「译文」）。执行在 `commitHighlightedCandidateTranslation`：先结束组字，再上屏**当前高亮候选**的第一目标语言第一条义项。取值与释义卡片完全相同（`CandidateGlossCardContent.primaryTranslation`，离线释义按 `;`/`；`/换行拆条），因此与卡片上的「上屏译文」不会给出不同答案。
- 没有组字、没有候选、没有译文时**什么也不做**：这个动作只写译文，不拿候选原文顶替（点候选和点空格本来就能上屏原文）。译文开关（`CandidateGlossPreference`）、候选来源（`CandidateTranslationStore.sourceGetsNoGloss`）与「仅当前高亮」都沿用候选条那套判定，只有真正画着译文的候选才取得到。
- 新增 `BottomKeySwipeKey`（空格、中/英、符、表情、回车、常用标点、句号）与 `BottomKeySwipePreference`，配置存本机 App Group 的 `keyboard.gesture.bottomKeySwipeBindings`，不写共享文档、不随账号同步——与字母键的逐键动作同一条理由：手感跟着设备走。
- 出厂默认只有 `space.up = commitTranslation`，其余键、其余方向都是「无」。显式选「无」会存一条 `action=none` 压住默认（与字母键「无」压住角标同理），`clear(key:)` 删掉这一条后空格上滑又回到「译文」。
- 不在这张表里的三个键：123 / ABC（上滑已经打开 `#+=`）、⌫（上滑清空、左滑逐格删）、地球（点击交给系统的键盘切换，`allTouchEvents` 上不能加跟踪）。底行之外的同类键一并接上：九键左列的「符」、九键右侧大回车、iPad 第二排的回车。
- 手势复用 `KeyboardKeyButton.swipeActions` / `onSwipeAction`，与字母键同一套 `LetterSwipePolicy`（14pt 阈值、先过的方向锁住这次触摸），并要求**纵向占优**：空格左右滑是移光标、常用标点和句号键上横滑也会移光标，斜着滑不该被当成上下滑。空格自己的拖动 pan 本来就只在横向占优时开始（`gestureRecognizerShouldBegin`），所以竖直滑动仍归按键判定，两者不抢。
- 执行路径拆成三层：`performLetterSwipeAction` 只处理角标与自定义文本，`performBottomSwipeAction` 处理底行自有语义（没有角标，「角标符号」当作不接），其余动作统一走 `performSwipeAction`。底行键**不受**「滑动输入符号」总开关约束：那个开关是字母键角标的总闸，底行本来就没有角标。
- 设置页「键盘 → 手势」下面新增「底行按键滑动」组：每个键一行，摘要写「上滑 X · 下滑 Y」，点开是 `BottomKeySwipeEditorSheet`，交互与字母键的编辑页相同（每次选择立即写 App Group、可填自定义文本、可单键恢复默认），但选项里没有「角标符号」。
- 「重置所有设置」的本地清单补上 `LetterSwipePreference.key` 与 `BottomKeySwipePreference.key`：这两张表原先都没被重置，属于既有缺口。

## Alternatives considered

- **底行也走全局「滑动输入符号」总开关**：实现省一行判断，但用户关掉字母角标滑动时会把空格上滑的译文一起关掉，两件事没有关系。
- **给底行单独做一个 `UISwipeGestureRecognizer`**：与字母键的判定、预览气泡、方向互斥都要重写一套；而且空格上已经有 pan 和长按，再挂识别器要额外仲裁。
- **空格上滑没有译文时退回上屏候选**：能保证「滑了总有反应」，但用户滑的是「译文」，上屏原文会和点空格混淆，事后难解释。
- **把译文动作只留给空格**：设置里就会出现「空格能配、别的键不能配」的特例；预览和编辑页还要分叉。
- **底行配置写进共享文档随账号同步**：与字母键的逐键动作不一致，也要新增跨端字段；其他客户端读到未知动作会校验失败。
- **给几个键预置出厂下滑动作**：等于替所有用户打开这些手势，与「不改变现有手感」的目标冲突；动作仍由用户在设置页逐键配置。

## Consequences

- 空格上滑默认可用，下滑默认不接；其余底行键默认不接任何方向，升级后手感不变。
- 底行滑动的预览气泡显示动作短名（「译文」「粘贴」「123」），没有角标符号可显示；底行没有「下滑动作提示」那行字，只有字母键有。
- 译文上屏依赖候选条已经拿到译文：在线翻译需要完全访问与服务配置，离线释义需要英文释义开关；都没有时不动作，释义卡片里同样是「暂无译文」。
- 「仅当前高亮」打开时只取高亮的那一个（本来就只取高亮候选）；展开网格里的候选不在候选条上，空格上滑取不到它，仍走长按卡片。
- 底行键的滑动不写键面提示，用户只能从设置页的摘要看到；如果以后要提示，需要给这些键安排底部那一行的位置（现在那行只属于字母键）。
- 预览图徽标只标用户存过的键，没有配置的键不显示——徽标语义是「用户改过的键」。

## Testing

`KeyboardTests/input/LetterCornerHintTests.swift` 新增两条：

- `testBottomKeySwipeDefaultsAndStoredBindings`：默认只有空格上滑是译文、其余键与方向是「无」；显式「无」压住默认；选了自定义文本却没填不算数；`clear(key:)` 后空格上滑回到译文。
- `testSpaceKeyCarriesTheTranslationSwipe`：控制器把空格键的 `swipeActions` 接到「译文」、下滑为空，「滑动输入符号」关着时底行照常；存「无」之后上滑不再接。

`testLetterSwipeShortTitleAbbreviatesForBadges` 补了 `commitTranslation` 的完整名与短名。

本机是 Windows，没有 Xcode / Swift 工具链：Swift 单测**没有编译、没有运行**，只做了静态核对。跑通的是 `scripts/test-ios-key-layout-settings.py`（32 项）、`test-ios-settings-group-header-linkage`、`test-ios-settings-save-failure`、`test-ios-account-settings-lifecycle`、`test-preference-suite-cleanup`、`test-android-preference-keys`、`test-preferences-intersection-parity` 与 `git diff --check`。需要在 macOS 上跑 `MSIMEKeyboardTests`，并在真机验收：组字时上滑空格上屏译文（中文词与英文词各一次）、没有译文时不动作、空格左右拖光标仍正常、底行各键上下滑不误触横滑移光标。

- 设置页预览图里的底行也可以点了（`KeyboardSkinPreview.onBottomKeyTap`）：空格、中/英、符、表情、回车、逗号、句号分别打开 `BottomKeySwipeEditorSheet`，与点字母键进逐键编辑同一条路；123、地球和 0 不是可配的底行键，点了不响应。原先只有列表里的入口，验收时「点预览图的空格没反应」被当成缺陷。

## Related notes

- [iOS 26 键上滑、下滑各一个可配动作](2026-11-23-ios-letter-two-way-swipe-actions.md)：承接其 `LetterSwipeAction`、`LetterSwipeBinding` 与 `KeyboardKeyButton` 的双向判定；本次把动作表扩到底行按键，并把动作执行拆成共用的 `performSwipeAction`。
- [iOS 符号页输入后返回字母键盘](2026-11-27-ios-symbol-return-to-letters.md)：同一批设置页改动，互不影响。
- [候选释义卡片](2026-11-05-ios-candidate-gloss-card.md)：译文取值与「上屏译文」按钮同源，本次只是多了一条一步可达的手势入口。
