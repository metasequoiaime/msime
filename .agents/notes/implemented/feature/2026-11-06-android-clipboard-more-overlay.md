# Agent Note: Android 剪贴板「更多」菜单改为浮在列表上面

Status: implemented

## Problem

#10969：Android 键盘剪贴板面板点顶行「更多 ▾」后，分类、导出、清空这一列菜单是作为列表第一行插进 `clipboardPanel` 的（[「更多」菜单](2026-10-29-android-clipboard-more-menu.md)，#10037）。它贴右对齐、只有约 100dp 宽，左边留下一整块空白；下面的卡片被整体往下推约 140dp；滚到中间时打开还会先跳回列表顶部。报告者（#10037 的同一位）希望它像系统的溢出菜单那样浮在列表上面、是一个单独的小窗口，不改变原来的布局。#10037 的笔记当时就把「跳回顶部、卡片下移」记成代价，写着用户反馈烦再考虑浮层方案，这次就是那条反馈。

iOS 和鸿蒙的剪贴板没有这个菜单（#10177 只改了 Android，两端代码里没有「更多剪贴板操作」），所以只改 Android。

## Decision

- `MSIMEInputService` 建剪贴板面板时，在 `clipboardSurface`（FrameLayout）里最后加一层 `clipboardMoreOverlay`：同样从顶行下沿（topMargin 32dp，与列表 `ScrollView` 一致）铺到底，背景透明，默认隐藏，elevation 8dp。最后加入加上抬高，保证它在「返回顶部」「撤销」两个浮动按钮之上：ViewGroup 按 Z 排序决定绘制和触摸分发顺序，按钮有自己的 elevation 时只靠加入顺序不够。
- `ImePanels.renderClipboardMoreMenu` 把菜单加进浮层，`FrameLayout.LayoutParams` 取 `WRAP_CONTENT` 和 `Gravity.TOP | Gravity.END`，topMargin 2dp、marginEnd 8dp（和顶行的左右内边距一样），菜单右缘对齐「更多」右缘，紧贴在它下方，然后显示浮层。`renderClipboardHistory` 开头总是先清空并隐藏浮层，所以详情、分词、云端、确认清空、多选、分类选择器这些不画菜单的分支都不会留着旧菜单。
- 浮层可点：点菜单以外的地方调 `ImePanels.closeClipboardMoreMenu`，只收起菜单、重画面板，和系统溢出菜单点外面就关一样。浮层开着时接住列表区的全部触摸，点不到下面的卡片（不会误上屏、误打开长按操作行），列表也滚不动。浮层的无障碍说明是「收起剪贴板菜单」，读屏用户也能直接用它收起。
- 顶行不被浮层盖住：「更多 ▴」再点一下照样收起，「本机 ▾」「多选」照样可用，它们本来就会先收起菜单（`clipboardMoreOpen = false`）；「搜索」去应用里搜索，重开面板时菜单已收起。
- 删掉两处只为「菜单是列表一行」补的代码：展开时的 `s.clipboardScroll.scrollTo(0, 0)`（浮层不随列表滚动，滚到哪里打开都看得见，列表停在原处），以及 `clipboardScrollYWithoutMoreMenu`（菜单不占列表高度，`openClipboardDetail` 直接记 `getScrollY()`）。
- 菜单外框仍由 `ImeStyler.applySkin` 在换肤遍历之后调 `styleClipboardMoreMenu` 补画；阴影从 2dp 加到 6dp，菜单现在叠在卡片上面，要看得出是单独的一层。浮层本身没有背景，换肤遍历只给按钮和文字上色，不会给它涂底色。
- 仍然不弹 `PopupWindow`、`PopupMenu` 或对话框：浮层只是同一窗口里的 FrameLayout，`check-host.sh` 的无对话框守卫照旧覆盖 `renderClipboardMoreMenu`。

## Alternatives considered

- **浮层只盖菜单本身，菜单外的触摸透传给列表**：实现最少，不用管拦截。但菜单开着时点一张卡片会直接上屏、长按会打开操作行，而菜单还开着；滚动列表时菜单浮在一张张卡片上不动，看着像残影。系统溢出菜单点外面是先关菜单、不触发下面的东西，这里照做。
- **浮层盖住整个面板，包括顶行**：最像系统模态菜单，点哪里都只是收起。但顶行的「更多 ▴」本身就是开关，被盖住后读屏用户在它上面双击得到的是浮层的动作，选中态和「已展开」的状态说明也读不到；「多选」「本机 ▾」本来就会收起菜单，让它们照常可用代价更小。
- **把菜单挂进固定顶栏 `clipboardHeader`**：#10037 笔记里当时考虑过的形式。顶栏是 `LinearLayout` 竖排，菜单挂进去要么撑高顶栏（列表 topMargin 固定 32dp，会被顶栏盖住一截），要么另做负边距；固定顶栏的设备验收也要区分两种高度。`clipboardSurface` 本来就是 FrameLayout，「返回顶部」「撤销」已经用同样的方式浮在上面，多一层浮层更直接。
- **用 `PopupWindow`（`INPUT_METHOD_NOT_NEEDED`）做成真正的独立窗口**：观感最接近报告附图。但它是另一个窗口，键盘收起、旋转、面板重画时要自己关，且正是无对话框守卫防的那一类（#5605、#5653）；同一窗口里的浮层已经能做到不挤动布局。

## Consequences

- **收益**：打开菜单时列表的滚动位置和卡片位置都不变，左边不再留空白；滚到中间也能直接打开，不跳回顶部；进入详情记下的滚动位置不用再按菜单高度折算。
- **代价**：菜单开着时列表区的第一下点按只用来收起菜单，想直接点某张卡片要点两下；「返回顶部」「撤销」被浮层盖住时，点它们的位置也只是收起菜单，撤销倒数照走。
- 菜单不再随列表滚动，必须整个放得进浮层：三项各 36dp、间距 4dp、上下内边距各 4dp，加上 2dp 上边距共 126dp；键区最矮是 `KeyboardHeightPolicy.keyAreaHeightDp` 在调整量 -46dp 时的约 162dp，减去 32dp 顶行还剩约 130dp，放得下，所以菜单外不再套一层滚动。以后菜单再加项，或键区下限再调低，要重新算这笔账。
- 依赖菜单的设备验收（`ClipboardCategoriesDeviceSmoke`、`ClipboardExportDeviceSmoke`、`ClipboardSegmentDragDeviceSmoke`）按无障碍说明找节点、走 `ACTION_CLICK`，不做坐标命中，不受浮层影响。

## Verification

`ToolbarEdgesDeviceSmoke.checkClipboardMoreMenu` 在 360dp × 300dp 下先把列表滚到 60dp，再 `performClick`「更多」：断言滚动位置不变、第一条记录的 ⓘ 在面板上的位置不变；菜单不在 `clipboardPanel`、不在列表 `ScrollView` 里，父视图就是 `clipboardMoreOverlay`，宽度不到键盘一半；浮层可见、可点，从顶行下沿铺到面板底边，是 `clipboardSurface` 的最后一个子视图，Z 高于「返回顶部」和「撤销」。点「清空」后菜单和浮层都收起；重新打开后 `performClick` 浮层，`clipboardMoreOpen` 变为 false、浮层隐藏、菜单项消失。`checkClipboardMoreDetailReturn` 改为在整个 surface 里找菜单，继续守住菜单开着时进详情再返回、那一条不错位。原有的菜单项样式、换肤补画外框的断言不变。`bash platforms/android/check-host.sh` 编译面板和设备套件、跑 JVM 冒烟和无对话框守卫。设备断言由 PR 上的 Android device（API 35）任务执行。
