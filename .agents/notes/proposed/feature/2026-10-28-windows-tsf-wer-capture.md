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

`CrashCaptureContext.h`、`TelemetryWer.cpp`、`.def` 和共用 `TelemetryWer.cmake` 已实现有界远端快照读取与三个回调。`TsfCrashCapture` 已接入产品 TSF 的激活、失败回滚、停用与析构；独立后台观察任务持有 DLL 加载引用，持续检查共享授权、发布有代次的快照。现代与 Win7 的构建、暂存、符号采集和安装器已接入 helper，Arm64X 沿用 TIP 的两阶段链接。**合成宿主的系统 WER 实际投递已通过，完整产品包与真实编辑器验收仍未完成。** 本篇保留 `proposed`，不把源码接线和专项测试当作产品闭环。

Windows 11（10.0.26200）、MSVC 19.44.35228 和 SDK 10.0.26100 上，x64/x86 实际构建与回调 CTest 均通过，分别为 0.13 秒和 0.17 秒；ARM64 的同源 helper 和测试程序构建通过，但未在 ARM64 系统运行。测试从另一个合成进程的实际地址读取上下文，覆盖非致命/停用/未准备/奇数代次、截短事件、损坏头与模块、外部模块边界、不可读地址/无内存读取权限、相对路径与备用数据流、目录重解析点、原头保留、自有模块偏移、首份完整及临时文件不覆盖，以及三个 WER 回调不改宿主输出。此次重解析点夹具实际运行，没有因权限跳过。未注册系统模块或制造真实致命故障，不能代替 WER 系统投递与 TSF 外部宿主验收。

初次 x64 实跑通过，但 x86 出现宿主输出损坏。静态类型断言和验证机原版 SDK 证明：微软的三个回调均为 `cdecl`，本机 MinGW 却声明为 `WINAPI`；32 位的调用栈因此不平衡，64 位不暴露该区别。`WerApiCompatibility.h` 统一使用微软 ABI，MSVC 下与原版 typedef 静态核对；x86 修正后通过。不能依据 MinGW 头文件或只跑 x64 再推断 x86 ABI 正确。

本机 MinGW x64 对同一 CMake 定义的 `-Wall -Wextra -Werror` 编译通过，helper 静态链接工具链运行时；这不代替 Win7、ARM64、Arm64X 的产品包验证。本机磁盘不足 50 GB，只编译此小型独立夹具，没有启动完整 Windows/Rust 构建树；完整 quick 仍在远端运行，尚未通过。

### 租约接线与卸载专项

最后租约释放只在生命周期锁内解除武装、增加代次并唤醒观察任务，不等待文件锁。新租约即使在旧任务准备中途加入也必须重新准备，旧结果不能短暂武装新租约。观察任务在注销成功前始终保留上下文；系统注销失败时保持停采并重试。回调通过 [FreeLibraryWhenCallbackReturns](https://learn.microsoft.com/en-us/windows/win32/api/threadpoolapiset/nf-threadpoolapiset-freelibrarywhencallbackreturns) 将加载引用交还线程池，避免在 DLL 代码仍执行时卸载自身。它只依赖 Vista 起提供的线程池接口，未引入 Win7 之后的新要求。

Windows 11 上，同源 MSVC x64/x86 严格编译及两条 CTest 通过：生产 helper 的跨进程边界测试分别为 0.10、0.13 秒，新增生命周期与落盘联动分别为 3.93、4.03 秒。新增测试将真实 `TsfCrashCapture.cpp` 编成可反复装卸的 DLL，仅替代共享 ABI 返回、路径发现和 WER 注册这三个边界，实际执行线程池、加载引用、租约与快照发布。验证多实例共存、读失败的同代次恢复、关闭重开、未观察到中间关闭的代次变化、阻塞偏好读取期间立即停用、加载引用保持、迟到准备结果、注销失败重试，以及二十次实际 DLL 装卸。准备结果另交给生产 WER helper，核对原头和自有模块偏移确实落盘，回调不认领宿主异常。ARM64 的相同源码严格编译通过，未在 ARM64 系统执行。26 个源码与配置文件逐项 SHA-256 与本机冻结快照一致。

第一版负对照移除代次比较后仍通过：唤醒事件让下一轮观察立即覆盖了短暂的旧武装，原测试只看最终状态。现用测试专用的刷新完成屏障固定这个窗口，再复制受锁保护的真实快照。移除代次比较后，同一回归稳定以 `late prepare armed replacement lease` 失败；恢复源码后两条 CTest 再次通过，生命周期 3.90 秒。首轮 PowerShell 包装器把预期的 CTest 标准错误当作终止异常，未算负对照通过；修正结果捕获后才得到上述证据。首轮 MSVC 测试导出声明、full 版 Host API 文件名夹具错误均已修正，失败轮不计通过。

同源 helper 另用 MSVC ARM64 和 ARM64EC 两阶段实际链接，最终 Arm64X DLL 已产出；这还不是 ARM64 设备运行或完整 TIP 产品链接。现代及 Win7 五份改动打包脚本按 UTF-8 解析通过，安装器既有 TSF 注册契约与新增 WER 允许列表契约通过，确保只添加完整版本路径、只删除自己的两个值而不删除系统键。Windows PowerShell 5 的默认文件编码曾使直接 ParseFile 失败，改为明确按仓库 UTF-8 源码 ParseInput 后通过，没有改动产品脚本来迁就测试机器编码。

整轮 quick 的上一冻结快照有两项 Windows 设置 PowerShell 检查分别在 20、40 秒超时，整轮结果为失败；它不包含这次 TSF 接线。后续同源 Android 完整 Release APK 与鸿蒙原生库、设置资源及 HAP 均已产出。旧基础 WER quick 已取消并核对全部子进程结束；`b3a071317d6d262ab38ce7efa0cbe1f3a281d286` 的 81 个文件已冻结校验，新接线的整轮 quick 在远端执行，尚未通过。不覆盖正在被任务读取的源码；以上专项均未向产品后台发送合成崩溃。


### 合成外部宿主的实际系统投递

`Invoke-SystemWer.ps1` 显式启动七组系统验收，不加入默认 CTest。夹具仍编译生产 `TsfCrashCapture.cpp` 与生产 helper；共享 ABI 和路径是合成边界，注册与注销边界改为调用真正的 Windows API。由未处理访问冲突触发系统 WER 服务，没有直接调用生产回调来冒充系统投递。两个自有模块的故障落盘保留控制器发布的原头、异常码和模块偏移；外部 EXE 的故障不记录，授权撤回后的自有故障不记录，重开后仅写新身份，释放最后租约并等待 DLL 卸载后无旧记录，宿主捕获并恢复的异常正常退出且不记录。所有致命用例核对实际退出码 `0xc0000005`，被捕获的用例核对退出码 0。

Windows 11 的同源 MSVC x64/x86 七组用例均通过；两架构原有回调和生命周期 CTest 也再次通过。ARM64 的相同系统夹具严格目标编译通过，未在 ARM64 系统运行。脚本只临时写入当前用户、本架构视图中属于此次 helper 完整路径的允许列表值，拒绝覆盖已有值，任何成功或失败都在 `finally` 删除自己的值并核对；不改变系统通用策略，不调用产品上传入口。输出目录必须新建，全部偏好和记录使用合成数据。

首次真实探针已证明生产 helper 可被 WER 调用。扩展夹具的首次 x64 编译与 CTest 通过，但 PowerShell 5 将无 BOM 的中文脚本误读而导致解析失败，未执行系统验收；新脚本显式使用 UTF-8 BOM 后，直接 `-File` 与脚本调用均可执行，没有改动既有产品打包脚本。MinGW 的严格编译拒绝直接常量空地址写入的边界警告，夹具改为读取 `volatile` 故障地址再触发；不关闭编译警告。实际 TIP 还需完整产品包、安装器、真实编辑器及后台验收，不能据七组合成进程推定这些均已完成。

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
