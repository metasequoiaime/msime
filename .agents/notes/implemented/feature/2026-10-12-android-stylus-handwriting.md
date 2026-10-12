# Agent Note: 安卓用手写笔直接在输入框里写字

Status: implemented

## Problem

#6792 第四点：安卓上拿手写笔没法「像 Gboard 一样」直接在应用的输入框上写字。键盘里的手写面板能用笔写，但要先弹出键盘、切到手写方案。Android 14 起系统给输入法开了手写笔接口：输入法在 `method.xml` 里声明 `android:supportsStylusHandwriting`，系统在用户拿笔在输入框上写字时调 `onStartStylusHandwriting` 问要不要接，接了以后笔的事件全部交给输入法的书写窗口。#7080 把这一点拆出来，仓库负责人决定实现。

## Decision

- **声明**：full、拼音、五笔版的 `method.xml`，以及 Tauri 合包的 `tauri_method.xml`，声明 `supportsStylusHandwriting="true"`。日文、越南文、藏文版没有中文手写模型，不声明。`scripts/test-editions.py` 要求两个包里每个版本的声明都等于版本表的 `features.handwriting`。
- **接不接**（`StylusHandwritingPolicy.start`）：以下情况不接，笔照常交给应用：
  - API 34 以下；
  - 本版本没有手写；
  - 不走引擎的输入框（密码、数字）；
  - 不在输入中文：专用英文、日韩越藏方案、本地模式。

  模型就绪或还在检查时接下。模型没下载、或构建里没有识别器时，接下这一次只显示提示；同一个输入框里提示过一次之后就不再接。
- **书写**：
  - 键盘不弹出。笔迹画在系统书写窗口里铺满屏幕的透明 `StylusInkView` 上。
  - 笔的事件由服务的 `onStylusHandwritingMotionEvent` 直接交给 `StylusInkView.handle`，按 `getRawX/Y` 记成屏幕坐标，书写区域取屏幕大小（`getMaximumWindowMetrics`）。仍调用 `super`，保留系统「闲置五分钟回收书写窗口」的计时；系统派发到视图的那份，视图的 `onTouchEvent` 不处理。
  - 接下时把书写窗口交给 WindowManager 铺满屏幕（`ImeStylusHandwriting.fillScreen`）。
  - 停笔后按手写面板的识别等待时间（服务防抖 550 ms 与设置取大者，满 64 笔立即识别）整行识别，光标前的上文交给识别器。
  - 识别结果直接上屏第一候选。有组字时先按首选结束组字（命令 9），再经简繁转换，以 `TypingSource.HANDWRITING` 上屏。
- **识别请求**（`StylusHandwritingPolicy.request`）：笔迹裁到外接框，四周留 8 dp，宽高至少 64 dp，笔迹居中；超过 4096 时等比缩小。
- **会话**：
  - `setStylusHandwritingSessionTimeout` 设为 3 秒，笔停 3 秒系统结束会话，笔回到应用。
  - 会话结束时还没识别的笔迹照常识别上屏。新会话接着写的笔会改变笔迹的 revision，迟到的结果因此被拒，旧笔迹和新笔迹在下一次停笔时一起识别。
  - 离开输入框（`onFinishInput`）时作废排着的识别，丢掉没上屏的笔迹。
- **识别器**：`ImeStylusHandwriting` 持有自己的 ML Kit 识别器，不和手写面板共用。面板的识别器跟着面板建和关，这里的在 `onPrepareStylusHandwriting`（笔悬停，或系统问要不要接之前）时建，`onStartStylusHandwriting` 里只做判断，不让建 ML Kit 识别器拖慢给系统的答复；进程退出时关。模型没下载、检查失败时重建一个重新查，用户可能刚在面板里下载完。这里从不下载模型。
- **模型没就绪的提示**：说法与手写面板的状态行一致（「首次下载后可离线手写：请在键盘的手写面板里下载中文手写模型」「此构建不含手写识别」）。提示画在落笔处上方，显示时把会话的闲置计时重新算，2.5 秒后由输入法结束会话。
- **模型检查迟迟不返回**：到了该识别时识别器还在检查，最多再等 `CHECK_TIMEOUT_MILLIS`（1 秒），之后丢掉笔迹，提示「正在检查中文手写模型，请稍后再写」。这条不算这个输入框提示过，查完以后照常能写。
- **笔迹颜色**：用手写设置的颜色和粗细。「跟随皮肤」时取皮肤的强调色，不取面板用的键帽文字色。

## 系统默认行为里的两个坑

两者都是在 API 35 arm64 模拟器上看到的，对照 AOSP `android14-release` 的 `InputMethodService`、`InkWindow` 源码确认。

- **书写窗口不是全屏。** `InkWindow` 构造时 `setLayout(MATCH_PARENT, MATCH_PARENT)`，但它用输入法的 context 建，主题是 `Theme.DeviceDefault.InputMethod`（浮动窗口）。`PhoneWindow` 装 decor 时按 `windowIsFloating` 又 `setLayout(WRAP_CONTENT, WRAP_CONTENT)`，`dumpsys window` 里是 `wrapxwrap`、frame `[120,0][960,2400]`，两边的笔迹和提示被裁掉。decor 在系统调 `onStartStylusHandwriting` 之前就已经 `addView`，所以改尺寸后要 `updateViewLayout`。判断 decor 加没加用 `getParent() != null`：`isAttachedToWindow()` 要等第一次 traversal，第一次书写时还是 false，跳过更新后本地属性已是铺满，以后就再也不更新了，最早的实现就是这样只铺满了一半。
- **默认派发会丢事件。** 默认的 `onStylusHandwritingMotionEvent` 在视图「对用户可见」之前把事件放进环形缓冲，挂一个 global layout 监听，可见后再补发。同一个书写窗口第二次书写时，服务收到了全部事件，视图却一个也没收到；第一次补发时视图收到的是成串的 DOWN、CANCEL。直接交给墨迹模型后，每次书写都完整。

## Alternatives considered

- **模型没就绪时直接不接，用 Toast 提示。** 最省事，笔完全不受影响。不用的原因：键盘没弹出时输入法算后台，Android 13 起没有通知权限的应用从后台发的 Toast 会被系统吞掉（`MSIMEInputService.notice` 的注释记了这件事），而这个功能只在 Android 14 以上出现。用户写了字却什么也没发生，也不知道为什么。
- **在书写窗口里放一个「下载」按钮，就地下载模型。** 少走一步。不用的原因：书写窗口是 `FLAG_NOT_TOUCHABLE`，手指点不到，只有笔能点；下载要走网络、要用户明确同意，面板里已经有带进度和失败重试的完整流程，这里再做一份就要维护两份。
- **识别后在候选栏里列出候选让用户选。** 和手写面板一致，写错了能挑。不用的原因：系统开始书写会话时把 `mShowInputRequested` 置为 false，键盘不会弹出，没有候选栏可放；为它把键盘叫出来又会顶起应用、挡住正在写的那一行。Gboard 也是直接上屏。
- **和手写面板共用一个识别器。** 少占一份模型内存。不用的原因：面板的识别器随面板的建立和销毁开关，手写笔书写往往发生在键盘根本没弹出的时候，共用就得把面板的生命周期改成常驻，牵动面板现有的过期结果判定。
- **用系统默认的 10 秒会话超时。** 不用设置。不用的原因：会话期间笔的事件全被输入法拿走，写完后 10 秒内拿笔点不了应用里的任何东西。
- **沿用系统默认的事件派发，在视图上画。** 文档说这是默认路径，代码最少。不用的原因见上一节，第二次书写整笔丢失。
- **把整块屏幕当书写区域交给识别器。** 不用换算。不用的原因：ML Kit 按书写区域估计字的相对大小，一行字放在一张屏幕大小的纸的角落里，识别器会当成很小的涂鸦。

## Consequences

- **收益**：
  - 有手写笔的 Android 14 平板和手机，在中文输入框里能直接写字上屏，不用先弹键盘、切手写。
  - 规则集中在纯策略类里，有 JVM 冒烟。
  - 版本声明由 `test-editions.py` 钉住，新加一个没有手写的版本时不会误声明。
- **代价**：
  - 只上屏第一候选，写错了只能删掉重写，没有候选可挑。
  - 手写面板和书写会话同时活着时进程里有两个识别器。
  - 没有手写笔手势（划掉删除、插入空格等）。
  - 用户不想要这个功能时，只能关系统设置里的手写笔书写开关，输入法自己没有开关。
- **平台对等**：iPadOS 的 Apple Pencil 随手写由系统处理，不经过键盘扩展，iOS 不需要对应改动。鸿蒙有没有对应的系统接口还没查。

## Verification

- `platforms/android/tests/handwriting/StylusHandwritingPolicySmoke.java`（`check-host.sh` 自动发现）覆盖以下几项：
  - 各种条件下接、提示、不接；
  - 识别等待时间；
  - 三个时限之间的关系：识别等待加检查上限短于会话超时，提示时长短于会话超时；
  - 笔迹到请求的裁剪、居中、最小边、等比缩小，以及笔画顺序和时间戳。
- `scripts/test-editions.py` 核对 12 个输入法声明：去掉五笔版的声明、给日文版的 Tauri 声明加上，都会报错。
- `check-host.sh` 用 `-Xlint:all -Werror` 编译服务与新类；Gradle 编译 full、五笔、日文版的 Java 并跑 `lintFullRelease`，都通过。
- lint 兜不住这里的版本判断：去掉 `ImeStylusHandwriting.start()` 开头的 `SDK_INT` 判断后 `lintFullRelease` 照样通过，它不对经子类实例调用、继承自 `InputMethodService` 的 `getStylusHandwritingWindow()`、`setStylusHandwritingSessionTimeout()`（API 34）报 NewApi；同一位置放一个 `Path.of` 则会被拦下。所以 `start()` 和 `showHint()` 里的版本判断是手写维护的：系统在 Android 13 上也会调 `onStartStylusHandwriting`，而 `setStylusHandwritingSessionTimeout` 在那里不存在。
- 专用 API 35 arm64 AVD（在 Mac Studio 上无窗口运行，APK 由 `build-apk.sh` 构建、开发密钥签名）：
  - `smoke.sh` 基础套件全部通过。
  - `dumpsys input_method` 显示 `mSupportsStylusHandwriting=true`。
  - 用 `adb shell input stylus swipe` 在合成编辑器的普通输入框上划一横，系统发起书写会话：笔迹从 x=100 画到 x=1000（整宽）；模型还在检查时约 1 秒后换成「正在检查中文手写模型，请稍后再写」，提示完整显示在左边距内；输入框文字不变；3 秒闲置后系统结束会话。
  - 窗口在 `dumpsys window` 里是 `fillxfill`。
  - 同一个输入框里间隔 10 秒连写三笔，前两笔各自发起会话、画出笔迹并出提示。
- 没有验证到的：
  - 模型就绪后识别并上屏第一候选这条主路径。这台模拟器上 ML Kit 的模型检查几十秒不返回，`smoke.sh --handwriting` 里手写面板自己的可用性检查也在 30 秒时超时，模型一直没能下载下来。
  - 注入的手写笔事件偶尔不会让系统发起会话：
    - 有时系统报 `HandwritingModeController: Cannot start handwriting session: No stylus gesture is being recorded`，即输入法答复时那一笔已经结束。注入的一笔只有 400 ms，这台模拟器当时每帧要 0.7–1.7 秒。
    - 有时日志里什么也没有。
  - 重装输入法后，编辑器应用要重启才会再发起会话，看起来是应用进程里缓存的输入法信息没有刷新。真机上换输入法版本后是否也这样，没有验证。
  - 需要一台 Android 14 以上、带主动式手写笔的真机验收。
