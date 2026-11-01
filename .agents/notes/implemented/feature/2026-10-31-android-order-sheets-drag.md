# Agent Note: Android 排序面板改为按住把手拖动

Status: implemented

## Problem

#10392：Android「键盘 › 布局」里的「文本编辑按键顺序」和「符号分类顺序」两张排序面板，每行是「序号. 名称」加两个 48 dp 的「↑」「↓」文字按钮。点一下和相邻项交换，然后整表 `removeAllViews` 重画，没有过渡动画，一项要挪十几格就得点十几下。报告人参照 WM 键盘的 GIF，希望按住行右侧的把手上下拖动：被拖的行浮起跟着手指走，其余行让位，行首序号随位置实时变化。

这两项设置只有 Android 有：`touch_text_edit_button_order` 在 `preferences.rs` 里写明其他宿主保留但不使用，符号分类顺序是本机设置 `AndroidLocalSettings.SYMBOL_CATEGORY_ORDER`。所以这次只改 Android，不涉及 iOS 和鸿蒙。

同一个 App 里「九键左侧符号」编辑页（`NineKeySidebarSheet`，见[左侧符号编辑](2026-10-18-android-nine-key-sidebar-editor.md)）已经能按住把手拖动，但它是在 `LinearLayout` 上手写的：越过半行就跳位，没有动画和序号；数据用 `Collections.swap`、视图用 remove/add 挪动，一次跨过两行以上时两者不一致；拖动中 `removeViewAt` 移除的正是触摸目标所在的行。同一时间 #10396（PR #10452）在重写那张页的拖动：行只改 translationY 跟手，落点按布局中线算，贴近面板上下边缘时滚动外层 `NestedScrollView`；读屏把可见序号隐藏，名称念「第 N 项」，「上移」「下移」挂在把手上。

## Decision

两张面板共用新的 `home/ReorderList`，拖动模型和读屏模型都照「九键左侧符号」编辑页（#10396）的做法，同一个「按键布局」页里的三张排序面板手感和念法一致。每行是「序号. 名称 + `ic_ms_drag_handle` 把手」，行放在一个 `LinearLayout` 里。

- 拖动只从把手开始：把手 `ACTION_DOWN` 就拿起，不等长按，也不和面板的纵向滚动抢手势。报告人说的「长按」在他自己的 GIF 和 #10396 的措辞里都是按住把手。拿起时请父容器不拦截触摸，面板和 BottomSheet 不跟着手指走。
- 被拖的行换成不透明的圆角 `rowBackground` 底色、`translationZ` 抬到 8 dp，盖在让位的行上面；拖动中只改它的 translationY 跟手，视图不换位置、不被销毁。
- 落点按拿起时各行的布局中线算（`ReorderPolicy.dragTarget`），不受让位动画的位移影响。落点变了，夹在原位和落点之间的行用 150 ms 的短动画让出一行高，行首序号和读屏描述按 `ReorderPolicy.dragSlot` 实时改写。
- 松手才把草稿按 `ReorderPolicy.moved`（remove 再 add）挪好并重画；其余行此刻已在新位置，重画后不动，被拖行从手指处滑进自己的格子。保存的顺序就是屏幕上的顺序。
- 贴边自动滚动：列表放在 `SettingsSheet` 的 `NestedScrollView` 里、自身不滚动。手指停在滚动区上下 56 dp 的边缘带里时，每一帧按进入的深度滚动那个 `NestedScrollView`，最多 12 dp（`ReorderPolicy.edgeScrollStep`），滚过的距离按列表在屏幕上的位移算进跟手位移，被拖行不离开手指、落点照常更新。被拖行已经夹在列表首尾时不再朝那一头滚（`ReorderPolicy.scrollTowardRoom`），因为下面还有操作卡片，面板往往还能滚，列表会带着它滚走。「符号分类顺序」20 行比一屏高，把末尾一项拖到最前可以一次完成。
- 去掉箭头。读屏模型与左侧符号页相同：可见的「1. 复制」里的序号不单独念，名称的读屏描述是「第 1 项，复制」；把手的读屏描述是「拖动调整 复制 的顺序」，状态念「第 1 项」，自定义操作「上移」「下移」挂在把手上，首行没有「上移」、末行没有「下移」，保存进行中两个都撤掉。执行后重画，读屏焦点放回挪过去那一行的把手上，念出新位次。
- 实体键盘和方向键：把手是可聚焦的按钮，焦点在把手上时 Alt+↑ / Alt+↓ 挪一格，和「上移」「下移」走同一个 `moveBy`，挪完键盘焦点跟到新位置的把手上；不带 Alt 的方向键照常移动焦点。
- 把手的触控区与原来的箭头一样宽 48 dp、高占满整行。`iconButton` 给的是 48 dp 方形参数，但加进行时若再传 wrap 参数，会缩成 24 dp 图标加内边距，约 43 dp，按偏一点就落到没有反应的行上。
- 草稿、保存、取消、重置的语义不变；保存进行中 `setLocked(true)`，把手变灰、不能拖。
- 两张面板的说明文字从「用上下箭头调整」改成「按住右侧把手拖动调整」。

纯逻辑（挪动、能否上移下移、序号和读屏描述、拖动位次、落点、贴边滚动步长）放在不引用 androidx 的 `home/ReorderPolicy`，在 `check-host.sh` 的 JVM 编译清单里点名，由 `tests/home/ReorderPolicySmoke` 覆盖。`ReorderList` 引用 androidx，JVM 冒烟碰不到，`check-host.sh` 钉住接线：两张面板经 `ReorderList` 画行且没有箭头；不用 `ItemTouchHelper` / `RecyclerView`；落点按 `dragTarget`、松手经 `ReorderPolicy.moved`；跟手位移算进滚动距离、贴边滚动外层面板并经 `scrollTowardRoom` 停在首尾；「上移」「下移」和「第 N 项」在把手上，把手上有 Alt+↑ / Alt+↓；把手宽 48 dp。

## 范围

- #10392 时 `NineKeySidebarSheet` 不动，它的拖动由 #10396 重写，两个 PR 同时在审、谁都不能依赖对方，于是拖动规则在 `ReorderPolicy` 和 `NineKeySidebarPolicy` 里各有一份，拖动控制代码在 `ReorderList` 和 `NineKeySidebarSheet` 里也各有一份。
- #10546 在两个 PR 都合进 develop 后把它们合成一份，见下面「#10546：一个控制器、一份规则」。
- `SymbolCategoryOrderPolicy.move` 和 `ToolbarOrderPolicy.move`（相邻交换）不再被设置页调用，语义与 `ReorderPolicy.moved` 在相邻一格时相同，留着由各自的冒烟继续覆盖。

## #10546：一个控制器、一份规则

规则只留 `home/ReorderPolicy` 一份。`NineKeySidebarPolicy` 删掉了逐字重复的 `moved`、`dragSlot`、`dragTarget`、`edgeScrollStep`、`scrollTowardRoom`、`positionDescription`、`ordinalDescription`，只保留可见序号的写法 `ordinal`（「1.」），因为 iOS `NineKeySidebar.ordinal` 和共享设置页 `nineKeySidebarOrdinal` 的注释把它当三端同一写法引用。符号牌的读屏描述改成 `ReorderPolicy.ordinalDescription(position, "符号：" + text)`，念出来仍是「第 N 项，符号：X」。`NineKeySidebarPolicySmoke` 里的拖动用例按两份的并集并进 `ReorderPolicySmoke`，那边只留序号一条。

`ReorderList` 改成只管拖动的控制器，行由调用方画：

- `RowFactory<T>.create(item, index)` 交回 `Row(row, params, handle, renumber)`：整行、整行的布局参数（行高由调用方定）、行里的把手、按位次改写可见序号和读屏描述的回调。把手统一由 `ReorderList.addHandle` 加进行里，48 dp 宽、高占满整行，读屏描述「拖动调整 名称 的顺序」。
- 控制器负责拿起、跟手、让位、贴边滚动外层 `NestedScrollView`、落位，把手上的「上移」「下移」（按 `canMove` 给，锁定时撤掉）、「第 N 项」状态、Alt+↑ / Alt+↓，锁定时把手变灰，挪动后把键盘焦点和读屏焦点放回新位置的把手。常量（浮起 8 dp、圆角 12 dp、动画 150 ms、边缘带 56 dp、每帧 12 dp）只在这里。
- 落位动画开始前记下行原来的底色，结束后换回，所以无底色的行和带点按水波纹的行都对，不需要调用方传参。
- 只有名称的面板用 `ReorderList(context, items, label)`，内部画成「序号. 名称 + 把手」、行高 48 dp，与之前一样。

`NineKeySidebarSheet` 去掉了自己的拖动字段和方法，行里照样画序号栏、符号牌、把手和删除键。删除、添加、输入和恢复默认都改成拼出新表后 `setItems`，不再依赖 `moved` 返回可变表；空表提示从列表容器里挪到「当前顺序」卡片里、列表的下面，按表是否为空切换可见。统一后这张页有两处用户能感觉到的变化：把手触控区从 wrap 参数的约 43 dp 变成 48 dp 宽、高占满整行；多了实体键盘的 Alt+↑ / Alt+↓，挪动后键盘焦点跟到把手上。

`check-host.sh` 的接线钉子随之改写：`NineKeySidebarSheet` 必须经 `ReorderList` 画行，不得再有 `setOnTouchListener`、读屏操作或 `NineKeySidebarPolicy` 的拖动规则；`NineKeySidebarPolicy` 不得再声明那几个函数；落位后必须换回原来的底色。

## Alternatives considered

- **`RecyclerView` + `ItemTouchHelper`**：本 PR 最初的做法，拖动、换位和让位动画都由 `ItemTouchHelper` 和 `DefaultItemAnimator` 现成做好。放弃的原因是贴边自动滚动：`ItemTouchHelper` 只滚 `RecyclerView` 自己，而这里要滚的是外层 `NestedScrollView`；它的拖动位置是按触摸事件私下记的手指位移，手指停在边缘不动时没有新事件，外层面板滚了它也不知道，被拖的行会离开手指、也不会换到新位置。要补就得每帧滚动后自己合成移动事件喂给它。结果是「符号分类顺序」拖到可见区边缘就停，要松手、滚动、再拖，而同一页的「九键左侧符号」一次就能拖到底，两张排序面板的行为不一样。照左侧符号页的拖动模型自己跟手，滚过的距离直接算进位移，没有这个问题，两边也能共用同一套规则。
- **让 `RecyclerView` 自己滚动（固定高度）**：`ItemTouchHelper` 的自动滚动就能用。但面板里会出现第二个嵌套滚动区，和 BottomSheet、外层 `NestedScrollView` 争手势，而且面板高度要另定。
- **箭头保留，与拖动并存**：读屏用户不需要额外操作菜单。但报告人原话是箭头「看着很别扭，也操作不方便」，参照的 GIF 里也没有箭头；读屏用户用把手的「上移」「下移」能完成同样的事，而且焦点留在被移动的那一项上，比原来点箭头后整表重画、焦点丢失更好用。
- **「上移」「下移」挂在整行上，读屏念可见的「1. 复制」**：本 PR 最初的读屏模型，整行一个焦点。放弃是为了和同一页的左侧符号编辑页一致：读屏用户在两张排序面板里找操作的位置和听到的位次念法应当相同。

## Consequences

收益：两张面板拖一次就能把一项挪到任意位置，包括比一屏长的「符号分类顺序」；被拖的行跟手、其余行有让位动画、序号实时变化；保存的顺序和屏幕一致；读屏和键盘用户的排序入口、位次念法与左侧符号编辑页相同，焦点不再因整表重画而丢失。

代价与已知上限：

- 拖动规则和控制代码曾经各有两份（见范围），#10546 已合成一份；以后新的排序面板应当用 `ReorderList` 画行，不要再写一份拖动。
- 行首加了 12 dp 内边距，好让浮起的圆角卡片不贴着文字，名称比面板标题往右缩进一点。

验证：

- `platforms/android/check-host.sh`（经 `rbuild` 在 Studio 上跑）：JVM 冒烟（含 `ReorderPolicySmoke`：相邻挪动、跨两行是挪动而不是互换、首尾项、逐格挪动的累加、越界、不可变、上移下移的边界、序号和读屏描述、拖动途中各行位次与松手后的表一致、落点按中线、贴边滚动的步长和首尾停止）和设备套件源码编译通过。
- Gradle `:app:compileFullDebugJavaWithJavac` 和 `:app:lintFullRelease`（NewApi）在 Studio 上通过。
- 设备套件 `TextEditRecentClipboardDeviceSmoke` 的排序验收执行「复制」这一行把手上的「下移」读屏操作，保存、取消、重置、重开的断言不变。
- #10546 合并时：合并前后各跑一遍 `check-host.sh`（经 `rbuild` 在 Studio 上），冒烟个数和输出除了 `NineKeySidebarPolicySmoke` 的汇总一行外完全相同；Gradle 编译和 lint 通过。「九键左侧符号」编辑页的拖动、删除和添加后再拖、空表提示、保存中把手变灰、落位后底色回到水波纹、TalkBack 念法，都还没有在模拟器上看过。
- 没有在模拟器或真机上看过：拖动跟手和让位动画、序号实时变化、面板不跟着滑、贴边自动滚动的速度和跟手、快速拖过多行后保存再重开的顺序、TalkBack 的「上移」「下移」和位次念法、实体键盘的 Alt+↑ / Alt+↓、深色主题下浮起卡片的样子。
