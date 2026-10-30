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
| Windows | Server、现代/Win7 设置与面板、MCP、看门狗、准备程序；TSF 租约与进程外 helper 接线、x86/x64 实际 DLL 生命周期及合成宿主七组真实系统 WER 投递；Arm64X helper 链接；callback/lifecycle 门禁通过；候选 `1ae2cf6c67` 已经 PVE 跳转完成 MSVC x64/x86 新计数与最终授权屏障、反复装卸及各七组系统 WER；目录变化通知与三十秒兜底，x64/x86 专项及旧轮询负对照通过；2026-10-30 同源 Server 的 WER→本地队列→隔离后台已收到两组 `crash`/`session_crash` 与两条 `active`，字段和匿名安装身份逐项一致；固定 `b8356db26b` 的 x64 Notepad 真实输入、TSF/Host 自有访问冲突、两条独立 `tsf` 后台收件、TSF 精确 PDB/独立 ZIP 还原到 `KeyEventSink.cpp:2415` 与重启后输入恢复通过；`e746981d18` 的 x64 实际 Notepad 输入、Host/TSF WER→共享队列→隔离后台复验通过，TSF 以重新链接后的独立精确 PDB 还原到同一源码行；同源实际 Notepad 的活跃撤回重开、测试处理器恢复后继续真实输入及 ntdll 非自有故障对照已完成到共享队列；同源 x86 Notepad++ 实际输入、活跃授权换代、真实自有访问冲突经 32 位 WER、x64 Server 重启到隔离后台及独立 PDB 源码行通过，正确 helper 下撤回故障不采集 | 完整安装包；ARM64 实际产品故障；完整 Win7 同源符号包与后台；最新集成候选与其余产品入口复验 |
| macOS | 输入法、Tauri 设置/面板、MCP、本地语音 helper；原生故障；实际 universal DMG 五模块双架构符号、UUID、哈希、源码行表和签名核对；当前分支 Intel 登录会话的真实 IMK 编辑器验收（workflow run `40760896167`，双布局通过）；候选 `1ae2cf6c67` 的 arm64 输入进程实际故障、原产品重启、后台入库及 `input_method_main.mm:214` 还原；`e746981d18` 的原生回退设置及包内语音 helper 外部信号、设置入口补报、独立 dSYM 源码行和活跃授权对照 | Tauri 设置及其余辅助入口后台与源码行；硬件访问冲突与完整编辑器验收 |
| Linux | IBus/Fcitx5、设置/面板、MCP、准备程序、12 个原生辅助程序、八个 Python 入口；aarch64 输入框架真实故障及后台/源码行；6bea x64 设置启动器代收、真实内存故障与源码行；Mousepad 经 IBus/X11、Fcitx5/X11 和原生 Wayland 的输入、故障恢复、撤回重开及后台；正式在线服务入口的 systemd 自动重启与授权队列专项，`e746981d18` 的在线、语音与剪贴板三服务已补齐相同验收；`e746981d18` 的 Ubuntu x64 暂存完整产品已复验 IBus/X11、原生 Wayland/Fcitx5 与设置启动器，七条 crash 到后台、独立符号源码行和授权对照通过，真实按键内存访问还原到 `FcitxEngine.cpp:7705` | 其他架构与部分发行版符号；已安装系统包的原生 Wayland、IBus/设置真实访问冲突及后台复验；其他发行版的系统安装与 systemd 生命周期 |
| Android | App Java、IME Java/JNI 原生信号、API 30+ 系统退出及持久同意 epoch；API 28/35 专项故障/补报、API 35 实际系统退出和撤回重开；API 28/35 活跃 IME/App 动态同意；API 35 App 真实 ANR、补录、去重和撤回抑制；同源 arm64 Release 包身份核对，API 28 App/IME Java/JNI 真实故障到隔离后台及精确源码行还原；API 35 两个日语原用例和原样触控板专项单独通过；API 35 device workflow 先用 `smoke.sh --crash-only` 独立运行 `telemetry-watch.py`、`telemetry-consent.py` 和 `telemetry-anr.py`，再跑完整输入冒烟 | 截至 `ab3a30ecd1` 整作业通过但旧语音润色失败根因未确认；6bea API 35 整作业成功，含共享设备单测、APK、崩溃专项及完整输入套件；真机与其他架构、最新候选后台传输仍缺 |
| iOS | App/键盘异常与 C 信号记录、独立活跃同意观察、当前捕获身份、MetricKit 持久授权与队列代次认领、UUID 帧；同源模拟器回归；iOS CI 现用 `iphonesimulator` SDK 做 arm64 目标编译，并运行真实 C 信号故障夹具和完整 MetricKit 栈解析器，核对故障首帧、Mach-O UUID 与历史模块身份；固定 `6bea262fd0` 的原生 Release App 模拟器信号、真实内存故障、重启补报、隔离后台、动态授权、SIGKILL 对照，独立符号 ZIP 还原到 `consent.rs:105` 与 `UsageReporting.swift:346/299`，正式产物恢复；同源 iOS 27 模拟器由系统在合成 UIKit 文本框拉起真实键盘，外部信号和真实内存故障到后台及独立 dSYM 源码行、活跃撤回、新代次、SIGKILL 对照和恢复输入通过 | 系统 MetricKit 回调；真机 dSYM 与后台链路；iOS 26.5 扩展框架异常独立复核 |
| HarmonyOS | App/键盘 APP_CRASH、APP_FREEZE 监听，共享只读权威授权与独立观察，系统自定义参数固定原 UUID/构建头，ELF 帧身份；逻辑、适配器回归、同源 arm64 原生库/HAP 与两模块符号身份核对；API 22 全新空白 2in1 模拟器的原生 App 实际内存故障、重启补报、后台及双模块独立符号源码行，设置页撤回/重开和 SIGKILL 负对照；修复超过 512 模块时遗漏自有库的问题；`4d179327a4` 的 API 22 手机原产品键盘实际上屏、外部 SIGSEGV 后重新输入及无共享授权目录时不采集的对照 | API 21 本进程兼容入口实际致命故障（原生与主线程 ArkTS 已接入，三架构严格目标编译与四组实际适配器故障回归通过）；具备共享权威授权时的键盘故障、后台与精确符号化；原身份实际系统投递；真机共享目录与系统回调 |

所有平台继续保留“栈展开尽力而为、备用栈仅安装线程、后加载模块快照要重新准备”的边界。TSF 进程外 WER 适配遵循 Microsoft 的[注册接口](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werregisterruntimeexceptionmodule)、[不认领异常的回调语义](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nc-werapi-pfn_wer_runtime_exception_event)和[注销要求](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werunregisterruntimeexceptionmodule)。[TSF WER 提案](2026-10-28-windows-tsf-wer-capture.md)持有完整方案与分级验证：产品生命周期、安装器和符号已接线，x86/x64 的实际 DLL 装卸、授权变化、阻塞读取、迟到结果和生产 helper 落盘联动通过，负对照能识别短暂的旧租约武装；ARM64 目标编译与 Arm64X helper 两阶段链接通过。x86/x64 合成外部宿主的七组真实系统 WER 投递现已通过，覆盖 TIP/Host API 自有故障、外部故障、撤回重开、释放卸载和被宿主捕获的异常；x64 暂存产品在真实 Notepad 的自有模块故障、后台与源码行现已通过；完整安装包、其余架构和真实编辑器的授权及非自有故障场景仍待验收，不能据此认定 TSF 全范围完成。

[共享偏好的持续同意存储](../../implemented/bug-fix/2026-10-28-persisted-reporting-consent.md)提供原子代次；[采集与队列绑定](../../implemented/bug-fix/2026-10-28-epoch-bound-crash-collection.md)把 iOS 采集、旧原生记录过滤、MetricKit 补录与逐条发送接入同一代次。它不代替实际致命故障、系统 MetricKit 投递与后台精确符号化验收。iOS 与共用 `CaptureOnly` 辅助采集器已迁入共享队列代次协议；辅助采集的 macOS 与 Linux 共用真实子进程回归通过；Linux 产品入口的授权目录夹具修复已由 Debian/Fedora CI 全量重建与故障套件复验通过，Windows 目标运行仍待验证，MCP 与桌面设置发送端已接入共享协议，MCP 与桌面 macOS 隔离专项及实际 Linux crate 全 target 严格 Clippy 通过，Linux 普通用户运行 MCP 126 项和桌面两项崩溃专项已通过；macOS、Windows 与 Linux 的输入会话宿主及同目录发送端已共同迁入，并增加无绑定入口保护；macOS 与共用本地回归已覆盖撤回、旧原生记录晚写和在途发送边界，Windows/Linux 产品运行与整合后的完整门禁仍须单独核验，不能由共享接口可用推定全平台均已完成验收。Android 现已将 App、IME、Java 同意镜像、发送和系统退出补录迁入共享协议；Rust 回归、宿主检查和 API 28/35 的授权镜像专项已通过，同源原生 Release APK `8a006d7ed7` 已构建，API 28/35 的 Rust 共享只读授权应用 UID 专项均通过；该源码的 API 35 真实系统退出、App ANR 补录/去重及撤回重开拒绝旧历史已通过，API 28/35 活跃 App/IME 观察器复验均通过；最新源码的完整产品与完整冒烟仍待结果。鸿蒙已接入本沙箱 NAPI 共享代次，严格拒绝读失败时退回旧布尔协议，迟到清理与会话结束已有适配器回归；完整逻辑套件及复用缓存原生库的 ArkTS/HAP 编译通过。[共享只读权威授权](../../implemented/architecture/2026-10-28-harmony-reporting-authority.md)现已统一 App/键盘授权历史，并接入不依赖焦点或网络完成的独立观察器。九项核心与十二项 Host API 专项、严格 Clippy、完整鸿蒙逻辑套件及十一组适配器回归通过，真实 HAP 编译打包通过；[系统原身份传递](../../implemented/feature/2026-10-28-harmony-system-capture-identity.md)现已用 HiAppEvent 自定义参数固定故障前 UUID、构建头、PID 和准备时间，29 项策略断言、四组异步适配器回归及真实 HAP 编译通过；API 21 原生记录器与主线程默认错误处理器已接入，三架构目标严格编译、四组实际适配器致命故障回归与实际 ArkTS/HAP 编译通过；本地旧缓存缺少四个 ABI，严格链接已拒绝；后续 `d91b0b9242` 已同源重编 arm64 Rust/NAPI 并打包 HAP，两个模块的包内 build-id 与归档一致，授权入口按精确身份还原到源码行。API 22 App 原生故障到后台的验收见上节；API 21、键盘、原身份的实际系统投递、鸿蒙真机共享目录与真实系统回调仍未完成。

## 隔离发送与共享后台验收

`client-services::telemetry::backend_sender` 统一 Host API 两条发送分支与桌面独立发送器。显式 `telemetry-loopback` 特性和编译时 `MSIME_TEST_TELEMETRY_ORIGIN` 共同限定验收目标；未启用特性时保持正式发送器。工具与边界见 `tools/crash/README.md`。当前 C ABI 的测试握手在访问队列前校验，错误库或错误目标不能先建目录再失败。工具还先用两个不联网的 `begin` 核对字段确实被解析；忽略未知字段的合成旧 ABI 已验证会在任何 `flush` 前被拒绝，不能用可能联网的 `flush` 探测旧库能力。

`client-services/build.rs` 现在也声明监听 `MSIME_TEST_TELEMETRY_ORIGIN`；改变隔离回环端口会触发重新编译，避免复用嵌入旧端口的 Host API 产物而把本地队列误判为后台未收。Windows Cargo 缓存中未改源码连续以 `48153`、`48729` 两个端口构建，分别核对 DLL 的嵌入目标，证明端口变化确实触发了 `client-services` 和 Host API 重编；最后产物的唯一完整隔离目标为 `http://127.0.0.1:48729`。

同源验证已覆盖无配置与有效配置：共享传输专项各三项，Host API 在测试和正式配置下各十三项通过；两个配置下 `client-services` 与 `host-api` 全 target 严格 Clippy 通过。正式库与缺配置测试库均拒绝工具的开始及发送请求；有效配置的真实 C ABI→HTTP、故障原版本补报、503 后同一崩溃事件重试、不带凭据、关闭后不发送通过。HTTP 专项在系统代理环境变量指向不可用回环代理、`NO_PROXY` 为空时仍能直连；不跟随重定向由传输专项核对。

真实后台使用默认分支固定提交 `ab391f834cb14baa88c98b1b8a2dbf03ac0da93a` 对应的官方 `0.111.2` 镜像，arm64 digest 为 `sha256:5c1e305b5be5586ce4c749369f35344a5afb94103d8a0178c6da8b57d8304994`，镜像 revision 标签与源码一致。后台与临时 PostgreSQL 18 仅接入 Docker 内部网络，经只转发 `/v1/telemetry/events` 的专用回环代理收件，不复制后台邻仓的未提交内容，不连接生产服务。持久授权的 C ABI 以六种平台值各补报一条合成崩溃，实际入库十二条事件、六个崩溃分组；逐字段核对事件身份、匿名安装身份、原版本、摘要、构建头和栈，三个重复请求未增加事件，撤回后六个队列均不再发送。证据及产物 SHA-256 留在 `target/isolated-backend/verification.json`，容器在验证后停止。

此处只验证同一共享 ABI 的六种平台字段，使用合成落盘记录；不代表六个原生宿主已实际崩溃、重启或精确还原源码行。完整 quick 与桌面外壳两个配置的最新结果另列，不能用上述专项代替。

Linux aarch64 的 IBus 入口已另行完成真实故障闭环：以 `e275758170e1651f907e8023c3abdbca5b1a65b3` 的完整源码归档重建原生宿主与 Host API，显式固定提交、构建运行和重跑编号；原生宿主保留源码行，Host API 最终 crate 显式保留调试信息，三个模块经严格符号归档。真实 IBus 输入上下文用合成拼音完成上屏，实际发送 `SIGSEGV` 后由原 launcher 重启并继续输入，再用 `SIGKILL` 核对不增加崩溃；维护停止与 IBus 退出也通过。原生发送器经仅接受遥测路径的回环代理，将四条事件送进上述固定后台及临时 PostgreSQL，其中恰好一条 `crash` 和一条 `session_crash`；数据库与收到请求的各字段逐项一致，崩溃身份保留原提交、`aarch64` 和 `input`。按实际后台栈的 build-id `86a9699b595e11cced2e312bffd9768d7a38ea01` 及偏移 `0xebec`，归档符号还原到 `platforms/linux/src/entrypoints/ibus_main.cpp:296`；另一个 `_start` 帧没有源码行，不将其记作源码还原通过。小证据留在 `target/linux-native-backend-evidence/`，远端符号包在规定的 `target/linux/native-exact-symbols.zip`。这是显式启用回环特性的验收构建，只在 Docker 内部网络运行，临时容器与网络已清理，不能用于安装或发布。此前 revision 为 `unknown` 的构建门禁产物仍只算输入、恢复和入队验收，不能混用其符号或将本次结果扩大到 Fcitx5、其他架构或六个平台。

随后按远端固定提交 `aaebaca94456ad5a9bfec5cbf76f0e98c07e9851` 重建并复验同一真实链路。源码归档 SHA-256 为 `48fe65d3cd265408aee5e1f0188b09965974539bfc03d660fa8d9e91bcb4e64d`，12,410 个条目的 Git blob 与文件模式逐项一致。实际 SIGSEGV、重启继续输入、SIGKILL 负对照、维护停止与 IBus 退出均通过；固定后台仍仅入库一条崩溃和一条崩溃会话，共四条事件，字段与请求逐项一致，身份保留该提交与 `isolated-linux-native-aaeb-20261028`。实际后台栈按 build-id `b5a150ba97d5d6c475ac4edf93f039018f46f2f2` 和独立归档 `target/linux/native-aaeb-symbols.zip`，还原到 `ClientEngine.cpp:3863`、`ClientEngine.cpp:8933` 及 `ibus_main.cpp:296`；`_start` 无源码行，仍不计通过。小证据另存 `target/linux-aaeb-backend-evidence/`，不覆盖 e275 的记录或符号。

固定提交的 `git archive` 保留提交时刻。复用 Cargo 缓存时，该时刻可能早于旧产物，导致内容已变化的工作区 crate 被误判为新鲜；本次首先出现源码已有 `CustomRequired`、旧核心 rlib 却没有该接口的编译错误。验收副本在核对归档及全部 blob/模式后只更新文件时间，触发工作区 crate 重编，源码字节保持固定提交不变；重建与真实链路随后通过，不将缓存组装问题记作产品修复。符号化只用这次实际输出的 build-id 与对应归档，不能仅因缓存目录或版本相同复用上一份符号。

同一 aaeb 源码还完成 aarch64 Fcitx5 原生插件的独立闭环。按 Linux 顶层工程重建真实插件并收集两模块符号，在隔离 D-Bus 中先验收合成拼音上屏；调试器在真实插件的按键入口设置一次性断点，投递 SIGSEGV，不替换产品实现或写入合成故障文件。实际 daemon 退出后显式重启，继续输入、原生补报、SIGKILL 后再次重启输入和正常退出均通过。固定隔离后台共存储三条事件，其中恰好一条 crash 与一条 session_crash，数据库逐字段等于收到的三次请求；构建身份仍固定 aaeb、原构建运行、aarch64 与 input。实际后台帧按插件 build-id `8c776079f99a6106e6a7405f03505aae500196b1` 和偏移 `0x1721c0`，由 `target/linux/native-aaeb-fcitx-symbols.zip` 还原到 `FcitxEngine::keyEvent`、`platforms/linux/fcitx5/FcitxEngine.cpp:7677`；两条重复帧只算一个源码位置。前序在外部 Fcitx 事件循环注入信号时，故障和补报虽通过，栈里没有产品帧，符号验收已按原断言拒绝；不拿外部帧或调试器自身的错误代替本次成功结果。最终调试器正常退出，容器及内部网络清理完成。小证据另存 `target/linux-aaeb-fcitx-backend-evidence/`；这是 aarch64 隔离测试构建与显式 daemon 重启，不扩大到其他架构、发行版或自动恢复。

### macOS arm64 输入进程的真实故障闭环

固定候选 `1ae2cf6c67207114fa19a388d44a797082193397` 的 12,444 个源码条目，在既有构建缓存重建启用回环验收能力的静态 Host API 与真实原生输入法。arm64 `0.55.0` 产品在合成 HOME 与运行时偏好中启动，沙箱只允许专用回环端口；外部 `SIGSEGV` 后由原产品重启并发送，固定官方后台与临时 PostgreSQL 共收到四条事件，恰好一条 `crash` 和一条 `session_crash`。逐字段核对数据库与 HTTP 请求一致，原捕获身份保持原版本、提交、运行编号、架构与 `input`。实际后台帧的 UUID `efc384f710873096a1df3a7c36637bc3`、偏移 `0x9bbc` 经本轮独立符号归档还原到 `input_method_main.mm:214`。再次以 `SIGKILL` 终止并重启不增加崩溃。证据为 `target/macos-consent-input-evidence/`，远端完整记录与符号位于 `target/macos-runtime-acceptance/capture-final-consent-input-attempt1/`。

这只验收原生输入进程启动路径及外部致命信号，不代表硬件访问冲突、IMK 真实编辑器、Tauri 设置及其他辅助入口通过。原生回退设置入口也实际故障并入库，但捕获的前 64 帧没有可匹配的自有模块，源码行断言拒绝通过；失败证据单独保留。LLDB 另因非交互调试权限被拒绝，未改变系统调试授权；改用向实际产品发送 POSIX 信号，不用测试桩伪造记录。前序动态库 `LINKEDIT`、沙箱地址语法、LLDB 模块命名和测试后台中继准备失败均保留，静态准备工具只操作合成偏好及离线握手，实际补报仍由原生产品执行。结束时已停止产品、临时后台、数据库、内部网络与 SSH 回环转发；重建不带回环特性的正式静态库和原生宿主，实际握手拒绝测试来源且未创建队列目录。运行前后的 12,444 个源码条目一致，没有安装或发布产品。

### macOS 原生回退设置与语音 helper 的补报及授权对照

固定 `e746981d18261e39cf6f0530934985fad6afcf9b` 的干净 worktree，复用规定的 `target/macos-cargo` 与 `target/macos-isolated`，重建实际 arm64 `0.55.0` 原生输入法及包内 `msime-voice-local`。仅设置任务专属 `CFFIXED_USER_HOME`，Foundation 实际查询确认 Application Support 位于合成目录；不改写 `HOME`，不安装或注册输入源。共享偏好 ABI 只准备合成授权，故障与补报均运行实际产品；网络沙箱仅允许专用回环端口。

原生输入法的 `--preferences` 回退设置入口和包内 helper 各通过开启时外部 SIGSEGV、原设置入口重启补报、SIGKILL 无伪报、活跃撤回后 SIGSEGV 无记录、重开后新 epoch 及重新补报。helper 保持实际 stdio 协议并返回 hello，故障后直到设置入口启动前后台没有新增请求，未冒充 helper 自己联网。新代次保持匿名安装 ID，实际记录的 epoch 不同；没有输入会话。通过轮新增四条 crash，数据库与 HTTP 七字段按缺省空串逐项一致，active 按原 ID 幂等。

两模块 dSYM 均从实际产物收集，校验 UUID 与非空源码行后打包，再仅用独立解压目录还原后台事件；ZIP SHA-256 为 `910e64906ad2cf432c28c3b9d07d4bf8d8ef3754451d7776b7226a89e5ecb9b8`。设置帧 UUID `25b7f5039d133d4fb19d13ce4cfc1df1`、偏移 `0x8d28` 还原到 `input_method_main.mm:139`；helper UUID `66176351ec2d3f63ada74904768f1d26`、偏移 `0x4fbc` 还原到 `LocalAsrHelper.cpp:591`。每条实际自有帧均验证错误 UUID 被拒绝；不把系统帧当作产品源码证据。

保留两类夹具失败：LLDB 首次因模块文件名含连字符拒绝导入，改名后系统明确拒绝非交互调试权限，未改系统权限；信号首轮误要求重开后匿名安装 ID 下只有一条 crash，实际协议保留安装 ID，已正确收到两条，等待断言失败。原两条后台记录保留，独立新合成目录完整复跑通过四条，不改后台记录、产品、断言超时或授权协议。实际内存故障仍待调试权限，外部信号成功不能代替。

正式 Cargo、两个产品入口均已重建恢复，产品二进制无完整测试 origin；新链接的正式 ABI 在建目录前拒绝 `test_origin`。五份合成用户的授权逐项关闭并读回，产品进程无残留；临时后台、数据库、内部网络和专用转发全部停止清理。证据为 `target/macos-e746-product-evidence/`。范围限 macOS arm64 原生回退设置和语音 helper；Tauri 设置、其余辅助入口、硬件访问冲突及完整 IMK 编辑器范围继续保留。

### Android API 28 的真实故障到后台与符号还原

固定 `e865a66d0c6fe88ca7eaee550c36a2081acb6ada` 的 12,410 个源码条目，构建原生 full 版 arm64 Release APK；GitHub 分发配置按原规则省略按需资源，采用已有开发证书，不是正式签名发布包。包内八份固定词库及通知逐字节对照锁定生成物；Gradle 剥离后的两个原生模块与 APK 字节一致，未剥离符号模块的 build-id 与包内对应。正式 APK SHA-256 为 `2fce6751485dee16b0ee11d09da26fea9b5166a2651920a90f87d20a146f3f60`，JNI build-id 为 `8d9f57b69892fd6d0bdfbbd4edca3dcbef9beb97`，正式 Host API build-id 为 `96f546150ad35dfb62728e296f50fd15c2a9577c`。不能用剥离前后的完整文件哈希相等来验收 Gradle 产物；最初的错误比较已保留失败记录，后续按实际剥离产物与模块身份分别核对。

专用 API 28 AVD 上原 `smoke.sh --core` 跑完，只有数字键盘的笔画场景失败，其他独立检查通过。该瘦包省略的按需词库尚未安装：六组固定资源先由主机按锁准备，再经设备真实应用 UID 的产品 JNI 收编和校验；保持原用例不变，单独复验数字键盘七种方案／宽度场景全部通过。保留首轮整套退出 1 与独立复验退出 0，不能把两者记成首轮整套通过，也不能将主机资源准备冒充设备验收。

同源显式回环构建在实际 `app.msime.android:ime` 进程分别触发 Java 致命异常及 JNI 入口的真实 `SIGSEGV`。原生故障由调试器在 `telemetryConsentRaw` 入口将 `JNIEnv` 参数置空触发；调试器报告进程实际退出，不能以主动停止调试器或杀进程代替故障。产品重启后由原发送器向固定 `0.111.2` 隔离后台补报，数据库共五条事件，恰好两条 `crash` 与两条 `session_crash`，与接收请求逐字段相等；已入队的原始记录确认移除，`SIGKILL` 后再次启动不增加崩溃。两次故障均保留原提交、构建运行、`aarch64` 与 `keyboard` 身份；Java 摘要不包含合成异常正文。

实际后台的原生栈按 JNI build-id 和独立符号包还原到 `platforms/android/native/client_jni.cpp:105`、`:114`；另两帧是相同偏移的 NDK `jni.h:852`，四个记录帧不能计成四个独立产品源码位置。此项限 API 28 arm64 模拟器的 IME 入口，不扩大到 App、API 35、其他架构、真机或正式发布。

小证据保存在 `target/android-e865-backend-evidence/`，含实际数据库事件、接收请求、调试器退出、符号结果及设备收尾记录；远端完整产物保留在规定的 `target/android-runtime-acceptance/e865/`。收尾核对正式 Cargo/JNI/Gradle 输出恢复，恢复基础编辑器、强停产品、关闭专用 AVD 的上报并清空合成队列；只移除本任务标记的 IPv4/IPv6 规则与端口转发，停止专用代理，删除精确命名的临时容器和内部网络。未向生产后台发送测试事件。

同一 APK 与符号另在 API 28 的实际 `HomeActivity` 应用进程验收，未使用键盘 instrumentation 代替 App。系统 `am crash` 触发 Java 未捕获异常，调试器在 App 授权观察线程的同一 JNI 入口触发真实 `SIGSEGV`；原进程退出后重启 App，独立隔离后台收到三条事件，恰好两条 `crash`，没有 `session` 或 `session_crash`，全程核对键盘进程未启动。故障头组件为 `application`，实际后台栈再次还原到 `client_jni.cpp:105`、`:114`，`SIGKILL` 后重启不新增崩溃。数据库与请求逐字段一致，证据位于上述本地目录的 `backend-app/`；`app-cleanup.json` 核对专用后台、代理、转发与设备规则均移除，正式 APK 恢复，上报关闭，合成队列清空。

API 35 首次完整冒烟因专用 AVD 沿用之前专项的双拼九键布局而停在最初的全拼字母点按。保留该轮失败与清理记录；将专用夹具设为原用例要求的全拼 26 键后，新一轮的输入主路径、十八键、组字与剪贴板等独立检查继续执行。日语用例随后暴露实际夹具缺陷：仅从 APK assets 补文件，忽略已通过真实 JNI 校验的按需日语包，正式瘦包因此报 `FileNotFoundException`。两处准备入口共用资源前提：已有完整资源保持；否则经产品 `resourcePackVerify` 校验日语包及其沙箱内发布路径，直接使用运行时已经支持的安装包；没有安装包时仍从当前被测 APK 补齐。两种来源都不可用仍失败，不跳过原输入断言，不改变产品默认值或网络。259 个设备源码以替换后的实际文件通过 `javac --release 17 -Xlint:all -Werror`，包含该修改与 macOS 符号清单身份修复的完整 quick 已退出 0，前后逐一核对 12,410 个源码条目；详细通过与跳过范围见宿主采集笔记。原 API 35 整套已结束，退出 1，恰有两处日语资源前提失败及触控板与实体候选共存断言失败；保持原失败结果并核对设备清理完成。随后在同一已逐项校验 APK 哈希的正式瘦包、同一 API 35 AVD 上执行修正后的两个原用例，`japanese-toggle` 与 `chinese-language-return` 均通过；退出 0，原测试 APK 已恢复、任务禁网规则已移除。随后在相同正式瘦包上原样单独复跑触控板完整用例，包含放大镜与实体候选订阅共存，退出 0；产品 APK 哈希仍为上述 `2fce6751…`，原测试 APK 哈希为 `f6ec96b8…`，任务规则和编辑器已恢复。首次共存失败的根因仍未确认，不能由单独通过推定修好；没有修改断言、超时或失败基线，原整套仍记失败。`e865a66d0c` 的 GitHub API 35 设备任务 `122917247513` 已成功结束，该 CI 结果与此处的正式瘦包验收分开记录，当前不能记为 API 35 正式瘦包整套通过。

### 最新 API 35 PR 设备门禁

`23c1784272` 的 workflow `40823292706` 在 x86_64 API 35 上完成原生 App/IME、Java/JNI 故障、实际系统退出补录、输入超时 ANR 与撤回重开的专项，专项步骤通过。随后完整输入套件执行结束，唯一失败为 `voice polish replies (AssertionError)`；日语方案和触控板场景本轮通过。整套仍为失败，语音润色失败根因尚未确认，不加失败基线、不删断言。此轮不含后续持久收据修复，尚不能证明队列发送后重启仍去重。

### 最新收据修复的原生 APK 专项

`029e18636e` 的同源原生 arm64 Release 配置包（开发证书）在空白 API 35 专用 AVD 完成 `smoke.sh --crash-only`，退出 0。App/IME 的真实 Java/JNI 故障、持续授权撤回/重开、系统退出归属和实际输入超时 ANR 通过；系统退出及 ANR 的隔离队列清空后再次重启不重复入队。后一步模拟发送确认，不表示本轮已向后台传输。APK、Gradle 剥离产物及未剥离符号的实际身份已核对，三份新装包、合成数据与外网规则已移除，AVD 已停止；详细证据与首次包装器前提失败保留在[队列代次笔记](../../implemented/bug-fix/2026-10-28-epoch-bound-crash-collection.md)。API 28/35 的四个共享 crate 同候选设备测试均通过，原完整输入套件的语音润色失败和其他平台缺口仍未关闭。

该提交 CI 的两条仓库契约门禁失败于新上游的引擎许可证检查：把 `include_str!("diagnostics.rs")` 引入的本 crate Rust 源码当作待登记数据。`origin/develop` 的固定提交 `05d9917cd1548f1392578ed40c2e90596beb4b2e` 已在 PR #10214 修复该检查；本地分支尚无该新检查，所以原 quick 通过不等于这条合成合并检查通过。后续候选 `6bc13ef19d` 的 CI 两条仓库契约门禁均已通过；旧失败仍保留，不改许可证声明。该候选的 macOS arm64 与 iOS 模拟器作业通过，Windows x64/x86 WER 和 Android API 35 崩溃专项步骤通过；Windows 原生设置所在完整作业随后已完成并报告 success，包含 Win7 手写、版本检查与页面检查；Android 完整输入套件仍在运行，不把其崩溃专项单步通过写成整作业通过。新增同代次快照复用与计数/屏障回归不包含在该 CI 候选中。

### iOS 原生 App 的模拟器故障与动态授权闭环

固定候选 `a03cced2a553206dae77071539faeac422bc991c`，在既有 `target/ios-cargo/simulator`、`target/ios/derived-simulator` 缓存构建真实原生 `MSIMEApp`，Release、arm64、产品版本 `0.52.0`，没有以键盘测试 host 替代产品。首次构建与源码审计均通过，12,444 个条目逐项一致。验收限定本任务独立的 iOS 26.5 模拟器，不覆盖真实用户设备或其他会话的模拟器。

`telemetry-loopback` 只约束遥测，不改变 App 启动时的账号注册目标。因此最终验收产物额外显式链接仅测试用的回环传输护栏：Foundation 默认、临时、共享会话在发往网络服务前拒绝非指定目标，原生解析与连接也拒绝非指定回环；构造器没有审计日志路径或写日志失败即退出。独立探针确认三种 Foundation 会话、DNS 与原生连接均拒绝负例，指定回环正常收件；产品每次启动同时检查构造器日志及 `vmmap` 中的实际护栏映射，不能只相信启动命令的环境变量。此库不进入提交、正式产品或发布流程，测试链接不改产品源码。

最终 `linked-acceptance-attempt2` 实际执行五个 App 进程：开启时 `SIGSEGV`，原 App 重启补报；`SIGKILL` 后重启不新增崩溃；活跃 App 经实际共享 Rust 偏好 ABI 撤回，观察器清掉捕获后 `SIGSEGV` 不留记录，关闭状态重启也不补报；同一活跃 App 重开授权后 `SIGABRT`，原 App 再次补报且使用新 epoch。固定官方后台与临时 PostgreSQL 在基线外新增两条事件，恰好两条 `crash`，没有 `session` 或 `session_crash`；数据库与 HTTP 收件的事件 ID、平台、版本、消息、栈和安装身份逐字段一致。两份实际故障记录均保留 `application`、原提交、运行编号及架构。

实际后台自有帧 UUID `034f751d09483bceb6df057f22d20ea7`、偏移 `0x44bcd4` 与该轮 App/dSYM 匹配，`atos` 仅还原到 `main (MetasequoiaImeApp.swift:0)`。这证明构建身份、模块偏移和函数归属，该轮没有通过非零源码行验收；后续固定 `6bea262fd0` 已以真实内存故障补齐 App 非零行，见下节。系统 MetricKit 实际投递、键盘扩展故障、真机签名与后台仍未验收。历史证据为 `target/ios-product-local-evidence/`，远端进程、记录与核对材料在 `target/ios-product-loopback-a03/linked-acceptance-attempt2/`。

准备失败全部保留：模拟器缺少 `sandbox_init` 所需的库，外层 `sandbox-exec` 没有约束被系统启动的子进程，动态注入与额外调试签名的部分启动未获确认或被拒绝，外部标准流路径出现 `Launchd job spawn failed`，最终采用包内必须依赖、App Group 内审计路径及逐进程映射检查。不能将这些失败算通过，也不能仅凭后续成功认定它们各自的唯一原因。首次显式链接专项在发送信号后没有记录而失败，没有建立可单独核验的就绪证据，根因未确认；保留原目录，后续完整重跑退出 0，不放宽故障必须留记录的断言。

验收后已经重新编译无回环特性的 iOS Host API 静态库与原生 App，真实模拟器 ABI 调用拒绝 `test_origin` 且没有创建握手目录；App、键盘扩展均不再依赖护栏，包内护栏文件已删除。正式静态库 SHA-256 为 `c37fab70a07eaab32577a69b6671fb4084073b38b27c1bc3bbe3230a809e88ab`，正式 App 二进制为 `0d0b13471b7795497ef6de9cd93e91eee8ca7ed18c31d937324c4250c16085df`；恢复前后的 12,444 个源码条目相同。产品进程、任务模拟器、临时后台与数据库、内部网络和专用 SSH 转发均已清理，未改动其他模拟器。

### iOS 修订版真实内存故障与非零源码行

固定 `6bea262fd0055e90cb94b74e6addbb9ada037135`，在独立干净 worktree 按规定目录重建 Rust 与原生 Release App，12,444 条源码及文件模式逐项相同，十二份资源按当前固定锁验证 SHA-256 和大小。限 iOS 26.5 arm64 专用模拟器、版本 `0.52.0`；测试构建继续使用已验证的回环传输护栏，每次启动核对依赖、映射及护栏日志。外部 SIGSEGV/SIGABRT 重启补报、SIGKILL 负对照、活跃撤回后不捕获、关闭重启不追认、重开使用新 epoch 均通过，数据库与 HTTP 字段一致，没有输入会话。外部信号的生成入口仍为零行，单独保留该结果。

LLDB 在真实 App 的活跃授权观察器调用共享 ABI 时暂停，按实际反汇编将栈内保存的请求指针改为 `1`，请求长度保持真实值，实际产生 `EXC_BAD_ACCESS (address=0x1)`。调试器退出后确认原进程死亡，原记录器留下 SIGSEGV；重启原 App 后，官方隔离后台收到事件 `crash-8878d958-d7ba-4350-83cc-5683208c48a5`。App/键盘原始 dSYM 按 UUID、架构及行表验证后打成独立符号 ZIP，再解压，仅按后台实际帧还原：App UUID `2f1939c5e89e3e818c0eeb44c6ad7d36` 的偏移 `0x9b3ca8` 到 `msime_client_telemetry_consent (consent.rs:105)`，`0x645f90` 到 `UsageReporting.swift:346`，`0x64439c` 到 `UsageReporting.swift:299`；错误 UUID 被拒绝。首帧是 serde `de.rs:1829`，不把第三方首帧或仍为零行的生成帧算成产品源码位置。数据库与请求字段逐项相同，没有 `session/session_crash`。第一次只改入口寄存器被优化代码覆盖，原六十秒期限失败，原始失败保留。

收尾正式 Rust/原生 App 重建成功；首轮末尾夹具误读清单的 `base` 字段而退出 1，保留该失败，修订夹具以实际 `revision` 复核同一正式产物后退出 0。实际模拟器 ABI 在建目录前拒绝 `test_origin`；App、扩展及静态库不含完整测试来源，两个包均无护栏依赖或护栏文件。正式静态库 SHA-256 为 `7d1c63dd68223b775ea0e3a846563264244bd08477be616f6dcd7ea5fc94226e`，App 为 `a26eee9c59b271d83016f7680fa0ffaed33e1bf9ddad2886fb9218b11eeefbda`。前后源码和模式一致；合成同意已关闭，专用模拟器删除，独立进程审计无残留，后台/数据库/内部网络与转发已回收。独立符号 ZIP 保留，重复解压目录已删除；本机小证据在 `target/ios-6bea-product-evidence/`、`target/ios-6bea-backend-attempt1-evidence/`。本项完成模拟器 App 的真实内存故障到后台和非零行，不扩大到键盘扩展、MetricKit 系统投递或真机。

### PVE x64 Linux 与 Android 整轮补验

实际 `msime-linux-settings` 启动器在独立 Xvfb、D-Bus、合成 HOME 与匿名网络命名空间内，已完成原生 helper、Python HTTP worker 的代收，以及设置窗口 `SIGABRT` 后原产品重启补报；运行中撤回重开与 `SIGKILL` 对照通过。两轮末项 GDB 外部 `SIGSEGV` 均在六十秒期限失败：进程没有结束，既有处理器返回，但产品已经发送一条 crash，确认了误报，不能记为完整故障闭环通过。共用 POSIX 记录器改为跨前任处置持续持文件锁，前任返回时清空 inode 再删除记录；macOS/Linux 严格原生回归、Apple 故障帧与归档源码行专项及 Android 两 ABI 语法检查通过。补充前任更换回调、恢复默认后真实故障重发的回归，避免删除误报后丢失 Rust 的后续致命故障。PVE 最新完整 quick 与原提交干净基线同在两个候选画布像素断言失败，未加豁免，整轮不能记通过；修复后的真实设置产品、后台与归档已在固定 `6bea262fd0` 另行补验，结果见下段。iOS 独立 C 记录器也由负对照确认存在返回信号误报，现同步持锁撤销和真实上下文委托；Apple 宿主/模拟器 SDK/符号专项通过；同源 App 已另以真实内存故障完成后台与非零行，键盘扩展仍待验收。一次性前任的 Linux 回归通过，Apple 系统查询缺失标志的子项明确未执行。原失败、后台实际收件和清理证据保留在 `target/pve-linux-settings-acceptance/` 与本机 `target/pve-linux-settings-backend-attempt{1,2}-evidence/`。

固定 `6bea262fd0055e90cb94b74e6addbb9ada037135` 的修复版已完整 Release 重建，并通过原设置启动器的 helper 代收、窗口故障重启、动态授权与 SIGKILL 对照。GDB 先确认可恢复 SIGSEGV 返回，随后让实际 Rust store 访问零地址；原有六十秒期限内进程实际因 SIGSEGV 终止，重启后由产品补报。隔离官方后台收到四条 crash、六条 active，没有输入会话，数据库与请求身份逐项一致；实际后台帧按 build-id 与本轮 17 模块独立符号包还原到 `desktop_crash_reporting.rs:120`，错 build-id 仍被拒绝。随后恢复无测试回环的正式桌面与 Host API，安装后两条真实 ABI 握手在创建目录前拒绝测试来源，四条相关原生回归通过；完整 12,444 条输入前后不变。后台、网络、转发、私有 Xvfb/D-Bus 已回收，独立 `/proc` 检查无合成 HOME 进程残留。证据在 `target/pve-settings-6bea-product-evidence/` 与 `target/pve-linux-settings-backend-handled-attempt1-evidence/`；不代表正式安装或真实编辑器验收，完整 quick 的两个基线画布失败仍未消除。

`c713178d8e` 的 x64 Linux 优化构建已完成八个 Python 与十二个原生辅助入口的实际信号/异常、关闭对照、共享 ABI 代收、隔离官方后台与数据库核对，二十条故障保留原身份，没有输入会话；十二份原生实际后台返回帧经独立符号包还原到非零源码行。X11 监听器使用独立 Xvfb；代收/发送由验收程序调用真实 ABI，完整设置宿主的代收重启在后续 `6bea262fd0` 实际产品中另验通过。HTTP worker 的普通队列采集额外中位数约 3–4 ms，64 条、约 908 KB 队列约 6–7 ms；在线真实入口开启/关闭各二十四次均完成 100 ms 总预算。本轮不削弱短命采集或重写传输。恢复正式 Host API 后，九十七项原生测试全部通过，正式库拒绝回环测试握手；12,444 个源码条目前后一致。配置不匹配的首次原生代理测试失败与后台验收程序排序切片错误均保留，原因、范围、符号与清理见[宿主采集笔记](../../implemented/feature/2026-10-27-native-crash-capture-and-symbol-identity.md)。

Native Platform CI `40853458086` 的 `95cc68519e` Android API 35 作业于 `2026-10-29T11:05:39Z` 完整成功：四个共享 crate 的设备单测、实际 APK、崩溃/系统退出/ANR 专项及整套输入冒烟全部通过，语音回复与自动插入用例亦实际通过；本轮通过不证明此前 `voice polish replies (AssertionError)` 的根因已经修复。该运行整体验收仍为失败，因为 Windows x86 系统 WER 的目标退出超时；它已留完整记录，新增诊断的 PVE 两架构专项与完整 quick 通过，仍待新 CI。不能将 Android 整作业通过写成全仓 CI 绿，也不能将旧提交结果写成尚未运行的新 head 验收。

后续 Native Platform CI `40863279233`（`ab3a30ecd1`）的 Android API 35 整作业也已成功，包含四个共享 crate 设备单测、实际 APK、崩溃/退出/ANR 专项和完整输入冒烟；仍不包含随后新增的返回信号修补。该运行的 Windows job `123411749829` 失败：x64 七组通过，x86 的 `tip/host/external` 通过后，`withdrawn` 目标在原有 45 秒期限仍活着，诊断没有 `.crash/.werpartial`；与上一轮 `host` 已有完整记录的超时是否同因尚未确认。PVE 用相同 `ab3a30ecd1` 的 243 个源码条目前后核对，两架构 CTest 与十四组系统 WER 全部通过，原允许列表已清理；这不是 CI 超时原因已修复的证据。

最新 `6bea262fd0` 的 Native Platform CI `40875035104` 中，Windows job `123452209115` 在 MCP 严格 Clippy 阶段被上游新注册表读取代码的两处 `manual_is_multiple_of` 阻断，WER 步骤未执行；这次失败不能当作旧 x86 退出超时复现或已解决。Android job `123452209374` 已于 `2026-10-29T13:50:06Z` 整作业成功结束：四共享 crate 设备单测、APK、崩溃/退出/ANR 专项和完整输入冒烟均通过，包含返回信号修补；旧语音润色失败根因仍不由此次通过反推。

整合 develop 固定提交 `b24711854bf3b5f825b1b2af19c6e0754cff024f` 时，唯一文本冲突位于偏好字段校验表：同时保留上游 `translation_ai_migrated` 和本分支 `usage_reporting_consent` 的非 UI 字段归属，其他偏好保存与授权接口由 Git 自动整合。候选树 `727cd7f895380cd951c390a6fe7df2038c7873b8` 的 12,663 条源码及模式前后逐项相同；731 项 client-core 单测、66 项共享遥测、16 项 Host API 遥测、Rust 格式、Windows GNU core/Host/MCP 全 target 严格 Clippy 与整轮 quick 通过。quick 执行 203 项 Windows 可移植用例，仓库契约、Rust workspace/Android/wasm、Android Java/JNI、macOS 原生、Apple bridge 和 x64/x86 pipe-only 通过；鸿蒙 SDK/设置依赖、Docker/Linux 原生与设置、Windows 完整原生工具缺失的阶段明确跳过，不算平台运行验收。证据为 `target/merge-b247-quick-evidence/`；随后只补验证文档，不把旧设备通过转记为本候选通过。此前 `cdb4fe7021` 的一份 contracts job 在发布说明测试清理临时 Git 目录时收到 `Directory not empty: objects`，同 head 另一份 contracts 通过；尚未定位清理竞态来源，不能据此宣称已修复。

### Ubuntu x64 实际系统安装与用户级服务

固定 `1ba6b6ee05e461683e556111fa38b650781edbcd`，复用 Linux 构建树正式重建 Host API、MCP、打包配置的共享设置组件和原生宿主，未启用遥测回环特性。CPack 生成 Ubuntu 24.04 x64 的 `msime-linux_0.14.0_amd64.deb`，SHA-256 为 `f4b3834954beedee9e8f7cc16b6c725e779b7b363b4c6c62c3e607cc7e2c1145`；包含锁定核心词库与语音运行库，没有冒充 Debian 发版容器或带齐所有可选资源的官方发布产物。

在 `pve9` 创建本任务的空白、非特权 LXC106，实际 apt 安装到 `/usr`。安装时没有登录用户，随后移除容器网络接口，再创建合成用户 `crash-test` 并由真实 `loginctl` 用户管理器运行产品首次配置。实际词库校验与运行配置生成成功；匿名账号初始化因无网络失败后按既有降级继续，没有生产注册。在线、语音和剪贴板分别使用安装包原有的用户级 systemd 单元，不用 transient unit 替代，逐项核对原 `ExecStart`、`Restart=on-failure`、三秒重启延迟、`RestartPreventExitStatus=2` 和 `UMask=0077`。

三个活跃服务各完成开启时 SIGABRT 落盘、systemd 自动重启、原构建头代收到共享队列，SIGKILL 重启不新增崩溃，活跃撤回后 SIGABRT 不采集，重开新 `consent_epoch` 后再次记录与补报，以及主动停止不重启。每组故障后的 crash 计数为 `1/1/0/1`，没有输入会话事件。授权关闭后原记录和队列清空，独立审计确认 159 份实际安装文件哈希不变、无测试产品进程和外网路由。本轮只到共享队列，不借用旧构建的后台结果或把 Python 系统帧说成自有源码行。

完整词库 Release 门禁首轮 128 项通过、两项失败：`ibus-translation-scope` 的第二行实际为按规则不翻译的 Emoji；`ibus-chaizi` 仍把缺省关闭后的 `Shift+C` 当成透传，而上游已允许它开始普通大写组字。测试只限定专项的可翻译候选，并同时验证关闭拆字时普通原文组字/提交、开启时按部件找到「焱」。中间版以连续输入探测菜单应用时序，完整门禁仍有一项失败，独立复跑也失败；最终以实际菜单回包为应用屏障，不再靠文件落盘或接收首键判断。最终两专项各连续三次通过，完整 130 项全部通过、无跳过（294.75 秒）；沿用命令专属 `FONTCONFIG_FILE=target/windows-x86-fonts.conf`，没有改产品默认值、译文来源过滤或输入行为。改动分别同步到既有[大写输入笔记](../../implemented/bug-fix/2026-10-14-uppercase-pinyin-start.md)与[翻译范围笔记](../../implemented/feature/2026-10-26-candidate-translation-scope.md)。

随后的原仓库 `scripts/verify-local.sh --quick`（Linux 原生 119/119）、笔记门禁与 tracked 输入审计均退出 0，验证前后 12,666 份文件的字节和模式一致；字体配置只作用于该命令。缺少 Android SDK、鸿蒙暂存环境及部分宿主专用编译工具的阶段明确跳过，不能用本轮 quick 代替那些平台的设备结果。

独立归档按安装包筛出十九个自有 ELF，逐份核对实际安装文件、包内容的 SHA-256，以及符号归档与剥离后文件的 build-id 和 `.text` 摘要；排除构建目录里的测试与旧依赖产物，包内重命名的桌面程序以实际安装名登记。此项证明精确符号身份，不将外部 SIGABRT 的 Python/系统帧计为产品源码行还原。

远端已恢复原 `e746981d18` 干净源码及正式 Rust/原生缓存，原 CMake 的 107 个配置项逐项一致；不是只退回源码而遗留安装测试配置。实际共享 ABI 返回原 revision、运行编号与架构，begin/flush 在创建目录前拒绝测试目标，身份探针队列清空，任务构建锁移除。首次恢复在清理传输附带的五份 AppleDouble 元数据前停止；只在核对路径和文件头后删除本任务元数据，再按原源码清单复验与恢复，没有删其它会话文件。

两轮夹具失败保留：初始关闭未运行清理观察器而留下 active 队列，以及 systemd 重启时读取已退出旧 PID 的 `/proc` 竞态。第三轮完整通过，没有放宽崩溃落盘、授权或重启断言。独立清理审计首次误要求已清理的队列文件仍存在，改为同时接受缺文件或空数组后通过。`apt remove` 按 dpkg 约定保留 `/etc` 的 conffile；随后实际 purge 再审计，159 个产品文件全部移除，五个原服务/socket 不活跃、未启用且单元文件消失。LXC106 及其 16 GiB 任务卷已停止销毁，PVE 上本任务传输文件全部删除，Ubuntu 模板保留。114 份构建、门禁与恢复证据已回读，独立压缩包 SHA-256 一致；连同三服务和卸载证据保存在 `target/linux-system-install-e746-evidence/`。目录名沿用准备时的 e746，实际包、原故障头与服务验收候选均为上述 `1ba6b6ee05`。

### Ubuntu x64 系统安装后的三个图形入口

同一固定候选 `1ba6b6ee05` 的原 DEB 再次安装到本任务新建的非特权 Ubuntu 24.04 LXC。三个入口分别使用全新实际用户、原 HOME、私有 Xvfb 和 D-Bus；启动产品前移除网络接口，读取到 IPv4 路由与 IPv6 默认路由均为空。包内原 `/usr/bin/msime-linux-ibus-launcher`、系统 Fcitx5 加载的实际安装插件和原 `/usr/bin/msime-linux-settings` 运行，未用暂存前缀替代。

IBus 与 Fcitx5 均经真正的 GTK 3 Mousepad 文本框输入合成拼音并保存中文，初次及每次故障重启后共五次上屏；设置入口核对真实可见窗口及 PID。三者各通过开启采集的 SIGABRT、SIGKILL 不增加故障、活跃撤回后 SIGABRT 不采集、重新授权换代后 SIGABRT 再次补报，四组队列中的 crash 数均为 `1/1/0/1`。IBus 使用包内原监督器自动重启；Fcitx5 与设置用同一原命令显式重启。撤回会清空队列，因此后一组的 1 不是累计数。设置入口没有输入会话事件。

另以第四个空白用户，在实际 GTK 按键进入安装插件 `FcitxEngine::keyEvent` 时由 GDB 将事件引用参数寄存器改为 1，继续执行产品原指令，核对真实 `SEGV_MAPERR`、故障地址 `0x11` 和系统致命结束。原 daemon 重启后入队且再次中文上屏。实际队列的 `libmsime-fcitx5.so+0x1c389c`、build-id `2994fdfdec3b1e6a125d6fdd8d84410959c6d6f4` 由原十九模块独立符号 ZIP 还原到 `FcitxEngine.cpp:7705`；首个内联帧是 Fcitx 头文件 `inputcontext.h:288`，必须和仓库符号工具一致带 `addr2line -i` 才能看到自有调用位置。另一台 Linux 环境运行仓库原符号工具得到相同位置，错误 build-id 拒绝。此项没有修改安装程序或插件字节。

夹具失败记录保留：第一轮把偏好正文当成完整快照被真实 ABI 拒绝；IBus 第二轮缺少 `ibus-gtk3` 而原样上屏英文，第三轮复用了残留总线 socket；补齐签名索引 SHA-256 核对的 GTK 前端、改用独立总线地址后第四轮通过。真实访问冲突的前两轮分别缺少继承的 zsh 和重用配置目录，第三轮通过；首次符号审计漏了内联栈，原记录未改，补用原工具的 `-i` 后核对。安装依赖期间的 DNS、慢源、绕本机传输及 GTK 前端下载失败均不算产品通过，日志保留。

本轮只到共享队列与精确独立符号，不声称正式产品向隔离后台发送；真实访问冲突只覆盖 Fcitx5，IBus/设置的外部信号不当成访问冲突。没有覆盖安装包的原生 Wayland、其他架构或其他发行版。最终独立审计确认四个合成用户的授权均关闭、队列清空、159 份安装文件字节未变，随后实际 `dpkg --purge msime-linux` 移除全部产品文件且无残留产品进程。证据包与宿主日志分别以 SHA-256 `341d79776f4acc518660883b345b2dd3b85dc8491ab3c40b9499cfbb79017fa4`、`e47dc7ec1f093237f97e7342d2b62b4230479b12c81cc69235a9017dc9a7a961` 回读核对，保存在 `target/linux-installed-gui-evidence/`。再次核对任务 hostname、description 和 rootfs 后停止销毁 LXC106，独立确认配置与 16 GiB 任务卷消失；PVE 上本任务前缀的全部传输文件及四份逐一核对哈希的通用名夹具均已删除，Ubuntu 模板保留。Linux 构建机源码与原生缓存恢复到原干净候选，原正式 Rust 产物哈希相同，临时 LAN 服务和构建租约已清理。

### Linux 三个产品服务的 systemd 生命周期复验

固定 `e746981d18261e39cf6f0530934985fad6afcf9b` 的 Ubuntu x64 暂存前缀，在真实 systemd 中逐项运行 `provider-session online`、`provider-session voice` 与 `clipboard-monitor`，核对安装单元的原重启策略；语音同时保留 `KillMode=mixed` 和九十秒停止时限。任务专用 transient unit 以原用户运行，增加 `PrivateNetwork=yes` 并从实际进程网络空间确认无外网路由。仅以 XDG 隔离配置、状态、缓存和运行目录，没有改写 `HOME`，不连接用户的剪贴板显示或录音会话。

三个服务分别完成四次真实致命结束后的自动重启：开启时 SIGABRT 留原记录，SIGKILL 不新增崩溃，活跃撤回后 SIGABRT 无记录，重开后 SIGABRT 使用新授权代次。原捕获头保留固定提交和各自的 `online-provider`、`voice-provider`、`clipboard-monitor` 身份；由共享控制器只代收入队，不联网，不产生 `session/session_crash`。再次活跃撤回后等到实际解除武装并清空旧崩溃，再主动停止；停止不重启，真实参数错误均以 `2` 退出且不重启。

前两轮夹具失败保留：首轮合成运行目录使 Unix socket 路径超过系统上限，产品按参数错误退出；首轮清理又错误要求仅关闭偏好就能清空停止服务的活跃事件。缩短 socket 路径后在线服务生命周期通过，但第二轮在停服务之后才撤回，旧崩溃队列没有观察器清理，清理断言失败。最终独立第三轮在停止前等待实际撤回清理，三个服务完整通过，没有放宽产品断言或修改产品。清理时以共享控制器确认关闭状态且不武装，并保存失败轮原队列快照；没有修改失败结论。

独立审计确认十八个已用或预留 unit 名均不活跃，五份合成授权关闭，队列与原记录清空，`/proc` 无对应测试进程，两个短运行目录删除；暂存产品全部 151 个文件哈希及 tracked 源码不变。小证据为 `target/linux-systemd-service-matrix-e746-small-evidence/`，包含实际服务属性、原记录、分项结果、失败日志及独立清理结果。本项只补齐三项本地 systemd 生命周期与授权队列验收，不扩大为系统级安装、后台收件、其他发行版或随后 C++ 待送达信号修复的产品验收。

### Linux 原生 Wayland 与 systemd 验收

固定 `6bea262fd0` 的已有 Wayland 结果经重新审计并收回小证据：Ubuntu x64、Fcitx5 5.1.7、真实 Mousepad，独立 Sway 关闭 Xwayland 且无 `DISPLAY`，IPC 确认窗口为 `xdg_shell`。开启故障、SIGKILL 对照、活跃撤回与重开均恢复同一文档输入，后台字段与数据库一致；原记录的授权代次不同。另在真实 GTK 按键进入原生 `keyEvent` 时，以 GDB 将事件引用参数改为 `1`，确认系统 `SEGV_MAPERR` 和实际故障地址 `0x11` 后继续原处置；重启同一 daemon 后后台收件，独立两模块符号包将实际自有帧 `0x1c301c`、build-id `56d2a5ceb327d8cf54dcf545537b3652addaec8b` 还原到 `FcitxEngine.cpp:7678`，错 build-id 拒绝。执行前后 12,444 条源码及模式一致。原键盘映射、前端焦点和故障夹具的失败轮仍保留，不将其算作通过；通过证据位于远端 `target/linux-wayland-editor-{acceptance-attempt4,access-attempt2}/`，本地小证据为 `target/linux-wayland-editor-product-evidence/`。后台容器、网络和转发已回收，两份通过轮的合成 HOME 经独立 `/proc` 核对无残留进程；这不是当前整合候选或系统级安装的复验。

同一固定版本的正式安装前缀另在真实 systemd 中运行 `msime-linux-provider-session online`：任务专用 transient unit 使用产品在线服务相同的 `Restart=on-failure`、三秒延迟、`RestartPreventExitStatus=2` 与 `UMask=0077`，增加 `PrivateNetwork=yes` 隔离外网。实际读取记录器的武装位，并核对服务网络空间没有外网路由；四次致命结束由 systemd 自动重启，SIGKILL 不新增崩溃，活跃撤回解除武装并清空旧队列，重开使用新代次。主动停止不重启，真实参数错误退出码 `2` 不重启。真实共享控制器代收入队，没有 `session/session_crash`，本项未联网，不冒充服务到后台的验证。七份相关安装文件前后哈希一致，两份任务 unit 已停止并清理，合成同意关闭且 `/proc` 无残留。证据为 `target/linux-systemd-capture-evidence/`；范围不扩展到剪贴板、语音服务或发行版桌面安装。

### iOS 键盘扩展的实际系统入口

固定 `6bea262fd0` 的 iOS 27.0 arm64 模拟器经系统设置启用键盘和完全访问，由合成 UIKit 文本框实际拉起原生 Release 扩展。外部致命信号与调试器在真实授权 ABI 中注入的无效指针访问均落盘，经原生 App 到官方隔离后台；两份键盘 `crash` 各对应一份 `session_crash`，HTTP 与数据库字段一致。独立两模块 dSYM ZIP 按扩展 UUID `c707155290a63930a248811cecafec45` 还原到 `consent.rs:105` 与 `UsageReporting.swift:346/299`，错误 UUID 被拒绝。活跃撤回后的故障不留记录、不补报，重新开启使用新代次，`SIGKILL` 不误报；恢复后通过实际键盘按键输入合成文本“你好”。12,444 条源码及模式与固定清单相同。正式 Rust 与 App/扩展已恢复，静态库 SHA-256 与验收前一致（`7d1c63dd68223b775ea0e3a846563264244bd08477be616f6dcd7ea5fc94226e`）；正式 ABI 在建队列前拒绝测试握手，App/扩展的 dSYM UUID 匹配，正式二进制和库无完整测试地址，包内无网络护栏依赖或文件。两份合成同意关闭，两个专用模拟器删除且无残留进程，隔离后台容器、网络、监听与 SSH 转发已回收。

iOS 26.5 轮在扩展框架的 `___nsx_pingHost:` 未识别选择器异常后退回系统键盘，未计通过，根因尚未确认。未签名 UI 运行器复用旧代码、缺少 Scene 的测试宿主、元素定位和护栏初始化等待，以及汇总清单字段误读的失败均保留；修订夹具后独立验收通过。与外部拉起 App 抢焦点的 `ios27-hold-attempt3` UI 用例仍失败，不把它算成通过；撤回和 SIGKILL 结论只来自各自独立进程、队列和后台证据。本专项限模拟器中的真实扩展及合成文本宿主，不能替代真机、MetricKit 系统投递或发布安装验收。证据为远端 `target/ios-keyboard-proof-attempt1/` 和本地 `target/ios-keyboard-product-evidence/`。

### HarmonyOS API 22 应用的真实故障闭环

固定源码 `130263a9873b1e0ab8f1e55b7f58d28daba0083b` 在干净 detached worktree 同源重建 Rust、NAPI、资源、设置页和原生 HAP（版本 `0.3.1`）。专用 arm64 2in1 模拟器只复用镜像与设备配置，数据盘全新创建，没有复制现有用户数据；宿主进程继承只允许本机回环的网络沙箱。通过真实产品界面同意隐私政策后，外部 SIGSEGV、在实际每秒授权入口将请求指针改为 1 后脱离调试器产生的真实内存访问故障，均经原 App 重启进入固定官方后台与临时 PostgreSQL。初轮测试特性注入使用 shell `cargo()` 函数，但产品脚本经 `env` 执行外部 Cargo，实际仍是正式库；包内目标检查揭示该错误，网络沙箱阻止了正式外网传输。后续显式编译 `telemetry-loopback`，核对包内完整回环目标与双模块 build-id 后重跑，不把首轮计为后台通过。

第二条故障暴露 512 项模块快照截断，自有库全部变为 `<unknown>+绝对地址`。在上述固定源码上仅应用 `CrashRecorder.cpp` 补丁（SHA-256 `f8ecd666c236c5707ce1f0cf98813b9e1c8dc6c4824ff743e4fe67ce7b4fd35c`），复用未改动的 Rust 产物，重新构建 NAPI、HAP 和独立符号 ZIP，再触发同一路径的真实无效指针故障。后台直接返回 Rust 与 NAPI 的模块偏移和原 build-id，独立 ZIP 还原到 `consent.rs:105`、`client_napi.cpp:303`；直接故障帧在 `serde_json` 的 `read.rs:562`，不能把返回帧称作直接故障指令。错误 build-id 确实拒绝。最终数据库有三条 `crash` 与一条 `active`，没有输入会话；HTTP 为三次 crash 与两次同 ID active 请求，后台幂等保存一条 active。按接口将 active 缺省的 message/stack 规范为空字符串后，所有实际 HTTP 字段与数据库一致。只修正验收夹具的缺省值比较、构建头 JSON 解析及符号命令的位置参数，未修改历史后台记录。HAP 剥离调试信息，首次整文件相等断言失败；改为核对包内 build-id、归档未剥离文件 SHA-256，保留原失败。

在仍运行的原设置页切换“匿名使用统计”，共享权威授权实际变为关闭；再次 SIGSEGV 退出不产生原生记录。原 App 重启、通过原设置页重新开启后取得不同授权代次，10 秒观察窗口内未补发关闭期间故障；开启时 SIGKILL 后原 App 重启，12 秒窗口内没有伪造 crash。合成授权最终关闭。正式 Cargo、NAPI、staged 库与 HAP 已恢复，均不含完整回环目标；设备上的正式 ABI 接受普通关闭请求，却在建队列前拒绝带 `test_origin` 的 begin/flush 请求。最初探针误要求返回内部解析详情，而产品按契约只返回 `invalid request document`，修订探针保留错误封装并用无测试字段的成功请求作正对照。应用、调试 server、专用模拟器、设备临时文件、HDC/SSH 转发、隔离后台、数据库和网络均已清理。

模块上限回归在 macOS 先拒绝旧实现，修复后 macOS/Linux 均通过；Apple 共同记录器与 iOS C 信号专项通过。PVE 完整 quick 首轮有两项候选画布精确颜色断言失败（117/119 通过），本机字体启用 RGB 次像素渲染；保持产品和测试源码不变，命令专属 `FONTCONFIG_FILE` 关闭抗锯齿后，原两项断言及第二轮完整 quick 均通过（Linux 原生 119/119）。没有修改系统或产品设置；该 Linux 环境缺 Android/Apple/HarmonyOS SDK、Windows/MinGW 等阶段明确跳过，鸿蒙原生/HAP 与 Apple 记录器由上述真实 SDK 专项覆盖，不能写成所有平台运行通过。小证据在 `target/harmony-api22-product-evidence/` 与 `target/harmony-api22-backend-attempt2-evidence/`。本项限 API 22 原生 App；模拟器明确报共享 data group 不可用，不能替代真机共享目录、API 21、键盘、ArkTS 致命错误或 HiAppEvent 实际系统投递。

### HarmonyOS API 22 手机键盘的实际输入与共享目录边界

固定提交 `4d179327a4ce6ecb0f6065a0bbe3bcf2f6392541` 在同一干净 detached worktree 复用既有 `target/ohos-cargo`，同源重建 arm64 Rust、NAPI 与正式 HAP。正式包内 Rust build-id 为 `dae1ca6d800e0281bbe817c96ac174f7fb64afec`，NAPI 为 `db2840ae8cb78a1756d69d078cb3690a7f2d7ed0`，均与各自独立符号清单一致；版本、提交、运行身份和资源文件已核对。安装目标是只复用只读 API 22 镜像的全新手机实例，继承只允许宿主回环的网络沙箱，没有复制用户数据。初始 3 GB 数据盘安装返回 `9568288`，改为官方手机配置的 6 GB 数据盘后，同一冻结正式 HAP 安装成功；原失败与两个实例的证据均保留。这里没有 API 21 镜像，不把 API 22 运行写成 API 21 兼容验收。

通过原产品界面同意隐私政策，在系统启用并切换原输入法，在原「试用键盘」的 `textField` 逐个点击 `NIHAO`，出现并选择“你好”，实际编辑框上屏成功。App 私有权威授权处于开启状态，但 App 与键盘的真实 `getGroupDir` 均返回空目录；键盘私有目录没有隐私政策镜像、权威授权或遥测目录。对精确键盘进程发送一次外部 SIGSEGV，系统故障文件确认原 PID、`app.msime.hmos:inputMethod`、版本 `0.3.1` 和 `SIGSEGV(SI_USER)`，不是受控无效内存指令。故障及系统重建后，键盘仍没有建立遥测目录；同期 App 队列只有一条 `active`，没有 `crash`、`session` 或 `session_crash`，App 的原生记录目录也为空。该结果证明无共享权威授权时不降级采集，**不构成已授权键盘故障、后台收件或符号化验收**。

重启输入首轮只有键面而没有本进程 `attached to editor` 证据，逐字点击没有候选，未计通过；收起键盘并重新聚焦同一个原输入框，核对真实编辑连接后再次点击 `NIHAO`、选择“你好”，编辑框严格为“你好你好”。测试始终没有发送对话。键盘沿用空白沙箱的本机学习默认值，合成词只写入这份一次性本地沙箱；没有共享授权、账户联网或外网通道，最后连同沙箱删除，不声称已经从设置应用同步关闭键盘的学习。独立调试根权限请求被镜像拒绝，没有改动权限或私有文件。首次信号夹具错误要求 `devicedebug` 打印 success，但它在发出真实信号后静默返回；后续按系统故障文件继续核验，没有重复发信号或改写首次失败。

名为 loopback 的第二份构建实际未包含测试目标：`env cargo` 绕过 shell `cargo()` 函数，包内检查发现 `test_origin_present=false`，它没有用于故障或后台验收。之后已按正式参数重建 Rust/NAPI/HAP，两个未剥离模块的 SHA-256 与第一份正式产物逐一相同，均不含完整测试目标；设备上的正式 ABI 接受不带测试字段的关闭请求，却在建队列前拒绝带 `test_origin` 的 begin/flush 请求。收尾通过真实隐私页关闭合成上报授权，核对系统输入法、卸载产品、清理临时探针及两个专用模拟器的数据盘；初次重复切换输入法返回 `77`，独立 `ime -g` 已确认基线输入法，后续按实际状态继续清理，不把错误码当成功。第二次清理在设备侧检查通过后观察到模拟器进程仍在短暂收尾，后续独立进程审计确认退出后才删除两个数据盘。12,663 个原 tracked 文件哈希与构建前清单一致，远端源码干净。114 份小证据为 `target/harmony-phone-4d179-small-evidence/`，归档 SHA-256 `73940fe4bcb4604e82315c2c034dbcbf40af943122e90a2f574ef93c548a552c`，已在本机逐项回读并重新核对实际上屏、系统故障、队列与包内身份；无后台、数据库或端口转发在本轮启动。真机共享目录、具备权威授权的键盘故障闭环、API 21、ArkTS 致命异常和 HiAppEvent 原身份实际投递仍未完成。

本次验收记录提交前的完整 `scripts/verify-local.sh --quick` 退出 0：Linux 原生 119/119、177 项宿主可运行 Windows 用例、仓库契约、Rust/Android/wasm、Java/JNI、鸿蒙 HAP、Linux 设置壳、Apple bridge 与 x64/x86 pipe-only 通过；本机未配置的完整 Windows/macOS 原生阶段明确跳过。笔记校验与 `git diff --check` 通过，日志为 `target/harmony-phone-quick.log` 和 `target/harmony-phone-notes-final.log`。最后只更新该验证结论，不把目标编译或旧设备结果记成缺项的运行验收。

## Alternatives considered

- **用产品生产后台直接验收合成故障**：无需增加构建入口，收件与符号检索也最接近线上；但会污染用户正在查看的崩溃计数和聚合，故拒绝。验收构建显式开启 `telemetry-loopback`，由 `MSIME_TEST_TELEMETRY_ORIGIN` 在编译时固定 `http://127.0.0.1:<非零规范端口>`，配置缺失或错误一律拒绝发送，不回退生产。正式构建不读取该配置；账号、公告等其他服务不改源站。桌面独立发送器与 Host API 两条发送分支共用同一工厂，MCP 只采集，由宿主补报。设备经显式端口转发连接隔离后台，不能据此声称六平台设备到后台链路已验收。
- **允许运行时设置任意测试后台地址**：方便直接连接远端验收环境；但扩大正式产品的配置面和网络目标。测试构建只接受唯一规范回环形式，拒绝域名、公网、HTTPS、凭据、路径、查询和片段，沿用禁止代理和重定向的传输。C ABI 验收请求附 `test_origin` 握手：只在测试特性下解析并核对编译目标；正式库按未知字段拒绝，避免测试工具误载正式库后联网。该字段不是运行时改地址入口。

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

### 与偏好容错的整合

整合 `db1ea0d73e` 的 develop 时，持续授权接口与运行时容错接口保留各自职责：输入可在内存里救下可读设置，持续授权仍严格拒绝坏文档；修复配置保留未知字段，同时清空旧授权元数据。恢复回归同时核对这三项边界。合并结果的核心/Host API 回归、严格 Clippy 和整轮 quick 另行冻结排队；合并前的通过结果不算合并结果通过。

本轮整合 develop 固定提交 `afb1166847efb35e075989a1c252edc8313929fb`。Android JNI 同时保留持续授权的 `telemetryConsentRaw` 与上游布局迁移的 `migrateKeyLayoutRaw`，原生导出门禁按双方条目的并集核对；不以整文件取一方的方式解决冲突。自动整合的共享偏好与账号同步仍保留授权元数据的本地归属。编译发现布局迁移仍调用旧的四参数 `save_locked`；迁移与普通保存一样显式传 `false`，只保留已有授权，不能因升级布局而初始化上报历史。三种状态的回归同时核对关闭、开启但未初始化、已有持续授权，迁移布局后开关和身份不变且可重新读盘。整合结果另存源码 SHA-256 清单，复用原构建树运行 Android 契约、授权/恢复/补报回归、严格 Clippy 与完整 quick；未得到结果前不将旧快照的通过记录归到本轮。

后续整合 develop 固定提交 `0117616dc0037a67c2c0287ba65062f748259d31` 时，现代 Windows 设置的保存接口已从布尔成功值改为可选失败分类。采集刷新只放在新文档成功解析并更新修订号之后，成功返回 `std::nullopt`；存储错误、修订冲突或响应损坏仍走原失败分支，不用未保存的开关刷新采集。CI 同时保留 MCP 崩溃/授权回归与上游共享存储占用重试用例。上游对遥测队列临时文件发布的替换重试通过共享存储层接入，不改变授权锁顺序或故障记录确认规则。

该整合结果的候选树 `2ea45401422e83ed7099bef90bbf27c349e92f9b` 经完整 12,409 条目核对，存储、持续授权、恢复、账号同步、Host API 遥测和四个共享 crate 的严格 Clippy 通过，整轮 `scripts/verify-local.sh --quick` 退出 0。Linux 原生 118 项注册测试中 117 项执行通过、按原规则跳过一项首次激活测试，full/wubi 安装后八个 Python 入口均通过；真实 IBus 首次激活由上述独立运行验收覆盖。macOS 原生、共享 Apple bridge、Android 宿主、鸿蒙 HAP 和 Linux 桌面编译通过；Windows 原生与 pipe-only 因构建机缺少 MinGW 而跳过，不算目标验证通过。前一份 e275 源码的整轮门禁曾出现收集器原十五秒超时，以及剪贴板原三秒超时和 Fcitx5 最新偏好断言失败；原脚本复跑与本轮阶段均通过，但根因尚未确认，未放宽上限或新增失败基线。提交前只更新本覆盖笔记并重新验证笔记，不将文档更新视为代码重新验收。Linux 验收构建的回环产物另存后，运行时缓存已重建为正式配置，并通过正式库在建队列与发送前拒绝测试握手的专项。

随后整合远端固定提交 `aaebaca94456ad5a9bfec5cbf76f0e98c07e9851`，保留共享发送器、桌面和 MCP 的逐条授权保护，以及拒绝未绑定入口绕过已迁移队列的约束。候选树 `6c4fb106e9fbe1cbf24d3dcbeef97217412716b1` 的 12,410 个源码条目全部核对一致；格式、笔记、存储 19 项、持续授权 13 项、运行时读取 1 项、恢复 9 项、账号同步 34 项、Host API 遥测 16 项、共享遥测 61 项、MCP 捕获 2 项及五个相关 crate 的全 target 严格 Clippy 通过。MCP 忽略的子入口由父用例实际启动。完整 `scripts/verify-local.sh --quick` 退出 0，Linux 注册 118 项、117 项执行通过和一项按原规则跳过，full/wubi 安装后八个 Python 入口通过；鸿蒙 HAP、Android 宿主、Linux 设置壳、macOS 原生及共享 Apple bridge 编译通过。Windows 原生、pipe-only 和 x86 语法因构建机缺少交叉工具链明确跳过，不算目标验证通过。证据保留在 `target/merge-aaeb-evidence/`；相对此候选树只更新覆盖笔记，代码与远端 aaeb 提交一致。

Fcitx5 子目录独立配置且两个波形后端均未启用时，`handler` 与 `force_auxiliary` 没有使用，原有 `-Werror` 拒绝编译。两个标识符显式标为 `[[maybe_unused]]`，不改变后端选择或运行行为；原配置失败记录与修复后的同配置完整插件编译均保留，Linux 顶层工程也已重编并恢复链接正式 Host API。修复候选代码树为 `ec01fefae214ef4676d11167584c8db32ae98975`，12,410 个条目逐一核对。首次完整 quick 的其他阶段通过，但五项 Windows 可移植测试在链接阶段失败；单独重编和运行其中的 `ai_candidate_worker` 两次通过，原链接诊断只保留了驱动末行，根因尚未确认。后续显式设置 `PYTHON_CPU_COUNT=2` 后，203 项 Windows 可移植测试与完整 `scripts/verify-local.sh --quick` 均通过，退出 0；Linux 注册 118 项中 117 项执行通过、一项按原规则跳过，full/wubi 安装后八入口通过，macOS 原生、共享 Apple bridge、Android 宿主、鸿蒙 HAP 和 Linux 设置壳通过。没有放宽断言、超时或新增失败基线，不能由复跑通过推定首次链接失败的根因。两次整轮结果分别保存在 `target/fcitx-first-quick-evidence/` 和 `target/fcitx-limited-quick-evidence/`。此后只更新覆盖笔记，代码与已核对候选树一致。

### macOS 符号发布仍需补验

发布脚本已显式保留源码行、收齐 Swift 后端原始 dSYM，并按每个架构的 UUID 与实际行表拒绝空包，细节由上述宿主采集笔记持有。Clang/Swift/Rust 合成用例和实际 Release Swift 后端的双架构源码还原通过；完整同源发布包和实际设备故障到源码行的链路仍未验收，不能只凭 zip 存在宣称完成。

### 共用输入会话的持续授权

Windows Server、IBus、Fcitx5 与 Win7 设置/面板的共用 `Telemetry` 包装器已接入持久授权代次与独立观察线程。macOS/Linux 真实子进程用例通过，覆盖在途发送期间撤回、重开与旧结果收尾；Fcitx5 与 IBus 已用同源 Host API 完成实际产品重建，共用层与 IBus 专项通过，Windows 目标运行仍待补验。macOS 输入法已复用共用协议，同源 Host API 重建和真实输入法链接成功，最终六项真实 CMake 专项通过；现代 Windows 原生设置已复用共用包装器，实际 Windows 编译运行待新一轮 CI；Android 与鸿蒙仍须逐入口核对共享队列协议，不能将这次共用包装器迁移写成全平台完成。具体协议与验证由[采集与队列绑定](../../implemented/bug-fix/2026-10-28-epoch-bound-crash-collection.md)持有。

本地集成候选 `c4f21a9a6073e9ce40d2f34469903bb3bb26e570` 合入固定 develop `1ffd90e137` 和两行注册表奇偶校验等价修补。12,612 条源码及模式与远端干净候选逐项一致，整轮 quick 退出 0，结束后再次逐项核对相同；Windows GNU 目标的 Host/MCP 全 target 严格 Clippy 和格式检查通过，不代替 MSVC 结果。两行切片首次 quick 仅语音目录冷编译一百二十秒超时，第二轮退出 0。新增 WER 超时快照的原生状态与退出竞态专项通过，旧 x86 超时仍未定位；详见 TSF 笔记。此前 Linux/iOS 产品验收仍归属原固定 6bea，不转记为此次集成产物；PVE Linux 旧整轮的两个画布断言失败也不因另一台机器的 quick 通过而视为修复。

最终诊断候选 `b753f82b9adfa720d10ea41c344684c786759700` 在同一既有缓存执行整轮 quick，退出 0；12,612 条源码及模式在开始和结束后均与冻结清单逐项相同。203 项 Windows 可移植用例、契约与笔记、Rust workspace/Android/wasm、Android 宿主 Java、macOS 原生、Apple bridge 和 x64/x86 pipe-only 编译通过；鸿蒙 SDK/设置依赖、Docker 和 MinGW 原生工具缺失的阶段明确跳过，不算平台运行验收。证据为 `target/wer-snapshot-quick-evidence/`。Linux 旧两项画布失败与 Windows 旧 CI 退出超时仍保留，不从本轮通过推断根因。

固定 `6bea262fd0` 的 Ubuntu x64 真实 Mousepad 编辑器已通过系统 GTK IBus 模块连续上屏，原监督器依次完成开启时 SIGABRT、SIGKILL 对照、活跃撤回后的 SIGABRT、重开后的 SIGABRT；每次重启后都在同一文件继续输入，最终为五次合成“你好”。隔离官方后台收到两条 crash、两条 session_crash 和一条 active，数据库与 HTTP 七个字段逐项一致，撤回重开的原记录 epoch 不同且发送消息不含 epoch。两条实际后台自有帧都通过独立两模块符号包还原到 `ibus_main.cpp:296`，错 build-id 拒绝；两份合成 HOME 均经独立 `/proc` 检查无进程残留，后台、内部网络和专用转发已回收。

首轮已完成输入和故障后监督器重启，但后台为空，验收失败；检查构建输入确认设置验收前缀只有桌面程序启用回环，IBus 实际加载的 Host API 是正式配置，发送被禁外网空间挡住。保留首轮证据后，复用原 Release 缓存将同源 Host API 明确按 `telemetry-loopback` 重建，复制独立产品前缀，运行前实际 ABI 接受指定回环，运行中确认加载的共享库来自该前缀，再完整重跑通过；原生 IBus 文件本身未改，原正式安装前缀未替换。十二份固定资源从原校验归档取回，逐块、整档及原锁 SHA/大小全部重验；原来源超时与两条分块连接失败保留，不改锁或用合成资源冒充。证据在 `target/pve-editor-runtime-attempt{1,2}.log`、`target/pve-editor-symbols-attempt2.log` 及远端 `target/linux-editor-acceptance-attempt{1,2}/`；Release 缓存已重建为正式 Host API，实际 begin/flush 在创建目录前拒绝测试来源，库内无完整测试 origin；原正式安装前缀的桌面与 Host API 哈希未变，12,444 条输入及模式最终核对一致。范围仍限 Ubuntu x64/IBus/Mousepad，不扩大为其他发行版、Fcitx5 或原生 Wayland 编辑器。

`d955936174` 的 Native Platform CI `40891776963` 已通过 Windows MCP 的 MSVC 严格 Clippy 和全部测试，但之后编译 client-core 测试时，合成合并的新版 `migrate_android_key_layout` 已增加两份九键符号表参数，本分支新增的授权迁移回归仍按旧三参数调用，报 E0061，WER 未执行。整合固定 develop `c1c0c0d781` 的十四份已有提交后，该回归显式传入两个空表，保留原迁移选择和三种授权状态断言；不改出厂默认值。此集成结果的目标编译、迁移回归与整轮 quick 单独核对，旧通过记录不转记为新结果。

整合候选树 `22ddd3af98853b3219332188a76310c9b040925f` 的整轮 `scripts/verify-local.sh --quick` 已退出 0，12,626 条源码及模式在开始和结束后逐项一致。授权迁移回归通过，Windows GNU 的 core/Host/MCP 全 target 严格 Clippy、203 项 Windows 可移植用例、仓库契约及笔记、Rust workspace/Android/wasm、Android Java/JNI、macOS 原生、共享 Apple bridge 与 x64/x86 pipe-only 编译通过。鸿蒙 SDK/设置依赖、Docker 及 Windows 完整原生工具缺失的阶段明确跳过，不计平台验收。证据为 `target/merge-c1c0-quick-evidence/`；随后只补验证笔记，不把既有 6bea 产品或 b753 WER 的通过转记为本次集成产物。

固定 `6bea262fd0` 的 Ubuntu x64 / Fcitx5 5.1.7 已在真实 Mousepad 经原生 GTK 模块完成连续上屏、SIGABRT 后以原命令重启补报、SIGKILL 对照、活跃撤回与重开，每次恢复后继续同一文档输入。两次外部信号的后台帧停在框架事件循环，只证明采集和补报；另用 GDB 在真实 GTK 按键进入 `FcitxEngine::keyEvent` 时投递 SIGABRT，由原 daemon 重启补报并恢复同一编辑器输入，实际后台自有帧经独立两模块符号包还原到 `FcitxEngine.cpp:7677`，错 build-id 拒绝。首轮夹具在插件尚未按需加载时过早读武装状态的失败保留；源码行首轮以字母触发故障，框架在 daemon 死亡后把该字母回退给编辑器，原严格内容断言失败；改用不插入文本的 Right 键触发后完整通过，不放宽上屏断言。隔离后台共四条 crash、四条 session_crash 和三条 active，其中一条故障来自该失败轮，未冒充全部通过轮次。数据库与 HTTP 一致，授权重开的原记录 epoch 不同；正式 Host API 和缓存插件已恢复，正式 ABI 在建目录前拒绝测试来源，原正式安装前缀哈希不变。12,444 条源码及模式相同，四份合成 HOME 经独立 `/proc` 审计无残留；后台、数据库、内部网络与专用转发已回收。证据为 `target/linux-fcitx-editor-product-evidence/`（88 项）。范围不含 systemd 自动重启、原生 Wayland、硬件访问冲突或系统级安装；实际输入使用独立前缀的同源 Release 插件。

PR 在 `b8acf14da7` 推送后，因 develop 已合入另一份 KVM 修补而出现两处冲突，平台 CI 没有启动；原 head 只有标题与标签检查通过，不能当作新平台验收。随后整合固定 develop `de5d51623a`，保留已验证的直接设备权限检查及 `emulator -accel-check`，并接入上游在 `SERIAL` 未设定时跳过 adb 清理和限制清理时长的修补。候选树 `56b8841f3765b02e3994d841eb49fd7d51fb9058` 的整轮 quick 退出 0，12,631 条源码及模式前后相同；授权迁移回归、Windows GNU core/Host/MCP 全 target 严格 Clippy、203 项可移植 Windows、仓库契约与笔记、Rust workspace/Android/wasm、Android Java/JNI、macOS 原生、Apple bridge 和 x64/x86 pipe-only 通过。缺少鸿蒙 SDK、Docker 和 Windows 完整原生工具的阶段明确跳过，不算平台运行通过；新 CI 另验。证据为 `target/merge-de5d-quick-evidence/`，其后只补验证文档。

当前 head `cdb4fe7021` 的 Native Platform CI `40926324997` 已通过 Android API 35 完整设备门禁（job `123634080314`）和 Windows native settings 全部 x64/x86 WER 门禁（job `123634080235`），并通过 Linux/Fedora/HarmonyOS host、Android host、Rust、Web、Windows GNU 及 Apple 相关 job。Android 设备作业实际完成 KVM、共享 crate 设备单测、APK、崩溃/系统退出/ANR 专项和完整输入冒烟；Windows 作业实际完成 MCP、共享存储替换、跨线程学习重置、x64/x86 TSF WER、Win7、设置页和 recorder。此次结果只证明该候选的 CI 路径，不能扩大到真机、完整产品编辑器、ARM64 实际故障或各平台发布后台链路。


2026-10-30 后续准备 macOS 共享设置产品验收时，源码检查确认 `desktop_crash_reporting::directory` 直接按 `HOME` 解析队列，不能沿用原生入口的 `CFFIXED_USER_HOME` 隔离方式。为避免写入真实用户队列，在启动产品之前主动停止本任务编译（退出 143），没有执行故障或发送；原生正式缓存随后重建并重新通过拒绝测试目标的检查。该准备结果不计共享设置验收通过，证据为远端 `target/macos-e746-settings-acceptance/isolation-review.json`，不改写 `HOME` 来绕过隔离要求。

### e746 的 Ubuntu x64 完整暂存产品复验

固定 `e746981d18261e39cf6f0530934985fad6afcf9b`，在 PVE Ubuntu x64 的干净检出复用 `target/release` 与 `target/linux-ibus`；没有替换系统安装或旧正式前缀。完整前端、Host API、Linux 0.14.0 设置组件与原生产品构建后安装到独立暂存前缀。设置组件使用正式打包的 `TAURI_CONFIG` 和 `tauri/custom-protocol`，从产品的 `msime-linux-settings --route=settings` 启动；不以开发 URL 或单独 Tauri 工程验收。十二份资源按当前锁逐份校验大小与 SHA-256，固定 generation 为 `641a853a12de31e99579e3370742c08b182033f35a19cf62e0722df81367975d`。原下载超时保留，后从 macmini 的同锁缓存校验归档传入并再次由真实 ResourceStore 校验，没有修改锁或生成替代词库。

实际 Mousepad 经系统 GTK 模块分别运行在私有 Xvfb/IBus 与关闭 Xwayland、没有 `DISPLAY` 的 Sway/Fcitx5 环境；Sway IPC 确认编辑器是 `xdg_shell`。两个入口均完成五次合成上屏、开启时 SIGABRT、SIGKILL 不误报、活跃撤回后 SIGABRT 不采集、重开新代次后再次采集及原命令重启补报。IBus 使用产品原监督器自动重启；Fcitx5 显式重启 daemon，不冒充 systemd 自动恢复。实际加载的插件与 Host API 来自本轮暂存前缀，源码和模式前后逐项相同；仅设置 XDG 合成目录，没有改写 `HOME`。

另一次原生 Wayland 真实 Right 按键进入 `FcitxEngine::keyEvent` 时，GDB 在参数复制前把 x64 `rdx` 置为 `1`，确认最终实际 `SIGSEGV/SEGV_MAPERR`、访问地址 `0x11`，再由原 daemon 重启补报并恢复同一编辑器输入。后台实际插件帧的 build-id `6341ddc0f9a2b97d43b13cc8a82175831442ce9a`、偏移 `0x1c387c` 经独立符号 ZIP 还原到 `FcitxEngine.cpp:7705`；第一内联位置是 Fcitx 头文件 `inputcontext.h:288`，产品源码位置另行核对，两条重复帧只算一个位置。错误 build-id 被拒绝；外部 SIGABRT 的框架栈只算采集和补报，不能替代此真实内存故障。

实际产品设置启动器在原生 Wayland 窗口另行完成 SIGABRT、原启动器重启补报、SIGKILL、活跃撤回和重开新代次；从实际进程读取原生武装状态确认开关生效。两条 crash 没有输入会话，独立归档还原到 `apps/desktop/src-tauri/src/lib.rs:6499`，错误 build-id 拒绝。IBus 的两条 crash 同样还原到 `ibus_main.cpp:296`。十八模块符号 ZIP 的 SHA-256 为 `3d0c5e3c0ffb7377a208370e9763b3a9a17fa2bf94700eadf828bbd4ba12ddbd`，正式恢复后再次核对暂存产品全部 151 个文件哈希与该 ZIP 均未变化。

固定官方后台镜像与临时 PostgreSQL 在 Docker 内部网络共接收二十一次请求，按事件 ID 幂等后为七条 crash、五条 session_crash、四条 active；数据库与实际 HTTP 的七个字段按缺省空串逐项一致，没有合成输入、目录或授权 epoch。七条 crash 分别来自 Wayland 信号两条、真实内存故障一条、IBus 两条、设置两条。首轮 Wayland 合成目录过长导致 UNIX socket 超限、IBus 夹具误改 `/home/` 路径、设置夹具过早读取 relay 均保留失败记录；缩短 XDG 目录、修正夹具路径和增加有界就绪等待后另建目录完整复跑，没有修改产品或放宽输入断言。最初未带正式设置打包配置的编译结果也没有用于通过轮。EGL 与私有 D-Bus portal 告警保留，不改变渲染后端。

正式缓存已去掉回环特性与测试 origin 重编，CMake 的原 Fcitx5/资源选项恢复；正式 ABI 的 begin/flush 在创建目录前拒绝测试目标，正式两个 Rust 二进制均不含完整测试 origin。合成授权关闭并读回，独立 `/proc` 审计无测试进程残留；后台容器、内部网络、监听与专用 SSH 转发均已清理，没有停止共享 Docker 引擎。小证据为 `target/linux-e746-product-evidence/` 的 126 份文件及 `target/linux-e746-backend-attempt2-evidence/`。范围仍限 Ubuntu x64 的独立暂存前缀，不扩大到其他架构、发行版、系统级安装或其他 systemd 服务。

### e746 的 Windows x64 暂存产品复验

固定 `e746981d18261e39cf6f0530934985fad6afcf9b` 的干净 Windows 检出复用 `target/debug` 与 `target/windows-full/x64`，只为最终 Host API crate 增加调试信息。Server、Watchdog、准备程序、TSF 与 WER helper 从同源构建暂存至专用测试目录，在专用交互账号运行；三个自有 EXE 路径的外网阻断和回环可用性经实际探针确认。真实打包版 Notepad 通过原 TSF 注册、候选显示和合成「你好」提交。非管理员交互注册失败后，同一注册器在具备机器级权限的上下文成功，失败原件保留。

两次 Host API 故障均由系统事件确认 `c0000005`、`msime_host_api.dll+0x3811897`；首次允许列表夹具采用 `/` 路径，修正为系统实际使用的反斜杠后再起新编辑器。第二次故障在 Server 重启时补报为独立 `tsf` 事件。此前瞬时看到 `.crash` 目录为空，不能据此认定 helper 没有写盘：TSF 的 `capture_only` 准备会收集共享原件，实际共享队列及收据随后包含该记录。用独立精确 PDB 可还原 Host API 的 `serde_json::read::SliceRead::peek`，行号仍为 `??:0`，不把它算作产品源码行通过。

首次 TSF 符号门禁发现同 GUID 的 PE age 为 1、PDB age 为 2，拒绝该配对；保留原快照后仅删除本任务 TSF 的链接产物并重新链接。新 GUID `{4EE83849-515C-4DC7-A3E8-AFE66200044D}`、age 1 在 PE/PDB 中一致，四模块独立 ZIP 逐份哈希回读通过，ZIP SHA-256 为 `11bb017058d59d106ce158c26cb8c56f2bd5a8184f747b97194e0dc88e0299ae`。真实 Notepad 的只读进程检查确认 `sequence=2`、`armed=1`、持续同意代次、原构建头和两个自有模块范围。随后受控改变寄存器并转到经反汇编核对的 TSF 写指令，系统 WER 确认为 `MetasequoiaImeTsf.dll+0x7bdcc`，共享队列和后台均取得原 UUID `28579f18-ff03-4f36-8e8a-167f2df9ebd1`，独立归档还原到 `KeyEventSink.cpp:2415`。故障夹具的严格退出码断言仍失败：打包 Notepad 的最终退出码为 `0xff`；实际访问冲突与归属使用独立系统事件、产品收据和后台证明，不将夹具退出码改写为通过。

后续排查曾错误地把 x64 `CONTEXT` 的 `R15` 偏移 240 判断为错误，并改到 200；240 才是正确偏移。该后续尝试产生系统 `unknown` 地址故障，不计自有故障验收，也没有新增产品后台事件。未改产品源码或放宽门禁，原成功 TSF 事件来自使用正确偏移的前一次尝试。

固定后台 `ab391f834cb14baa88c98b1b8a2dbf03ac0da93a` 最终恰有两条 `crash` 和一条 `active`，无 `session_crash`；HTTP 请求与数据库七字段逐项一致，保留原版本、revision、运行 `windows-editor-e746-20261030`、attempt 1、`x86_64` 与组件 `tsf`，授权代次没有外发。Server 停止再经原任务入口启动后发送队列；这是强制测试收尾后的启动，不冒充正常退出或看门狗恢复验收。

正式 Host API 与原生缓存已去掉回环特性和目标重新构建；正式 begin/flush 均以 `invalid request document` 拒绝测试字段，且未创建测试队列。合成授权已关闭；独立审计确认本任务进程、任务计划、防火墙规则、helper 允许列表、TSF CLSID/TIP 及三个状态根注册值恢复空基线，专用账号合成遥测文件清空。系统组件仍加载的旧 DLL 没有强制卸载或结束系统进程。后台容器、内部网络、监听与精确 SSH 转发已清理。小证据为 `target/windows-e746-product-final-evidence/` 的 54 份文件及 `target/windows-e746-backend-attempt1-evidence/`。范围仍限暂存 x64 产品，不替代完整安装器、其他架构、授权负对照或捕获恢复。

当前候选的 Native Platform CI `40998000611` attempt 2 已整作业成功，Windows job `123884843202` 完成 x64/x86 系统 WER、Win7 和原生设置门禁；Android API 35 完成四共享 crate 设备测试、APK、崩溃/系统退出/ANR 专项与完整输入冒烟。CI 通过不替代矩阵里的真机、系统延迟投递或其他产品入口验收。

### Windows 真实编辑器的授权与故障归属对照

固定 `e746981d18261e39cf6f0530934985fad6afcf9b` 的同一份 x64 暂存产品，在专用交互账号实际运行打包版 Notepad。所有通过轮均先经真实虚拟键、候选窗与空格提交合成「你好」，不以直接写入编辑框或 Unicode 注入代替输入。测试偏好显式选择中文初始模式，只改合成配置；产品默认值不变。TSF/PDB 按前述精确哈希核对，采集武装状态直接读取精确 PDB 定位的实际租约快照。

第五轮通过三个场景。其一，同一编辑器在关闭状态启动，依次实际开启、撤回、重开；逐步确认 `armed`，重开取得新 epoch。自然授权回调处制造真实 TSF 写地址 `1` 的访问冲突，恢复断点并脱离调试器后，由系统事件 `1000` 与关联 WER `1001` 确认为 `c0000005`、`MetasequoiaImeTsf.dll+0x7bdcc`。先保存原 `.crash`，再停止本任务 Watchdog/Server 并经原 Server 任务启动，代收恰好一条 crash；原 UUID、构建头与新授权代次一致，公开消息没有 epoch，也没有输入 `session` 或 `session_crash`。该轮验证到共享队列，没有连接后台。

其二，只在本轮拥有的 Notepad 内加载测试处理器，精确匹配同一 TSF 指令、访问地址 `1` 与单次命令，恢复原调用上下文。处理器恰好命中一次、原进程存活；同一编辑器随后再次完成实际按键、候选与空格提交，严格内容为「你好你好」，没有新增原记录、队列 crash 或系统故障事件。此项验证的是测试处理器恢复后的宿主语义，不声称 Notepad 自带该恢复能力。其三，在实际 TSF 已武装的 Notepad 中让 `ntdll!RtlFillMemory` 访问地址 `1`；系统事件与 WER 确认为 `ntdll.dll` 的真实 `c0000005`，没有新增产品原记录或队列 crash。

第六轮独立复验撤回负对照：正确注册同一 WER helper，先确认真实编辑器已武装，活跃撤回后确认同一进程解除武装，再制造同一自有 TSF 访问冲突。系统故障与关联 WER 完成后，没有原始记录或队列 crash。此前使用正斜杠 helper 允许项的撤回轮次不能单独证明撤回有效，以本轮正确注册与第五轮正对照为准。

失败与夹具修正保留：PowerShell 5 对无 BOM 中文脚本解析失败、准备程序拒绝已预建的空状态目录、两轮候选前置失败、中文异常写入遇默认代码页限制。独立诊断显示英文初始模式、IMM 打开状态与实际 TSF 输入不能混同；仅修改合成初始模式也曾失败，不把它认定为唯一根因。按专用账号、进程创建时间、`RestartByRestartManager` 命令及同期自有模块 WER 核实早先故障自动重启的 Notepad 后，只清理该任务进程，独立输入复验通过；不把该结果扩大为原 Server 下的恢复原因已完整定位。后续夹具曾等待三十五秒内代收，而产品刷新间隔为三十分钟，修订为先验原记录、再由实际 Server 重启代收。第四轮还确认 WER 允许列表中的 `/` 路径不匹配实际反斜杠 helper 注册，导致系统故障发生而没有原记录；仅修测试注册与对应清理路径，第五轮正对照才完成。测试没有修改产品二进制、放宽系统故障断言或补造崩溃记录。

独立收尾审计确认九个自有产品文件哈希不变、远端源码仍是干净固定提交；合成授权关闭，产品与本任务记事本进程、计划任务、防火墙规则、遥测文件、TIP/CLSID、状态根注册项及 helper 允许项均恢复空基线。收回的 233 份小证据与夹具逐项核对大小、SHA-256，并在本机复核四场景、系统事件关联、原记录到队列身份、epoch 不外发和清理收据。证据为 `target/windows-editor-negative-e746-small-evidence/`，ZIP SHA-256 为 `30e3f39cc0a7302f4560da29f8c735a9ea3836aee5f49e78e369b7af2178ec96`，独立复核为 `target/windows-editor-negative-e746-local-verification.json`。范围限固定版本 x64 暂存产品及共享队列，不扩展为新候选完整重编、安装包、其他架构、Win7、其余入口或全平台验收。

### Windows x86 实际编辑器与跨架构代收

固定干净 `e746981d18261e39cf6f0530934985fad6afcf9b`，在有 90 GiB 空间的 macmini 复用任务 worktree，以 Rust 1.97.1、`cargo-xwin` 0.23.1、Clang 23.1.1 和官方 Microsoft x86 SDK/CRT 编出真正的 i686 Host API、TSF 及 WER helper。Windows 构建机仅余约 24 GB，没有新建完整 Rust 构建树；x64 Server 复用同一提交已冻结的产品，两个架构使用相同隔离目标。原 TSF CMake 源与既有固定依赖头文件保持原样。正式与测试 Host API、TSF、WER 四组 PE32/PDB 的 GUID、age 与逐文件哈希分别冻结，交叉编译本身不记作运行通过。

在 Windows 专用账号实际运行官方哈希校验的 Notepad++ 8.9.8.1 x86 便携版，两个成功场景均通过真实虚拟键 `NIHAO`、产品候选窗和空格提交合成「你好」，从实际 Scintilla 文档读回严格结果。只读进程模块表确认加载本轮 `bin32` 下的 TSF 和 Host API；SDK 夹具实际回读指针宽度 4、`CONTEXT` 大小 716 与寄存器偏移，没有复用 x64 上下文。学习、云候选、翻译和 AI 等仅在合成偏好中关闭，产品出厂值不变；产品、编辑器及控制器路径均实际验证出站阻断，遥测只走固定回环。

正对照在同一编辑器依次验证武装、活跃撤回解除武装、重开取得新 epoch。调试夹具只在自然授权 ABI 入口短暂设断点，核对实际请求后恢复原指令；随后设置寄存器和指令地址并脱离调试器，执行精确 PDB/反汇编核对的 TSF `mov [eax], ecx`，不可写地址为 `1`。系统事件 `1000` 与关联 WER `1001` 确认 `c0000005`、`MetasequoiaImeTsf.dll+0x11c6`。保留原 `.crash` 后停止本任务 Server/Watchdog，再经实际 Server 任务重启代收，原 UUID `eb9ab07f-33fb-45f2-8409-06fbfa87b49a` 进入共享队列并确认删除原件。这是受控硬件访问冲突，不冒充自然缺陷复现或看门狗恢复。

隔离官方后台固定镜像与 revision `ab391f834cb14baa88c98b1b8a2dbf03ac0da93a`，最终恰有一条 `crash`、一条 `active`。原 HTTP 请求与数据库七字段逐项一致，保留捕获时的 `0.3.0`、e746、运行 `windows-x86-e746-20261030`、attempt 1、`x86` 与 `tsf`，没有被 x64 代收者改写，epoch 不外发，无 `session`/`session_crash` 推断。独立 TSF 符号 ZIP 重新解包核对哈希，PE/PDB GUID `{4739BCEF-9BED-1EB0-4C4C-44205044422E}`、age 1 匹配；显式使用归档 PDB 还原首帧到 `Candidate/EnumTfCandidates.cpp:11`，换用 WER 的 PDB 被身份检查拒绝。

独立撤回负对照使用同一正确的 32 位注册视图和反斜杠 helper 允许项：新的实际编辑器先完成中文输入并武装，活跃撤回后确认同一进程解除武装，再制造同一自有指令访问冲突。系统故障和 WER 完成后，没有产品原记录或队列 crash，后台两行保持完全一致。两个编辑器控制器均恢复原会话输入源并退出 `0`。

失败记录保留：PowerShell 5 的无 BOM 中文解析、生成依赖配置重复导入、CMake/Ninja 的源码依赖别名重复、缺少 `FMT_HEADER_ONLY` 的夹具配置、原生夹具遗漏 COM 头及链接器选择，均在产品运行前失败。冻结源码显式关闭自动重配置后正常构建，没有修改产品源码或忽略链接错误。注册夹具使用正斜杠绝对路径仍得到错误 `126`，仅改夹具依赖搜索及反斜杠路径后实际注册成功；首轮后台验证误把逐事件请求当批量 `events`，在 HTTP 已收件后失败，保留原脚本并以只读核对完成验证，没有重发故障。

初次清理检测到 COM 注销残留的本任务 TIP 根、合成 `telemetry-state.json` 和锁文件，没有将其记为清理成功。按原空基线精确删除自有 GUID，确认队列锁可独占后清理合成状态，独立审计确认双视图状态根、CLSID/TIP、helper、进程、计划任务、防火墙、队列、后台容器和精确 SSH 转发均清理，Windows 源码仍为干净 e746，产品及编辑器二进制哈希不变。macmini 已重建不含回环特性的正式缓存，并在 Windows 实际 PE32 ABI 中验证 `begin`/`flush` 拒绝测试字段且未建队列；重建的 DLL/PDB 与初次正式快照哈希不同，两套均保留，不声称字节可复现，import library 相同。原 worktree HEAD 与任务 lease 已恢复。

Windows 小证据为 `target/windows-x86-e746-small-evidence/`，94 份文件 ZIP SHA-256 `eff078d345b31c159f07d4dedf195dbb5dfc708162a1ce8ec0db71328b2227be`；交叉构建小证据为 `target/windows-x86-native-small-evidence/`，81 份文件 ZIP SHA-256 `508c2b4c755c2f3d76b1cee66ca11285990f3f7b3ddc427180f7e1011a2c4dd2`。该结果仅覆盖固定 e746 的 Windows 11 x86 暂存产品与同源 x64 代收，不扩展为完整安装包、ARM64、Win7、全部入口或全平台验收。

### Android 当前基础产品的待送达致命信号漏报

`e746981d18` 的 API 28 arm64 正式原生 APK 已重建，剥离后的四模块与 Gradle 实际输出一致，自有库 build-id 与独立未剥离符号归档一致；正式 APK SHA-256 为 `1df42a255c48e44059d85b80f3545c11ce1c4e738bcf627be6f20ad661945e2f`。新建专用 AVD 在首次启动前按应用 UID 禁网，实际 App/IME 的动态授权与故障落盘专项通过。但原始 JNI 真实访问冲突没有记录，不能把之前的 CI 或 SIGABRT 用例算作这个路径通过。

诊断确认前任返回时记录已存在，却仍有待送达的默认 SIGSEGV；修复与独立失败/通过回归见[待送达致命信号](../../implemented/bug-fix/2026-10-30-android-pending-fatal-signal.md)。仅更新经过固定哈希核对的共同 C++ 源后，实际 IME 和 App 两轮隔离后台分别收到五条与三条事件，JNI 帧通过精确独立 ZIP 还原到 `client_jni.cpp:108/117`。这是 e746 基础包加精确修补的产品复验，不是全新提交完整重编或真机验收；本轮夹具失败亦在该笔记保留。

Android 正式缓存已逐字节恢复，合成授权关闭、任务包卸载、AVD 停止，四轮后台与转发清理完成；本机从独立 ZIP 还原八个后台帧一致，完整 quick 与笔记校验通过。原 App 夹具的清理布尔值曾与真实恢复值不符，已保留原件并以独立审计纠正，详见上述修复笔记。

本轮 Windows x86 验收记录的完整 `scripts/verify-local.sh --quick` 首轮退出 1，仅 Linux 原生 119 项中的两项画布精确颜色断言失败。使用本次命令专属 `FONTCONFIG_FILE` 关闭字体抗锯齿后，两项原断言与整轮 quick 复跑均通过，Linux 原生 119/119；没有修改系统字体设置、产品或测试源码。笔记校验退出 0，12,666 个 tracked 条目的实际字节和模式在远端验证前后相同，并与本机 `78c7eb1f7a` 全部字节及执行位逐项一致。证据为 `target/windows-x86-quick-evidence/`。此 Linux 环境未配置的 Windows 完整原生/pipe-only、Android/wasm SDK、鸿蒙 SDK、macOS 原生与 Apple bridge 阶段明确跳过，不算对应平台通过；首轮失败日志保留。
