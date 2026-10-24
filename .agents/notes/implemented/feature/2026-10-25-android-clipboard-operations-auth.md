# Agent Note: Android 剪贴板敏感操作使用系统认证

Status: implemented

## Problem

剪贴板历史包含本机复制内容。键盘面板可以搜索、导出和清空历史，其中搜索会读取全部记录，导出会把记录写到应用外，清空会删除记录。输入法服务运行在没有 Activity 窗口令牌的 `:ime` 进程，不能直接创建 `BiometricPrompt`。

## Decision

搜索、导出和清空都先通过 `ClipboardOperationAuthenticationPolicy`，再执行原操作。应用设置页中的搜索页和导出动作直接使用 AndroidX `BiometricPrompt`；键盘服务通过显式启动同包的 `ClipboardAuthenticationActivity`，用 `ResultReceiver` 把认证结果返回到 `:ime` 进程。认证桥只允许 `BIOMETRIC_STRONG | DEVICE_CREDENTIAL`，取消、失败、认证不可用或服务回调代次过期时不执行操作。

清空仍保留键盘内的二次确认行，只有用户再次点「清空」并完成认证后才调用存储；搜索和导出在认证成功后才关闭键盘面板或启动导出。视图、服务回调和认证 Activity 都有一次性完成及生命周期清理，迟到结果不能覆盖下一次操作。

## Alternatives considered

- 让输入法服务直接创建 `BiometricPrompt`：AndroidX 只接受 `FragmentActivity` 或 `Fragment`，输入法服务没有这种窗口宿主，编译和运行时都不成立。
- 只保护设置应用里的搜索页和导出按钮：键盘面板仍能直接清空、搜索或导出，安全边界不完整，因此增加跨进程认证桥。
- 用系统文件选择器或键盘内确认代替认证：文件选择器不能证明操作者身份，键盘确认也不能防止已解锁设备上的其他操作者，均不足以满足 issue。

## Consequences

三项剪贴板操作多一个系统认证步骤，认证失败时保留本机记录。新增一个非导出的认证 Activity、AndroidX Biometric 依赖和 `ResultReceiver` 跨进程回传；输入法核心通过显式组件名启动桥，避免主机 smoke 的核心编译依赖 Activity 类。原有剪贴板存储格式、撤销机制、目录选择和搜索筛选规则不变。真实 API 28/35 模拟器上仍需在合并前验收认证、取消、回调和键盘返回路径。

验证：JDK 21 下 `compileFullDebugJavaWithJavac` 成功；`platforms/android/check-host.sh` 成功，包含 267 项 Android JVM smoke、240 项设备套件源码编译和服务 Java/API 检查；`git diff --check` 成功。
