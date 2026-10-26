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

2026-10-25 develop 推送后的 CI（运行 40173789170）API 35 停在 verify 段的「picker permission confirmation: write stream failure retains partial document」，断言是 `synthetic control missing`。原先猜是从选择器回来后设置页重排、「导出文件夹」又滚出屏幕，日志否定了这一点：选择器根本没有返回。第三次打开的选择器进入 `failed-write` 后，确认框 08:15:33.18 弹出、0.65 秒后关闭，documentsui 紧接着记下 `InputEventReceiver: Attempted to finish an input event but the input event receiver has already been disposed`，没有 `PickerActionHandler: onFinished()`，选择器 Activity 一直留到套件结束被强停。本机 API 35 上逐一对照过三种关闭方式：点框外是同一条日志、没有 `onFinished`、选择器留在原目录；点「CANCEL」没有这条日志；点「ALLOW」有 `onFinished`。也就是验收按刚读到的「ALLOW」节点坐标 `input tap`，点到了框外，确认框被当作点外部取消。旧写法在找到按钮的同一拍就按坐标点，不等位置稳定。日志只证明点到了框外，读到的坐标为什么偏没有直接证据；最可能是 CI 的 x86_64 模拟器很慢（同一时段 SystemUI 单帧最长 10 秒），确认框窗口还没摆好就读到了它。#9682 之前同一轮里更早的套件把系统动画缩放留成 0，之后的套件都在没有动画的系统上跑，这次是恢复动画后第一次在 CI 上完整走到这里。本机把窗口动画放慢到 10 倍、或把模拟器限成单核都没能复现同一步，所以修法不依赖时序：确认框的「ALLOW」改成对按钮节点直接发无障碍点击，不再经屏幕坐标；`performAction` 不接受时下一轮再试，之后照旧等设置页的「导出文件夹」和可用的「导出」，断言不变。`DiagnosticsExportDirectoryDeviceSmoke` 的确认框是同一写法，一并改掉。

导出验收在 CI 上还有一处时序问题：verify 段「real keyboard panel export action」在运行 40187763661 里等满 15 秒失败。日志里 `:ime` 进程 11:33:06.9 才冷启动，11:33:09.47 键盘显示，11:33:09.9 主线程报 `Skipped 124 frames`，之后再没有动作，也没有拉起认证桥。输入法服务的偏好由后台线程读（`loadAppearanceWithoutSession`），读到之前 `clipboardHistoryEnabled` 是 false，工具栏「剪贴板」按钮存在但不可用；验收找到它就点，点在不可用的按钮上没有任何反应，面板不打开，之后的「导出全部剪贴板历史」自然等不到。两个键盘内控件改成等到可用再点，断言不变。

在同一段里写入系统剪贴板后，Android 的剪贴板监听、输入法服务和共享历史文件仍可能跨线程排队；只等工具栏按钮可用不能证明这条合成记录已经落盘。验收现在先用测试进程自己的 `ClipboardHistoryStore.load()` 轮询合成全文，确认记录捕获后再点击剪贴板入口并查找导出按钮。捕获、剪贴板入口、导出控件和认证动作各自使用独立阶段名，后续若再失败可以区分是监听落盘还是界面重画。

长符号条目同样有异步的无障碍数据：符号键的边界先稳定，不代表 `refreshWithExtraData` 立刻能返回每个字符的矩形。API 35 重新打开常用符号面板时曾在边界已稳定的瞬间拿到空数组。`LongSymbolCellDeviceSmoke` 现在在 15 秒内轮询字符位置扩展数据和布局，重试只覆盖数据尚未就绪的窗口，仍会在超时后报告缺失或裁切；重开面板和重启输入法的两段也写出独立阶段。

触控板双击、三击的验收事件也必须按系统输入时间排序。原测试为了抵消 Binder 注入延迟，把整组点按的 `eventTime` 设到了当前时刻之前；API 35 慢模拟器上三击事件到达输入队列时可能已经过期，编辑器只保留键盘状态。`TextTrackpadDeviceChecks` 现在把完整点按序列排在当前时刻之后一次注入，仍用 40 毫秒间隔表达双击窗口，随后继续由真实编辑器选区断言收尾。

导出验收通过之后，API 35 完整套件后面还有几项是在 #9682 修好动画缩放之前就被挡住、一直没跑到的旧断言，这次一并跟上已经记录的产品决定，没有放宽：全拼组字在输入框里按音节分词显示（[全拼宿主预编辑分词](../bug-fix/2026-10-23-quanpin-host-preedit-segmentation.md)），`LetterHintPunctuationDeviceSmoke`、`ChineseHelpcodeOffDeviceSmoke`、`NumberRowDeviceSmoke`、`HardwareTranslationDeviceSmoke` 和 `PreferencesDeviceSmoke` 对输入框的期望从 `nihao`、`wode` 改成 `ni'hao`、`wo'de`（`PreferencesDeviceSmoke` 里「不再含 nihao」的检查在分词后会恒真，也一并改掉）；输入页按方案族分行、双拼排法移到键盘页「中文键盘」（[三端手机设置按输入方案分行](./2026-10-25-mobile-scheme-rows.md)），`FourteenKeyShuangpinDeviceChecks` 改走新入口；横向候选条连续列出全部候选（[Android 横向候选条连续浏览](./2026-10-23-android-continuous-candidate-strip.md)），候选条不再按每页几个截断，`PreferencesDeviceSmoke` 改为核对候选条旁的引擎页码：同一组字每页 5 个时 P5 页，换成每页 2 个后页数必须落在 ⌈(5·P5−4)/2⌉ 到 ⌈5·P5/2⌉ 之间，任一次偏好没生效都落不进去。

同一轮完整 API 35 套件里另外三处已由别的 PR 合入：日语 9 键换输入框后中英键停在「英」（#9770，每次重画按当前状态重写键面），英文键盘非 ASCII 角标滑动没有输出（#9741 恢复了被 #9482 合并冲掉的 #9486 判断），字母长按验收先上滑再打 @（#9741）。

CI 上还有两类只在慢模拟器上出现的时序问题。一是剪贴板常用语提示套件的 `launch` 时有时无地失败（develop 运行 40175764000、40211864536 和 #9688 分支的多次运行）。加上失败时写出输入法节点的说明后，运行 40212793727 显示面板其实打开了，但写着「本机已存 0 条」。上一个套件结束时恢复的偏好里剪贴板历史是关的，`:ime` 进程没有重启，实时读到「关」就按 `ClipboardHistoryRetentionPolicy` 清空历史；在慢的模拟器上，这次清空落在本套件写完合成记录之后。现在先打开面板，等到只有实时偏好开着历史时才画的「本机已存 N 条」，再写入记录，然后关开一次面板。其他直接写 `clipboard_history.json` 的套件也是同样的写法，目前在 CI 上都通过，没有一并改。二是字母滑动验收逐个同步注入事件、每次再睡 40 ms，从按下到抬起的实际时间超过长按时长，关掉滑动时那一下就成了长按（运行 40205049019）。现在一次手势的全部事件一起排进输入队列。
