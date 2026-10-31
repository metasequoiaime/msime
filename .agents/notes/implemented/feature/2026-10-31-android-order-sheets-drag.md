# Agent Note: Android 排序面板改为按住把手拖动

Status: implemented

## Problem

#10392：Android「键盘 › 布局」里的「文本编辑按键顺序」和「符号分类顺序」两张排序面板，每行是「序号. 名称」加两个 48 dp 的「↑」「↓」文字按钮。点一下和相邻项交换，然后整表 `removeAllViews` 重画，没有过渡动画，一项要挪十几格就得点十几下。报告人参照 WM 键盘的 GIF，希望按住行右侧的把手上下拖动：被拖的行浮起跟着手指走，其余行让位，行首序号随位置实时变化。

这两项设置只有 Android 有：`touch_text_edit_button_order` 在 `preferences.rs` 里写明其他宿主保留但不使用，符号分类顺序是本机设置 `AndroidLocalSettings.SYMBOL_CATEGORY_ORDER`。所以这次只改 Android，不涉及 iOS 和鸿蒙。

同一个 App 里「九键左侧符号」编辑页（`NineKeySidebarSheet`，见[左侧符号编辑](2026-10-18-android-nine-key-sidebar-editor.md)）已经能按住把手拖动，但它是在 `LinearLayout` 上手写的：越过半行就跳位，没有动画和序号；数据用 `Collections.swap`、视图用 remove/add 挪动，一次跨过两行以上时两者不一致；拖动中 `removeViewAt` 移除的正是触摸目标所在的行。

## Decision

两张面板共用新的 `home/ReorderList`：`RecyclerView` + `LinearLayoutManager` + `ItemTouchHelper`，每行「序号. 名称 + `ic_ms_drag_handle` 把手」。

- 拖动只从把手开始：把手 `ACTION_DOWN` 时 `startDrag`，`isLongPressDragEnabled` 返回 false，与左侧符号页一致，也不和面板的纵向滚动抢手势。报告人说的「长按」在他自己的 GIF 和 #10396 的措辞里都是按住把手。
- `ItemTouchHelper` 逐格回调 `onMove`，草稿和列表都按「挪动」改：`ReorderPolicy.moved`（remove 再 add）配 `notifyItemMoved`。即使一次回调跨过多行，保存的顺序也就是屏幕上的顺序。每次挪动后只原地改写已绑定行的序号文字、读屏描述和读屏操作，不整表重建，被拖的行不会被销毁。
- 被拖的行浮起：`onSelectedChanged` 换成 `accentSoft` 圆角底色并把 `translationZ` 抬到 6 dp，`clearView` 复原。其余行的让位动画是 `RecyclerView` 默认的 `DefaultItemAnimator`。
- 列表放在 `SettingsSheet` 的 `NestedScrollView` 里，`setNestedScrollingEnabled(false)`、高度随内容，整张面板一起滚。拖动开始时 `ItemTouchHelper` 让父容器不拦截触摸，面板和滚动区不会跟着手指走。
- 去掉箭头。读屏用户的排序入口是每一行的自定义操作「上移」「下移」（`ViewCompat.addAccessibilityAction`），首行没有「上移」、末行没有「下移」，保存进行中两个都撤掉。整行是一个读屏焦点，描述与可见文字相同（「1. 复制」）；把手对读屏隐藏，因为读屏用户没法按住它拖。
- 实体键盘和方向键：原来的箭头是可点的视图，API 26 起默认能拿键盘焦点、按回车就挪。去掉箭头后整行改为可聚焦，焦点在行上时 Alt+↑ / Alt+↓ 挪一格，和「上移」「下移」走同一个 `moveBy`，保存进行中同样不动；不带 Alt 的方向键照常在行间移动焦点。把手 `setFocusable(false)`，不留一个按了没反应的焦点。
- 把手的触控区与原来的箭头一样宽 48 dp、高占满整行。`iconButton` 给的是 48 dp 方形参数，但加进行时若再传 wrap 参数，会缩成 24 dp 图标加内边距，约 43 dp，按偏一点就落到没有反应的行上。
- 草稿、保存、取消、重置的语义不变；保存进行中 `setLocked(true)`，把手变灰、不能拖。
- 两张面板的说明文字从「用上下箭头调整」改成「按住右侧把手拖动调整」。

纯逻辑（挪动、能否上移下移、序号文字）放在不引用 androidx 的 `home/ReorderPolicy`，在 `check-host.sh` 的 JVM 编译清单里点名，由 `tests/home/ReorderPolicySmoke` 覆盖。`ReorderList` 引用 androidx，JVM 冒烟碰不到，`check-host.sh` 钉住接线：两张面板经 `ReorderList` 画行且没有箭头，只从把手开始拖、`onMove` 经 `ReorderPolicy.moved` 配 `notifyItemMoved`，列表不自己滚，每行有「上移」「下移」。

## 范围

- `NineKeySidebarSheet` 不动。它的序号、动画和 swap/move 不一致归 #10396，可以改用同一个 `ReorderList`（它的行里还有删除键，需要给 `ReorderList` 加行尾操作，届时再定）。
- 工具栏「按钮顺序」（`ToolbarOrderSheet`）也是箭头，不在 #10392 范围内，另开 issue。
- `SymbolCategoryOrderPolicy.move`（相邻交换）不再被设置页调用，它的语义与 `ReorderPolicy.moved` 在相邻一格时相同，留着由 `SymbolCategoryOrderPolicySmoke` 继续覆盖。

## Alternatives considered

- **照 `NineKeySidebarSheet` 在 `LinearLayout` 上手写拖动，再加 `LayoutTransition` 做让位动画**：不需要换容器，三张排序页的写法一致。但 `LayoutTransition` 只给布局变化加动画，被拖的行仍是越过半行就跳一下，做不到跟手的浮起卡片；手写还要自己处理逐格换位、触摸取消和边缘情况，而 `androidx.recyclerview:recyclerview` 本来就是依赖（`CommunityFragment` 在用），`ItemTouchHelper` 把这些都做好了。[剪贴板左滑删除](2026-10-09-android-clipboard-swipe-delete.md)当时没用 `ItemTouchHelper`，是因为那块面板在 `core/` 里要过 check-host 的 JVM 编译、而且是每次重建的 `LinearLayout`；设置页在 `home/` 里本来就只由 Gradle 编译，没有这个约束。
- **箭头保留，与拖动并存**：读屏用户不需要额外操作菜单。但报告人原话是箭头「看着很别扭，也操作不方便」，参照的 GIF 里也没有箭头；读屏用户用每行的「上移」「下移」能完成同样的事，而且焦点留在被移动的那一行上，比原来点箭头后整表重画、焦点丢失更好用。
- **长按整行拖动**（`isLongPressDragEnabled` 返回 true）：不需要把手。但长按要等系统长按时长，按住后才能拖，和面板的纵向滚动也会争手势；左侧符号页已经是按住把手立即拖，同一个 App 里保持一种交互。

## Consequences

收益：两张面板拖一次就能把一项挪到任意位置，被拖的行跟手、其余行有让位动画、序号实时变化；保存的顺序和屏幕一致；读屏用户的排序不受影响，焦点不再因整表重画而丢失。`ReorderList` 可供 #10396 和以后的工具栏排序复用。

代价与已知上限：

- 列表不自己滚动，拖到面板可见区的上下边缘时不会自动滚动。「符号分类顺序」有 20 项，约 960 dp，多数手机一屏放不下；要把末尾一项拖到最前，得拖到可见区顶端、松手、滚一下面板再接着拖。`ItemTouchHelper` 的自动滚动只滚 `RecyclerView` 自己，而这里滚的是外层 `NestedScrollView`。这次没有补：拖动位置是 `ItemTouchHelper` 按触摸事件私下记的手指位移，手指停在边缘不动时没有新事件，外层面板滚了它也不知道，被拖的行会离开手指、也不会换到新位置；要补就得在每帧滚动后自己合成移动事件喂给它，这种做法离不开在设备上反复调，没上设备前不放进来。留作后续，在设备上做。
- 行首加了 12 dp 内边距，好让浮起的圆角卡片不贴着文字，名称比面板标题往右缩进一点。

验证：

- `platforms/android/check-host.sh`（经 `rbuild` 在 Studio 上跑）：319 项 JVM 冒烟（含新增的 `ReorderPolicySmoke`：相邻挪动、跨两行是挪动而不是互换、首尾项、逐格挪动的累加、越界、不可变、上移下移的边界、序号）和 259 份设备套件源码编译通过；新钉子在 develop 上的两张面板（有箭头、没有 `ReorderList`）和去掉「下移」的 `ReorderList` 上都会失败。
- Gradle `:app:compileFullDebugJavaWithJavac` 和 `:app:lintFullRelease`（NewApi）在 Studio 上通过。
- 设备套件 `TextEditRecentClipboardDeviceSmoke` 的排序验收从点「下移 复制」箭头改成执行「1. 复制」这一行的「下移」读屏操作，保存、取消、重置、重开的断言不变；只做了源码编译，没有在模拟器上跑。
- 没有在模拟器或真机上看过：拖动跟手和让位动画、序号实时变化、面板不跟着滑、快速拖过多行后保存再重开的顺序、TalkBack 的「上移」「下移」、实体键盘的 Alt+↑ / Alt+↓ 和把手的焦点、深色主题下浮起卡片的样子。
