# Agent Note: 前任返回但仍有默认致命信号时保留原始记录

Status: implemented

## Problem

固定 `e746981d18` 的 Android API 28 arm64 实际 IME，在真实 JNI 入口把 `JNIEnv` 改为空后因 `SIGSEGV` 退出，却没有原生崩溃记录。独立断点确认采集已启用、配置锁可取、文件描述符为 42；前任返回后执行了清空与删除。故障线程的 `/proc` 显示 `SigPnd=0x400`，进程 `SigCgt` 中 SIGSEGV 位为零，随后退出码为 11。Java 致命故障到隔离后台已成功，不将其扩大成 JNI 通过。

Android [9.0.0_r55 的 debuggerd](https://android.googlesource.com/platform/system/core/+/android-9.0.0_r55/debuggerd/handler/debuggerd_handler.cpp) 会恢复默认处置、重新投递原信号后返回；[ART 信号链](https://android.googlesource.com/platform/art/+/android-9.0.0_r55/sigchainlib/sigchain.cc) 可拦截 `sigaction` 查询，查询到的用户回调不等于实际内核处置。因此「前任返回」不能独立证明进程恢复。这修正[原生采集笔记](../../implemented/feature/2026-10-27-native-crash-capture-and-symbol-identity.md)中返回即撤销的适用边界，不撤销可恢复信号不得上报的要求；未发现其他持有该决定的独立笔记。

## Decision

前任返回后，在清空记录之前检查同一信号是否仍待送达且实际处置为默认动作。满足时保持原 inode 与独占文件锁，交由内核在致命退出时释放，不重新安装包装器。普通返回、保护页恢复和仅恢复默认而没有排队信号的路径继续撤销并恢复采集。

共同 C++ 与 iOS 独立 C 记录器采用相同的待送达默认致命信号判断。Android 在普通安装路径解析并持有 libc 的 `sigaction` 查询函数，信号内只调用该已准备入口，绕过 ART 的逻辑查询；不在信号中加载库、不改系统信号链。其他 POSIX 宿主使用正常 `sigaction` 查询。

## Alternatives considered

- **只相信前任返回**：能避免可恢复信号误报，且已有恢复保护页、Rust 恢复默认和旧 inode 撤销回归；真实 Android 已证明系统可以先排队致命信号再返回，因而会漏报。
- **所有返回都保留**：最少代码即可避免本次丢失，但把真正恢复的信号误报为崩溃，违反既有负对照。
- **只查询 ART 导出的 `sigaction`**：不用解析额外入口，但 ART 的逻辑回调与 debuggerd 改写后的内核处置不同，不能据此确认默认致命动作。

## Verification

- 父进程提前打开原始 inode，前任恢复默认并排队同号致命信号后，退出时仍能读到原始记录。共同 C++ 与 iOS C 旧实现均在旧描述符读回断言失败，修后 macOS 两套完整回归通过；PVE/Linux 的 `-O3 -D_FORTIFY_SOURCE=2 -Werror` 旧实现被同一断言拒绝、修后通过。
- 原有返回、保护页恢复、默认重试、首份记录及锁竞态回归继续通过。Apple 真实故障首帧、UUID、独立符号源码行专项通过；iPhoneOS arm64、模拟器 arm64/x86_64 及 Android API 28 的 arm64/x86_64 严格语法编译通过。Apple 仍不暴露一次性标志，相应子项明确跳过。
- 固定 `e746981d18` 基础产品，仅以 SHA-256 `d0ea75824ab301c0f2ccfad9289a7093303b8ef81d0cf25bf04263459b55bb4b` 的本次 C++ 源替换 JNI 记录器，Rust 保持同源不变。API 28 arm64 实际 IME 与 HomeActivity App 分别完成 Java 致命异常、真实 JNI 无效指针、产品重启补报及 SIGKILL 负对照。两轮独立官方后台最终分别为两条 crash、两条 session_crash、一条 active，以及两条 crash、一条 active；实际 HTTP 与数据库七字段相等。
- 新 JNI build-id `e97cdf8dce6f1c0aff4743bccdba37df74b73fb4` 与独立精确符号 ZIP 匹配，实际后台帧还原到 `jni.h:852`、`client_jni.cpp:108/117`；错误 build-id 拒绝。ZIP SHA-256 为 `76166f8f5b9406e2ef7dca64a74e7d2fba98e8843645800362448b58eff36b7d`。本轮是基础包加精确 C++ 修补，不冒充整包已按新提交重建，也不扩大为真机验收。
- 初次夹具选错资源缓存、把剥离后的 APK 与未剥离文件逐字节比较、把仅支持 API 35 的调度/同意入口用于 API 28、首次修补轮仍给 LLDB 旧模块目录，均保留失败。API 28 支持的 `telemetry-watch.py` 实际通过；调度失败不改记通过，修正符号目录后另起空后台重验。App 夹具曾误设“原授权必关闭”的前提；新 AVD 的真实原值为开启，改为快照与恢复实际原值，不改变产品默认值。

正式 Cargo、JNI 与 Gradle 输出已去掉测试特性恢复，与本轮最初正式二进制和 APK 库逐字节一致；合成授权在回收前显式关闭，三个本任务包卸载，新建 AVD 已停止，独立审计确认产品进程、UID 规则和 ADB 转发无残留。四轮后台、网络与专用 SSH 转发已清理。App 初始清理夹具写死的 `reporting_disabled` 字段与实际恢复的开启值不符，原件保留，由独立最终审计纠正；没有改变产品默认值。

本机再次从独立传回的 ZIP 还原全部八个后台帧，与设备侧逐条一致。小证据为 `target/android-e746-product-evidence/` 的 108 份文件，首轮漏报、四轮诊断与符号夹具失败均在内；后台各轮证据为 `target/android-e746-backend-attempt{1,2,3,4}-evidence/`。本轮完整 `verify-local.sh --quick`、笔记校验及差异检查通过，Linux 原生 119/119；本机未配置的 macOS 原生 CMake 和 Windows 完整原生阶段明确跳过，不转记设备通过。

### 固定 9bab 候选的完整重建与独立后台复验

固定 `9bab02959fc5ce4c4917e1f1ed3518d8727ef026` 全源码，复用本任务原 arm64 缓存重建原生 Release 配置正式 APK（开发证书）。正式 APK SHA-256 为 `863da2b02c80ace47e7f6b6d23769bc0be72796e4ec1dfa258efbaf737da5599`；四份包内库与 Gradle 实际剥离输出逐字节相等，自有库 build-id、未剥离哈希与独立符号归档一致。正式库不包含专用回环目标。此次替代先前“e746 基础包加单文件修补”的验证边界，仍不扩大为其他 ABI 或真机。

自有 API 28 AVD 复用既有目录，改用独立 `emulator-5600` 避开其他任务；忽略目录中的原专项副本仅更换端口白名单，仍核对真实 AVD 名、API 和实际进程，产品及原测试断言不改。正式包 `telemetry-watch` 退出 0，覆盖活跃撤回不采集、不可读暂停/恢复同一会话、重开换代、Java/原生故障入队与真实 HomeActivity 独立采集。首次预检因将“未安装包”的 `pm path` 退出 1 当作命令错误而停止，发生在安装前；原失败日志保留。

同源隔离测试特性另建 APK，SHA-256 为 `0abffd4252be205311804525b3eb196e5f9fa9ae87a6479efeb2172fbd2d0ca8`。固定官方后台镜像与 revision `ab391f834cb14baa88c98b1b8a2dbf03ac0da93a`，分别为真实 IME 和 App 开启空白数据库；实际 Java 致命异常及在原 JNI 调用入口注入空 JNIEnv 的真实 SIGSEGV，经产品重启后分别收到五条、三条事件，恰有四条 crash、两条 session_crash、两条 active，App 不产生输入会话事件。每条原 HTTP 与数据库七字段一致；SIGKILL 不增加崩溃。实际身份保持 9bab、原运行、attempt 1、aarch64 和对应组件。

测试 JNI build-id 为 `5911f547714d4b5eda11abef3f5088086c66c960`，独立 ZIP SHA-256 为 `0dedfcd59f0b723f22d90d4638f0c9e59d07b43ef77edd68c11d07ddbae3c770`。四条自有源码帧为 `client_jni.cpp:108/117`，另保留 `jni.h:852` 的两组帧；本机仅用传回 ZIP 再还原八个后台帧，与设备侧完全相等，错误 build-id 拒绝。

重新构建默认特性，Cargo/JNI 正式库与最初冻结正式库、Gradle 包内库全部逐字节一致，专用回环目标消失；完整原源码恢复到干净 `4d179327a4`。独立审计确认回收前关闭合成同意、卸载两个本任务包、清除精确 UID 规则/转发/调试文件并停止自有 AVD，其他两台共享 AVD 保持运行。两轮临时后台、数据库、内部网络和专用 SSH 隧道均清理。小证据 ZIP 共 60 份，SHA-256 为 `6ffe703a66077e13146c60ddbd8bea1cca77dd0ba513294a8541a7d02510c5b1`，位于 `target/android-9bab-small-evidence.zip`；本机独立复核另见 `target/android-9bab-evidence/independent-verification.json`。API 35 系统退出/ANR、本轮未跑的完整输入套件和真机仍为独立缺口。

## Consequences

这次只以同号待送达信号与默认处置确认致命动作，不从故障地址猜测是否可恢复。Android 的 libc 查询必须在安装前准备，解析失败不安装新的处理器；实际系统投递与其他平台的设备边界仍由全平台矩阵记录。
