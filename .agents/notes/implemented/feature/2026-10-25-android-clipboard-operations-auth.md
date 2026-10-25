# Agent Note: Android 剪贴板敏感操作使用系统认证

Status: implemented

## Problem

剪贴板历史包含本机复制内容。键盘面板可以搜索、导出和清空历史，其中搜索会读取全部记录，导出会把记录写到应用外，清空会删除记录。输入法服务运行在没有 Activity 窗口令牌的 `:ime` 进程，不能直接创建 `BiometricPrompt`。

## Decision

搜索、导出和清空都先通过 `ClipboardOperationAuthenticationPolicy`，再执行原操作。应用设置页中的搜索页和导出动作直接使用 AndroidX `BiometricPrompt`；键盘服务通过显式启动同包的 `ClipboardAuthenticationActivity`，用 `ResultReceiver` 把认证结果返回到 `:ime` 进程。认证桥只允许 `BIOMETRIC_STRONG | DEVICE_CREDENTIAL`，取消、失败、认证不可用或服务回调代次过期时不执行操作。

允许的认证方式统一由 `SystemAuthenticators.allowed()` 给出，设置页的导出备份、重置设置、关闭剪贴板历史和统计页也用它：Android 11 起是 `BIOMETRIC_STRONG | DEVICE_CREDENTIAL`；Android 9、10 上 AndroidX Biometric 1.1.0 不支持这个组合（`canAuthenticate` 固定返回 `BIOMETRIC_ERROR_UNSUPPORTED`，`PromptInfo.Builder.build()` 抛异常），设了锁屏密码的设备也永远提示「没有可用的系统认证」，因此这两个版本改用它唯一能带锁屏凭据后备的 `BIOMETRIC_WEAK | DEVICE_CREDENTIAL`。设备没有锁屏凭据时仍然拒绝操作、说明原因，不导出、不读取、不清空。

Android 9、10 的锁屏凭据确认是设置应用里的独立 Activity，输入凭据期间宿主页会暂停。搜索页和统计页原本每次重新可见都清掉授权再认证，凭据确认返回时的那次 `onResume` 会立刻再弹一次认证，认证成功后永远读不到记录。但「暂停时认证还开着」不等于「暂停是认证引起的」：用户也可能在认证窗口开着时回到桌面、切换应用或熄屏，这时 AndroidX 在 Android 10 以下的 `onStop` 里取消认证，报 `ERROR_CANCELED`；Android 11 起 SystemUI 发现发起认证的应用已不在前台，就把认证窗口关掉（logcat 里是 `AuthController: Evicting client`，随后 `AuthSession: Dismissed! … Reason: 3, Error: 10`），报的是 `ERROR_USER_CANCELED`，和用户自己按取消是同一个错误码。只凭暂停时的状态跳过重新认证，回来时页面既没有授权也没有认证窗口，停在「正在读取…」；已授权后点导出再离开，则会带着授权回来。

因此两页把这件事交给 `AuthenticationPauseTracker`，按认证怎么结束来决定：暂停时认证（包括历史页页内导出的认证）还开着，就记下它；结束时是成功或用户自己取消（在凭据确认页按返回报 `ERROR_USER_CANCELED`），回来的那次可见不再重新认证，成功回调自己会读；被系统取消，就按离开处理，清掉授权并重新认证。错误码由 `AuthenticationPauseTracker.systemCanceled` 归类：`ERROR_CANCELED` 一直算系统取消；Android 11 起认证界面画在 SystemUI 的窗口里，不会让宿主页暂停，跨越暂停的认证出任何错误都只能是离开页面后被系统收走，也算系统取消；只有 Android 9、10 的凭据确认 Activity 里按返回报的 `ERROR_USER_CANCELED` 才是用户自己取消。结果可能在页面重新可见之前或之后才送达：AndroidX 只在它的 Fragment 启动后才把结果交给回调，再经主线程执行器转发，所以可见时认证看起来还在进行就先不下结论，等结果到了再判断，系统取消在可见之后才到时立刻重新认证。这条记录只跨一次暂停，被系统取消后重新发起的认证不再受它影响，不会因为传感器反复报取消而循环弹窗。普通的离开再回来仍然重新认证。

清空仍保留键盘内的二次确认行，只有用户再次点「清空」并完成认证后才调用存储；搜索和导出在认证成功后才关闭键盘面板或启动导出。视图、服务回调和认证 Activity 都有一次性完成及生命周期清理，迟到结果不能覆盖下一次操作。

## Alternatives considered

- 让输入法服务直接创建 `BiometricPrompt`：AndroidX 只接受 `FragmentActivity` 或 `Fragment`，输入法服务没有这种窗口宿主，编译和运行时都不成立。
- 只保护设置应用里的搜索页和导出按钮：键盘面板仍能直接清空、搜索或导出，安全边界不完整，因此增加跨进程认证桥。
- 用系统文件选择器或键盘内确认代替认证：文件选择器不能证明操作者身份，键盘确认也不能防止已解锁设备上的其他操作者，均不足以满足 issue。
- 只在 Android 9、10 上跳过认证结束后的那次可见：同一版本上有指纹时认证窗口是系统对话框，离开再回来照样需要重新认证；按版本判断也解决不了已授权后导出被打断的情形。
- 只按错误码区分系统取消和用户取消：API 35 模拟器上认证窗口开着时切到别的应用，SystemUI 收走窗口报的是 `ERROR_USER_CANCELED`，与用户取消无从区分，回来后页面停在「正在读取…」，设备验收的「history page prompt interrupted by leaving」就停在这里。错误码只在 Android 9、10 有区分意义。
- 在任何 `ERROR_CANCELED` 回调里都立刻重新认证：页面可见时传感器不可用也会报这个错误，会循环弹窗；只处理跨越暂停的那一次认证。

## Consequences

三项剪贴板操作多一个系统认证步骤，认证失败时保留本机记录。新增一个非导出的认证 Activity、AndroidX Biometric 依赖和 `ResultReceiver` 跨进程回传；输入法核心通过显式组件名启动桥，避免主机 smoke 的核心编译依赖 Activity 类。原有剪贴板存储格式、撤销机制、目录选择和搜索筛选规则不变。真实 API 28/35 模拟器上仍需在合并前验收认证、取消、回调和键盘返回路径。

设备验收用合成锁屏 PIN（`DeviceCredentialFixture`）驱动真实的系统认证界面：专用 AVD 默认没有锁屏凭据，`ClipboardExportDeviceSmoke` 先确认此时设置页导出被拒绝、提示原因且不写文件、不打开选择器，再设置 PIN，覆盖设置页、重启后直接导出、键盘面板经认证桥导出、历史页打开与页内导出的认证，以及历史页打开时和页内导出时认证窗口开着就切到别的应用再回来（Android 10 起系统应已关掉认证窗口，回来要重新认证并读出记录、导出不发生；Android 9 回到仍在进行的凭据确认，认证后照常读取或导出，不弹第二次）；`ClipboardSegmentDragDeviceSmoke` 同样先确认无凭据时键盘清空被拒绝、记录保留，再带 PIN 走清空。PIN 在套件结束时清除，`clipboard-export.sh` 和 `smoke.sh` 只在确有凭据时兜底清除：Android 9 把无凭据时的 `clear --old` 记作一次输错，累积几次后 `set-pin` 都会被限速拒绝。

验证：JDK 21 下 `compileFullDebugJavaWithJavac` 成功；`platforms/android/check-host.sh` 成功，包含 267 项 Android JVM smoke、240 项设备套件源码编译和服务 Java/API 检查；`git diff --check` 成功。

2026-10-25 补做的设备验收（本机 arm64 专用 AVD，产品 APK 在 Mac Studio 构建后用同一把发布密钥重签）：API 35 上 `clipboard-export.sh` 的 select、verify 两段通过，含历史页打开时和页内导出时离开再回来两段；只按 `ERROR_CANCELED` 判断系统取消的版本停在「history page prompt interrupted by leaving: prompt after return」，logcat 里是 `AuthController: Evicting client` 和 `Reason: 3, Error: 10`。API 28 上 `smoke.sh --core` 一路通过到剪贴板导出之后，导出两段走 Android 9 的分支（回到仍在进行的凭据确认，认证后照常读取或导出）。`check-host.sh` 276 项 JVM smoke 通过，含 `AuthenticationPauseTrackerSmoke` 新增的错误码归类。
