# Agent Note: Android「防止截屏」开关与认证规则

Status: implemented

## Problem

#10098：设备离手时，别人拿起已解锁的手机可以截屏或录下设置应用里的剪贴板历史搜索页，或者在任意应用里弹出键盘、打开剪贴板面板后截屏，把本机和云端剪贴板内容带走。原来设置应用的 Activity 和输入法窗口都没有任何截屏保护；现有的剪贴板操作认证（[剪贴板操作认证](2026-10-25-android-clipboard-operations-auth.md)）只管删改、导出，管不到截屏。

用户要求：设置里加一个默认关闭的开关；第一次打开不需要验证，打开过以后再开或再关都要系统级验证，只有在系统设置里清除应用数据才能绕过；打开后设置应用的任何界面都不能截屏、录屏，其他应用里弹出的键盘截出来是黑的。

## Decision

只做 Android。隐私页「本机」分类下新增「截屏与录屏」卡片和「防止截屏」开关，默认关闭。

存储是两项 Android 本地设置（`AndroidLocalSettings`），都不随账号同步、不进本地备份：

- `platform.android.screen_capture_protection`：开关本身。
- `platform.android.screen_capture_protection_enabled_before`：是否打开过。第一次打开时与开关同一次写入，之后关闭也不清除。

切换规则在 `ScreenCaptureProtectionPolicy.decide`：值不变什么也不做；设备没有可用的系统认证（`BiometricManager.canAuthenticate(SystemAuthenticators.allowed())` 不成功）时一律拒绝，第一次打开也拒绝，否则打开以后就再也关不掉；从未打开过时的第一次打开直接保存；其余（打开过以后的开和关）先过 `BiometricPrompt`，认证方式与剪贴板历史、重置设置相同（强生物识别或锁屏凭据，Android 9、10 上为弱生物识别或锁屏凭据）。认证取消、失败时开关回到原值。开着却没有「打开过」标记的状态只可能来自被改过的文件，按打开过处理。

窗口效果由 `ScreenCaptureProtection` 落到 `FLAG_SECURE` 上。这个标志只作用于设了它的那一个窗口，所以每类窗口各自处理：

- 设置应用的全部 9 个 Activity 在 `onCreate` 里调用 `ScreenCaptureProtection.apply(this)`，窗口显示前就带上标志；`HomeActivity` 在 `onResume` 里再读一次；隐私页写入成功后当场改所在 Activity 的窗口（写完由工作线程直接投到主线程，不经页面视图的结果回调：那个回调在用户退出隐私页后会被丢掉，而写入可能排在共用工作线程上晚于退出完成，曾导致文件写成「开着」、其余设置页仍可截屏到下次 `onResume`）；认证过的「重置所有设置」把它和其他本地设置一起恢复为关闭，开发者页当场去掉窗口标志。
- 设置应用里的对话框和底部面板是独立窗口，不继承 Activity 的标志。全部 `MaterialAlertDialogBuilder`、`BottomSheetDialog` 以及 `InputDialog` 内部的 `AppCompatDialog` 改为经 `ScreenCaptureProtection.show` / `protect` 在显示前设好。`InputDialog` 是 AI 凭据、WebDAV 账号和应用密码、自建中继令牌、语音服务密钥、常用语正文、QQ 绑定码共用的输入框，第一版只数了 `MaterialAlertDialogBuilder` 和 `BottomSheetDialog`，漏了它，评审时补上。
- 输入法窗口在 `MSIMEInputService.applyLocalSettings` 里跟随设置，`onStartInputView` 每次重读本地设置，所以在设置应用里切换后，下一次弹出键盘就生效。
- 输入法自己开的三个 `PopupWindow`（剪贴板全文预览、九键长按选项、文本触控板放大镜）没有设置窗口标志的公开接口，显示之后立即改根视图的 `WindowManager.LayoutParams` 再 `updateViewLayout`；第一次布局是排到下一帧执行的，所以第一帧已经带着标志。常用语操作提示（`ImeOperationNotice` 在 Android 11 起单开的子窗口）在 `addView` 前直接带上标志。

`platforms/android/check-host.sh` 钉住这些接线：每个 Activity 都调用 `apply(this)`；对话框和底部面板的创建处（按出现次数数，类型包括 `MaterialAlertDialogBuilder`、`AlertDialog.Builder`、`Dialog`、`AlertDialog`、`AppCompatDialog`、`BottomSheetDialog`；`InputDialog` 这类自家包装不是窗口，数的是它内部那一处 `new`）都经过保护；创建 `PopupWindow` 的文件都在显示后处理；带 `TYPE_APPLICATION_ATTACHED_DIALOG` 的子窗口都经 `ScreenCaptureProtectionPolicy.windowFlags`；`applyLocalSettings` 处理输入法窗口。新加的窗口漏掉这一步时检查会失败。

本地备份恢复不会改写这两项：`LocalBackupPolicy.backsUpLocalSetting` 对它们返回 false，导出时不写进包，恢复时即使包里有也跳过。理由是恢复不经认证，一份关着的旧备份会悄悄关掉保护，一份改过的备份能抹掉「打开过」。

## 不覆盖的窗口与已知效果

- 输入法和设置页里用 `PopupMenu` 弹出的菜单（候选菜单、常用语菜单、世界时钟、词库管理等）拿不到它的窗口，没有单独设标志。它们是挂在输入法窗口或 Activity 窗口下的面板子窗口；据 AOSP SurfaceFlinger 的 `Layer::isSecure` 实现，子图层沿父图层继承保护，截图时同样涂黑。这一点没有在设备上验证，Android 9 的行为也不确定。
- Android 10 及以下的常用语操作提示用系统 `Toast` 显示，`MsToast` 也是系统 `Toast`，这两种窗口归系统所有，不能设标志；它们只显示操作结果，不显示剪贴板正文。
- 手写笔书写窗口（`ImeStylusHandwriting`）铺满整个屏幕，加上标志会让开着手写时整屏截图全黑，这次没有处理。
- 输入法窗口在 Android 9、浮动键盘、跟随光标的候选栏以及剪贴板全文预览展开时会铺满屏幕（`applyFloatingLayout`），这些情况下截图里被涂黑的是整个输入法窗口，也就是几乎整屏，而不只是键盘那一块。

## iOS 与 HarmonyOS

iOS 没有公开接口阻止系统截屏或录屏，键盘扩展同样不行，不做。

HarmonyOS 有接口：`inputMethodEngine.Panel.setPrivacyMode`（API 11）和 `window.setWindowPrivacyMode`，需要 `ohos.permission.PRIVACY_WINDOW`，SDK 里这个权限是 `normal` 级、`system_grant`，不是高风险权限。这次没有做：鸿蒙的设置界面是共享 React 设置页，开关和认证要经 ArkTS 原生桥接 `userAuth`，设置要放进键盘扩展能读到的本机存储，还有软键盘、浮动工具条和输入模式提示三块面板要分别处理；这些只能做类型检查，HAP 编译和设备验证都做不了。#10194 跟进了鸿蒙，做法和与本篇的不同见 [HarmonyOS 防止截屏](2026-10-29-harmony-screen-capture-protection.md)。

## Alternatives considered

- 只用一个开关、按「当前开着」决定要不要认证（与剪贴板历史相同）：实现最简单，但关掉以后再打开就不用认证了，不符合「打开过以后再开或再关都要验证」的要求。「再打开」本身不会泄露内容，但用户明确要求它也要认证，所以多记一项「打开过」。
- 用 `Application.ActivityLifecycleCallbacks` 统一给所有 Activity 设标志：新 Activity 不会漏，但仓库没有 `Application` 子类，为此新增一个并改清单，影响面比在 9 个 `onCreate` 各加一行大；漏加的风险改由 check-host 的检查兜住。
- 让「防止截屏」进本地备份、恢复时若会关闭保护就要求认证：能让换机时带走这项设置，但恢复流程要在中途插入认证，失败时还要和已经写入的其他设置一起撤销；这项设置本来就该在每台设备上由用户自己打开，所以直接不进备份。
- 设备没有系统认证时也允许第一次打开：最强理由是打开保护本身没有风险；但打开以后就只能清除应用数据才能关掉，用户会被困住，所以没有系统认证时一律拒绝，并提示先设置锁屏密码。
- 「重置所有设置」保留这两项：更接近「只有清除数据才能绕过」的字面意思；但重置本身已经要求系统认证，经过认证后回到出厂值与用户的验证要求一致，而重置的语义是所有设置恢复出厂，所以不特殊处理。

## Consequences

收益：打开后设置应用的页面、对话框、底部面板，以及弹出的键盘和它自己开的弹窗在系统截屏、录屏、投屏和最近任务缩略图里都是黑的；切换受系统认证保护，云同步、备份恢复和键盘进程都改不了它。

代价：设置应用里新增或改动对话框、弹窗时必须经 `ScreenCaptureProtection`，否则 check-host 失败；打开后用户自己也无法截屏反馈问题，需要先关掉（要认证）。全屏的输入法窗口在截图里整块变黑，见上一节。

验证：`compileFullDebugJavaWithJavac`（JDK 21）成功；`platforms/android/check-host.sh` 成功，299 项 Android JVM smoke（含新增的 `ScreenCaptureProtectionPolicySmoke`、补了关键词的 `SettingsSearchIndexSmoke`）和 255 项设备套件源码编译通过。check-host 的新检查分别删掉一处 `apply(this)`、一处弹窗保护（含 `InputDialog` 的 `AppCompatDialog`）、一处 `PopupWindow` 处理、子窗口标志和输入法窗口接线后都会失败。设备套件新增 `ScreenCaptureProtectionDeviceCheck`（`SettingsDeviceSmoke -e screen-capture true`，在 `smoke.sh` 默认路径上，PR 的 API 35 设备验收会跑），读系统真正拿到的窗口属性：开着时 `HomeActivity` 和经 `ScreenCaptureProtection.protect` 的对话框窗口带 `FLAG_SECURE`，关上后重新打开的窗口不带。没有在模拟器或真机上验证截图是否变黑、认证弹窗与回滚、`PopupMenu` 是否随父窗口涂黑以及厂商 ROM 的截屏和录屏行为。
