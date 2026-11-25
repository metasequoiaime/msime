# Agent Note: iOS 展开候选的「系统样式」

Status: implemented

## Problem

iOS 的展开候选只有一个样子：64pt 起自动分列的按键式卡片网格，装在整个键区上。候选条留在它上方，里面那九个候选与网格第一行完全相同，同一屏出现两套「第一页」，各自分页、各自高亮，用户得分清点哪个；`yi` 这种几百个候选的查询里，第一行还白占一格高度。

系统的拼音展开是另一种做法：点候选栏的展开箭头之后，候选栏原地向下展开成一张六列的纯文字列表，第一行仍然是从前那几个候选，只是下面接上了其余的全部。右侧一窄列放滚动条、手写和删除。用户要的是在保留现有卡片样式的前提下，再增加这样一套样式，设置里切换。

## Decision

- 新增 `CandidatePanelStylePreference`（`SharedUI/candidate/CandidatePageSizePreference.swift` 同文件，避免为一个枚举改工程文件），`candidate.panelStyle` 存 App Group，取值 `card` / `system`，缺省 `card`。不写共享文档：面板样子是 iOS 自己的观感，其他平台各有一套。
- 设置入口在「候选栏」组末尾的「展开候选样式」，文案「卡片」「系统样式」。改用 `DesignSelectRow`，切换立即生效，键盘下次打开面板时读取。
- `KeyboardCandidatePanelView` 接受 `style`。系统样式的差异集中在四处：
  - `columns(width:style:)` 固定返回六列，不再按 64pt 最小列宽自动分列；
  - `span(...)` 里注释不再把格子撑成两列（六列下每列才五十来点，撑成两列一页只剩三格，与系统的一页六格差太远），注释跟着格子截断；
  - `makeChip` 里格子不画底色、描边、阴影，选中格照候选条首选词块画：同一个底色（`palette.hover` / `skin.keyBackground`）、同一个圆角（`candidateChipRadius` 9pt）、同样的字色（`selectedText` / `candidateAccent`）和半粗体，展开前后高亮不换样子；
  - 格子高度 46pt、间隔 2pt、内边距横向 10/4，一屏正好六行。
- 面板从候选条那一行铺起：`installPanel(_:keepsKeyTouches:coveringCompositionRow:)` 新增最后这个参数，为真时面板的顶边贴 `compositionContainer.topAnchor` 而不是 `.bottomAnchor`。九键三栏面板不走这条（`opensNineKeyPanel` 时 `columns != nil`，样式不生效）。
- 被盖住的候选条那一行要清空，面板自己不铺底色（`panelCoversCompositionRow`）。面板铺一块 `skin.background` 是不行的：原生皮肤下键盘底色几乎透明（`nativeBackgroundFillAlpha`，让系统底板透出来），再盖一层就比周围深了一档，真机上能一眼看出展开前后不是同一个背景。所以改成用 `alpha = 0`（不用 `isHidden`，行高留着，面板顶边和下面几行都不动）清空 `candidateContent` 与 `readingRow`，面板留 `.clear`，透出的就是那一行原来的底色；`renderCandidateStrip` 按 `panelCoversCompositionRow && candidatePanel != nil` 复核一次，键盘收起、换面板时不会卡在清空状态。
- 系统样式在右侧加一窄列（44pt）：顶部收起箭头、中部手写、底部删除，`scrollView.trailing` 锚到它的左边。收起必须有——候选条连同它的展开箭头都被盖住了。手写接 `selectInputScheme(.handwriting)`（先关面板），删除接 `handleBackspace()`，两者都是现有实现。图标用键盘自己的 `KeyboardIcon.handwriting` / `.backspace`（与工具栏、回车键同一套），不换成 SF Symbols——`pencil.tip`、`delete.left` 跟键盘上那个手写、退格长得不一样。
- 右列的删除只换一代候选，面板不收起：`handleBackspace()` 外面套上 `candidatePanelFollowsSelection`，与面板里选候选同一条路（组字退空之后照样由 `refreshCandidatePanelAnnotations` 收起）。
- 卡片样式的格子底色可以调：`CandidatePanelChipOpacityPreference`（`candidate.panelChipOpacity`，0–100，缺省 100，即原来那套），设置页在「展开候选样式」下面多一条「候选格子不透明度」，只在卡片样式时出现。`makeChip` 按它给 `skin.keyBackground` 乘 alpha；选中那一格不跟着淡（否则看不出首选），调低之后也不再画按键阴影（半透明按键配实心影子很脏）。系统样式本来没有格子底，不受影响。
- 滚动条用系统自带的 `UICollectionView` 竖直指示条：本来就在滚动时出现、停下淡出，符合「不常显的细线」。最初自绘了一条 3pt 的，和系统那条同时出现，同一屏两根线；自绘的还落在窄列按钮上，删掉。
- 卡片样式后来也改成原地展开（用户要求两种样式统一）：铺排不再按样式推断，改由显式参数 `expandsInPlace` 决定（`inlineLayout = expandsInPlace && columns == nil`），键盘控制器在 `columns == nil` 时传 true。右列窄列、网格止于窄列、候选条那一行清空都是两种样式共用的；差别只剩格子怎么画（卡片：64pt 起自动分列、44pt 行高、6pt 间隔、按键式的底色与阴影、居中；系统：六列、46pt、2pt、纯文字、左对齐）。控制器的 `coversCompositionRow` 同样变成 `columns == nil`，于是卡片样式展开不再出现「上面一条候选栏 + 下面一块网格 + 两个收起箭头」。
- 用显式参数而不是继续按 `style` 推断，是为了不动其它调用方（标题栏面板、九键三栏、二十来个测试构造点）：不传就是原来的布局。
- 右列的删除键接上底部 ⌫ 的整套滑动手势：控制器在 `showCandidatePanel` 里建一个名字为 `backspaceSwipe` 的 pan 交给面板，面板挂到删除键上。于是横滑逐格删、上滑越过待命线松手清空、往右滑恢复都跟 ⌫ 一样，起手方向与能不能滑仍由 `gestureRecognizerShouldBegin` 和 `handleBackspaceSwipe` 原来那套判断，面板只提供按键。滑动期间 `candidatePanelFollowsSelection` 为真，候选换代时面板原地重建而不是收起（`handlePanelDeleteSwipe`）。
- 候选栏可下滑展开：`candidateStrip` 上挂一个 pan（`candidateStripSwipeDown`），向下拖过 18pt 就调 `showCandidatePanel()`，等于点一下展开箭头。起手方向在 `gestureRecognizerShouldBegin` 里判（只接向下、纵向为主），与候选列表的横向滚动同时识别；`cancelsTouchesInView` 让手指落在某个候选上往下拖时不会同时把那个候选上屏。一次手势只展开一次。
- 原地展开的卡片网格列宽下限放宽到 58pt（`inlineMinimumCellWidth`）：右边让出 44pt 的窄列之后，仍按 64pt 一排就只剩四列，一屏空得厉害（真机反馈）。375/390pt 恢复五列、430pt 六列、SE 的 320pt 仍是四列（五列每格五十来点，放不下两个字）。非原地展开的面板（标题栏、九键）不受影响。

## Alternatives considered

- 另起一套卡片面板组件，两种样式各一份：网格布局、测量、选中态、长按释义、无障碍标签会整份复制一遍，以后改候选逻辑要同步两处，上游跟进成本翻倍。现有实现本来就是一个类里按样式分支画格子，改成「布局共用、只分支格子画法」更省。
- 只给系统样式改原地展开，卡片样式保留「候选条在上、网格在下」：改动最小，也不动现有用户的手感，但用户明确要求两种样式统一，展开时上面不再多出一条重复的候选栏。代价是本 PR 改变了卡片样式用户的展开手感，这一条在 PR 正文单列一节请维护者确认。
- 继续按 `style` 推断布局（`style == .system` 时才原地展开）：改卡片样式时要把这个判断再翻一次，还会波及带标题栏的面板、九键三栏这类不传样式的调用方；改用显式 `expandsInPlace`，不传就是原来的布局，二十来个测试构造点一个都不用动。
- 被盖住的候选条那一行可以铺一块不透明背景把它遮掉，而不是清空内容：原生皮肤下键盘底色几乎透明，再盖一层就比周围深一档，真机上能一眼看出展开前后不是同一个底。见上面 `panelCoversCompositionRow` 那一条。

## 不做

- 九键三栏面板不套系统样式：它需要候选条上的拼音栏和读音，铺到顶会把这两行盖掉。
- 顶部不画标题栏（品牌标志、拼写、候选数）。系统的展开没有这一行，用户明确要求去掉；拼写留在候选条上，只是被面板盖住。
- 候选条本身不改。两处风格不统一是已知取舍：改条会波及所有输入状态，不只展开时。

## Consequences

- 卡片样式的展开从「候选条在上、网格在下」变成候选栏原地向下展开：上面那条候选栏（含拼写）不再保留，收起只能点面板右列的箭头。这是本 PR 唯一改变现有用户默认手感的地方，PR 正文单列一节请维护者确认；要退回只需控制器里让 `expandsInPlace` 多认一个 `panelStyle == .system`，其余不动。
- 新增两个 App Group 键：`candidate.panelStyle`（缺省 `card`）、`candidate.panelChipOpacity`（缺省 100，即原来那套）。缺字段都按原样读，老用户升级后看不出变化，设置页只多一行「展开候选样式」。
- 展开面板多出右侧 44pt 窄列：原地展开的卡片网格可用宽度少一条，靠 58pt 的列宽下限拿回五列；SE 的 320pt 仍是四列，430pt 六列。
- 两条上游用例（`testCandidateChipsAndTheExpandChevron`、`testQuanpinKeepsTheFullCandidatePanel`）的布局断言随新规格改写。它们守的不是某个 bug，而是旧布局本身（网格在候选条下沿展开、面板里没有收起键）；改写的理由写在这里与 PR 正文，不是为了让改动通过去关掉回归。
- 九键三栏面板、带标题栏的面板（`showsHeader: true`）行为不变：不传 `expandsInPlace`，顶边仍贴候选条下沿。
- 面板里的删除仍没有长按连删，只有点按与滑动；需要时再补一条 `BackspaceRepeatTiming` 的转发。

## Testing

- `CandidateTranslationTests.testTheSystemStyleKeepsSixColumnsOnEveryPhoneWidth`：320/375pt 都是六列，卡片样式仍是 4/5 列；带注释的候选在六列下只占一列、卡片样式仍占两列。
- `CandidateTranslationTests.testTheSystemStylePanelDrawsOneAlignedListWithTheSideColumn`：网格止于窄列左边，首行六格同一行且等宽，窄列里能找到收起、手写、删除三个按钮，系统样式不画标题栏。
- `CandidateTranslationTests.testTheCardStyleSharesTheInPlaceLayoutAndKeepsItsOwnChips`：卡片样式展开后同样有窄列、窄列里三个按钮、网格止于窄列左边；375pt 上首行五格同一行、第六个候选换到第二行；卡片格子的底色与圆角跟不原地展开时一致（格子画法没被布局改动带掉），系统样式的格子仍是透明纯文字；没传 `expandsInPlace` 的面板不会凭空多出窄列。
- `CandidateTranslationTests.testTheSystemStyleKeepsSixColumnsOnEveryPhoneWidth` 里也钉住了原地展开卡片的列数：窄列让出之后的 276/331/346/386pt 分别是 4/5/5/6 列。
- `CandidateTranslationTests.testThePanelDeleteCarriesTheBackspaceSwipeGesture`：两种样式的删除键都挂着控制器交进来的那个 pan（`backspaceSwipe`），无障碍提示写明横滑删、上滑清空。
- `NineKeyKeyboardTests.testCandidateChipsAndTheExpandChevron` / `testQuanpinKeepsTheFullCandidatePanel`：这两条上游用例原来断言「网格从候选条下沿展开」。原地展开之后面板顶边就是候选条顶边，候选条那一行的内容清空（alpha 0，行高不变），收起和删除由面板右列接手，所以断言随之改成新的布局，而不是把旧布局放回来。
- `scripts/test-ios-key-layout-settings.py` 等契约脚本不受影响，回归全过。
- 真机第一轮验收提出的三处（同一屏两根滚动条、高亮格与候选条不一致、面板底色比周围深一档）按上面的做法改掉；样式缺省在验收期间临时设为 `system`（`fallbackStyle`），验完改回 `card`。
