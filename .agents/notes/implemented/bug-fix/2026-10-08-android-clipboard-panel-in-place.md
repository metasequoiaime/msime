# Agent Note: Android 剪贴板面板的确认和操作画在面板里

Status: implemented

## Problem

#5605：点剪贴板面板顶行的「清空」，键盘直接消失，没有出现确认框；重新打开面板，那一条还在。

两个原因叠在一起。其一，`confirmClearClipboardHistory` 用 `new AlertDialog.Builder(this)`，`this` 是 `InputMethodService`。它没有 Activity 的窗口令牌，对话框的窗口加不上去，输入法进程随之结束，键盘被系统收起，清空根本没有执行。其二，就算清空执行了，`ImePanels.showClipboardHistory` 打开面板时会补读系统剪贴板（为了补上键盘进程不在时复制的内容），而系统剪贴板里还是刚才那一条，它会被重新记进历史，看上去仍是「清空不生效」。删除单条也有第二个问题。

#5653：在 Via 这类用系统 WebView 的浏览器里，长按一条历史，「固定 / 删除」菜单闪一下，键盘随即被收起；Firefox 和普通应用里正常。菜单是 `PopupMenu`，它的弹出窗口可获得焦点，打开时编辑器所在窗口失去窗口焦点。录屏里菜单出现后键盘才收起，且只发生在 WebView 浏览器里，据此判断是 WebView 在窗口失焦时让输入框失焦、收起输入法，而 GeckoView 不这样做；这个机制是从录屏和差异推断的，没有在设备上抓日志确认。

## Decision

- 清空的确认画在面板顶行：点「清空」（#10037 起在顶行「更多」菜单里，见 [「更多」菜单](../feature/2026-10-29-android-clipboard-more-menu.md)）后顶行换成「确认清空全部记录（含固定项）？ 取消 清空」，分段在这时让出位置；换分段或重开面板都会撤销。确认用的「清空」是回车键的强调键帽（`KeyboardKeyRole.RETURN`，和详情页的「插入」同一种），「取消」仍是普通文字按钮（#10030）。点确认先把顶行重画回普通状态，再由 `MSIMEInputService.clearClipboardHistory` 拉起系统认证、通过后清空；认证没通过或被取消时，面板上不会留着一行已经失效的确认。剪贴板面板 never 弹 `AlertDialog`。
- 确认行不放进横向滚动，直接挂在 `clipboardHeader` 下，按键盘宽度 `EXACTLY` 量：「取消」「清空」按内容取宽，问句只分到剩下的宽度，在 13sp 到 10sp 之间按可用宽度缩小（`KeyboardTypographyPolicy.setFittedKeyTextSize`），再放不下才单行省略。所以无论键盘多窄、按键字号多大，两个按钮都在键盘宽度以内（#10030）。其余状态的顶行仍套在 `HorizontalScrollView` 里，放不下时横滑。
- 每条剪贴板内容有一个身份：`ClipDescription.getTimestamp()`（读不到时为 0）加文字散列和长度，由 `ClipboardCapturePolicy.identity` 算出，只用来比较，不含文字。上一次处理过的身份存在 `:ime` 进程的 SharedPreferences `android-clipboard-capture`。
- `captureClipboard(trigger, announce)`：复制时的监听（`Trigger.COPIED`）一律记录；打开面板的补读（`Trigger.PANEL_OPENED`）只记录身份与上一次不同的那一条。共享存储有了答复之后才记下身份：收下或明确拒收（`add` 返回原因）都记，拒收的内容不会每次打开面板都重试；存储写不进去时 `add` 抛异常，这一条不记，下次补读再试。第一版在交给存储之前就记下，写入抛异常时这一条被当成处理过，之后的补读永远跳过它。
- 清空和删除单条之后，把系统剪贴板当前那一条记为已处理（`forgetCurrentClip`）。系统标为敏感（`EXTRA_IS_SENSITIVE`）的内容跳过：补读本来就不记录它，而身份里的散列和长度足以穷举还原短密码或验证码，不能为它落盘。
- 长按一条历史，操作（固定或取消固定、删除、编辑、分词、添加到常用语、发到云剪贴板、收起）画成紧贴在它下方的两行按钮（`ImePanels.renderClipboardItemActions`；后加的几项见 [存为常用语](../feature/2026-10-09-android-clipboard-to-phrases.md) 和 [在应用里编辑](../feature/2026-10-09-android-clipboard-edit.md)），执行由 `MSIMEInputService.setClipboardItemPinned` / `removeClipboardItem` 负责。剪贴板面板 never 弹 `PopupMenu`。
- 确认行的问句、「取消」和「清空」高度一律是 `MATCH_PARENT`（问句用 `weightedMatchParentParams(1)` 并垂直居中）。#10027 时确认行和其余状态的顶行一样套在 `HorizontalScrollView` 里、以 `WRAP_CONTENT` 挂上，量高时拿到的是 `AT_MOST 32px`；这时横向 `LinearLayout` 只要有一个子视图不是 `MATCH_PARENT`，行高就只取非 `MATCH_PARENT` 子视图的高度。问句原来是 `weightedZeroParams(1)`（高 0），27ea3d75e4 给顶行套上横向滚动后整行塌成 0 高：问句和两个按钮都看不见，手指点不到确认，清空也就走不到系统认证（#10027）。#10030 之后确认行按 `EXACTLY 32px` 量，不再经过这条路径；普通状态的顶行仍在横向滚动里，它的子视图同样要保持 `MATCH_PARENT` 高。同一顶行的计数和「N 项已选择」本来就是 `MATCH_PARENT`。其余横向滚动行（工具栏、候选、九键拼音、自动填充、符号分类）的内容行要么以 `MATCH_PARENT` 挂上、量高是 `EXACTLY`，要么子视图是固定高度，不受影响。
- `ClipboardSegmentDragDeviceSmoke` 先点「更多剪贴板操作」再点菜单里的「清空」，点确认之前先等问句、「取消」「清空」可见，并断言它们在屏幕上的宽高都大于 0：`DeviceSmoke.tap` 对本应用节点走 `ACTION_CLICK`，0 高度的按钮照样点得中，所以塌行在设备验收里一直是绿的。
- `ToolbarEdgesDeviceSmoke` 用合成历史把面板置于确认态，在 240、320、600dp 宽下排版，断言问句、「取消」「清空」都有宽高、都落在顶栏去掉内边距后的宽度以内，且两个按钮不重叠；再断言确认用的「清空」是 `KeyboardKeyRole.RETURN`、「取消」不是，只量几何时去掉强调键帽也会通过。无障碍节点的屏幕坐标会被父视图裁剪，按钮半截在屏外时看不出来，所以溢出只能在视图坐标里查。
- `check-host.sh` 守住：清空、面板渲染和条目操作路径里没有 `new AlertDialog`/`new PopupMenu`，`manageClipboardItem` 不再出现，捕获经过 `ClipboardCapturePolicy.captures`，`forgetCurrentClip` 跳过敏感内容。
- 双列末行使用 `android.widget.Space` 补空位，保留与卡片相同的列宽权重和 `MATCH_PARENT` 行高参数；真实卡片继续按同一行的最大内容高度对齐。普通 `View` 在 `AT_MOST` 下会占满可用高度，会把唯一卡片撑满列表视口，令 #9589 的长按操作行落到屏外；[`Space` 的测量实现](https://raw.githubusercontent.com/aosp-mirror/platform_frameworks_base/master/core/java/android/widget/Space.java)按最小内容高度测量，由真实卡片决定行高。卡片与操作行仍由原列表管理，全文预览不承担管理菜单的职责。

## Alternatives considered

- **给对话框设置输入法窗口的令牌**（`lp.token = keyboardRoot.getWindowToken()`，`TYPE_APPLICATION_ATTACHED_DIALOG`，AOSP LatinIME 的做法）— 改动最小，确认框还是系统样式；但对话框仍是可获得焦点的窗口，会把窗口焦点从编辑器拿走，WebView 一类的编辑器失焦就会收起键盘（见 #5653），用户要求的「清空时键盘不关闭」做不到。
- **弹出窗口设为不可获得焦点**（`PopupWindow.setFocusable(false)`，九键长按候选就是这样）— 不抢焦点，菜单仍浮在键盘上；但 `PopupMenu` 不暴露这个开关，要自己写一个菜单弹窗和它的定位、换肤、无障碍，面板里一行按钮更简单，也不会被键盘外框裁掉。
- **清空时连系统剪贴板一起清掉**（`ClipboardManager.clearPrimaryClip()`）— 补读自然没东西可读；但这会动到用户在别处还要粘贴的内容，而用户点的是「清空剪贴板历史」。
- **补读时按文字去重**（历史里已有就不加）— 刚清空时历史是空的，挡不住；身份必须记在历史之外。
- **打开长按菜单后自动滚动到底部**：能让屏外的按钮暂时可见，但唯一卡片仍被空位撑高，用户每次管理都要移动列表；修正补空位的测量能同时保留卡片尺寸和原地管理。

- **只把补空位高度设为零**：能避免它撑高列表，但水平 `LinearLayout` 里其他单元格仍为 `MATCH_PARENT`，整行被测为零高；API 28/35 的设备截图都显示条数为一、卡片却不可见。`Space` 保留整行等高参数，同时提供符合空位含义的内容测量。
- **把顶行在 `HorizontalScrollView` 里改成 `MATCH_PARENT` 高**：顶行量高变回 `EXACTLY 32px`，所有状态都不再依赖子视图参数；但普通状态的顶行也会跟着换测量方式，#10030 接着要改确认行的样式，这里只修出问题的那个子视图。
- **确认行留在横向滚动里，只给问句限宽或缩字**（#10030）：普通状态的顶行本来就要横滑，改动面最小；但 `HorizontalScrollView` 以 `UNSPECIFIED` 宽度量内容，带权重的问句拿不到「剩余宽度」这个约束，限宽要写死一个 dp 值，键盘宽度、按键字号一变就又溢出或留白。离开滚动容器后宽度由键盘给定，按钮在不在屏内不再取决于文字多长。
- **照报告者的草图做成系统弹窗，或在面板中部画一张卡片式确认框**（#10030）：系统弹窗即 `AlertDialog`，上面已经否掉（#5605、#5653）。面板中部的卡片更接近草图，但要盖在历史卡片上、自己管遮罩和无障碍焦点，iOS 和鸿蒙现在的「清空 → 确认清空」两段按钮也得跟着改；顶行确认本身没有错，看不见的原因是 0 高和横向溢出，修好这两处、再把确认按钮做成强调键帽，就能满足「提示明显」。

## Consequences

- **收益**：清空和长按操作都在键盘里完成，任何编辑器里键盘都不再被收起；清空和删除之后，系统剪贴板里剩下的那一条不会回来；键盘进程被回收后这份记忆仍在。
- 删除候选词的确认按同一条理由改成了长按菜单里的子菜单（[Android 删除候选词的子菜单确认](./2026-10-14-android-candidate-removal-submenu.md)）。
- **代价**：同一段文字再次复制时，只有复制时刻变了才算新的一条；系统给不出复制时刻（返回 0）的设备上，同样的文字清空后再复制，监听仍会记录，补读则不会。删除一条时系统剪贴板里若是别的内容，那一条也被记为已处理；面板开着就说明它在打开时已经补读过，不会因此漏记。
- **#10027 留下的横向溢出已由 #10030 处理**：原来 `HorizontalScrollView` 以 `UNSPECIFIED` 宽度量顶行，横向 `LinearLayout` 在这种约束下把宽 0 带权重的问句按 `WRAP_CONTENT` 量成整句宽；`fillViewport` 只在行比视口窄时才按视口宽重量。整行约 270dp，键盘比这窄或按键字号放大时整行溢出，每次重画的滚动位置又是 0，确认用的「清空」落在右侧屏外，要横滑才点得到。确认行离开横向滚动后不再有这个问题。
- **代价（#10030）**：极窄的键盘或很大的按键字号下，问句缩到 10sp 仍放不下时会被省略，用户看到的可能只剩「确认清空全部记录…」；按钮和「含固定项」的取舍里先保按钮。

## Verification

`platforms/android/tests/clipboard/ClipboardCapturePolicySmoke.java`；`bash platforms/android/check-host.sh` 编译服务和面板代码（`-Werror`）并执行源码守卫。真机上清空、删除、长按操作和重新打开面板的流程，以及 Via 浏览器里的表现，没有在设备上验证。

#9589 的旧宿主在 API 28/35 均复现：通过面板实际清空、系统剪贴板复制唯一合成短条，双列长按后全文预览可见、内联管理按钮不可见；同一夹具的单列短条与长条通过。`ClipboardSegmentDragDeviceSmoke -e first-record-only true` 覆盖清空后首次采集、单列与双列、短条与多行条目，以及展开不改写历史或上屏。

修复后 API 28/35 的首条记录专项均通过：单列、双列的短条与多行条目，以及双列复制第二条后的两种场景，共六组真实长按；两行操作可见，历史与编辑器不变，收起后浮层消失。夹具按报告的清空、复制、重新进入面板顺序同步，再取历史快照，避免把设置重载或补读未完成时的临时状态算成长按副作用；专项同时接入默认完整套件。两版完整 `ClipboardSegmentDragDeviceSmoke` 通过，包含全文浮层像素、滚动与生命周期、固定摘要、列表滚动、横滑删除、分词拖选、全文插入及编辑返回。原生 arm64 宿主在 JDK 21 下构建，五个原生库的依赖与 16 KiB 对齐通过；266 项 JVM 冒烟、240 份设备源码、JNI/资源契约和最终远端 `scripts/verify-local.sh --quick` 通过。该结果覆盖专用模拟器，报告的 vivo API 30 尚未真机验收。

#10027 的修复只在 macOS 上经 `check-host.sh` 编译了面板和设备套件；新加的尺寸断言没有在模拟器上跑过，报告的 vivo V2046A（Android 11）也没有真机复测。

#10030 的改动在 Mac Studio 上经 `check-host.sh` 编译了面板和设备套件（298 项 JVM 冒烟、255 份设备源码）；`ToolbarEdgesDeviceSmoke` 新加的确认态排版断言和 `ClipboardSegmentDragDeviceSmoke` 的新问句没有在模拟器上跑过，强调键帽在各皮肤下的样子、vivo V2046A 上确认行是否完整可见，都没有截图或真机确认。
