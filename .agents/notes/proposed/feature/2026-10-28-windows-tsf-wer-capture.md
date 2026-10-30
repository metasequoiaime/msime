# Agent Note: TSF 在外部宿主中的进程外故障采集

Status: proposed

## Problem

TSF DLL 在其他应用进程内执行，直接安装全进程未处理异常或 terminate 处理器会改变宿主处置，首次异常 VEH 还会误记已经被外层捕获的异常。共用独立采集器因此不能直接用于 TSF，当前缺少这一入口。

## Proposal

正常路径按共享偏好准备独立捕获 UUID、原构建头和自有模块范围，组件为 `tsf`，不建立输入会话或联网。通过 Windows 7 起提供的 `WerRegisterRuntimeExceptionModule` 注册独立 helper DLL；回调只接收致命事件，读取一致的预备快照，故障地址必须落在 TSF 或本 edition Host API 模块内。始终返回不认领，不修改事件名、签名与调试器参数，宿主继续原 WER 处置。只保存异常码、模块名与偏移，不读取宿主内存内容、用户文本或转储。

同一授权代次且快照已武装时，每轮只读取一次同意，不重新发布模块快照；首次准备和读失败后恢复仍在发布前复读授权，租约换代仍拒绝迟到结果。回归用读取计数及刷新/发布屏障固定窗口，不靠睡眠猜测一轮是否完成。

每个激活实例持有采集租约。独立后台任务先订阅偏好目录的文件名、写入及权限变化，再核对持续授权；另监听父目录的目录名变化，目录删除或替换后重新订阅实际路径。稳定状态不再每 500 ms 读文件，只在通知、租约唤醒及 30 秒兜底时复核。通知建立失败、读取失败、准备失败或注销失败时仍每 500 ms 重试；网络盘不依赖可能不投递的通知。兜底时重建观察句柄，覆盖更上层目录替换。读失败暂停，撤回重开换身份，晚准备不能重新武装已停用实例。最后租约释放立即停写，后台任务持有 DLL 加载引用，注销完成后才释放上下文和加载引用；停用不等待文件锁或网络，不在 DllMain 内做 I/O 或线程等待。安装器只登记自己 helper 的 WER 模块允许列表，卸载只移除对应值，不改变宿主或系统 WER 策略。

目录通知优化的专项保留原有最终授权屏障、迟到结果与卸载测试，并核对空闲跨过多个旧轮询周期不增加读取，实际文件原子替换、目录移走和重建仍及时停采和恢复。采用 Win7 已有的 `FindFirstChangeNotificationW` / `WaitForMultipleObjects`，不引入新系统要求；[目录通知契约](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstchangenotificationw)明确不报告被观察目录自身的变化，因此必须另看父目录，并只在句柄实际触发后调用 [FindNextChangeNotification](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findnextchangenotification)。

WER 回调使用目标进程地址，必须通过 ReadProcessMemory 读取，不把 pContext 当作 helper 本地指针。回调运行在 `WerFault` 进程，不能把 helper 自身的令牌和崩溃目标令牌直接比较；注册的 WER helper 边界负责确认调用来源，写盘前仍复制并模拟目标进程令牌，路径、目录句柄和模块归属检查拒绝跨上下文越权。预备头和记录路径有固定上限，记录先写临时文件、同步后无覆盖发布，原首份记录保持。故障现场未准备、外部模块首帧、WER 被宿主或策略绕过时保守不记录，不声称百分之百投递。

文件操作前复制并模拟目标进程令牌，结束恢复调用线程的原令牌；受限令牌与 AppContainer 的访问能力随复制保留，不能仅凭同 SID 与完整性等级让 helper 用较宽权限代写。打开并保留每级父目录句柄，不共享删除，拒绝任何重解析点以及相对路径、设备路径、网络路径或备用数据流；临时记录使用 `CREATE_NEW`，同步后再次核验快照，最终文件无覆盖发布。

## 当前基础切片与验证

完整安装包的首次实际 ISCC 编译在 WER 注册项失败：`[Registry]` 不支持 `Flags: 32bit`，原契约检查却接受了这个写法。x86 helper 现在按 [Inno Setup 的注册表视图约定](https://jrsoftware.org/ishelp/topic_registrysection.htm)用 `Root: HKLM32` 明确选择 32 位视图，原生 helper 保持默认视图；两条记录仍只以 `uninsdeletevalue` 删除自己的完整路径值。修正后的契约检查在旧源码上实际失败，替换成正确注册项后通过；完整安装包随后实际编译成功，耗时 125.719 秒。空白 Windows 安装验收另行记录，不能以文本检查或编译代替。

### Windows 11 完整安装包验收（2026-10-31）

固定二进制源码 `ffc17be201` 已完成 x64、x86、ARM64 的同源 Rust、MSVC 产品与 WinUI 设置构建，原构建脚本 PE 架构门禁通过。官方插件复用 Ubuntu nightly `40966284976` 的 artifact `12937468491`，21 份固定输入和插件内容哈希一致，未改锁。第一次独立符号归档拒绝四份同 GUID、PE age 1 / PDB age 2 的 x64 产物；保留失败后只重新链接 TSF、Server、Watchdog、准备程序，再暂存签名和归档。最终独立 ZIP SHA-256 为 `6e3d50e9def86e7ac8865ad1897c4088bfa4d499c149f029cb4206b8afcb7410`，15 组 PE/PDB 的 GUID、age、逐文件哈希全通过，错 PDB 负例被拒绝。

`231529bbac` 只修正安装器源码，完整包版本仍为 `0.3.0`；包 SHA-256 为 `c66c0a49a2f5146c6226b1934ebe832b889f88c6835d614b2034893ab38f86f5`，使用本机测试证书签名，`uiAccess=false`，未发布。专用空白 Windows 11 VM 106 全程保持默认出站阻断与显式外网阻断，只允许任务控制通道。传输后实际核对包、公钥证书哈希和 Authenticode。公钥证书首次导入失败发生在安装前，使用系统 `certutil` 正常导入后核对签名链，不跳过真实签名验证。

完整包首次安装因缺少 x86 VC++ 运行库在 x86 `regsvr32` 阶段失败，原安装器回滚，三个产品目录均不存在。Microsoft 官方完整签名运行库补齐后，同一包的原 `install-smoke.ps1` 完成 x64/x86 COM/TIP 注册与 x64 TIP 激活、Server/MCP 启动、原地升级保留数据、数据目录迁移及卸载，最终 `JOB_EXIT=0`。安装日志 ZIP SHA-256 为 `d07285a2480994c4aa234c88c6c47be5e46d5ebf04aeb1611faa9bdb20e9880f`。运行库预检修复与 Pascal 正负例见[独立笔记](../../implemented/bug-fix/2026-10-31-windows-x86-runtime-prerequisite.md)。

随后重新安装同一签名包，实际通过 .NET 的 Registry32/Registry64 视图读取自己的完整 helper 路径、值 `0` 和已安装文件。包内正式 Host API 的 `telemetry_begin`、`telemetry_flush` 均在创建队列前拒绝测试字段。再次经原卸载器移除产品与数据，两个视图中自己的 WER 值全部删除，最终 `JOB_EXIT=0`。首轮夹具因把无安装值的空注册表容器误认为安装残留而在安装前退出；独立盘点确认三个产品目录及安装值均无残留后，只修正夹具判定并保留失败原件。

后续运行库预检修复提交 `2c2587184a` 复用同一已签名 payload，15 份自有映像哈希再核对不变，重新执行原完整 ISCC，113.047 秒编译成功。新包 SHA-256 为 `d22d65a22eb3f235394cb0ef6de306a94858d0d67422330d0dbc7f0931490841`；其安装器签名使用当前用户可用的另一张明确核对身份的本机测试证书，公钥 SHA-256 为 `3a6b137a6cf8a8938018ffd97e58a2250f3f7cddeecd3624aa8dc438e139e279`，产品 payload 仍保持原签名。首轮包装器错误地要求两次签名者相同而拒绝，保留失败；没有跳过 Authenticode Valid 或证书精确核对。内网传输首次经过代理得到 HTTP 502，改为本次调用的 `-NoProxy` 后通过，未修改机器代理或外网隔离。

新包按实际大小和哈希完成传输，公钥经系统证书工具信任并实际验签，随后同一原烟测再次全程通过，最终 `JOB_EXIT=0`，单独使用新的合成数据目录及日志，未覆盖前一包证据。新日志 ZIP SHA-256 为 `ffcdc03e8c0923684759028ec040c8ca834f795f3dfaa9112dabae578680825e`。新包另一次实际重装也通过两个注册表视图的 WER 核对、正式 ABI 拒绝测试字段、原卸载及自有 WER 值移除，最终 `JOB_EXIT=0`。新预检的 Pascal 逻辑矩阵、源码门禁和新包安装结果分别记录；这是 `2c2587184a` 的安装器及 `ffc17be201` 的产品二进制组合，不声称全部二进制来自新提交重编。

本轮是完整安装生命周期、实际 WER 允许项和正式上报隔离边界验证，不把注册项存在当作已安装包的真实编辑器故障或后台闭环；ARM64、Win7 与其余入口的产品运行缺口仍在。小证据保存在 `target/windows106-complete-install-evidence/`，独立符号明细在构建机 `target/windows-complete-package/exact-symbol-pairs.json`。两包收据和 15 组符号证明均已在本机独立复核，归档 ZIP SHA-256 为 `e80ab5c386325be7c22b1b3506f2b97925100f72cad3160e56c3d7d15c55c32d`。独立卸载审计确认产品目录、双视图安装值、自有 WER 值和产品进程均无残留，防火墙隔离仍有效。任务三项构建/打包/传输计划均退出 0 后移除，私有传输脚本和控制服务已清理；专用 VM 106 关机保留，供尚未完成的已安装包编辑器验收继续使用。构建缓存及冻结符号保留。

`CrashCaptureContext.h`、`TelemetryWer.cpp`、`.def` 和共用 `TelemetryWer.cmake` 已实现有界远端快照读取与三个回调。`TsfCrashCapture` 已接入产品 TSF 的激活、失败回滚、停用与析构；独立后台观察任务持有 DLL 加载引用，持续检查共享授权、发布有代次的快照。现代与 Win7 的构建、暂存、符号采集和安装器已接入 helper，Arm64X 沿用 TIP 的两阶段链接。**合成宿主系统 WER 和固定候选 x64/x86 真实编辑器的故障、授权、符号证据已取得，后续完整包生命周期见上节；已安装包真实编辑器故障、ARM64 与 Win7 产品运行仍未完成。** 本篇保留 `proposed`，不把源码接线和专项测试当作产品闭环。

WER helper 的 x86 分支现在沿目标进程的 EBP 链最多读取 64 个返回帧，远程读取失败、回环、倒退或超过 1 MiB 的跳跃立即停止；只有 TSF 与当前 edition Host API 的帧写入记录，外部宿主帧不泄露。x64/ARM64 继续保留直接故障帧，因为 `RtlVirtualUnwind` 不能直接解引用 helper 地址空间之外的目标栈。x64 与 x86 生产 helper 均通过 MinGW `-Wall -Wextra -Werror` 语法检查；当前 macOS 无法执行 Windows x86 回调，真实 x86 运行回归仍由 Windows CI 验证。

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

为避免系统投递专项只在维护者机器上运行，Windows 平台门禁现在在 Windows runner 中用 MSVC 分别配置 x64 与 x86 的独立 `capture_lifecycle` CMake 测试树，复用前序 Win7 设置工程已有的 nlohmann-json 头文件配置，不重新构建完整产品依赖或错误混用 x64 Rust 导入库。构建生产 `TsfCrashCapture.cpp`、`MetasequoiaImeCrashWer.dll` 和合成宿主，再直接调用 `Invoke-SystemWer.ps1`。每个架构先执行回调与生命周期 CTest，再执行七组系统 WER 用例；artifact 仅上传系统专项合成目录与 JUnit 结果，不上传整个构建树。首轮用相同独立工程在 Windows 11 实跑发现最新发布前授权屏障的三个夹具函数没有列入 `.def`，生命周期 CTest 在加载 `msime_test_tsf_block_publish` 时失败；补齐显式导出后继续复验。候选 `6bc13ef19d` 的 CI `40834862691` 已实际完成 x64/x86 WER 系统投递步骤并报告 success；这不包含后续同代次快照复用优化及新增计数/屏障回归，也不能据此唯一归因此前 runner 的系统投递失败。用例不加载完整产品、不注册输入法、不调用生产后台，所以它证明的是 WER 注册、回调边界、授权撤回和宿主处置在持续集成中的可重复性，仍不等于完整安装包、真实编辑器、ARM64 设备或后台源码还原。

### 自动化进程的错误模式

系统故障夹具在自身进程读取并清除继承的 `SEM_NOGPFAULTERRORBOX`，使用 `WER_FAULT_REPORTING_NO_UI` 关闭弹窗，同时记录继承值与实际值。微软 `SetErrorMode` 契约明确前者禁用整个 WER，子进程默认继承父进程错误模式；不能把关闭错误弹窗等同于保留系统投递。此调整只作用于合成 EXE，生产 TSF 不修改加载它的外部应用。Windows 11 上本轮同源 MSVC x64/x86 的两条 CTest 与各七组真实系统投递均通过，243 个输入文件逐项核对。该机器继承值为 32769，不含禁用位；因此正常环境通过不能据此认定 CI runner 的失败根因，受控父进程显式设置禁用位后，旧夹具在原三十秒窗口没有记录，新夹具继承 32771、清除后为 32769，并经真实系统 WER 留下一份正确原头的记录；父进程模式、固定源码与允许列表均恢复。对照最初两次分别缺少临时 helper 允许列表、以及复制旧源码后因时间戳使 MSBuild 复用新二进制，均作废；补齐边界、强制更新时间并断言实际旧二进制后重跑通过。仍等待 CI 的实际模式证据，不能将这个受控缺陷直接当作 runner 的已确认根因。

### 暂存与签名边界

现代包在替换暂存前明确要求 x86、x64、Arm64X 三份 `MetasequoiaImeCrashWer.dll`；Win7 x64 要求 x86/x64，Win7 x86 只要求 x86。不能仅靠扫描依赖目录推断 helper 存在，否则预编译或手动指定输入会静默漏装。helper 仅暂存在各架构 TIP 目录，Server 目录清除重复 DLL 与 PDB；符号继续由构建输出归档，不向安装目录重复复制 helper PDB。

SignPath 的 Stage 与 Restore 将三个架构的 helper 列为必需自有二进制，保留不签上游 DLL 的规则；Stage 先核对全部源文件再重建请求目录，缺 helper 不破坏上一份请求。此前 CI 的 Build-Client 探针没有生成新增 helper 的合成 PE，因此在生产架构检查处失败；夹具补齐相应架构，并增加缺失和错架构负例。安装器回归覆盖现代、版本化与 Win7 包，逐架构缺 helper 必须预检失败且原暂存逐文件哈希不变；签名回归核对 Stage 精确集合、缺 helper 和 Restore 返回不完整的拒绝。完整 `package-files.ps1` 同时纳入 Windows release script CI，避免它只在手工运行时才验证暂存行为。

Windows 11 上，冻结源码的四组构建工具探针和七组安装器脚本测试共十一组全部通过，1236 个输入文件的 SHA-256 与本机逐项一致。未构建、签名、安装产品，也未向产品后台上传。首次命令在文件传输完成前执行，未找到脚本，不能算验证；完成传输后整套重新执行并退出 0。最新整合快照的完整 quick 仍待执行，不把这些脚本测试当作真实签名服务或完整产品安装验收。

### 本轮真实产品 Server 验收（2026-10-30）

在 PVE 的 Windows 11 runner 上，使用当前分支同源构建的 `MetasequoiaImeServer.exe`、`msime_host_api.dll`、`MetasequoiaImeCrashWer.dll` 和锁定资源目录，先通过 `msime-client-prepare.exe` 准备全新状态目录，再以专用注册器调用真实 `DllRegisterServer` 完成 TSF profile 与 WER helper 注册。注册器必须先 `CoInitializeEx`，并为 DLL 搜索路径加入暂存的 `bin` 目录；直接从非 COM 上下文运行 `regsvr32` 返回 `E_FAIL`，不能把它记作注册成功。

随后对真实运行中的 `MetasequoiaImeServer.exe` 的自然 `msime_client_telemetry_consent` 调用设置断点，读取真实请求后恢复原始首字节，将请求指针改为 `1` 并继续执行。目标进程退出码为 `0xc0000005`，看门狗自动重新拉起 Server；`C:\ProgramData\Microsoft\Windows\WER\ReportArchive` 生成 `APPCRASH` 报告，签名模块为暂存的 `msime_host_api.dll`，异常码为 `c0000005`，同时留下两个 `MetasequoiaImeServer.exe.*.dmp`。这证明完整产品 Server 的原生 WER 处置与重启链路已经实际运行，仍未证明 WER helper 将该真实产品事件写入后台队列或完成符号还原。

该报告的 `msime_host_api.dll` PE CodeView GUID/Age 与冻结的 `msime_host_api.pdb` 精确一致（`{22A9F508-7A04-4D9B-8155-DC44AF05FAA4}`, age `3`）；用镜像基址加 WER 偏移交给 `llvm-symbolizer` 可还原到 `<serde_json::read::SliceRead as serde_json::read::Read>::peek`。行号仍为 `??:0`，因为故障是人为把真实 consent 请求指针改成 `1` 后在 JSON 读取路径触发的访问冲突，不能把该位置当作自然业务缺陷。

此前跨进程启动或激活 Notepad 未产生 TSF worker 的尝试不算输入通过；2026-10-30 的实际编辑器验收见下节。Server 的故障证据仍不能替代 TSF，全部取证使用专用用户、专用状态目录与隔离后台。

随后发现前一轮产品包中的 `msime_host_api.dll` 仍嵌入旧的隔离端口 `http://127.0.0.1:48153`，而当前专用回环转发为 `48729`；队列与持续授权均正常，故此前后台为空只是发送目标不一致。强制让同源 Windows 工作区重新编译 `msime-client-services`/Host API，并替换暂存 DLL 后重启 Server，回环后台固定 revision `ab391f834cb14baa88c98b1b8a2dbf03ac0da93a` 收到并确认了两条 `active`、两条 `session_crash` 和两条 `crash`，每条的 `platform=windows`、版本、匿名安装身份、构建头、`x86_64`、`input` 组件与模块偏移栈均与本地队列一致，`received` 与 `rows` 完全对应。这个真实 Server WER→本地队列→隔离后台链路已通过；x64 Notepad 的独立 TSF 证据见下节，ARM64/Win7 产品运行仍未完成。

## Windows 11 Notepad 的 x64 产品链路

固定产品源码 `b8356db26bca126193edbc5d5246ef51795eab8d` 的暂存 Server、TIP、WER helper 与 Host API 在 PVE Windows 11 专用交互用户下运行。测试 helper 通过 `IApplicationActivationManager::ActivateApplication` 和稳定 AUMID `Microsoft.WindowsNotepad_8wekyb3d8bbwe!App` 启动真实打包版 Notepad；带版本号的 package full name 不是 AUMID。只选择返回 PID 所属的编辑器窗口，连接编辑器线程输入队列后确认 `RichEditD2DPT` 真正持有焦点。产品 worker 数量为一，真实虚拟键 `NIHAO` 显示 Server 候选，空格提交合成的「你好」。不以自定义编辑器、直接写文本或仅激活返回 `S_OK` 代替验收。微软[激活接口](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateapplication)返回实际履行启动契约的 PID。

第一次有效故障在 Notepad 的自然授权调用断点恢复原始指令后，将请求指针改为 `1` 并在继续前脱离调试器。进程以 `0xc0000005` 结束，系统 WER 记录 `msime_host_api.dll+0x35b3d97`，生产 helper 写出 UUID `629971d3-0a9c-40d2-9a71-c63dfc6136f8`。该 Host DLL 的 PDB 已被后续缓存重编覆盖，旧 PDB GUID 不匹配，不把这一帧算作精确符号通过。

第二次在另一轮真实 Notepad 进程中，恢复断点原指令后，把执行位置改到已加载 TSF 的真实写指令、目的寄存器改为无效地址，再脱离调试器。没有改磁盘产品或添加产品故障入口。这是受控故障注入，不能称为自然触发 `OnTestKeyDown` 或该业务位置的已确认缺陷。系统事件 1000/1001 和 `MoAppCrash` 报告均为 `0xc0000005`、`MetasequoiaImeTsf.dll+0x7bdcc`，helper 写出 UUID `0ad1c46f-66fd-4721-bed2-39b32a766563`。打包宿主最终退出码为 `0xff`，要求退出码必须等于异常码的首次包装器断言因此失败；保留该失败，不将退出码改写为通过，真实异常证据来自原系统 WER 与独立 helper。此前等待自然按键断点的两次尝试均超时，也不计通过。

Server 正常停机后由原产品启动入口重启，把上述两条 `component=tsf`、`x86_64`、原版本、原 revision、运行 `windows-editor-20261030`、attempt `1` 的记录发往固定隔离后台 `ab391f834cb14baa88c98b1b8a2dbf03ac0da93a`。各 UUID 恰好一条 `crash`，请求与数据库的七个事件字段逐项相等，均无 `session_crash`。授权代次只用于本地，不外发。第二条的后台帧由独立 x64 验收符号 ZIP 解压后还原到 `CMetasequoiaIME::OnTestKeyDown`、`platforms/windows/tsf/Key/KeyEventSink.cpp:2415`；PE/PDB GUID `{22A46558-51D6-429D-A5AF-84B3E41D9C7A}`、age `1` 精确相等，DLL/PDB SHA-256 与归档回读一致，另一份 Debug 构建的 PDB 身份被拒绝。该 ZIP 只覆盖这次 x64 TSF 验收，不冒充完整发布符号包。

原 Server 重启后，再次由真实 Notepad 完成按键、候选与「你好」提交，证明输入恢复。最终关闭本任务的编辑器、Server、Watchdog，移除任务计划、两种分隔符的任务 helper 允许列表项，并按原始不存在的基线恢复 TIP/CLSID 和任务写入的状态根注册值。验收脚本曾把 helper 路径以 `/` 登记；本轮改为模块实际使用反斜杠的路径后才取得有效 WER 投递，这属于测试环境修正。Session 0 向交互 Server 投递退出消息失败，改由同用户交互任务正常关闭；非提权用户的机器级注销失败，最终以具备机器权限的同一注销 helper 清理并核对基线。这些失败不作为产品修复或系统卸载验收。

小证据在 `target/windows-tsf-notepad-acceptance-evidence/`，冻结符号 ZIP 在远端暂存目录的 `evidence/tsf-exact-symbols.zip`。真实编辑器的撤回重开、宿主捕获异常后继续运行与非自有故障对照，完整安装器、Win7、x86、ARM64 和最新整合候选复验仍未完成，不因这条 x64 自有故障链路通过而关闭本提案。

## Related notes

检索活跃笔记的 WER、RuntimeException、TSF 故障和外部宿主崩溃后，[全平台提案](2026-10-27-cross-platform-crash-reporting.md)部分重叠，仍持有完整平台及后台验收；[独立捕获与符号身份](../../implemented/feature/2026-10-27-native-crash-capture-and-symbol-identity.md)仍持有记录头与符号格式；[采集与队列代次](../../implemented/bug-fix/2026-10-28-epoch-bound-crash-collection.md)仍持有授权和旧记录拒绝。不完全吸收旧笔记，其余按键与焦点记录无关。

## 官方依据

[注册接口](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werregisterruntimeexceptionmodule)要求三个命名导出、模块允许列表和退出前注销；无处理器认领时保留 native WER。[事件回调](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nc-werapi-pfn_wer_runtime_exception_event)明确不认领时不能填写其余输出。[系统设置](https://learn.microsoft.com/en-us/windows/win32/wer/wer-settings)明确 RuntimeExceptionHelperModules 的完整路径值名，值内容被忽略。

三个回调的调用约定以[微软 SDK 头文件](https://github.com/microsoft/win32metadata/blob/main/generation/WinSDK/RecompiledIdlHeaders/um/WerApi.h)及验证机上的原版 SDK 为准，不能直接使用 MinGW 在 x86 下的不同 typedef。

同代次复用优化首先通过既有 MinGW x64 测试树的严格编译和链接，四个 C++ 源文件亦通过 MinGW x86 严格语法检查。随后经 PVE 跳转重新接通 Windows 11 验证机，复用既有 MSVC 构建缓存，实际运行固定候选 `1ae2cf6c67207114fa19a388d44a797082193397` 的 x64/x86 回调与生命周期 CTest，分别全部通过（7.12、7.18 秒）。读取计数、首次准备/暂停恢复的最终授权屏障与反复 DLL 装卸均实际执行；两架构各七组真实系统 WER 用例全部通过，任务 helper 允许列表值已清理，未调用产品上传入口。243 个源码输入在构建前后逐项 SHA-256 一致，证据为 `target/pve-consent-1ae2cf6c67-evidence/`。旧 `win11` 别名连接失败不能代表 PVE 内的验证环境不可达；没有修改 SSH 配置或重启虚拟机。

首次测试包装器因 PowerShell 5 误读无 BOM 中文而在解析阶段失败，未配置或执行测试；保留失败日志，为任务脚本补 UTF-8 BOM 后才取得上述完整结果。此前 `f21ba97116` 的 CI 被新提交推送取消，不将未执行完的 WER 步骤记作通过；本次目标运行证据也不替代完整安装包、真实编辑器、ARM64、Win7 或后台符号化验收。

PR head `6bc13ef19d` 对应的 Native Platform CI `40834862691` 中，`Build and run TSF system WER capture (x64 and x86)` 步骤已实际完成并报告 success。此前 runner 系统投递失败本轮未复现；该结果属于本轮同代次复用优化之前的候选，不替代新增读取计数与恢复屏障的运行验收，也不能单凭通过反推原失败根因。

## Alternatives considered

- **超时后保存进程内存转储**：可提供线程调用栈，最利于定位 WER 等待；但即使合成目标也继承运行环境，转储可能扩大诊断内容范围。本轮仅保留本目标最多 128 条线程的 ID、状态与等待原因，以及匹配该 PID 的最多 16 个 WER worker 的 ID、名字与父 PID，不输出内存、环境或命令行。
- **固定每 500 ms 轮询授权**：实现简单，目录通知不可用时仍能及时恢复；但每个加载 TSF 的应用都重复解析偏好并争用文件锁。正常状态改为通知和低频兜底，只有失败状态保留快速重试。
- **共用 CaptureOnly**：已有授权观察和版本头，但它安装全进程 unhandled filter 与 terminate 处理器，改变外部宿主行为，不能直接复用。
- **首次异常 VEH**：不需要进程外 helper，但无法判断异常后来是否被捕获，会把正常恢复算崩溃。
- **仅在 COM 方法外套 SEH**：边界容易部署，却会漏掉后台任务，并把已被更外层宿主处理的异常误记为致命故障。

### 完整安装包的已安装编辑器验收边界（2026-10-30）

新预检包在专用 Windows 11 VM 106 按生产 `--production` 启动 Server，三条 TSF 管道握手通过；系统记事本由原产品 TIP 接管，真实虚拟键 `NIHAO` 显示候选并以空格提交合成「你好」，安装版 TSF worker 数为 1。已安装 `MetasequoiaImeTsf.dll` 的 `current_state` 读取到 `armed=true`，头部为版本 `0.3.0`、二进制 revision `ffc17be20180fbb8b9a43ea1498dfb6db86de1c0`、`component=tsf`、`arch=x86_64`；15 组归档精确符号中该 DLL/PDB GUID `C993DD62-22AA-4E1C-B400-9E29D23BF73F`、age 1 匹配，`llvm-symbolizer` 将 `+0x7bdcc` 还原到 `KeyEventSink.cpp:2415`，反汇编是 `mov dword ptr [r15], eax`。

受控故障夹具将线程指令和寄存器改写后，目标记事本没有退出，Application 日志没有该 PID 的 1000/1001，独立诊断只看到无关的 `MoAppCrash`；没有生成或伪造产品故障记录。因此这轮只计真实编辑器输入和已安装采集状态通过，不计已安装包故障上报通过。失败的夹具输出、快照、无关系统事件和清理尝试全部保留在 `target/windows-installed-editor-ffc-evidence/`；后续需修正调试脱离/故障注入夹具后重跑，不能把合成宿主 WER 或旧暂存产品证据扩大为完整安装包闭环。

## Acceptance criteria

- 实际 helper 回调拒绝非致命、外部地址、损坏/不一致/停采上下文，不改其余 WER 输出；自己的异常按原头落盘且不覆盖首份。
- 产品 TSF 的激活、失败回滚、停用与析构租约配对；后台持有加载引用，注销后才释放上下文，没有 loader lock 内等待。
- x86/x64/ARM64 与 Arm64X 的产品构建、安装/卸载模块名单与符号包覆盖 helper。
- 真实 Windows 外部宿主里分别触发自有和非自有故障、捕获恢复、撤回换代及反复卸载；核对系统 WER 原处置和后台精确符号化。

## Risks

WER 可能被宿主自有处理器、调试器或系统策略绕过；系统回调没有交付保证。预备期间和模块范围外的故障保守不记录。实际 Windows、安全令牌和多架构验收未完成前只报告实现与已执行的目标编译，不能称 TSF 已完整覆盖。

### 目录通知与空闲成本验证

基于 `95cc68519e` 的目录通知候选在 Windows 11、MSVC x64/x86 严格编译通过；两架构回调与生命周期 CTest 分别为 4.26、4.24 秒，七组真实系统 WER 用例分别全部通过。新增回归跨三个旧轮询周期核对读取不增长；实际原子替换文件、目录移走/删除/重建后的撤回与恢复通过；读失败后不发文件通知也能快速恢复。首次准备和恢复的最终授权、阻塞 I/O 下释放租约、迟到结果、注销失败重试及二十次 DLL 装卸继续通过。243 个输入文件在执行前后逐项一致，helper 允许列表已清理，未调用产品后台。证据为 `target/pve-notifications-95cc-evidence/`。

负对照只把稳定等待改回 500 ms，新增回归实际以 `idle observer still polls consent every 500 ms` 失败；恢复逐字节生产源码并强制更新时间后重新构建，两条 CTest 通过。另一个合成 x64 进程不通知地撤回授权，最初两秒保持原状态，在约 28.985 秒后由三十秒兜底复核解除武装，只新增一次读取。低频兜底不是永久缓存授权。首轮测试的变量遮蔽被 `/WX` 拒绝，改名后完整重跑；一次传输未结束就启动包装器只得到文件缺失，未算执行通过。

同一代码快照的完整 quick 与包装器均退出 0，12,444 个源码条目前后相同。203 项可移植 Windows 用例、契约、Rust workspace/Android/wasm、Android 宿主、鸿蒙 HAP、macOS 原生和 Apple bridge 编译通过；远端没有 Docker/MinGW，Linux 原生/设置及 Windows 原生/pipe-only/x86 语法跳过。另复用本机已有独立测试缓存完成 MinGW x64 严格编译链接；外层残留 CMakeCache 没有 Makefile 的首次命令失败，改用实际 `capture-lifecycle` 缓存才编译通过。以上不代替完整产品、编辑器、Win7 或 ARM64 运行验收。


### CI 的目标退出超时

候选 `95cc68519e` 的 Native Platform CI `40853458086` 实际完成 x64 七组系统 WER；x86 `tip` 通过，随后 `host` 在 45 秒目标退出等待处失败。上传 artifact 已有该 `host` 的唯一完整原头记录及 `msime_host_api.dll+0x3fb0e`，不能把这个失败称为未采集，也不能把落盘等同于系统原处置已经结束。x64/x86 的 CTest 已通过。当前没有转储或仍存活的 runner 进程，目标为何未退出尚未确认。

包装器原先只在目标正常结束后保存 stdout/stderr，超时会丢失这部分证据。超时现在先写合成案例名、PID、目标结束状态和已完成/未完成记录名，失败清理后仅保存已经完成的输出任务，不能在诊断阶段再无限等待管道。四十五秒期限、致命退出码、原头、记录数量和允许列表清理断言保持原样；不据单份记录跳过失败，也不延长超时掩盖根因。更新后的同源脚本在 PVE 重新完成 MSVC x64/x86 构建、回调及生命周期 CTest、各七组真实系统 WER，用例全部通过。独立合成进程写入固定输出和完成/部分记录后休眠，实际包装器在原四十五秒期限失败，超时 JSON、输出保存、子进程回收及允许列表清理均通过；此探针不触发真实故障或上传。243 个输入文件前后相同，证据为 `target/pve-timeout-c713-evidence/`。同一快照的完整 quick 与包装器退出 0，12,444 个源码条目前后一致；可移植 Windows 203 项、契约/笔记、Rust workspace/Android/wasm、Android Java、鸿蒙 HAP、macOS 与 Apple bridge 通过，远端缺 Docker/MinGW 的 Linux/Windows 阶段跳过。证据为 `target/wer-timeout-quick-evidence/`，随后仅补充本篇验证结果；仍须核对新 CI，专项复跑通过不能解释旧 runner 的退出超时。

旧 CI 的 `host` 与 `withdrawn` 分别在有记录和无记录时超时，不能仅据记录有无判定卡住的位置。超时快照现额外保留本合成目标的有界线程状态及相关 WER worker；CIM 查询设置三秒操作期限，取状态失败只保留异常类型，不能为诊断再次无限等管道或存整个进程。原四十五秒退出期限和全部故障断言保持不变。本轮在 PVE Windows 11 上从实际候选 PowerShell AST 直接加载该函数，对真实休眠子进程及随后退出的同一目标执行：活跃线程可读取、无关 worker 未收录、完成/部分记录名保留、命令行及环境不输出、退出竞态仍可保存，子进程已回收。默认 x64 目标读取 28 条线程，约 0.170 秒；另用实际 PE machine 为 `0x14c` 的 SysWOW64 PowerShell x86 目标读取 26 条线程，约 0.165 秒，两者均通过。执行文件 SHA-256 与候选脚本逐字节相同。证据为 `target/pve-wer-timeout-snapshot-{attempt1.log,verification.json,live.json}` 与 `target/pve-wer-timeout-snapshot-x86-{attempt1.log,verification.json}`。这只验证新增诊断，不能声称旧 runner 超时已修复或最新整套 WER 已通过。

包含上述诊断的固定候选 `b753f82b9a` 已在 PVE Windows 11 复用原缓存完成 MSVC x64/x86 严格构建，两架构各两条回调与生命周期 CTest、各七组真实系统 WER 全部通过。243 个相关源码条目开始、每个架构结束及最终逐项一致；允许列表依旧由原用例清理，没有调用产品上传。该专项验证新诊断与正常完整流程兼容，仍不能解释旧 runner 的退出超时，完整产品 TSF、真实编辑器及 ARM64/Win7 的缺口不变。
