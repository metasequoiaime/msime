# Agent Note: Android 底部第二格从「社区」换成「对话」

Status: implemented

底部第二格先后被 [AI 对话改为二级页面](2026-10-16-mobile-chat-secondary-page.md) 与 [触屏宿主底部第二格恢复为社区](2026-10-16-mobile-community-bottom-tab.md) 取代。

## Problem

[社区并进功能页](2026-10-11-android-community-into-feature-pages.md)那一步把底部的「社区」tab 直接删了，底部只剩设置、统计、我的三格。产品的意思是换掉这一格而不是删掉它：底部保持四个 tab，原来社区那一格换成 AI 对话，顺序是 设置 / 对话 / 统计 / 我的。

Android 原生宿主已经有 AI 对话，就是设置首页状态卡上「试用键盘」打开的那一页（`KeyboardTryoutActivity`）：聊天样式的输入框，登录后发出的话经账号会话交给 `/v1/chat/completions`，回复流式显示。要求是复用它，不另写一套对话。难处在于它是一个独立的 Activity，打开后盖住整个宿主，底部导航没法把它当成一格来显示。它还有两处只在「独立 Activity」前提下才成立的行为：一打开就弹键盘（清单里的 `stateVisible`），账号变了就 `finish()`。

## Decision

**对话页是一个 Fragment，底部第二格承载它。** `KeyboardTryoutActivity` 的全部内容挪进 `home/ChatFragment.java`（布局改名为 `res/layout/page_chat.xml`），继承 `HomeTabFragment`，和其他三个 tab 一样由 `HomeActivity` 建一次、之后 hide/show。`res/menu/bottom_tabs.xml` 的第二项是 `tab_chat`（标题「对话」，图标 `ic_ms_chat`，按 Material Symbols「chat」的样子画：气泡外框与 `ic_ms_feedback` 同一笔画，里面三行文字线），`HomeActivity.TAB_IDS` 回到四项、不再有空位。

**作为 tab 时的行为和单独打开时不同：**

- 没有「‹ 设置」返回键，标题是「对话」；返回键照 `HomeActivity` 的规则回到设置 tab，键盘升起时系统先收键盘。
- 点底部那一格进来不弹键盘；切到别的 tab 时把键盘收起来，不让它盖在另一个 tab 上。
- 底部让开 tab 栏加系统导航栏；键盘升起时仍保留导航可见并同时避让导航和键盘，见[键盘弹出时保留 Tab](../bug-fix/2026-10-13-android-chat-keyboard-keep-tabs.md)；顶部和左右由 `HomeActivity` 给内容区垫好。
- 账号变了不能 `finish()`：每次回到眼前（`onBecameVisible`）和每个回调到达时核对会话，变了就停掉请求、清空对话、改绑现在的会话。聊天请求带着发起时的会话，旧账号那一次迟到的回复不会落到新账号的对话里。对话页不加载模型目录，模型由服务端决定，见 [Android 对话页不选模型](2026-10-11-android-chat-server-picks-model.md)。

**「试用键盘」按钮切到对话 tab 并弹出键盘。** 试用键盘本来就是在这个输入框里打字，对话 tab 接手以后不再另开一页，设置首页状态卡上的「试用键盘」改为 `HomeActivity.openChat()`：切到对话 tab，再经 `ChatFragment.focusInput()` 把焦点交给输入框、弹出键盘。按钮本身留着，它是状态卡上「启用 → 设为默认 → 打字试试」的最后一步；iOS 的同一处也是这样改的。Android 的「AI 润色与回复」页没有「AI 对话」行，设置搜索的索引（`PageId` 关键词）里也没有指向对话页的条目，这两处没有要改的入口。

**`KeyboardTryoutActivity` 只剩一个外壳。** 它用 `ChatFragment.standalone()` 承载同一个页面，保留返回键、一打开就弹键盘和账号变了就关掉的老行为，清单里的 `adjustResize|stateVisible` 不变。宿主自己不再启动它；它留给 Tauri 共享设置的「屏幕键盘」页，`AccountPlugin.openKeyboardTryout` 按类名启动它，那一侧没有底部 tab 可切。

**`HostDeepLink` 的下标 1 是对话 tab。** 常量从 `TAB_COMMUNITY` 改名为 `TAB_CHAT`，值仍是 1，其余下标不变（0 设置、2 统计、3 我的）。上一步给下标 1 加的「打开社区皮肤完整列表」特判去掉：下标 1 就是对话 tab。仓库里没有任何发送方用过下标 1（键盘进程只发 `TAB_SETTINGS` 和页面名，设备测试只发页面名），要打开某一类社区作品的完整列表，用的是页面名 `COMMUNITY`（参数 `kind`），这条深链不受影响。`tests/home/BottomTabsSmoke.java` 读菜单和 `HomeActivity.TAB_IDS` 的源码，核对两处的顺序都是 设置 / 对话 / 统计 / 我的，并且和 `TAB_*` 的值一一对应。

**`HomeActivity` 的 `windowSoftInputMode` 写明 `adjustResize`。** 原先没写，由系统按窗口里有没有可滚动的视图来猜。对话 tab 的输入栏要贴在键盘上面，这在 Android 9、10 上靠 adjustResize 才拿得到键盘的 inset，平移窗口会把输入栏连同顶栏一起顶出屏幕；各页的搜索框本来也是按 inset 让位的。没有加 `stateVisible`，键盘只在「试用键盘」切过来时由页面自己弹出。

## Alternatives considered

- **点「对话」那一格时启动 `KeyboardTryoutActivity`，选中项留在原来的 tab** — 一行页面代码都不用动，返回自然回到原来的 tab。不用的原因是它不是一个 tab：底部导航一进去就被盖住，「对话」那一格永远不会处于选中状态，深链下标 1 也只能是「启动另一个 Activity」，和其他三格的行为对不上。
- **把旧的下标 1 继续指向社区皮肤的完整列表，对话 tab 另用一个新下标** — 已经发出去的「社区」深链仍然落在社区内容上。不用的原因是下标就是底部导航的位置，第二格是对话，下标 1 却打开别的东西会让 `TAB_*` 和底部顺序脱钩；而且仓库里没有任何发送方发过下标 1，要打开社区列表本来就有页面名 `COMMUNITY`。
- **去掉设置首页的「试用键盘」按钮，入口只剩对话 tab** — 入口最少。不用的原因是这个按钮是状态卡上设好键盘之后的最后一步，用户刚启用、刚设为默认，下一步就是打字试试；去掉以后他得自己想到去点「对话」。按钮改为切到对话 tab，就不再是第二个对话页。
- **把 `KeyboardTryoutActivity` 也删掉，Tauri 的「屏幕键盘」改发深链切到宿主的对话 tab** — 只留一种承载方式。不用的原因是那会把正在 Tauri 设置里的用户切到原生宿主的界面上，离开他正在用的那套设置；外壳只有十几行，承载的是同一个 Fragment，没有两份对话逻辑。

## Consequences

- **收益**：底部回到四格，第二格就是已有的 AI 对话，打开即用；试用键盘和对话是同一个输入框，不再有两份页面。对话 tab 和其他 tab 一样 hide/show，切走再回来对话还在。深链下标、菜单和 `TAB_IDS` 三处由冒烟核对，不会再出现空位。
- **代价与已知上限**：对话只在内存里，进程被杀、换深浅模式或换季重建 `HomeActivity` 时清空，和原来关掉试用页一样。在「我的」里换账号以后，对话 tab 回到眼前时清空对话并改绑新会话，正在流式到达的回复会被丢掉。旧版本发出的下标 1 如果本意是社区，现在会打开对话；仓库内没有这样的发送方。`HomeActivity` 显式写了 `adjustResize`，理论上会影响原先靠系统猜测的页面，实际上这些页面都按 inset 让位，猜出来的也是 resize。

## Verification

- `platforms/android/tests/home/BottomTabsSmoke.java` 核对菜单、`TAB_IDS` 和 `TAB_*` 的对应；`platforms/android/tests/settings/HostDeepLinkSmoke.java` 钉住 `TAB_CHAT == 1`。
- `scripts/test-android-keyboard-tryout.py` 等八个源码契约脚本改为检查 `ChatFragment.java`，并新增一条：聊天请求的成功和失败回调都丢掉已经过期的那一次。
- `platforms/android/check-host.sh`、全部 `scripts/test-android-*.py` 和 Gradle `compileFullDebugJavaWithJavac` 通过。没有在模拟器上装包验证（tab 切换、键盘弹出与让位、返回键、换账号后的重置都没有在设备上走过）：本机没有现成的 Android 原生库，完整 APK 要先交叉编译 Rust，而本机磁盘剩余不足 50 GB。合并前需要在 API 28 和 35 模拟器上走一遍。
