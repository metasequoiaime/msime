# Agent Note: 九键滑动输入数字三端共享

Status: implemented

## Problem

#10229：拼音九键上想不切数字层就输入数字。2026-10-29 在真机上核对过三家竞品，都有这一项且都在九键上：微信「九宫格 - 上滑输入数字」，豆包「上滑输入数字符号」，千问「上滑输入数字和符号」（可选「数字和符号」或「仅数字」）。三家都只有上滑一个方向，默认都不开。

水杉的 Android 早就有同样的手势（#5580，[Android 九键滑动出数字](2026-10-08-android-nine-key-number-pad.md)）：本地设置 `platform.android.nine_key_swipe`，取值 `off` / `up` / `down`，判定在 `NineKeySwipePolicy.gesture`，另有本机的「九键滑动距离」（14–48 dp，默认 24 dp）。iOS 和鸿蒙没有这项手势，Android 的选择也不随账号同步，换设备要重新选。

## Decision

把它做成三端同一项共享偏好，iOS 和鸿蒙照 Android 的判定规则补上手势，Android 的旧本机值迁进共享偏好。

### 共享偏好是三值，不是布尔

`Preferences.touch_nine_key_swipe_digits: TouchNineKeySwipeDigits`（`crates/client-core/src/preferences.rs`），取值 `off` / `up` / `down`，默认 `off`，等于默认时不写进文档，默认文档逐字节不变。写法与 Android 旧本机值逐字相同。读文档是宽松的：不认识的字符串按 `off` 读，不是字符串照样报错，与 `touch_number_pad_space` 相同（[「符」与「123」互换、数字键盘空格位置](2026-10-29-touch-symbol-first-and-number-pad-space.md)是新增触屏偏好的模板）。各宿主读的时候缺键或写法不认识一律按 `off`：Android `NineKeySwipePolicy.mode`，iOS `NineKeySwipeDigits(in:)`，鸿蒙 `NineKeySwipePolicy.mode`，共享设置页 `nineKeySwipeDigits()`。

含义：`up` 是上滑输入数字、下滑弹出数字和字母；`down` 反过来；`off` 没有滑动，点按、长按照旧。竞品的那个开关就是这里的「上滑」。

### 同步

走触屏宿主共用的 `TouchHostKeys`：Android `platform.android.nine_key_swipe`，鸿蒙 `platform.harmony.nine_key_swipe`，值是字符串。同步键沿用旧本机键的名字，与 `touch_number_row` ↔ `platform.android.number_row` 的先例相同：旧本机键从来不在 `ANDROID_LOCAL_SETTINGS` 里、从没上传过，同名不会撞上。导出总是写这个键，`off` 也写，云端才能改回关闭；应用时按严格写法解析，不认识的写法只跳过这一个键。iOS 的同步走 `ios_settings.rs` 自有的映射，和其他触屏按键布局偏好一样这次不收录。服务端的账号字段表收录这两个键之前，上传前的字段表过滤会去掉它们，跨设备同步要等服务端加上才真正生效。

「九键滑动距离」`platform.android.nine_key_swipe_distance` 仍是 Android 本机设置，不进共享偏好、不同步；iOS 固定 24 pt，鸿蒙固定 24 vp。这是三端唯一的差异。

### Android 迁移

`KeyLayoutStyleMigration`（[按键布局样式](2026-10-27-touch-key-layout-styles.md)建立的迁移框架）多管一个旧键：本机有 `platform.android.nine_key_swipe` 时迁移照常执行，请求带上可选字段 `nine_key_swipe`，client-core 的 `migrate_android_key_layout` 在偏好锁里按原始文档判断：旧值是 `up` / `down`、共享字段缺键或明写 `off` 时写成旧值；共享里已经是别的设备选的方向、或是本版本不认识的写法时原样留着。迁移成功后删掉旧键，距离那个键不迁不删。恢复旧备份时，备份里明确存着的 `off` / `up` / `down` 转成共享编辑，同一备份里已有同步键时以同步键为准。旧键的规格声明留在 `AndroidLocalSettings`，否则读文件时会被丢掉、迁移读不到；除迁移类以外没有代码再读它，`check-host.sh` 钉住这一点。

### 键盘行为（三端相同）

只作用于全拼九键字母层网格里的 1–9 键（中文和英文模式都算，四种九键样式同一块网格）。数字层、九键双拼、笔画、日文和注音九键、别的键面借来的数字九宫格、底行和右列都不受影响。字母层没有 0 键，所以没有 0 的手势。

- 判定照 Android `NineKeySwipePolicy.gesture`：只看纵向位移，严格大于阈值；沿设定方向是输入数字，反方向且这个键有字母（2–9）是弹出和长按相同的数字与字母。1 键反方向什么也不做，抬手照旧是点按。一次触摸只判一次，判出就锁住；长按弹窗先弹出时不再判定，滑动先成立时取消这次的点按和长按。设置在每次触摸时读当前值，设置页改了下一次触摸就生效，不重建键盘。
- 输入数字那一刻键面换成这个键的数字、右上角数字角标藏起来，松手还原；上屏走各端「长按菜单里选数字」的同一条路径（组字中先结束组字再字面上屏），按键反馈和统计各一次、统计 id 与点按相同。系统取消时还原键面，什么也不输入。
- iOS 的判定在按键里（`KeyboardKeyButton` 的 `nineKeySwipe*`，规则在 `LetterHintTable.swift` 的 `NineKeySwipePolicy` / `NineKeySwipeTracker`），控制器的 `activeNineKeySwipeDigits` 在数字层、九键双拼、长按菜单已弹出、横滑移光标接管触摸（`cursorSwipeActive`）、VoiceOver 开着时返回关闭。鸿蒙在 `nineKeyHoldKey` 的 `onTouch` 里交给 `input/NineKeySwipePolicy.ts` 的 `NineKeySwipeGesture`，点击和长按先问它（`swallows` / `hold`），因为 ArkUI 不保证点击与抬手的先后；2in1 不判定。Android 的 `ImeLayoutRows.bindNineKeySwipe` 不变，只是方向改读服务上的 `nineKeySwipeDigits`。
- 反方向弹出的选项可以接着滑动选择（#10443，目前只有 Android）：报告者用上滑出字母，要先上滑、松手、再点一次，比 26 键「下滑弹出字母、接着滑到某个字母松手」多一步。Android 的 `bindNineKeySwipe` 判出 LETTERS 时以 `showNineKeyHoldOptions(..., trackDrag = true)` 弹出，弹窗登记到 26 键字母选项共用的拖选状态（`trackLetterOptions`，规则见 [字母选项支持滑动选择](../bug-fix/2026-10-21-android-letter-option-gesture-settings.md)），之后的移动交给 `moveLetterOptions`，抬手交给 `finishLetterOptions`，系统取消交给 `cancelLetterOptions`。滑进过某个选项再松手就输入它，滑进过又滑出再松手关闭弹窗，一次也没滑进过就松手则弹窗留着等点选，原来的点选不丢。九键长按弹出的同一组选项也跟着这次按压拖选，与 26 键长按一致；长按后拖选成功时给按钮补一个 CANCEL，收起它的按下态。新按压按下时，若上一次滑出后没选的弹窗还登记在这个键上，先关掉它，不让旧弹窗跟新按压拖选。14 键、18 键和上标弹窗仍不拖选。iOS 和鸿蒙的九键反方向弹窗还是松手再点，补齐留作后续。
- 26 键「滑动输入符号」的开关、方向和 14 dp/pt/vp 阈值与这一项互不影响；空格移光标、文本触控板都不在九宫格键上，不受影响。

### 角标

九键字母层 1–9 的右上角数字角标三端本来就一直画着（长按就能输入），开关打开或关闭都不改角标的显隐。issue 里「打开后在键上显示数字提示」已经由现有角标满足，不另加。

### 设置放在「手势」组，不进「按键布局」页

三端都在「键盘 › 手势」（鸿蒙是共享设置页「屏幕键盘 › 手势」）里，紧跟 26 键的「滑动方向」那一行；只在本版本提供拼音九键时列出。文字三端逐字相同、各端测试钉住：标题和选项单标题「九键滑动输入数字」，说明「沿所选方向滑动输入数字，反方向弹出字母。」，选项依次「关闭」「上滑」「下滑」。点哪一项写哪一项的写法（含 `off`），写共享偏好，不写本机键。它不跟着 26 键开关置灰；Android 的距离滑块在共享值为 `off` 时置灰。

「按键布局」页（[改版后的结构](2026-10-29-key-layout-page-redesign.md)）三端都不改：那一页是「整块预览 + 高亮与标准不同的键 + 样式胶囊 + 当前样式说明」，这一项不挪动任何键、不改任何键面，预览里没有可高亮的键，放进 9 键分段只会多一个和预览无关的控件。

## Alternatives considered

- **共享布尔（issue 和竞品的写法）**：最贴近竞品，设置页是一个开关，iOS、鸿蒙只实现上滑。不采用：Android 现有用户里选了「下滑」的，迁移后要么方向被反过来，要么这项选择直接丢掉，是对现有用户的破坏性改动；三值的写法与旧值逐字相同，迁移就是原样拷贝。iOS、鸿蒙多一个方向的代价很小，判定对两个方向对称，反方向复用已有的长按弹窗。
- **放进「按键布局」页的 9 键分段**：issue 讨论时提过，九键相关的设置集中一处。不采用：理由见上一节；另外 Android 用户今天就在「手势」组找它，设置搜索也已收录这个标题，挪走反而找不到。
- **距离也进共享偏好**：三端手感一致，换设备不用重调。不采用：iOS 和鸿蒙没有对应的滑块，为一个只有 Android 有界面的数值加共享字段和同步键，还要定 pt / vp / dp 的换算；保持 Android 本机设置，iOS 和鸿蒙固定等于 Android 默认的 24。
- **只在 Android 保持本机、iOS 和鸿蒙各自新加本机设置**：不用动 Rust 和迁移。不采用：三端各存一份，换设备、换平台都要重选，也和其他触屏按键偏好走共享偏好的方向相反。

## Consequences

- 收益：三端同一项设置、同一套手势，Android 老用户的选择原样保留并开始随账号同步（等服务端字段表收录后）。默认关，老用户的点按不变。
- 代价：手势判定在三端各有一份（Java、Swift、TS），规则靠各端测试逐条对齐，以后改规则要三处一起改。iOS 为了不碰提交着的 pbxproj，没有加新的 Swift 文件，偏好放在 `KeyboardLayoutPreference.swift`、判定放在 `LetterHintTable.swift`、用例放在 `NineKeyKeyboardTests.swift`。
- 已知差异：Android 的阈值可调（14–48 dp），iOS / 鸿蒙固定 24；iOS 在 VoiceOver 下不判定，Android 和鸿蒙没有单独的读屏门槛（读屏的触摸浏览不送移动事件，实际也判不出）。鸿蒙 2in1 没有这一行，从手机同步来的值在 2in1 上不起作用。
- iOS 不同步这一项；iOS 的「重置所有设置」会删掉 App Group 镜像 `keyboard.nineKeySwipeDigits`，共享字段由 `restoreDefaults` 恢复。共享设置页「屏幕键盘」的「恢复默认」不动这一项。

## Verification

集成时（2026-10-29，工作树 `feat/issue-10229-nine-key-swipe-digits`）在本机跑过：

- `ANDROID_SDK_ROOT=~/Library/Android/sdk bash platforms/android/check-host.sh`：通过，含 `NineKeySwipePolicySmoke`、`TouchKeyStyleSmoke` 和新加的旧键 / 接线检查。
- `bash platforms/harmony/tests/run.sh`：通过，`nine-key-swipe.test.ts` 15 组全过，新加的接线检查通过。
- `apps/desktop` 下 `npx vitest run tests/settings`：477 个文件、3086 项通过。
- `scripts/test-ios-*.py` 与 `platforms/ios/tests/settings/ProjectConfigurationTests.py`、`platforms/ios/tests/TestFlightSubmissionTests.py` 共 44 个脚本全部通过（`test-ios-key-layout-settings.py` 21 项）。
- `rbuild cargo test -p msime-client-core -p msime-client-services nine_key_swipe`：新增的 4 项（默认与往返、线上写法与不认识的值、迁移的各种组合、同步键）通过，两个 crate 的测试目标都编译通过；`rbuild cargo test -p msime-host-api migrate`：C ABI 迁移 1 项通过。没有跑这三个 crate 的全量测试，也没有跑 clippy。
- `pnpm run -s verify-notes`：通过。

各宿主实现时（同一天，结果引自各端的实现记录，集成时没有重跑）：

- Android：`platforms/android/tests/device/build-editor.sh` 通过；Mac Studio 上 Gradle `:app:compileFullDebugJavaWithJavac` 通过。改过的两个设备测试（`KeyLayoutBackupDeviceChecks`、`KeyboardFeedbackDeviceChecks`）只编译过。
- iOS：Mac Studio 上 `MSIMEApp` build 与 `MSIMEClientTests` build-for-testing 通过；`NineKeySwipeDigitsTests` 只编译，没有在模拟器上跑。
- 鸿蒙：Mac Studio 上 hvigor `assembleHap` 通过（ArkTS 编译通过；原生库借自旧镜像，包本身不可安装）；`apps/desktop` 下 `tsc --noEmit`、改动文件的 `vp fmt --check` 与 `vp lint` 通过。

#10443（2026-11-01，Android 拖选）：Mac Studio 上 `platforms/android/check-host.sh` 通过，含新加的拖选接线检查。没有在真机或模拟器上试过上滑和下滑两种设置下的拖选、松手不选留弹窗、滑出关闭、长按后拖选和系统取消。

没有验证的：三端都没有在真机或模拟器上试过真实滑动、键面换数字和还原、按下态、与长按的先后、iOS 横滑光标接管后的取消；三端设置页都没有在设备上看过；Android 迁移没有在装着旧本机值的真机上升级验证；跨设备同步要等服务端字段表收录 `platform.android.nine_key_swipe` 和 `platform.harmony.nine_key_swipe`。
