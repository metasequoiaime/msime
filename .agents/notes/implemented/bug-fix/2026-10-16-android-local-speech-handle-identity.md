# Agent Note: Android 本地语音使用正整数身份而非指针句柄

Status: implemented

## Problem

#8229 报告 arm64 Android 16 的两种本地语音模型均在启动时报通用失败。JNI 把 `new LocalSpeech` 的指针转为 `jlong`，Java 和 JNI 却都要求句柄为正。Android 11 起支持 TBI 的 64 位进程会在堆指针最高字节设置标记，合法指针可能转为负 `long`；启动、取消、销毁因此被拒绝，错误清理还会再次抛异常。

依据为 [Android Tagged pointers](https://source.android.com/docs/security/test/tagged-pointers?hl=en)。`ebe4b166ba` 引入了正值检查，但正数本身也不能证明是有效指针。

活跃笔记检索 `NativeHandlePolicy`、语音句柄和 tagged pointer，没有持有这一决定的笔记；[语音前台恢复](../../implemented/bug-fix/2026-10-15-android-speech-permission-foreground.md)管理启动时机，与句柄表示无关。

## Decision

JNI 持有 `LocalSpeech`，向 Java 返回不复用的正整数身份。所有入口先从加锁的注册表查找对象；单次调用持有共享所有权，销毁仅撤销身份。保留 Java 对零及负数的既有入口约束，释放后或未知的正整数不会被解引用为地址。识别算法、录音和模型加载流程不变。

## Alternatives considered

- **放行所有非零指针。** 改动最少，并保留完整标记；但只能判断空指针，不能拒绝过期、重复释放或任意正负整数，也会撤销原本希望提供的无效句柄保护。
- **清除指针最高字节再返回。** 可以继续满足正数检查；但 Android 明确要求释放时保留原始标记，修改位模式会破坏平台契约。
- **关闭堆指针标记。** 不改 JNI 的表示，也能避开这类负值；但它是平台的兼容逃生开关，不能修复把地址符号当作身份有效性的错误。

## Testing

API 35 arm64 模拟器的旧 APK 在真实 JNI 创建句柄后报告 `Native pointer sign escaped into the speech handle`，在模型加载前复现失败。新原生宿主 APK 在 API 28、35 上的 `LocalSpeechHandleDeviceSmoke` 均通过，覆盖创建、启动缺模型、取消、销毁、重复销毁、未知和过期句柄。该测试接入 `smoke.sh` 的核心路径，两种 API 均会持续运行。

`check-host.sh` 的 237 项 JVM 冒烟通过，139 个设备测试源码编译通过；`verify-local.sh --quick` 通过。原生 APK 用 Java 21 构建成功，默认 Java 25 与固定 Gradle 不兼容。

首次广泛设备验收时，两台模拟器并行启动后出现系统界面、桌面及 system 进程无响应弹窗；API 35 启动检查超时。API 28 的菜单复制两次失败，暂停另一台模拟器后，同一二进制和同一测试的单项复制通过。上述失败保留为环境和时序观察，不算整套通过。改为逐台运行后，同一新 APK 的 API 28 `smoke.sh emulator-5566 --core` 整套通过，覆盖首次启动、词库准备、系统输入、候选、菜单复制、新语音句柄回归及剪贴板保存生命周期。API 35 单独重启后同样通过核心路径和新句柄回归。

测试没有加载真实模型或访问麦克风，不能据此声称报告设备的整条识别链路完全修复；关联 issue 保持 `Refs #8229`。

## Consequences

注册表为每个活动会话增加一项和短暂互斥；任何调用期间持有对象避免同时销毁释放内存。启动、音频输入和结束仍遵守既有单工作线程约定，共享引用只保护对象生命周期。身份到达 `jlong` 上限时拒绝创建，不回绕或复用。JNI 对象的模型回调必须只捕获裸指针，避免对象通过自己的 session 回调形成所有权环。
