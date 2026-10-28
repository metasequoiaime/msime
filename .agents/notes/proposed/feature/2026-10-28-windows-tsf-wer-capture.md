# Agent Note: TSF 在外部宿主中的进程外故障采集

Status: proposed

## Problem

TSF DLL 在其他应用进程内执行，直接安装全进程未处理异常或 terminate 处理器会改变宿主处置，首次异常 VEH 还会误记已经被外层捕获的异常。共用独立采集器因此不能直接用于 TSF，当前缺少这一入口。

## Proposal

正常路径按共享偏好准备独立捕获 UUID、原构建头和自有模块范围，组件为 `tsf`，不建立输入会话或联网。通过 Windows 7 起提供的 `WerRegisterRuntimeExceptionModule` 注册独立 helper DLL；回调只接收致命事件，读取一致的预备快照，故障地址必须落在 TSF 或本 edition Host API 模块内。始终返回不认领，不修改事件名、签名与调试器参数，宿主继续原 WER 处置。只保存异常码、模块名与偏移，不读取宿主内存内容、用户文本或转储。

每个激活实例持有采集租约。独立后台任务每 500 ms 核对持续授权，读失败暂停，撤回重开换身份，晚准备不能重新武装已停用实例。最后租约释放立即停写，后台任务持有 DLL 加载引用，注销完成后才释放上下文和加载引用；停用不等待文件锁或网络，不在 DllMain 内做 I/O 或线程等待。安装器只登记自己 helper 的 WER 模块允许列表，卸载只移除对应值，不改变宿主或系统 WER 策略。

WER 回调使用目标进程地址，必须通过 ReadProcessMemory 读取，不把 pContext 当作 helper 本地指针。目标与 helper 的令牌用户和完整性必须匹配，拒绝跨安全上下文写盘。预备头和记录路径有固定上限，记录先写临时文件、同步后无覆盖发布，原首份记录保持。故障现场未准备、外部模块首帧、WER 被宿主或策略绕过时保守不采集，不声称百分之百覆盖。

文件操作前复制并模拟目标进程令牌，结束恢复调用线程的原令牌；受限令牌与 AppContainer 的访问能力随复制保留，不能仅凭同 SID 与完整性等级让 helper 用较宽权限代写。打开并保留每级父目录句柄，不共享删除，拒绝任何重解析点以及相对路径、设备路径、网络路径或备用数据流；临时记录使用 `CREATE_NEW`，同步后再次核验快照，最终文件无覆盖发布。

## 当前基础切片与验证

`CrashCaptureContext.h`、`TelemetryWer.cpp`、`.def` 和共用 `TelemetryWer.cmake` 已实现有界远端快照读取与三个回调；生产 TSF 的 CMake 和无需 Rust 的专项工程复用同一构建定义。**尚未接入 TSF 租约、授权观察、允许列表、安装器及 Arm64X，仍不构成产品覆盖。** 本篇保留 `proposed`，待完整接线后整体转为 `implemented`。

Windows 11（10.0.26200）、MSVC 19.44.35228 和 SDK 10.0.26100 上，x64/x86 实际构建与回调 CTest 均通过，分别为 0.13 秒和 0.17 秒；ARM64 的同源 helper 和测试程序构建通过，但未在 ARM64 系统运行。测试从另一个合成进程的实际地址读取上下文，覆盖非致命/停用/未准备/奇数代次、截短事件、损坏头与模块、外部模块边界、不可读地址/无内存读取权限、相对路径与备用数据流、目录重解析点、原头保留、自有模块偏移、首份完整及临时文件不覆盖，以及三个 WER 回调不改宿主输出。此次重解析点夹具实际运行，没有因权限跳过。未注册系统模块或制造真实致命故障，不能代替 WER 系统投递与 TSF 外部宿主验收。

初次 x64 实跑通过，但 x86 出现宿主输出损坏。静态类型断言和验证机原版 SDK 证明：微软的三个回调均为 `cdecl`，本机 MinGW 却声明为 `WINAPI`；32 位的调用栈因此不平衡，64 位不暴露该区别。`WerApiCompatibility.h` 统一使用微软 ABI，MSVC 下与原版 typedef 静态核对；x86 修正后通过。不能依据 MinGW 头文件或只跑 x64 再推断 x86 ABI 正确。

本机 MinGW x64 对同一 CMake 定义的 `-Wall -Wextra -Werror` 编译通过，helper 静态链接工具链运行时；这不代替 Win7、ARM64、Arm64X 的产品包验证。本机磁盘不足 50 GB，只编译此小型独立夹具，没有启动完整 Windows/Rust 构建树；完整 quick 仍在远端运行，尚未通过。

## Related notes

检索活跃笔记的 WER、RuntimeException、TSF 故障和外部宿主崩溃后，[全平台提案](2026-10-27-cross-platform-crash-reporting.md)部分重叠，仍持有完整平台及后台验收；[独立捕获与符号身份](../../implemented/feature/2026-10-27-native-crash-capture-and-symbol-identity.md)仍持有记录头与符号格式；[采集与队列代次](../../implemented/bug-fix/2026-10-28-epoch-bound-crash-collection.md)仍持有授权和旧记录拒绝。不完全吸收旧笔记，其余按键与焦点记录无关。

## 官方依据

[注册接口](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werregisterruntimeexceptionmodule)要求三个命名导出、模块允许列表和退出前注销；无处理器认领时保留 native WER。[事件回调](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nc-werapi-pfn_wer_runtime_exception_event)明确不认领时不能填写其余输出。[系统设置](https://learn.microsoft.com/en-us/windows/win32/wer/wer-settings)明确 RuntimeExceptionHelperModules 的完整路径值名，值内容被忽略。

三个回调的调用约定以[微软 SDK 头文件](https://github.com/microsoft/win32metadata/blob/main/generation/WinSDK/RecompiledIdlHeaders/um/WerApi.h)及验证机上的原版 SDK 为准，不能直接使用 MinGW 在 x86 下的不同 typedef。

## Alternatives considered

- **共用 CaptureOnly**：已有授权观察和版本头，但它安装全进程 unhandled filter 与 terminate 处理器，改变外部宿主行为，不能直接复用。
- **首次异常 VEH**：不需要进程外 helper，但无法判断异常后来是否被捕获，会把正常恢复算崩溃。
- **仅在 COM 方法外套 SEH**：边界容易部署，却会漏掉后台任务，并把已被更外层宿主处理的异常误记为致命故障。

## Acceptance criteria

- 实际 helper 回调拒绝非致命、外部地址、损坏/不一致/停采上下文，不改其余 WER 输出；自己的异常按原头落盘且不覆盖首份。
- 产品 TSF 的激活、失败回滚、停用与析构租约配对；后台持有加载引用，注销后才释放上下文，没有 loader lock 内等待。
- x86/x64/ARM64 与 Arm64X 的产品构建、安装/卸载模块名单与符号包覆盖 helper。
- 真实 Windows 外部宿主里分别触发自有和非自有故障、捕获恢复、撤回换代及反复卸载；核对系统 WER 原处置和后台精确符号化。

## Risks

WER 可能被宿主自有处理器、调试器或系统策略绕过；系统回调没有交付保证。预备期间和模块范围外的故障保守不记录。实际 Windows、安全令牌和多架构验收未完成前只报告实现与已执行的目标编译，不能称 TSF 已完整覆盖。
