# Agent Note: Android 的会话存储经 JNI 注入，回调胶水在桌面 JVM 上验证

Status: implemented

## Problem

#8201 要把 Android 的账号和云端业务从 `platforms/android/java/app/msime/android/account/`、`community/` 里的 Java 实现迁到 Rust 的 C 接口（`crates/host-api/include/msime_client.h`）。登录会话在 AndroidKeyStore 加密的 `SharedPreferences` 里，只有 Java 读得到，所以 Rust 要经 `msime_client_session_store_register` 拿宿主注入的存储（契约见 [宿主注入会话存储](../../proposed/architecture/2026-10-16-host-injected-session-storage.md)）。

头文件要求存储回调能在任意线程上被调用，包括 Rust 自己起的线程（例如 AI 皮肤出图遇到 401 时刷新令牌）：JNI 胶水不能用调用 `msime_client_*` 那条线程的 env，要缓存 `JavaVM`，没 attach 的线程先 attach、返回前 detach，挂起的 Java 异常要清掉。忘了 detach，线程退出时 ART 以「native thread exited while attached」中止进程；没清异常，下一次 JNI 调用就是未定义行为。这几条在模拟器上都验不到：Java 起的线程都已 attach，离线的设备测试触发不了 Rust 自己的线程回调存储。

## Decision

- `client_jni.cpp` 有 `JNI_OnLoad`，把 `JavaVM` 交给 `msime::android::remember_java_vm`。账号与云端的十六个入口（`sessionStoreRegisterRaw`、`accountRaw`、`communityRaw`、`cloudSyncRaw`、`snapshotQueueRaw` 等）照现有的 `bounded_request` 先按头文件的上限查长度。
- 回调进 Java 的那一半单独放在 `platforms/android/native/JniCallbacks.cpp`，不依赖任何 `msime_client_*` 导出：
  - `ThreadEnv`：每次回调先 `GetEnv`；`JNI_EDETACHED` 时以线程名 `msime-session-store` attach，析构时 detach。本来就 attach 着的线程 never 被它 detach。
  - 每次回调开一个局部帧（`PushLocalFrame`/`PopLocalFrame`），在调用线程上的很多次回调不会把局部引用攒到外层调用返回。
  - Java 方法抛出的异常 `ExceptionClear` 后按失败返回 -1；进入时线程上已经挂着别人的异常则不碰 Java、直接失败，也不替别人清。
  - `release` 放掉 Java 对象的全局引用；取不到 env 时只能留着那个引用，上下文照样释放。
  - 流式对话和 AI 皮肤的 listener 只在调用线程上触发。listener 抛出的异常先 `ExceptionOccurred` 存下、清掉，之后不再调它，调用结束后 `Throw` 回去并丢掉这次的结果：同一次调用里 Rust 还会在这条线程上调存储回调，异常 must not 挂在线程上。
- 注册写死布局 0（Native `SavedAccountSession`）、flags 0、不给 lock 和 unlock：`SharedPreferences` 跨进程不满足 SHARED 的要求，只在主进程里注册，`:ime` 经 `AccountSessionProvider` 取令牌。`client_agent` 含 NUL、为空或超过 256 字节时 JNI 先拒，因为 Rust 读到第一个 NUL 就停。注册失败时按回答是否以 `{"ok":true,` 开头决定由 JNI 自己 release。
- Java 一侧是 `NativeClient.SessionStore`（load、save、remove 和四个 item 常量）、`NativeClient.ChatDelta`、`NativeClient.AiSkinProgress`。Raw 入口是 private，调用方从迁移计划的 5b 起接上。

## Alternatives considered

- **只靠设备冒烟验证回调（计划原来的写法）。** 最强的理由是设备上跑的是真的 ART 和真的 Rust，桌面 JVM 的 JNI 行为与 ART 有差别（例如 HotSpot 不限制局部引用表的大小）。不够：设备冒烟造不出没 attach 的线程，attach、detach 这条最容易让进程崩掉的路径恰好验不到；CI 也不在 PR 上跑设备套件。所以两层都要：`SessionStoreDeviceSmoke` 验经真实 Rust 的注册、读出和异常变成 `account_storage`，桌面 JVM 验线程和异常。
- **给 Rust 或 JNI 加一个「在新线程上调一次存储回调」的测试入口，在设备上触发。** 理由是设备上就能覆盖 detach。不行：那是只为测试存在的生产导出，会进发布的 `libmsime_android.so` 和 `verify-native.sh` 的清单。
- **listener 照资源包进度回调的写法，把异常留在线程上直到调用结束。** 理由是现成、少一个字段。不行：资源包安装期间 Rust 不会在调用线程上回调 Java，账号入口会（401 后刷新令牌就在调用线程上读写存储），挂着异常时那些 JNI 调用都不合法。
- **listener 抛出时由 JNI 替它调取消。** 理由是对话和生成能马上停。没这么做：JNI 要从请求 JSON 里解析 `request_id`，而 Java 调用方本来就知道它，在 listener 里自己 catch 再取消更直接；JNI 只保证线程干净。

## Consequences

- **收益**：attach、detach、异常清理、局部帧和全局引用的释放每次 PR 都在 `check-host.sh` 里对着真的 JVM（`-Xcheck:jni`）跑，不用模拟器；`build-native.sh` 照常把 `JniCallbacks.cpp` 链进 `libmsime_android.so`。
- **代价**：`check-host.sh` 多依赖 JDK 的头文件、libjvm 和一个宿主 C++ 编译器，缺任何一个就跳过并说明；JNI 胶水从一个文件变成两个，`build-native.sh` 和 `check-host.sh` 的目标平台编译都要列上新文件。桌面 JVM 与 ART 的差别（局部引用表上限、CheckJNI 的报法）仍要靠设备冒烟兜底。
- **还没做的**：5a 没有调用方，`SessionStoreDeviceSmoke` 只在 `smoke.sh` 的核心部分接好、编译过，第一次在 API 28 和 35 上跑要在 5b 合入之前。5b 接上了调用方和 Java 一侧「一个进程只注册一次」的守卫，见 [Android 的登录会话改由 Rust 持有](2026-10-18-android-account-through-rust.md)。

## Verification

- `platforms/android/tests/native/JniCallbacksTest.cpp` 由 `check-host.sh` 编译运行，Java 一侧是 `tests/native/JniCallbacksFixture.java`：在 `std::thread` 上调 load、save、remove、release，核对线程名和每次返回后都已 detach，调用线程不被 detach；Java 抛出时按失败返回且线程上没有异常；进入时挂着异常则不调 Java；load 交出的数组和 save 传入的数组在回调返回后能被回收（局部帧）；release 后 Java 对象能被回收；listener 抛出后异常不挂在线程上、存储回调照常可用、调用结束时原样重抛。逐个去掉 detach、清异常、局部帧、`DeleteGlobalRef`、listener 的清理和进入时的检查，对应用例失败；把调用线程也 detach 时进程中止。
- `verify-native.sh` 核对十六个 JNI 导出、十六个 host 导出和 `JNI_OnLoad`；去掉 `JNI_OnLoad` 时报缺导出。`scripts/test-android-jni-names.py` 核对 Java 声明、C++ 导出和清单三处一致。
