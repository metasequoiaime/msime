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
  - `makeChip` 里格子不画底色、描边、阴影，选中改铺一层 `keyForeground` 16% 的圆角底，候选字统一颜色、不加粗、左对齐；
  - 格子高度 46pt、间隔 2pt、内边距横向 10/4，一屏正好六行。
- 面板从候选条那一行铺起：`installPanel(_:keepsKeyTouches:coveringCompositionRow:)` 新增最后这个参数，为真时面板的顶边贴 `compositionContainer.topAnchor` 而不是 `.bottomAnchor`。配上不透明的 `skin.background` 底色，候选条整行被盖住，视觉上就是候选栏原地向下展开。九键三栏面板不走这条（`opensNineKeyPanel` 时 `columns != nil`，样式不生效）。
- 系统样式在右侧加一窄列（44pt）：顶部收起箭头、中部手写、底部删除，`scrollView.trailing` 锚到它的左边。收起必须有——候选条连同它的展开箭头都被盖住了。手写接 `selectInputScheme(.handwriting)`（先关面板），删除接 `handleBackspace()`，两者都是现有实现。
- 滚动条自己画：3pt 圆角条贴右边缘，长度按可见比例、位置按滚动比例。`scrollViewDidScroll` 里点亮，停下 0.9s 后 0.25s 淡出。系统自带的常显指示条在键盘上太扎眼。
- `scrollView.delegate` 只在系统样式下设置（`UICollectionViewDelegate`），卡片样式不接，滚动回调为零成本。

## 不做

- 九键三栏面板不套系统样式：它需要候选条上的拼音栏和读音，铺到顶会把这两行盖掉。
- 顶部不画标题栏（品牌标志、拼写、候选数）。系统的展开没有这一行，用户明确要求去掉；拼写留在候选条上，只是被面板盖住。
- 候选条本身不改。两处风格不统一是已知取舍：改条会波及所有输入状态，不只展开时。

## Testing

- `CandidateTranslationTests.testTheSystemStyleKeepsSixColumnsOnEveryPhoneWidth`：320/375pt 都是六列，卡片样式仍是 4/5 列；带注释的候选在六列下只占一列、卡片样式仍占两列。
- `CandidateTranslationTests.testTheSystemStylePanelDrawsOneAlignedListWithTheSideColumn`：网格止于窄列左边，首行六格同一行且等宽，窄列里能找到收起、手写、删除三个按钮，系统样式不画标题栏。
- `scripts/test-ios-key-layout-settings.py` 等契约脚本不受影响，回归全过。
