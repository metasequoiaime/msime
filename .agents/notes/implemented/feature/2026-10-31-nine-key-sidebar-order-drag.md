# Agent Note: 左侧符号编辑页的序号与跟手拖动

Status: implemented

## Problem

#10396：「九键左侧符号」「数字键盘左侧符号」共用的编辑页（[三端共享](2026-10-29-nine-key-sidebar-shared.md)、[Android 可视化编辑](2026-10-18-android-nine-key-sidebar-editor.md)）有两处不顺手。一是「当前顺序」里每项前没有序号，表最多 30 项，数不清某个符号排第几。二是拖动排序很生硬：报告人给的参考 GIF 里，按住右侧把手后那一行浮起、跟着手指走，其余行平滑让位，序号随位置实时变化；而 Android 只把被拖行调成半透明，越过某行中线时直接 `removeViewAt` / `addView` 把行瞬移过去，共享设置页（鸿蒙）也是拖动中直接改表、行瞬移。

排查时还发现 Android `reorderRow` 的数据和画面不一致：数据用 `Collections.swap(symbols, current, target)` 交换两项，视图做的却是「把这一行挪到 target」。一次 `ACTION_MOVE` 越过两行以上时两者不同，例如 `[a,b,c,d]` 把 a 拖到第三位，数据成了 `[c,b,a,d]`，画面是 `[b,c,a,d]`，保存的是数据那份。

## Decision

三端在同一个 PR 里改，交互照参考 GIF：按住把手拿起（不需要长按，三端说明文案不变），序号写成「1.」「2.」…「30.」（Android `NineKeySidebarPolicy.ordinal`、iOS `NineKeySidebar.ordinal`、共享页 `nineKeySidebarOrdinal`）。

### 拖动模型（Android 与共享设置页相同）

- 拖动途中行的视图（DOM 节点）不换位置，表也不改。被拖行只加 translationY（共享页是 `transform: translateY`）跟着手指；位移是手指移动的距离加上拖动中滚动区滚过的距离，并夹在列表上下沿之间，不拖出卡片。
- 落点按拿起那一刻量好的各行布局中线算（Android `NineKeySidebarPolicy.dragTarget`，共享页 `follow` 里同一条规则）：数其余各行中有几行的中线在被拖行中线之上，被拖行中线碰到哪一行的中线就算到了那一行。被拖行自己不参与比较，各行身上的跟手、让位位移也不进判断：Android 用 `getTop()`，共享页用 `offsetTop` / `offsetHeight`，都是不含 transform 的布局位置。这是 #10396 里「`dragTarget` / `target()` 要排除被拖行的位移」的落实方式。
- 被拖行浮起：Android 换成不透明圆角底色并抬高 translationZ；共享页换成不透明底色并改写行自己的 `--p-row-shadow` 画阴影。共享页不能另加 `shadow-lg`：行模板的 `[box-shadow:var(--p-row-shadow)]` 在样式表里排在 `shadow-*` 工具类后面，各平台这个变量都是 none，会把阴影盖掉，鸿蒙上的行底色又和分组底色相同，被拖行看上去和别的行一样平。
- 共享页记下拿起这一行的那根手指（`pointerId`），之后只认它的移动和抬起：触摸时每根手指的事件都发给它最先按下的元素，另一根手指按在别的行的把手上，它的移动和抬起也会进同一组处理函数，不分手指就会把被拖行拽到第二根手指的高度、提前松手。Android 每个把手的触摸监听只认自己那一行（`dragged != views` 时不处理），没有这个问题。
- 夹在原位和落点之间的行朝原位一侧让出被拖行的高度（`dragSlot`），用 150 ms 的过渡滑过去；序号按 `dragSlot` 给出的位次实时改写。共享页的让位过渡带 `motion-reduce:transition-none`，系统要求减少动态效果时松手直接落进格子（没有过渡就等不到 `transitionend`，不能留着落位状态等它）；Android 的动画跟随系统动画时长缩放。
- 拖动中表被别的操作改掉（另一指点了删除、点了「恢复默认」）时作废这次拖动：拿起时量好的几何已经对不上了。Android 走 `render()` 清掉 `dragged`，共享页走 `cancelDrag()`。
- 松手时表按挪动语义改好（Android `NineKeySidebarPolicy.moved`，`symbols.add(to, symbols.remove(from))`，与共享页 `moved()`、iOS `NineKeySidebarDraft.move` 同一语义），整张表重画：其余行此刻已经停在新位置上，重画后不动；被拖行先停在松手处，再滑进自己的格子。
- 手指进入滚动区上下 56 dp/px 的边缘带时自动滚动，进得越深越快，每帧最多 12 dp/px（Android `NineKeySidebarPolicy.edgeScrollStep`，共享页 `edgeScrollStep`）。Android 滚的是 `SettingsSheet` 的 `NestedScrollView`，共享页滚的是编辑页自身那层 `fixed inset-0 overflow-y-auto` 的容器；滚动后立即重新跟手，手指不动时被拖行也跟着滚过的距离走。被拖行已经夹在列表首尾、不能再朝那个方向挪时不滚（Android `NineKeySidebarPolicy.scrollTowardRoom`，共享页 `scrollTowardRoom`）：两处滚动区在列表下面都还有添加、保存等内容，接着滚下去，列表会带着被拖行滚走，离开手指。
- 读屏：Android 把手补上自定义无障碍操作「上移」「下移」（`ViewCompat.addAccessibilityAction`），挪完把焦点放回挪过去那一行的把手，把手的状态描述是「第 N 项」；符号牌的描述是「第 N 项，符号：X」，序号视图对读屏隐藏。共享页的序号 `aria-hidden`，位置由 `<ol>` 本身给出；键盘在把手上按 ↑ ↓ 挪一格照旧，触屏读屏（鸿蒙手机没有方向键，把手只能拖）用每行视觉隐藏的「上移」「下移」按钮，首行没有上移、末行没有下移，与 Android 的无障碍操作一致；这两个按钮 `tabIndex=-1`，不给键盘用户多出看不见的 Tab 停留。挪完一格后焦点回到挪过去那一行的同一个控件（挪到首尾、那个方向的按钮没了就换成另一个方向的），因为表重排时 React 可能把那一行的节点摘下再插回，浏览器会把焦点丢回页面。

### iOS

保留系统 `List` + `.onMove`：浮起、让位动画和 VoiceOver 的移动操作都是系统的。行前加序号，序号在落下后刷新，拖动途中不变，这是与 Android、鸿蒙之间有意保留的差异。`ForEach` 的标识从 `\.offset` 换成随符号走的 `NineKeySidebarRow.id`，挪动落下时 SwiftUI 把它当成换位置而不是换内容。标识按码位取（`unicodeScalars`），没有用 `\.self`：表按码位去重（`NineKeySidebar.same`），组合前后的 é 在表里是两项，Swift 的 `==` 却按标准等价把它们当成同一个值，`\.self` 会让两行撞标识。VoiceOver 念「第 N 项，符号 X」。

## Alternatives considered

- **Android 改用 RecyclerView + ItemTouchHelper**：androidx RecyclerView 已经是依赖，ItemTouchHelper 自带浮起、跟手、换位判断和 `notifyItemMoved` 的让位动画，是平台现成的做法。不采用：编辑页在 `SettingsSheet` 的 `NestedScrollView` 里，下面还有添加、保存等操作卡片，RecyclerView 只能是撑满高度、自己不滚动的一段。ItemTouchHelper 的边缘自动滚动只滚 RecyclerView 自己，而且它在 RecyclerView 坐标里记手指位移；外层滚动区滚动时 RecyclerView 整体移动，手指不动就收不到新的触摸事件，被拖行会跟着内容一起走、离开手指，换位判断也按旧坐标算。要让它在外层滚动区里自动滚动，得改写它的私有状态或合成触摸事件。把列表改成自己滚动的一段又会在面板里套出两层滚动。手写的部分只有跟手、让位、落点和边缘滚动几段，规则都在 `NineKeySidebarPolicy` 里由 JVM 冒烟覆盖。
- **沿用拖动中边拖边改表（原来两端的做法），只给行加过渡**：共享页可以用 FLIP 补动画，Android 可以加 `LayoutTransition`。不采用：行在布局里换了位置，被拖行的位移每一步都要补偿，落点判断还要扣掉别的行正在进行的让位动画，容易抖；Android 在触摸分发途中把接收触摸的那一行摘下再放回，也让手势依赖 ViewGroup 摘除子视图时对触摸目标的处理。改成拖动中只动位移、松手才改表，落点只看固定的布局中线，结果不随动画抖动。
- **iOS 自己写拖拽，让序号在拖动途中也实时变化**：三端行为完全一致。不采用：要放弃系统 `List` 的拖拽、让位动画和 VoiceOver 的移动操作，自己补无障碍；维护者决定 iOS 保留系统拖拽，接受序号落下后才刷新。
- **iOS `ForEach` 用 `\.self`**：维护者决定里写的是「改成 `\.self`，表内不重复」。按码位去重和 Swift `==` 的标准等价不一致（见上），所以换成按码位的标识，效果相同、没有撞标识的边角。

## Consequences

- 收益：三端每项前都有序号；Android 与鸿蒙拖动时被拖行跟手、其余行滑开让位、序号实时变化，30 项的表也能把第 1 项一路拖到末尾；Android 快速拖动越过多行时保存的顺序和画面一致（这是行为修正：以前同样的手势会存成交换后的顺序）；Android 读屏有了「上移」「下移」，鸿蒙的触屏读屏也能挪动。
- 代价：Android 与共享页各多了一段手写的拖动代码（跟手、让位、落位、边缘滚动），规则（`moved`、`dragSlot`、`dragTarget`、`edgeScrollStep`、序号写法）在 Android 侧由 `NineKeySidebarPolicySmoke` 覆盖，共享页由 `apps/desktop/tests/settings/nine-key-sidebar-settings.test.tsx` 覆盖；共享页那几个函数是组件内的私有实现，没有和 Android 共用代码，两端规则靠注释互指和各自的测试对齐。
- 让位距离按被拖行的高度算，假定各行等高（同一种行模板，符号牌单行）；行高不一时让位会差几个像素，松手重画后归位。
- 有意保留的差异：iOS 拖动途中序号不变，落下后刷新。

## Verification

- Android：`ANDROID_SDK_ROOT=~/Library/Android/sdk bash platforms/android/check-host.sh` 通过（318 项 JVM 冒烟，含 `NineKeySidebarPolicySmoke` 新加的挪动、拖动位次、落点、拖到首末一格、边缘滚动、夹在首尾时停止滚动与序号用例）；Gradle `:app:compileFullDebugJavaWithJavac` 通过（`NineKeySidebarSheet` 在 `home/` 下，JVM 冒烟不编译它）。
- 共享设置页：`apps/desktop` 下 `npx vitest run tests/settings/nine-key-sidebar-settings.test.tsx` 24 项通过，新用例覆盖序号、拖动中让位与序号实时变化、落点不受行身上位移影响（测试里的 `getBoundingClientRect` 照真浏览器带上 translateY）、拖到底只到最后一格、松手后的落位过渡、减少动态效果时直接落位、拖动中删除一项作废拖动、贴近下沿时自动滚动并跟手、被拖行夹在列表末尾后不再滚、第二根手指按在别的把手上不影响拖动、浮起的行改写 `--p-row-shadow` 而不是另加 `shadow-*`、读屏「上移」「下移」按钮与挪动后焦点不丢；`npx tsc --noEmit -p .`、`pnpm run lint`、`pnpm run format:check` 通过；`bash platforms/harmony/tests/run.sh` 0 失败；`python3 scripts/test-nine-key-sidebar-parity.py`、`python3 scripts/test-ios-key-layout-settings.py` 通过。
- iOS：`MSIMEClientTests` scheme 在 iOS 27.0 模拟器上 `-only-testing:MSIMEKeyboardTests/NineKeySidebarSettingsTests` 7 项通过，含新加的 `testRowsNumberTheCurrentOrder`。
- 没有在任何真机或模拟器上看过拖动手感：Android 的浮起阴影、让位动画、`NestedScrollView` 边缘滚动和 TalkBack「上移」「下移」，鸿蒙 ArkWeb 里的跟手、浮起阴影、自动滚动和读屏「上移」「下移」，iOS 系统拖拽落下后序号刷新、有无闪烁，都要在设备上验收。
