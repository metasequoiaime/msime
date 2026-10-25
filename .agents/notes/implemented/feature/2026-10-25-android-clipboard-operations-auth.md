# Agent Note: Android 剪贴板敏感操作使用系统认证

Status: implemented

## Problem

剪贴板历史包含本机复制内容。键盘面板可以搜索、导出和清空历史，其中搜索会读取全部记录，导出会把记录写到应用外，清空会删除记录。输入法服务运行在没有 Activity 窗口令牌的 `:ime` 进程，不能直接创建 `BiometricPrompt`。

## Decision

搜索、导出和清空都先通过 `ClipboardOperationAuthenticationPolicy`，再执行原操作。应用设置页中的搜索页和导出动作直接使用 AndroidX `BiometricPrompt`；键盘服务通过显式启动同包的 `ClipboardAuthenticationActivity`，用 `ResultReceiver` 把认证结果返回到 `:ime` 进程。认证桥只允许 `BIOMETRIC_STRONG | DEVICE_CREDENTIAL`，取消、失败、认证不可用或服务回调代次过期时不执行操作。

允许的认证方式统一由 `SystemAuthenticators.allowed()` 给出，设置页的导出备份、重置设置、关闭剪贴板历史和统计页也用它：Android 11 起是 `BIOMETRIC_STRONG | DEVICE_CREDENTIAL`；Android 9、10 上 AndroidX Biometric 1.1.0 不支持这个组合（`canAuthenticate` 固定返回 `BIOMETRIC_ERROR_UNSUPPORTED`，`PromptInfo.Builder.build()` 抛异常），设了锁屏密码的设备也永远提示「没有可用的系统认证」，因此这两个版本改用它唯一能带锁屏凭据后备的 `BIOMETRIC_WEAK | DEVICE_CREDENTIAL`。设备没有锁屏凭据时仍然拒绝操作、说明原因，不导出、不读取、不清空。

Android 9、10 的锁屏凭据确认是设置应用里的独立 Activity，输入凭据期间宿主页会暂停。搜索页和统计页原本每次重新可见都清掉授权再认证，凭据确认返回时的那次 `onResume` 会立刻再弹一次认证，认证成功后永远读不到记录。两页在暂停时记下认证窗口（包括页内导出的认证）是否仍在进行，由认证引起的暂停结束时不再重新认证；离开页面再回来仍然重新认证。

清空仍保留键盘内的二次确认行，只有用户再次点「清空」并完成认证后才调用存储；搜索和导出在认证成功后才关闭键盘面板或启动导出。视图、服务回调和认证 Activity 都有一次性完成及生命周期清理，迟到结果不能覆盖下一次操作。

## Alternatives considered

- 让输入法服务直接创建 `BiometricPrompt`：AndroidX 只接受 `FragmentActivity` 或 `Fragment`，输入法服务没有这种窗口宿主，编译和运行时都不成立。
- 只保护设置应用里的搜索页和导出按钮：键盘面板仍能直接清空、搜索或导出，安全边界不完整，因此增加跨进程认证桥。
- 用系统文件选择器或键盘内确认代替认证：文件选择器不能证明操作者身份，键盘确认也不能防止已解锁设备上的其他操作者，均不足以满足 issue。

## Consequences

三项剪贴板操作多一个系统认证步骤，认证失败时保留本机记录。新增一个非导出的认证 Activity、AndroidX Biometric 依赖和 `ResultReceiver` 跨进程回传；输入法核心通过显式组件名启动桥，避免主机 smoke 的核心编译依赖 Activity 类。原有剪贴板存储格式、撤销机制、目录选择和搜索筛选规则不变。真实 API 28/35 模拟器上仍需在合并前验收认证、取消、回调和键盘返回路径。

设备验收用合成锁屏 PIN（`DeviceCredentialFixture`）驱动真实的系统认证界面：专用 AVD 默认没有锁屏凭据，`ClipboardExportDeviceSmoke` 先确认此时设置页导出被拒绝、提示原因且不写文件、不打开选择器，再设置 PIN，覆盖设置页、重启后直接导出、键盘面板经认证桥导出、历史页打开与页内导出的认证；`ClipboardSegmentDragDeviceSmoke` 同样先确认无凭据时键盘清空被拒绝、记录保留，再带 PIN 走清空。PIN 在套件结束时清除，`clipboard-export.sh` 和 `smoke.sh` 只在确有凭据时兜底清除：Android 9 把无凭据时的 `clear --old` 记作一次输错，累积几次后 `set-pin` 都会被限速拒绝。

验证：JDK 21 下 `compileFullDebugJavaWithJavac` 成功；`platforms/android/check-host.sh` 成功，包含 267 项 Android JVM smoke、240 项设备套件源码编译和服务 Java/API 检查；`git diff --check` 成功。
