# Agent Note: 六个平台的崩溃捕获、构建归属与符号还原闭环

Status: proposed

## Problem

六个平台已有共享遥测队列，但覆盖范围不一致：Android 只有 Java 异常，桌面的设置、面板和辅助进程没有同等捕获；Windows x86 的结构化异常没有展开调用栈。iOS 与 HarmonyOS 依赖系统的延迟诊断，独立崩溃记录按补报时版本归属，记录里也缺少构建号、架构和组件身份。同版本的正式版、测试版与每日构建无法可靠地和符号包对应。共享层每次只转换八条崩溃记录，却删除目录里所有记录，且先删记录再持久化队列，批量补报和写盘失败都可能丢失报告。

## Proposal

沿用 `client-services::telemetry` 的队列、同意开关、去重和投递协议，不引入新的第三方采集 SDK，不在崩溃现场联网。分层补齐四部分：

1. 共享层保存崩溃发生时的版本、源码提交、构建运行与重跑编号、目标架构和组件身份。旧记录继续可读，新记录使用带版本的头部；身份作为消息第二行的结构化诊断上传，不改变服务端事件 schema，也不占用后台聚合所用的摘要第一行与业务栈首帧。崩溃记录在队列持久化后才确认删除，超过单批额度的保留到后续补报。共享层的这一部分已在[会话归属笔记](../../implemented/bug-fix/2026-10-24-telemetry-session-ownership.md)落地，平台接入与符号闭环仍属于本提案。
2. 复用现有原生捕获机制，把 Android 的致命信号、Windows x86 的故障栈，以及桌面独立设置、面板和自有辅助进程接入。TSF 在其他应用进程里运行，只记录归属本产品模块的故障，并保持原处理程序和宿主进程的处置语义。进程启动但没有开始输入会话时只产生独立 `crash`，不得污染输入法的 `session` 和 `session_crash`。
3. 所有发布流程保存与具体构建身份对应的符号清单；原生帧保留模块相对偏移和模块身份。补齐 Linux、Android、HarmonyOS 与 Win7 的符号归档，并提供按构建选择符号的离线还原入口。iOS 的已有九十天 dSYM 保留与 macOS、Windows 的滚动符号 release 分别沿用，不把符号包放进用户安装包。
4. 平台原有系统诊断补录继续工作，增加可获得的 Android ANR、系统退出与 Apple/HarmonyOS 卡死诊断；把崩溃、系统强制退出和卡死分清，不把裸会话标记或关闭统计的用户当成崩溃。用合成故障逐项验证捕获、落盘、重启入队、收件和符号还原。

## Related notes

独立原生采集、部分系统诊断与发布符号接入已在[宿主采集和符号身份](../../implemented/feature/2026-10-27-native-crash-capture-and-symbol-identity.md)落地；该篇列出已执行的验证。MCP、Windows 看门狗、Windows/Linux 准备程序、Linux 原生 CLI 与 macOS/Linux 本地语音 helper 已接入；TSF、同源设备故障到后台收件及部分发布符号验收仍未完成，本提案保持 `proposed`。

逐入口核对后，Linux 的 `src/providers/*_request.cpp`、`src/voice/voice_request.cpp`、三个 `src/clipboard/*.cpp` 独立工具及本地语音 helper 已接入独立采集，真实二进制的故障/关闭/补报用例由本轮验证。`shared/voice/LocalAsrHelper.cpp` 的 macOS 平台适配也已接入并完成真实 helper 子进程故障测试；安装的 Python 在线/语音 provider、两个 HTTP 工作进程、剪贴板 monitor/事件、setup/provider-session 八个入口接入同一原生桥；新增两个入口已通过真实子进程故障与 full/wubi 禁网安装后回归。shell、desktop、systemd 和 Omarchy 的启动归属已逐项核对，外部系统程序的故障不冒充产品故障，具体边界见宿主采集笔记；共享库已链接或由输入法启动不等于已经采集，后续继续按进程验收。

动手前检索了 `proposed`、`implemented` 与 `rejected` 中的 `telemetry`、崩溃捕获、会话与符号归档。以下是部分重叠，保留原决定并互链：[按会话 id 归属](../../implemented/bug-fix/2026-10-24-telemetry-session-ownership.md)、[iOS 系统诊断认领](../../implemented/bug-fix/2026-10-25-ios-crash-diagnostic-session-attribution.md)、[iOS App 抛出点的栈](../../implemented/bug-fix/2026-10-27-ios-app-uncaught-exception-stack.md)、[Windows 符号归档](../../implemented/process/2026-10-20-windows-release-page-symbols.md)。[遥测本地队列绑定目录句柄](../../implemented/bug-fix/2026-10-09-telemetry-cache-directory-handle.md)、[清理绑定目录句柄](../../implemented/bug-fix/2026-10-09-telemetry-crash-cleanup-directory-handle.md) 与[空扫描不分配](../../implemented/testing/2026-10-11-telemetry-empty-crash-lazy-buffer.md)是本次必须保持的约束，不改变其决定。未发现本篇完全吸收的旧提案。

## 当前覆盖与未完成范围

这里的“已接入”指分支代码，不代表已合并、已发布或设备到后台验收通过。

| 平台 | 分支已有入口 | 必须继续补的覆盖或验收 |
| --- | --- | --- |
| Windows | Server、现代/Win7 设置与面板、MCP、看门狗、准备程序；TSF 租约与进程外 helper 接线、x86/x64 实际 DLL 生命周期及合成宿主七组真实系统 WER 投递；Arm64X helper 链接 | 完整 TSF 产品与真实编辑器验收；ARM64 实际故障；完整 Win7 同源符号包；设备到后台链路 |
| macOS | 输入法、Tauri 设置/面板、MCP、本地语音 helper；原生故障与部分符号还原 | 完整同源安装包及真实故障到后台和双架构符号 |
| Linux | IBus/Fcitx5、设置/面板、MCP、准备程序、12 个原生辅助程序、八个 Python 入口及启动归属核对 | 部分发行版符号与后台链路 |
| Android | App Java、IME Java/JNI 原生信号、API 30+ 系统退出及持久同意 epoch；API 28/35 专项故障/补报、API 35 实际系统退出和撤回重开；API 28/35 活跃 IME/App 动态同意；API 35 App 真实 ANR、补录、去重和撤回抑制 | 完整正式包和含按需资源的完整冒烟；后台精确符号化 |
| iOS | App/键盘异常与 C 信号记录、独立活跃同意观察、当前捕获身份、MetricKit 持久授权与队列代次认领、UUID 帧；同源模拟器回归 | 实际 App/键盘致命故障、系统 MetricKit 回调、设备 dSYM 与后台收件链路 |
| HarmonyOS | App/键盘 APP_CRASH、APP_FREEZE 监听，共享只读权威授权与独立观察，系统自定义参数固定原 UUID/构建头，ELF 帧身份；逻辑、适配器回归、同源 arm64 原生库/HAP 与两模块符号身份核对 | API 21 本进程兼容入口实际致命故障（原生与主线程 ArkTS 已接入，三架构严格目标编译与四组实际适配器故障回归通过）；原身份实际系统投递；真实设备系统回调与后台精确符号化 |

所有平台继续保留“栈展开尽力而为、备用栈仅安装线程、后加载模块快照要重新准备”的边界。TSF 进程外 WER 适配遵循 Microsoft 的[注册接口](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werregisterruntimeexceptionmodule)、[不认领异常的回调语义](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nc-werapi-pfn_wer_runtime_exception_event)和[注销要求](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werunregisterruntimeexceptionmodule)。[TSF WER 提案](2026-10-28-windows-tsf-wer-capture.md)持有完整方案与分级验证：产品生命周期、安装器和符号已接线，x86/x64 的实际 DLL 装卸、授权变化、阻塞读取、迟到结果和生产 helper 落盘联动通过，负对照能识别短暂的旧租约武装；ARM64 目标编译与 Arm64X helper 两阶段链接通过。x86/x64 合成外部宿主的七组真实系统 WER 投递现已通过，覆盖 TIP/Host API 自有故障、外部故障、撤回重开、释放卸载和被宿主捕获的异常；完整产品包、真实编辑器与后台仍未验收，不能据此认定 TSF 闭环完成。

[共享偏好的持续同意存储](../../implemented/bug-fix/2026-10-28-persisted-reporting-consent.md)提供原子代次；[采集与队列绑定](../../implemented/bug-fix/2026-10-28-epoch-bound-crash-collection.md)把 iOS 采集、旧原生记录过滤、MetricKit 补录与逐条发送接入同一代次。它不代替实际致命故障、系统 MetricKit 投递与后台精确符号化验收。iOS 与共用 `CaptureOnly` 辅助采集器已迁入共享队列代次协议；辅助采集的 macOS 与 Linux 共用真实子进程回归通过；Linux 产品入口的授权目录夹具修复已由 Debian/Fedora CI 全量重建与故障套件复验通过，Windows 目标运行仍待验证，MCP 与桌面设置发送端已接入共享协议，MCP 与桌面 macOS 隔离专项及实际 Linux crate 全 target 严格 Clippy 通过，Linux 普通用户运行 MCP 126 项和桌面两项崩溃专项已通过；macOS 输入法、现代 Windows 设置与共用输入会话宿主也已接入，仍须按目标平台核对撤回、旧原生记录晚写和在途发送边界，不能由共享接口可用推定全平台均已完成验收。Android 现已将 App、IME、Java 同意镜像、发送和系统退出补录迁入共享协议；Rust 回归、宿主检查和 API 28/35 的授权镜像专项已通过，同源原生 Release APK `8a006d7ed7` 已构建，API 28/35 的 Rust 共享只读授权应用 UID 专项均通过；该源码的 API 35 真实系统退出、App ANR 补录/去重及撤回重开拒绝旧历史已通过，API 28/35 活跃 App/IME 观察器复验均通过；最新源码的完整产品与完整冒烟仍待结果。鸿蒙已接入本沙箱 NAPI 共享代次，严格拒绝读失败时退回旧布尔协议，迟到清理与会话结束已有适配器回归；完整逻辑套件及复用缓存原生库的 ArkTS/HAP 编译通过。[共享只读权威授权](../../implemented/architecture/2026-10-28-harmony-reporting-authority.md)现已统一 App/键盘授权历史，并接入不依赖焦点或网络完成的独立观察器。九项核心与十二项 Host API 专项、严格 Clippy、完整鸿蒙逻辑套件及十一组适配器回归通过，真实 HAP 编译打包通过；[系统原身份传递](../../implemented/feature/2026-10-28-harmony-system-capture-identity.md)现已用 HiAppEvent 自定义参数固定故障前 UUID、构建头、PID 和准备时间，29 项策略断言、四组异步适配器回归及真实 HAP 编译通过；API 21 原生记录器与主线程默认错误处理器已接入，三架构目标严格编译、四组实际适配器致命故障回归与实际 ArkTS/HAP 编译通过；本地旧缓存缺少四个 ABI，严格链接已拒绝；后续 `d91b0b9242` 已同源重编 arm64 Rust/NAPI 并打包 HAP，两个模块的包内 build-id 与归档一致，授权入口按精确身份还原到源码行。设备入口仍未运行验收。原身份的实际系统投递、鸿蒙真机共享目录、真实系统回调和后台链路仍未完成。

## Alternatives considered

- **统一接第三方崩溃平台 SDK**：成熟的转储、符号化和设备采集覆盖最广，但会引入新的第三方网络与隐私边界，不能直接替换用户已经使用的自有后台。本次保留自有传输，复用平台已有诊断和原生捕获。
- **给事件直接增加构建和模块字段**：类型化最明确，但会把客户端的这次改动绑定到服务端部署顺序。先在既有 `message` 的第二行保留带版本的结构化身份，旧后台仍能完整储存，离线工具可确定性解析。不能放在 `stack` 顶部：现有后台会把元数据当业务帧，从而改变崩溃签名。
- **将所有独立进程也记作输入会话**：复用现有 begin/end 最简单，但会改变无崩溃率的分母，让打开设置也算一次输入法会话。独立捕获沿用同一队列但不产生输入会话。

## Acceptance criteria

- 六个平台的版本、组件、构建与架构在发生崩溃时固定，升级补报不得更换归属；同版本不同构建可区分。
- 保持关闭统计不捕获、不上传和清空待发记录的语义；不上传输入内容、私人目录、凭据或原始系统日志。
- 超过八条的记录仍可后续送达；入队失败不删除记录；重试不产生新的事件身份。
- Android Java 和原生崩溃都有设备用例；Windows Server、x86、TSF 和独立进程分别有验证；macOS、Linux、iOS、HarmonyOS 分别核对信号、异常或系统诊断的实际入口。
- 各平台发布符号和构建清单可核对，并用合成调用栈还原到确定的函数或源码位置。
- 跑相关测试、Rust 格式与 Clippy、笔记校验、quick 门禁，提交并推送后向 `develop` 开中文 PR。设备端与后台收件的未完成项持续保留，不用编译通过代替。

## Risks

信号与异常处理器可能面对栈耗尽、堆损坏和加载器锁；捕获应写入有限的预备缓冲，先保存故障摘要，再尽力展开栈，并交还原来的系统处置。TSF 不得接管其他应用的无关崩溃。系统诊断可能延迟或缺失，不能承诺百分之百投递，也不能把没有报告当成没有故障。本机可用磁盘低于五十 GB，新增构建树放在 `rbuild` 的 Studio 镜像，设备验收只拉回所需产物。

### macOS 符号发布仍需补验

发布脚本已显式保留源码行、收齐 Swift 后端原始 dSYM，并按每个架构的 UUID 与实际行表拒绝空包，细节由上述宿主采集笔记持有。Clang/Swift/Rust 合成用例和实际 Release Swift 后端的双架构源码还原通过；完整同源发布包和实际设备故障到源码行的链路仍未验收，不能只凭 zip 存在宣称完成。

### 共用输入会话的持续授权

Windows Server、IBus、Fcitx5 与 Win7 设置/面板的共用 `Telemetry` 包装器已接入持久授权代次与独立观察线程。macOS/Linux 真实子进程用例通过，覆盖在途发送期间撤回、重开与旧结果收尾；Fcitx5 与 IBus 已用同源 Host API 完成实际产品重建，共用层与 IBus 专项通过，Windows 目标运行仍待补验。macOS 输入法已复用共用协议，同源 Host API 重建和真实输入法链接成功，最终六项真实 CMake 专项通过；现代 Windows 原生设置已复用共用包装器，实际 Windows 编译运行待新一轮 CI；Android 与鸿蒙仍须逐入口核对共享队列协议，不能将这次共用包装器迁移写成全平台完成。具体协议与验证由[采集与队列绑定](../../implemented/bug-fix/2026-10-28-epoch-bound-crash-collection.md)持有。
