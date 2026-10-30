# Agent Note: Android 详情页在栈上时切底部 tab 不播回退动画

Status: implemented

## Problem

#10092：在「我的 → 皮肤」这类详情页上直接点另一个底部 tab，切换瞬间先出现一帧只剩背景的空白，接着两三帧里上一个 tab 的根页和目标 tab 半透明叠在一起，标题叠成「我杉拼音」。只要回退栈上压着详情页就会出现，和换不换皮肤无关；栈空时从根页切 tab 没有叠影。

`SettingsNavigator.push` 压详情页时给回退栈记录带上 `fade_through_in` / `fade_through_out` 作为 popEnter / popExit，同时把下面的 tab 根页 hide。`HomeActivity.selectTab` 换 tab 前用 `popBackStackImmediate` 清栈，这次弹栈照常播记录里的动画：详情页 90ms 淡出，tab 根页从 0.94 倍、全透明开始 280ms 淡入。紧接着 `show()` 用另一个事务把这个根页藏起来、让目标 tab 淡入。根页视图此时已是 GONE，但 `ViewGroup` 对带着未结束动画的子视图照样绘制，于是根页的淡入和目标 tab 的淡入同时进行；详情页的淡出同理，要等动画结束才离开容器。

## Decision

只在换 tab 前清栈的那一次弹栈里不播回退动画，目标 tab 的 `fade_through_in` 不变。

- `HomeActivity.selectTab` 在 `itemId != selected` 时，把 `clearingForTabSwitch` 置为 true，调用 `clearStack()`，再复位，然后才 `show(itemId)`。`popBackStackImmediate` 是同步的，标记只覆盖这一次弹栈。
- `HomeTabFragment.onCreateAnimation` 调 `HomeTabFragment.leavingWithoutAnimation(this)`：宿主处在这次弹栈里时返回零时长的 `AlphaAnimation(0f, 0f)`，否则返回 null，照常用记录里的资源动画。这次弹栈动到的视图都马上离开屏幕（详情页被移走，弹回来的根页随即被 `show()` 藏起），所以替换动画用全透明，动画结束前仍会被画的那一帧也是透明的。
- `CommunityFragment` 不继承 `HomeTabFragment`，但会作为 `PageId.COMMUNITY` 压进同一个回退栈，所以它的 `onCreateAnimation` 也调同一个方法。
- 系统返回键（`popBackStack`）、再点当前 tab 回根页、`openCommunityTab` 与深链里的 `clearStack()` 都不经过这个标记，回退动画不变。后两者先 `selectTabIndex` 切 tab，切 tab 会走 `selectTab`，栈在那里已经无动画地清空。

`platforms/android/tests/home/TabSwitchPopAnimationSmoke.java` 读源码核对：标记只在 `selectTab` 里设一次、包住 `clearStack()` 且先于 `show()` 复位；替换动画是零时长透明动画而不是 null；PageId 登记的每个已有页面类和 `HomeActivity.create` 建的四个 tab 根页，要么沿 `extends` 走到 `HomeTabFragment`，要么自己调 `HomeTabFragment.leavingWithoutAnimation(this)`。以后新加的页面直接继承 `Fragment` 时这个冒烟会失败。

## Alternatives considered

- **不弹栈，在 `show()` 里直接移除栈上的详情页**：能完全绕开回退动画，但 FragmentManager 记着的回退栈记录还在，之后的返回键和进程恢复会按一份与界面不符的栈执行，不采用。
- **在 `onCreateAnimation` 里返回 null 表示「不要动画」**：AndroidX 拿到 null 后会退回去加载记录里的 `nextAnim` 资源，等于什么都没改。
- **替换成 `AlphaAnimation(1f, 1f)`**：同样零时长，但动画结束前那一帧里详情页或根页仍以不透明画出来，和开始淡入的目标 tab 叠在一起。用全透明，这一帧里只剩目标 tab 自己的淡入起点，不和别的页面叠在一起。
- **去掉压详情页时的回退动画**：能修好切 tab，但系统返回键也会失去淡入淡出，超出这个问题的范围。

## Consequences

收益：栈上有详情页时切底部 tab，不再出现两个 tab 根页的重影，过渡和栈空时从根页切 tab 一样，只有目标 tab 的淡入；回退栈的内容和返回顺序都不变。

点击后的第一帧仍然只剩背景：被弹出的详情页和弹回来的根页这一帧都按全透明画，目标 tab 的 `fade_through_in` 从 `fromAlpha=0` 起步，第一帧也是全透明。栈空时从根页切 tab 同样如此（`show()` 的退出动画是 0，旧根页直接藏起，目标 tab 从全透明淡入），所以这一帧是普通切 tab 本来就有的，不是这次没修掉的问题。设备上逐帧检查时要看的是叠影消失、过渡和根页之间切 tab 一致，而不是第一帧非空。

代价：页面类多了一条约束，进 `home_content` 回退栈的 Fragment 必须继承 `HomeTabFragment` 或自己接上 `leavingWithoutAnimation`，由上面的冒烟兜底。动画效果本身只能在设备上看，JVM 冒烟只核对结构，逐帧确认重影消失需要真机或模拟器录屏。

iOS 的底部 tab 是系统 `TabView`，每个 tab 各自一个 `NavigationStack`，切 tab 不会弹另一个 tab 的栈，没有同样的机制，本次不改。
