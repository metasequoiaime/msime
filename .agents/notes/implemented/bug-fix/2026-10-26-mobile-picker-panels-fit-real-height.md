# Agent Note: 三端功能面板和输入方式面板按实际高度缩放磁贴

Status: implemented

## Problem

一位用户发来录屏：安卓键盘高度调到 90% 左右时，从工具栏品牌键打开的功能面板（四列三行、横向分页）能上下拖动几个像素；调到 100% 以上或 80% 以下都没有。没有对应的 GitHub issue。

根因在 `PagedTileGrid.pickerGrid`（#8199 引入）。它用 `KeyboardHeightPolicy.keyAreaHeightDp` 预测键区高度，再按「放得下行距 12 就三行，放不下先把行距收小，收到 4 还放不下才退成两行」挑行数和行距。磁贴固定 52 dp。面板放在 `moreToolsScroll` 这个 ScrollView 里，网格比视口高时多出来的部分就成了竖向滚动。在 85%–90% 这几档，这条规则把三行算得刚好放满、一点余量都没有，而真实面板比预测矮。Pixel 6 类 AVD（密度 2.625）90% 时的量法：

- 预测键区 218 dp，合 572.25 px。真实键区只有 570 px：四行键高各自取整（123 + 126 + 126 + 123），每行两份半行距也各自取整（9 px 而不是 9.19 px）。
- 面板内容反而向上取整：磁贴 137 px，按 52 dp 算是 136.5；加上内边距、页点和外层底边距，共 574 px。
- 比视口多出 4 px，ScrollView 于是能滚 4 px。

输入方式面板（卡片 56 dp）用的是同一条规则，同样的原因让它在 90% 时溢出 3 px。iOS 不会滚动，因为页高被夹到滚动区高度；但手机竖屏 26 键在最矮键盘、最小行距下只有 174 pt，三行原尺寸要 180 pt，于是各行被不均匀地压扁。鸿蒙的面板高度按逻辑行高精确算出，不会溢出；但默认 197 vp 的键区里，三行之间的行距被收到 6 vp，设计值是 12。

## Decision

三端用同一条规则、同一组常数，并且按面板排好后的真实高度来排，不用预测高度。

规则：

- 手机横屏两行，其余三行。
- 设网格能占的高度为 H（面板高度减去上下留白和页点），磁贴原尺寸行高为 R。三行原尺寸高 3R + 2 × 12，两行原尺寸高 2R + 16。
- 比例 s = min(1, H / 原尺寸)。行高、行距、图标、字形框、文字和勾标都乘 s，点按区域仍是整个格子。
- 三行的 R·s 不到 44（触控目标下限）时改成两行。两行的 s 也不低于 44 / R。
- s = 1 时各端保持原来的排法：安卓和 iOS 行贴着顶部，鸿蒙各行平分多出来的高度。
- 两行缩到下限仍放不下的情况，在三端现有的键区范围里都不会出现。

各端实现：

- **安卓**：`PagedTileGrid.pickerGrid(phoneLandscape, gridDp, rowHeightDp)` 返回 `PickerGrid(rows, scale)`。常数是 `PICKER_THREE_ROW_GAP_DP`、`PICKER_TWO_ROW_GAP_DP`、`PICKER_MIN_TILE_DP`。
  - 网格开 `setPickerFit` 后，在 `onMeasure` 里按量到的高度算排法，行高和行距向下取整到像素，所以排出来不会比给它的高度高。
  - ScrollView 第一遍不限高量子视图，只有子视图比它矮时才按视口高度再量一遍。所以不限高那一遍网格报最矮的排法（两行、磁贴 44 dp），让第二遍拿到真实高度。
  - LinearLayout 的权重在不限高时还会拿 0 高再量一遍，所以测量只记下排法，行数到 `onLayout` 才生效，页点更新 post 到下一帧，避免一次布局里行数来回翻、反复 requestLayout。
  - 功能面板的磁贴按拿到的高度自己算比例。输入方式卡片先按原尺寸量内容，再整体 `setScale` 居中。输入方式面板的网格改为拿去页点以外的全部高度（权重 1），选中项所在的页等排版后再算（`revealOnLayout`）。
- **iOS**：`KeyboardPagedGridView.pickerLayout(preferredRows:height:rowHeight:)` 加 `pickerMinTileHeight = 44`。选择面板用 `init(columns:pickerRows:rowHeight:columnGap:sidePadding:)` 建网格，在 `layoutSubviews` 里按滚动区的真实高度排：行数变了就重新分页，并停在原来那页第一个图块所在的页；只是比例变了就原地改行高约束。`KeyboardFunctionTileView` 和输入方式卡片按格子高度自己缩放。皮肤面板仍是固定行数。
- **鸿蒙**：`FunctionPanelPolicy.pickerGrid(phoneLandscape, gridVp, rowHeightVp)` 返回 `{ rows, scale, rowHeight, rowsGap }`，常数是 `THREE_ROW_GAP_VP`、`TWO_ROW_GAP_VP`、`MIN_TILE_VP`。`PanelTile` 用 `contentScale` 显式缩放各个尺寸。鸿蒙的面板高度本来就精确，不需要测量回调。

各档结果（默认行距）：

- 安卓：100% 及以上原尺寸，80%–95% 是缩小的三行，75% 两行。
- 鸿蒙：默认 197 vp 是缩到约 0.94 的三行，最矮 176 vp 是原尺寸两行。
- iOS：只有最矮键盘加最小行距的手机竖屏 26 键、以及手机横屏的少数组合会缩小，其余原尺寸。

## Historical audit

检索 `pickerGrid`、`PagedTileGrid`、`功能面板`、`keyAreaHeightDp`。#8199 引入每页三行和收行距规则时没有写笔记，这篇接手它的行数和行距决定。

- [功能菜单顺序](2026-10-13-android-function-menu-order.md)：持有条目顺序和「四列三行、最多三页」，部分重叠。这次改的是三行放不下时怎么排，不改顺序。
- [安卓分页网格接手位移](2026-10-19-android-paged-grid-intercept-delta.md)、[iOS 分页网格反向滑动](2026-10-18-ios-paged-grid-reverse-swipe.md)、[iOS 分页网格系统分页](2026-10-23-ios-paged-grid-system-paging.md)：持有横向翻页手势，与这里的竖向排法无关。这次也没有改它们的手势路径。

没有完全吸收或过时的提案。

## Alternatives considered

- **只把预测高度算准（按像素复刻每行取整），或给预测留几像素余量**：改动最小，三端的规则形状也不变。但预测要跟着键高分摊、行距、底部留白、数字行、手写多一行、外层内边距这些因素一起变，任何一处新增取整都会让它重新失准，这次的缺陷正是这样来的；余量留多少也没有依据。用量到的高度就不存在「预测」这一层。
- **保留「先收行距、再退两行」，只换成真实高度**：能消除溢出，改动也比缩放小。但行距被压到 4–6 dp 时磁贴挤成一团（鸿蒙默认高度下一直是 6 vp），三行和两行之间的跳变点也偏高。用户提的、微信输入法的做法是整体缩小，行距和磁贴保持比例，看起来仍是同一块面板，所以采用缩放。
- **去掉外层 ScrollView，让面板放不下就裁掉**：没有拖动问题。但「更多」面板的子页（本地输入、AI 回复与润色、世界时钟）需要竖向滚动，而且裁掉磁贴就意味着有入口点不到。
- **最小磁贴取 40 dp**：安卓 75% 也能保持三行。但 40 dp 低于 iOS 44 pt 和仓库里其他「44 dp 点按目标」的约定，文字缩到 9.6 sp 也偏小。44 是三端都站得住的下限。

## Consequences

- **收益**：两个面板在任何键盘高度下都不会竖向滚动，从 75% 到 110% 逐档由设备检查守着；三端从同一个高度得到同样的行数和比例。
- **收益**：iOS 不再被不均匀地压扁，鸿蒙默认高度下的行距回到按比例的 11 vp 左右。
- **代价**：鸿蒙默认高度下的磁贴比原来小约 6%（原来是原尺寸磁贴加 6 vp 行距）。安卓 80%–95%、iOS 少数矮键盘组合的磁贴同样会变小。
- **代价**：安卓的排法藏在测量和布局里，比原来在渲染时算好一个值多了一层间接，依赖 ScrollView「先不限高、再按视口量」和 LinearLayout 权重测量的行为。这两者变化时，`KeyboardHeightDeviceSmoke` 的 `panel-fit` 检查会失败。
- **代价**：`KeyboardHeightPolicy.keyAreaHeightDp` 不再被产品代码调用，留给 JVM 测试推算各档的大致高度。
- **重访信号**：调整面板的留白或页点、给面板加标题栏或其他固定行、改动最小点按目标约定时，三端的常数要一起改；下一个改这里的人可以从这篇往回查。

## Verification

- 安卓 JVM：`ms_w2_kb_ViewLogicSmoke` 覆盖八组共享用例、60–260 dp 每 0.25 dp 一档（排出来不高于给的高度、磁贴不矮于 44、变矮不加行），以及按预测键区算的各档结果。
- 安卓设备：`KeyboardHeightDeviceSmoke` 的 `panel-fit` 检查（`am instrument -e panel-fit true`）逐档写入 75/80/85/90/95/100/110% 的高度，重启输入法后打开两个面板，断言外层 ScrollView 不可滚动、在网格侧边距里竖拖时磁贴不移位、磁贴不矮于 44 dp。完整运行该套件时这个检查也在里面。
- iOS：`MorePanelFoldTests` 覆盖共享用例、按公式算出的真实键区高度、磁贴缩放和重新分页。
- 鸿蒙：`tests/keyboard-logic.test.ts` 覆盖共享用例、默认、最矮和最高键区，并遍历全部高度调整和行距组合。
