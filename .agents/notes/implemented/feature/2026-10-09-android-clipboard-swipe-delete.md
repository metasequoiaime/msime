# Agent Note: Android 剪贴板本机历史左滑删除

Status: implemented

## Problem

#5962：删除是剪贴板历史里最常用的操作，现在要先长按一条、再在操作行里点「删除」。用户希望像很多应用那样，对一条往左滑，露出垃圾桶，点一下就删掉。

## Decision

- 只对本机分段：Android 端没有删除云端条目的接口。两步完成：左滑露出垃圾桶，再点它才删；不做一滑到底直接删，也不做撤销（长按删除和清空也都没有撤销）。置顶的条目同样可以左滑删除，不要求先取消置顶。
- 每条本机历史放进一个 `FrameLayout`（`ImePanels.clipboardSwipeCell`）：底下靠右是画垃圾桶的 `KeyboardIconKey`（`Kind.TRASH`，图标是生成脚本里新加的 Lucide 风格 `TRASH`），上面盖着原来的卡片。卡片 `setTranslationX` 往左让开，露出它。删除走原有的 `MSIMEInputService.removeClipboardItem`，存储、`forgetCurrentClip` 和重绘都不另写。
- 手势判定在不依赖 Android 的 `ClipboardSwipePolicy`：
  - 手指越过系统 touch slop、并且横向位移至少是纵向的 1.5 倍时才算横滑（`claims`）；收着的卡片只认向左，打开着的左右都认，向右拖就是收回。
  - 跟手位移夹在 `[-reveal, 0]`（`offset`）；松手时横向速度达到 600 dp/s 就按甩动方向停，否则露出过半才停在打开（`settlesOpen`）。
  - 删除区宽 72 dp，双列等窄格里不超过格宽一半（`revealWidth`）。
- 和面板其他手势的分工（`ImePanels.bindClipboardSwipe`）：按下、没越过 slop 的移动、点按和长按都照常交给卡片自己（`KeyboardPressButton`）；判定为横滑的那一刻才接手，向上 `requestDisallowInterceptTouchEvent(true)`，让外层纵向 `ScrollView` 不再拦截，再给卡片补一个 `ACTION_CANCEL`，撤掉按下态、待触发的长按和点按，所以松手不会插入。斜着和竖着拖的，`ScrollView` 在越过 slop 时先拦截走，卡片收到取消。`check-host.sh` 守住这三点。
- 删除按钮只在卡片让开（拖动中或打开着）时显示，收着时是 `INVISIBLE`。第一版让它一直垫在卡片底下，模拟器上按住卡片时，按压反馈把卡片缩小一圈，删除按钮就从卡片右边露了出来。
- 卡片的位移动画用每一格自己的 `ObjectAnimator`（`ImePanels.ClipboardSwipeCell`），不用 `View.animate()`：卡片的按压反馈 `KeyboardPressFeedback` 也走 `animate()`，按下、松开时会 `cancel()` 它，共用一个动画器时两边互相打断，卡片会停在半路。没有拖起来的轻点不停掉正在进行的收回动画；真正接手横滑时才停，并从卡片当前位置开始跟手，越过 slop 时不跳。
- 同一时间只有一条打开，记在 `clipboardSwipedText`（共享存储以文字标识条目），渲染时按它把那张卡片直接画成打开的样子。开始滑另一条、点任何一张卡片（只收回，不插入）、长按、换分段、重开面板、点「清空」和进入分词都会收回；打开着的那条被别处删掉后也不再记着。
- 读屏：卡片描述是「点按插入剪贴板记录，左滑删除，长按管理」；垃圾桶收着时对读屏隐藏（它被卡片盖着），打开后才可读可点。长按操作行里的「删除」保留，读屏用户和不会滑的用户照样能删。

## Alternatives considered

- **`RecyclerView` 加 `ItemTouchHelper`** — 现成的滑动、回弹和无障碍动作；但面板是每次重建的 `LinearLayout`，`core/` 里的代码要过 `check-host.sh` 的 JVM 编译，那一步跳过任何引用 androidx 的源文件，换成 `RecyclerView` 等于重写整个面板。
- **一滑到底直接删**（Gmail 式） — 少点一下；但产品定为两步，误滑一下就丢一条历史，又没有撤销。
- **右滑固定、左滑删除**（iOS 设置 App 的做法） — 和 iOS 一致；但 issue 和产品取舍都只要左滑删除，固定仍在长按操作行里，多一个方向的手势也多一处要和纵向滚动分清的地方，以后需要时再加。
- **删除区写文字「删除」，不加图标** — 不用改生成脚本；但 issue 要的就是垃圾桶，键盘里的图标都来自同一个生成脚本，加一行即可，文字仍留作读屏的节点 text。

## Consequences

- **收益**：删一条只要一滑一点，不用先长按；纵向滚动、点按插入和长按管理的行为不变。
- **代价**：云端分段不能滑，两个分段的交互不一致；横向判定偏保守，很斜的滑动会被当成滚动。手势只能在 JVM 上验证判定逻辑，跟手、回弹和与 `ScrollView` 的配合要在设备上看。

## Verification

`platforms/android/tests/clipboard/ClipboardSwipePolicySmoke.java` 验证 slop 内不接手、纵向和斜向留给滚动、收着时向右不触发、打开时向右收回、位移夹紧、过半与甩动阈值、窄格限宽和垃圾桶图标；`bash platforms/android/check-host.sh` 编译面板代码、运行它并检查接手时的三个动作；`generate_keyboard_icons.py --check` 确认生成物与脚本一致。在 API 35 的专用模拟器（arm64，`start-emulator.sh 35`）上用 `adb shell input` 走过单列的情形：左滑露出垃圾桶、不插入；点打开着的卡片只收回；点垃圾桶删掉这一条，顶行计数随之从 2/50 变成 1/50；按住卡片时不再露出删除按钮。双列、读屏和真机的手感没有验证；那台模拟器在 CI 机上每帧要几百毫秒，`adb` 注入的长按时有时无，判断手感要在真机上看。
