# Agent Note: 语音自动插入设备验收的反射签名与失败输出

Status: implemented

## Problem

2026-10-29 起，`android-device-pr / Android device (API 35)` 的「Voice polish reply acceptance」在一部分 PR 上失败，日志只有一行 `MSIME_DEVICE_SMOKE_FAILED: voice polish replies (AssertionError)`。最近 8 次无关 PR 上的运行 4 过 4 挂、时间上交错，看起来像偶发。

实际上它是确定性的回归。`9d574a9ab9`（随 #10169 在 develop 的 `addc8f69c4` 合入）给 `ImeVoiceEntry.delivered` 加了 `String notice` 参数，变成 8 个参数；`VoiceAutoInsertDeviceCases` 用反射按旧的 7 个参数查找，`getDeclaredMethod` 抛 `NoSuchMethodException`。反射查找不经编译检查，`check-host.sh` 编得过设备套件。异常在 `runOnMainSync` 里被接住，包成 `AssertionError("语音自动插入回归失败")`，`VoicePolishReplyDeviceSmoke` 又只输出异常类名，于是和一条真正的断言失败看起来完全一样。

「交错」来自 PR 运行检出的是 PR 头与当时 develop 的合并提交。逐次核对检出日志里的 `Merge <head> into <base>`：

- 失败的 #10169（PR 头本身带这次改动）、#10157、#10164、#10165，以及 #10159、#10154，检出的树都包含 `9d574a9ab9`（基底是 `57f4fd3a54`，或 PR 头就是那次改动）。
- 通过的 #10190、#10185、#10162、#10147、#10178（#10178 挂在别的步骤，语音这步通过），以及 #10157、#10164、#10165 更早的运行，检出的树都不含它（基底是 `c455167a18`、`cdd46d4d14` 或 `e3b6a8f2e5`）。

失败运行的 logcat 也排除了等待超时：#10157 那次，`am instrument` 从起进程到退出约 77 ms，套件本身不到 15 ms，远小于第 133 行 `inserted.await(3, SECONDS)` 的 3 秒，失败发生在主线程那段同步代码里。在合成环境里，`voiceActivityResultReceiver` 记下的目标与第一次 `insertVoiceActivityResult` 时读到的连接、`editorContextRevision`、光标前后文字和选区都由同一个主线程同步设置，第一次尝试就能匹配，测试的 3 秒等待和产品约 3 秒的重试窗口重合并不构成竞态。

## Decision

- `VoiceAutoInsertDeviceCases` 按 `delivered(long, String, String, String, byte[], long, String, String)` 查找，调用时 `notice` 传 `null`，其余断言不变。查找处注释写明参数顺序和「反射不经编译检查，产品改了参数这里要同步改」。
- 失败输出分两类：
  - 本类 `check` 抛出的 `AssertionError` 原样抛出，消息是固定的测试文字；
  - 其他异常（反射找不到成员、产品代码抛的异常）包成 `AssertionError`，消息是「语音自动插入回归失败：」加每层异常的类型和最内层异常落在本类里的方法与行号，不带任何异常消息。这次的签名漂移是 `method()` 直接调 `getDeclaredMethod` 抛的，输出是 `语音自动插入回归失败：NoSuchMethodException at method:<行号>, lambda$run$0:<行号>, …`；只有产品代码在 `delivered.invoke` 之类的反射调用里抛异常时，才会出现 `InvocationTargetException <- <产品异常>` 这样的多层类型。
- `VoicePolishReplyDeviceSmoke` 对 `AssertionError` 输出类名和消息，其他异常仍只输出类名。这些消息只来自测试自己的固定文字和上面的类型、行号，不含服务正文、请求数据或用户输入。

## Alternatives considered

- **当成偶发失败处理，加长等待或重试整个套件**：任务最初的怀疑是 3 秒等待与产品 30 次 × 100 ms 的重试窗口相同，最强的理由是两者确实重合，真实设备上窗口归还焦点也确实可能慢。没有采用：失败运行的整个插桩不到 15 ms，根本没走到等待；加长等待只会把一个确定性的签名错误变成更慢的同一个失败。
- **把 `delivered` 改成包可见或加一个测试入口，让设备套件直接调用、由编译器检查签名**：能从根上消除这类漂移。没有采用：设备套件和产品分两个 APK、两个类加载器，测试包按约定只用反射访问产品私有成员，不为测试扩大生产 API（`VoicePolishReplyDeviceSmoke` 里写着同样的约定）；要改就得为整个设备套件换访问方式，超出这次修复。
- **在 `check-host.sh` 里静态校验设备套件的反射查找**：能在 PR 编译阶段就报出。没有采用：设备套件里的反射查找散在几十个文件、类名和参数类型多是运行时拼出来的，要可靠校验就得解释测试代码；成本与收益不相称。本篇只让失败输出能直接指出是反射找不到成员。

## Consequences

- **收益**：语音自动插入验收在含 `9d574a9ab9` 的树上恢复通过；以后同类失败在 CI 输出里直接显示是哪条检查失败，或是哪种异常、出在本类哪一行，不必下载 logcat 去猜。
- **代价**：失败输出比原来多一段固定文字、异常类型和行号。不输出异常消息，产品异常里的内容仍不会进日志。
- **仍然没有覆盖**：反射签名漂移在编译阶段仍发现不了，只能靠设备验收运行时报出；而 PR 上的 `android-device-pr` 不是必需检查（见 [改键盘或设置界面的 PR 在 API 35 上跑一遍设备验收](../process/2026-10-28-android-device-on-ui-prs.md)），#10169 自己的设备作业就是这条失败，仍然合进了 develop。

## Verification

`rbuild env ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh` 通过（304 个 JVM 冒烟，编译 255 个设备套件源文件）。在 Studio 上用 android.jar 编译产品类后反射核对：新的 8 参数查找找得到，旧的 7 参数查找抛 `NoSuchMethodException`。本机没有起模拟器；设备上是否通过以 CI 的 `android-device-pr / Android device (API 35)` 为准。

## 合成服务的配置回调生命周期

完整 OCR 卸载设备验收在 API 35 的语音专项通过后，到触控板阶段收到配置变化并崩溃，堆栈是 `MSIMEInputService.onConfigurationChanged` 读取未初始化的 `imeLayoutRows`。真实服务在 `super.onCreate()` 前创建该组件，之后没有清空路径；残留的反射假服务只执行 `attachBaseContext`，既不创建完整组件，也不调用 `onDestroy`。

Android 的 `WindowProviderService.attachBaseContext` 会将上下文附到窗口 token 并注册配置派发，`onDestroy` 才解除；通过反射调用 `ContextWrapper.attachBaseContext` 仍会虚派发到该实现，关闭线程池不能解除注册。因此本夹具使用完整的创建与销毁，不在生产配置回调增加空值分支。自动插入和权限回退都复用服务自己创建的语音组件，销毁前把合成 `session=1` 清零并移除合成编辑连接，避免将假身份交给 JNI 释放。

服务文件、缓存、应用上下文和结果存储指向本次独立临时目录；偏好从正式 JNI 默认值取得，只对合成目录关闭 `usage_reporting`，不读写用户配置或记录。销毁在主线程完成，再等待服务工作线程退出，最后递归清理该目录。Android 设备夹具使用 `Files.write` 写 UTF-8 字节，避免宿主 JDK 提供但目标运行时没有的 `Files.writeString`。

保留按钮几何、停止与取消、过期回调、跨 Binder 结果、重复回传及编辑位置变化的全部断言。创建后、安装合成 session 前直接重放配置回调：旧半初始化实现确定性出现 `InvocationTargetException <- NullPointerException`；完整生命周期实现的 API 35 自动插入与前台回退三项通过。API 35 冷启动后用 `--no-restart` 在同一主进程依次运行语音和完整触控板专项，通过此前崩溃阶段。最终源码在 API 28 的自动插入与 `result / ready-error / stop-before-resume / refused-start` 四项生命周期均通过（`voice-final-auto28.log`、`voice-final-lifecycle28.log`）。原完整开发包的 API 28 `smoke.sh --core` 已全部通过；API 35 原轮次仅在语音之后触控板阶段被上述残留假服务击穿，修复后的整套仍在运行，不能把专项当作整套通过。远端 quick 使用正确的 Homebrew Python 后退出 0；最初登录 Bash 选到 Xcode Python 3.9 的工具链失败不算通过。

备选是在生产 `onConfigurationChanged` 跳过未初始化组件，改动较少；但它掩盖了测试未完整创建且仍注册窗口回调的服务，不能解决残留 owner，因此不采用。
