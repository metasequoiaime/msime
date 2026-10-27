# Agent Note: 自有宿主的独立崩溃采集和符号身份

Status: implemented

## Problem

匿名遥测的共享队列不等于所有进程都有采集入口。Android 原生致命信号、桌面设置与面板、iOS App 信号、鸿蒙设置应用的系统诊断存在缺口；Windows x86 的异常只有直接故障帧。旧报告也没有足以区分同版本多次构建的身份，Linux、Android、HarmonyOS 和 Win7 缺少对应的发布符号归档。

## Decision

沿用共享遥测的同意开关、匿名安装身份、队列和上传协议。`capture_only` 为独立应用准备记录路径与构建头，不建立输入会话；输入法宿主仍持有自己的 `session_id`。崩溃摘要只留类型、信号和错误码，不上传异常消息或原始系统日志。构建身份的格式、原件确认删除与会话归属由[会话归属笔记](../../implemented/bug-fix/2026-10-24-telemetry-session-ownership.md)持有。

### 采集边界

- `platforms/common/CrashRecorder` 独立于 Host API，在正常路径预备路径、构建头和模块映射。信号回调使用有界静态缓冲区，只尝试一次配置锁，先保存摘要和故障帧，再尽力展开，不联网。保留第一份记录，并交还原有处理程序；关闭时先解除采集。
- Windows x86 通过 `ReadProcessMemory` 读取有界 EBP 链，x64 与 ARM64 使用系统展开表。Win7 构建保留帧指针。Server、现代原生设置、Win7 设置与面板以及桌面公共面板分别准备采集；独立界面不增加输入会话分母。
- Android 的 App 和 IME 分别准备独立捕获和输入会话，JNI 接入原生记录器。API 30 以上的系统退出报告只认领自身留下的同意标记，保留旧版本，ANR 和系统处置独立归属。Java 只写异常类型与栈，不写异常消息。
- macOS 输入法使用共同记录器，异常、退出与会话仍由原生宿主管理。Tauri 设置与公共面板独立捕获，在偏好变化时重读同意并解除采集。
- iOS App 与键盘共享 C 信号记录器，预计算构建头与路径成对发布，MetricKit 保留系统给出的原始版本；卡死独立归属。App Group 保存本轮持续同意的起点，关闭时删除；MetricKit 只给批次时间范围，跨越该起点的整批跳过，不能用当前同意追认历史诊断。C 文件放在 `SharedUI/core`，由 XcodeGen 的共享源组加入每个使用方，避免跨源组路径合并。
- HarmonyOS 的应用与键盘均监听 `APP_CRASH` 和 `APP_FREEZE`。只读取标准异常类型、原生帧、原始版本、进程号和时间；卡死不增加 `session_crash`，不读取 hilog、页面历史或外部日志。系统诊断的稳定文件身份同时作为共享层的 `diagnostic_id`，重送不产生随机新事件。每次回调重新读取共享同意；持久化连续同意的起点，关闭时删除，缺标记或发生时间早于本轮同意的历史诊断不补录。

### 符号归属

ELF 帧保存模块名、相对偏移和 GNU build id；Mach-O 帧保存模块偏移和 UUID。Android、Linux 和 HarmonyOS 构建保留原始调试行与 build id，打包时另行裁剪安装副本。`crash-symbols.py` 归档本产品模块，记录版本、提交、构建运行、重跑编号、架构、模块 build id 与 SHA-256；还原前核对哈希和 build id，拒绝同名异构建模块。Linux 源码包携带固定提交，离线构建仍有来源身份。

Win7 的归档脚本分别收集 x86、x64 的配套 PE 和 PDB，任一必需文件缺失或为空就失败。发布 workflow 同时保留九十天 artifact；只有维护者手动选择发布时才推到平台滚动符号 release。本次不触发发布。现代 Windows、macOS 的已有滚动符号和 iOS 的 dSYM 保留机制继续使用。

## Related notes

动手前按 `telemetry`、`CrashRecorder`、致命信号和符号归档检索活跃笔记。[全平台闭环提案](../../proposed/feature/2026-10-27-cross-platform-crash-reporting.md)部分落地到本篇，保留其 TSF、自有辅助进程、设备及后台收件验收要求；不把整个提案标为完成。[iOS 系统诊断认领](../../implemented/bug-fix/2026-10-25-ios-crash-diagnostic-session-attribution.md)、[iOS 未捕获异常栈](../../implemented/bug-fix/2026-10-27-ios-app-uncaught-exception-stack.md)和[Windows 发布符号](../../implemented/process/2026-10-20-windows-release-page-symbols.md)分别持有既有决定，部分重叠并保留。[诊断目录句柄](../../implemented/bug-fix/2026-10-09-diagnostics-crash-directory-handle.md)持有路径访问边界，不被本篇替代。其余模型删除、词库与语音记录不涉及此决定；没有完全吸收的旧笔记。

## Alternatives considered

- **统一引入第三方崩溃 SDK**：成熟的转储、符号化和设备采集最全面，但会新增第三方传输与隐私边界，也不能直接替换自有后台。本次复用系统诊断和本地捕获，继续使用现有传输。
- **独立进程也调用普通 begin/end**：最省接入代码，但会把打开设置计作一次输入会话，改变无崩溃率分母。独立捕获只产生 `crash`。
- **在故障时动态查模块和格式化完整异常**：信息丰富，但可能再次进入已损坏的堆、加载器锁，异常消息也可能含用户内容。模块映射和身份提前固定，故障时只做有限写入和尽力展开。
- **只归档工作流 artifact**：不需要滚动 release，但九十天后正式版仍在线而符号消失。发布构建额外留滚动符号，文件名含运行及重跑编号，安装包不携带调试符号。

## Verification

- 共享层遥测 50 项、Host API 遥测 4 项及相关 Rust 库 Clippy 均通过，覆盖损坏构建头、原子写入、超过单批额度的保留和入队失败后的原件保留。
- Windows runner 实际执行 x86、x64 异常子进程用例：访问违规落盘、关闭不记录、首份记录保留通过；ARM64 只完成 MSVC 编译。Win7 符号采集脚本通过 PowerShell 解析检查，尚未用完整同源安装产物验收。
- Android API 28、35 实际运行原生故障子进程；Java 宿主门禁通过。两个模拟器均跑过四个共享 crate 的全套设备测试：client-base 36、client-core 649、client-providers 274、client-services 1356 项通过；原有 ignored 与按设备限制跳过的 hard_link 用例不算通过。同源 Java/JNI 和 Rust debug 库的完整 APK 在两个模拟器上进一步验证 Java 异常、原生信号落盘及重启入队，关闭统计后两类故障均不记录；独立应用不产生输入会话事件。测试按应用 UID 阻断网络，结束后删除合成队列并撤销规则，没有把合成故障发到生产后台。该包因未优化的 Rust 库超过正式包 135 MB 体积门禁，不能当作发布打包通过。整套装包冒烟的基本输入通过，但 API 28、35 都在十八键中英文及数字往返用例失败，不能将专项通过扩大为全套通过。
- macOS 原生记录器子进程及目录、退出回归用例通过；Linux 容器中的记录器经 ASan/UBSan 运行通过。ELF 合成模块的归档、源码行还原、错误 build id 和错误哈希拒绝用例通过。
- HarmonyOS 逻辑测试和实际 ArkTS/HAP 构建通过。复用的原生二进制仅用于 ArkTS 编译与打包检查，不作为同源产品或设备验收。
- iOS 工程重新生成，实际 SDK Swift 类型检查与完整 Xcode build-for-testing 通过；iOS 27 模拟器实际执行 UsageReportingTests 16 项，全部通过（已核对 xcresult 数量）。App/键盘的真实致命故障、MetricKit 系统回调与后台收件仍未验收。仓库最终 quick 门禁仍在运行，不能将上述分项通过写成全平台验收通过。

## Consequences

新增入口让独立进程和原生故障进入既有队列，构建身份能指导选择精确符号。队列容量不再允许普通统计挤掉崩溃；全为崩溃时保留后续原文件。共享层异常写入完整落盘后原子发布，损坏的新格式身份不回退为当前版本。

限制：信号处理时的运行库展开是尽力而为，不能保证栈耗尽和堆破坏时仍有完整栈；备用栈目前只覆盖安装线程。运行中后加载的模块要重新准备后才在快照里。直接原生写入和并发采集仍有半份记录的竞争窗口。系统延迟诊断缺少精确源码提交时标为未知，不猜测当前构建；旧无头独立记录无法恢复原版本。TSF 位于外部宿主进程，尚未加入仅认领本产品模块的捕获；看门狗、准备程序、MCP、语音辅助进程也尚未逐项覆盖。以上设备、收件和覆盖缺口继续由全平台提案跟踪，不能据本篇声称百分之百采集或全平台闭环已完成。
