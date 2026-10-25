# Agent Note: HarmonyOS 双拼 14/18 键

Status: implemented

## Problem

HarmonyOS 已经支持全拼 14 键和 18 键，共享引擎也支持双拼的字母组码网格，但宿主只在 `quanpin` 下打开网格。设置页因此只显示全拼的 14/18 键，双拼用户无法使用窄屏组码键面。

## Decision

HarmonyOS 手机和平板沿用共享偏好 `touch_shuangpin_layout`，提供双拼 26/14/18 键布局。设置页切换这个布局只写 `touch_shuangpin_layout`，保留当前 `scheme`、`shuangpin_profile` 和启用列表；切换双拼方案时布局也保持不变。双拼 14 键使用 C ABI 网格值 `2`，双拼 18 键使用网格值 `5`，全拼 18 键继续使用网格值 `4`。

宿主会话从偏好读取双拼布局，创建、重建和获得焦点时重新设置网格；`KeyboardView` 对双拼复用现有 `FourteenKeyLayout` / `EighteenKeyLayout`，并在 L 键显示 `L;`，让微软双拼保留分号入口。英文、密码框、网址框、本地模式和 2in1 继续画 26 键，统计仍按当前双拼 profile 记录。

设置预览为双拼 14/18 键绘制独立的 `L;` 键面。全拼、双拼以外的方案不继承拼音组码网格。

双拼 18 键两个字母的键（`WE RT IO SD FG JK XC BN`）支持左右横滑限定字母（#8930），规则与 [Android](2026-10-24-eighteen-key-exact-swipe.md) 和 [iOS](2026-10-25-ios-eighteen-key.md) 相同：横向位移超过 24 vp 且大于纵向 1.5 倍，左滑取左字母、右滑取右字母，以大写精确标记交给 `pressGridKey`，引擎只在网格 5 把它当作这一组里的那个字母。24 vp 与 Android 的 24 dp、iOS 的 24 pt 是同一物理尺寸，换算方式与 `SwipeHintPolicy` 把 Android 的 14 dp 写成 14 vp 相同。判定放在纯 TypeScript 的 `input/EighteenKeySwipePolicy.ts`，按键字母分别跟踪按压，两个拇指按两个键互不影响；`KeyboardView` 只把这些键的 `onTouch`、`onClick` 和长按喂给它。拖动时键面改画选中的字母，用 `KeyOptions.swipeLetters` 在绘制时现算，不进 `ForEach` 的键标识，正按着的键不会被重建。越过阈值后这次按压不再是点按：回到起点、同一键上的第二根手指或系统取消都不输入，反向拖过另一侧可以重选；长按先生效时不再横滑，横滑先接管时长按不再弹出字母。抬手那一点也判一次，快速一甩只在抬手时过阈值的同样取那个字母。

ArkUI 对同一个键的 `onClick` 与抬手 `onTouch` 谁先到达没有契约，所以点击带着自己的窗口坐标问一次策略：先到的一方按抬手位置作出判定，后到的一方只照这个判定行事，同一次按压不会既输入字母又输入整组。全拼 18 键、14 键和单字母键（含 `L;`）只点按。18 键键面没有滑行输入、角标滑动或光标横滑，横滑不与它们争抢；统计与点按、长按记在同一个 `Eighteen*` 键位上。

本决定扩展了[全拼 14 键](2026-10-10-fourteen-key.md)中“只给全拼”的 HarmonyOS 范围；Android 和 iOS 的独立实现分别见[Android 14 键双拼](2026-10-23-android-fourteen-key-shuangpin.md)和[iOS 14 键双拼](2026-10-24-ios-fourteen-key-shuangpin.md)。

## Alternatives considered

- **继续只支持全拼。** 改动最少，但共享引擎和 Android/iOS 已有双拼组码能力，HarmonyOS 会继续出现同一套设置在不同移动宿主缺失的缺口，因此不采用。
- **把双拼 14/18 键写进 `touch_keyboard_layout`。** 这样能复用全拼方案选择器，但会把“当前双拼方案”和“键盘几何”绑定到同一字段，切换小鹤、自然码或微软时会破坏用户的布局选择，也无法与 Android/iOS 的共享偏好契约对齐，因此不采用。
- **横滑也给全拼 18 键。** 键面完全相同，可以少一个判断；但全拼网格 4 不接受大写精确标记，Android 和 iOS 的全拼 18 键也只点按，因此不采用。
- **只在抬手 `onTouch` 里判定，点击一律送整组或一律吞掉。** 写法最简单，但 ArkUI 不保证点击与抬手的先后：点击先到时要么多送一次整组，要么把真正的点按吞掉；让点击按自己的坐标参与同一次判定，两种顺序结果一致。
- **拖动时用 `ForEach` 键标识或重新构建键面显示选中字母。** 能直接复用现有 `KeyOptions.face`，但会把手指正按着的键换掉，ArkUI 不再把这根手指的后续事件送给新键，横滑就丢了，因此改为在绘制时现算。
- **双拼 18 键复用全拼网格值 `4`。** 可以少加一个常量，但引擎用网格值区分全拼和双拼 18 键；复用会把双拼组码按全拼解码，因此使用 C ABI 规定的值 `5`。

## Consequences

双拼用户在鸿蒙上可以独立选择 14/18 键，并在切换双拼 profile、重启会话和回到中文时保留选择。键表、长按和候选交互继续与全拼键面共享，减少了重复实现；代价是 HarmonyOS 需要同时维护共享偏好字段、宿主网格门控和设置预览三处投影。旧版鸿蒙客户端不认识新偏好值时会按其既有兼容策略回落到 26 键，当前版本的本地选择不会改变其他方案或桌面键盘。

横滑让鸿蒙与 Android、iOS 在 #8930 上一致，双拼 18 键可以不经长按弹窗直接限定字母，也不结束当前组字。代价是两个字母的键多了一层触摸状态：点击路径要先问 `EighteenKeySwipeGesture`，以后给这些键加别的手势时要一并考虑点击与抬手的先后。逻辑测试只驱动纯策略，ArkUI 真实事件的分发顺序、手感和系统边缘手势的冲突仍需设备确认。

## Verification

- `bash platforms/harmony/tests/run.sh`
- `pnpm --filter @msime/harmony typecheck`
- `pnpm --filter @msime/desktop typecheck`
- 真机签名 HAP 安装到 HarmonyOS 设备，切换双拼 14 键和 18 键，确认键面与日志中的会话网格加载。
- 横滑：`tests/keyboard-logic.test.ts` 的「双拼 18 键横滑限定组里的一个字母」覆盖阈值、方向、纵向比例、无效坐标、只有八个双字母键接手、点击与抬手两种到达顺序、抬手才过阈值、回到起点、两指、两个拇指各按一键、系统取消和长按互斥；`python3 scripts/test-harmony-eighteen-key-table.py`、`hvigorw assembleHap` 通过。真机横滑未验证：设备当时正被另一个会话操作，未做真实触摸。
