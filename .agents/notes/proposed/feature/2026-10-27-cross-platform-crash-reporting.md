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
| Windows | Server、现代/Win7 设置与面板、MCP、看门狗、准备程序；TSF 租约与进程外 helper 接线、x86/x64 实际 DLL 生命周期及合成宿主七组真实系统 WER 投递；Arm64X helper 链接；callback/lifecycle 门禁通过；候选 `1ae2cf6c67` 已经 PVE 跳转完成 MSVC x64/x86 新计数与最终授权屏障、反复装卸及各七组系统 WER；目录变化通知与三十秒兜底，x64/x86 专项及旧轮询负对照通过 | 完整 TSF 产品与真实编辑器验收；ARM64 实际故障；完整 Win7 同源符号包；设备到后台链路 |
| macOS | 输入法、Tauri 设置/面板、MCP、本地语音 helper；原生故障；实际 universal DMG 五模块双架构符号、UUID、哈希、源码行表和签名核对；当前分支 Intel 登录会话的真实 IMK 编辑器验收（workflow run `40760896167`，双布局通过）；候选 `1ae2cf6c67` 的 arm64 输入进程实际故障、原产品重启、后台入库及 `input_method_main.mm:214` 还原 | 回退设置、Tauri 设置及其他辅助入口后台与源码行；硬件访问冲突与完整编辑器验收 |
| Linux | IBus/Fcitx5、设置/面板、MCP、准备程序、12 个原生辅助程序、八个 Python 入口及启动归属核对；aarch64 IBus/Fcitx5 真实故障、原生发送到隔离后台与源码行还原 | 完整设置/输入宿主的辅助记录代收与发送；其余架构与部分发行版符号 |
| Android | App Java、IME Java/JNI 原生信号、API 30+ 系统退出及持久同意 epoch；API 28/35 专项故障/补报、API 35 实际系统退出和撤回重开；API 28/35 活跃 IME/App 动态同意；API 35 App 真实 ANR、补录、去重和撤回抑制；同源 arm64 Release 包身份核对，API 28 App/IME Java/JNI 真实故障到隔离后台及精确源码行还原；API 35 两个日语原用例和原样触控板专项单独通过；API 35 device workflow 先用 `smoke.sh --crash-only` 独立运行 `telemetry-watch.py`、`telemetry-consent.py` 和 `telemetry-anr.py`，再跑完整输入冒烟 | 旧完整输入套件语音润色失败未关闭；候选 `6bc13ef19d` 的共享设备单测、APK 构建与独立崩溃步骤已通过，完整输入套件仍在运行；真机与其他架构、最新候选后台传输仍缺 |
| iOS | App/键盘异常与 C 信号记录、独立活跃同意观察、当前捕获身份、MetricKit 持久授权与队列代次认领、UUID 帧；同源模拟器回归；iOS CI 现用 `iphonesimulator` SDK 做 arm64 目标编译，并运行真实 C 信号故障夹具和完整 MetricKit 栈解析器，核对故障首帧、Mach-O UUID 与历史模块身份；候选 `a03cced2a5` 的原生 Release App 模拟器 `SIGSEGV`/`SIGABRT`、重启补报、隔离后台入库、动态撤回重开及 SIGKILL 负对照 | 键盘扩展实际故障；系统 MetricKit 回调；真机 dSYM 与后台链路；App 当前仅还原到生成入口 `MetasequoiaImeApp.swift:0`，非零源码行仍缺 |
| HarmonyOS | App/键盘 APP_CRASH、APP_FREEZE 监听，共享只读权威授权与独立观察，系统自定义参数固定原 UUID/构建头，ELF 帧身份；逻辑、适配器回归、同源 arm64 原生库/HAP 与两模块符号身份核对 | API 21 本进程兼容入口实际致命故障（原生与主线程 ArkTS 已接入，三架构严格目标编译与四组实际适配器故障回归通过）；原身份实际系统投递；真实设备系统回调与后台精确符号化 |

所有平台继续保留“栈展开尽力而为、备用栈仅安装线程、后加载模块快照要重新准备”的边界。TSF 进程外 WER 适配遵循 Microsoft 的[注册接口](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werregisterruntimeexceptionmodule)、[不认领异常的回调语义](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nc-werapi-pfn_wer_runtime_exception_event)和[注销要求](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werunregisterruntimeexceptionmodule)。[TSF WER 提案](2026-10-28-windows-tsf-wer-capture.md)持有完整方案与分级验证：产品生命周期、安装器和符号已接线，x86/x64 的实际 DLL 装卸、授权变化、阻塞读取、迟到结果和生产 helper 落盘联动通过，负对照能识别短暂的旧租约武装；ARM64 目标编译与 Arm64X helper 两阶段链接通过。x86/x64 合成外部宿主的七组真实系统 WER 投递现已通过，覆盖 TIP/Host API 自有故障、外部故障、撤回重开、释放卸载和被宿主捕获的异常；完整产品包、真实编辑器与后台仍未验收，不能据此认定 TSF 闭环完成。

[共享偏好的持续同意存储](../../implemented/bug-fix/2026-10-28-persisted-reporting-consent.md)提供原子代次；[采集与队列绑定](../../implemented/bug-fix/2026-10-28-epoch-bound-crash-collection.md)把 iOS 采集、旧原生记录过滤、MetricKit 补录与逐条发送接入同一代次。它不代替实际致命故障、系统 MetricKit 投递与后台精确符号化验收。iOS 与共用 `CaptureOnly` 辅助采集器已迁入共享队列代次协议；辅助采集的 macOS 与 Linux 共用真实子进程回归通过；Linux 产品入口的授权目录夹具修复已由 Debian/Fedora CI 全量重建与故障套件复验通过，Windows 目标运行仍待验证，MCP 与桌面设置发送端已接入共享协议，MCP 与桌面 macOS 隔离专项及实际 Linux crate 全 target 严格 Clippy 通过，Linux 普通用户运行 MCP 126 项和桌面两项崩溃专项已通过；macOS、Windows 与 Linux 的输入会话宿主及同目录发送端已共同迁入，并增加无绑定入口保护；macOS 与共用本地回归已覆盖撤回、旧原生记录晚写和在途发送边界，Windows/Linux 产品运行与整合后的完整门禁仍须单独核验，不能由共享接口可用推定全平台均已完成验收。Android 现已将 App、IME、Java 同意镜像、发送和系统退出补录迁入共享协议；Rust 回归、宿主检查和 API 28/35 的授权镜像专项已通过，同源原生 Release APK `8a006d7ed7` 已构建，API 28/35 的 Rust 共享只读授权应用 UID 专项均通过；该源码的 API 35 真实系统退出、App ANR 补录/去重及撤回重开拒绝旧历史已通过，API 28/35 活跃 App/IME 观察器复验均通过；最新源码的完整产品与完整冒烟仍待结果。鸿蒙已接入本沙箱 NAPI 共享代次，严格拒绝读失败时退回旧布尔协议，迟到清理与会话结束已有适配器回归；完整逻辑套件及复用缓存原生库的 ArkTS/HAP 编译通过。[共享只读权威授权](../../implemented/architecture/2026-10-28-harmony-reporting-authority.md)现已统一 App/键盘授权历史，并接入不依赖焦点或网络完成的独立观察器。九项核心与十二项 Host API 专项、严格 Clippy、完整鸿蒙逻辑套件及十一组适配器回归通过，真实 HAP 编译打包通过；[系统原身份传递](../../implemented/feature/2026-10-28-harmony-system-capture-identity.md)现已用 HiAppEvent 自定义参数固定故障前 UUID、构建头、PID 和准备时间，29 项策略断言、四组异步适配器回归及真实 HAP 编译通过；API 21 原生记录器与主线程默认错误处理器已接入，三架构目标严格编译、四组实际适配器致命故障回归与实际 ArkTS/HAP 编译通过；本地旧缓存缺少四个 ABI，严格链接已拒绝；后续 `d91b0b9242` 已同源重编 arm64 Rust/NAPI 并打包 HAP，两个模块的包内 build-id 与归档一致，授权入口按精确身份还原到源码行。设备入口仍未运行验收。原身份的实际系统投递、鸿蒙真机共享目录、真实系统回调和后台链路仍未完成。

## 隔离发送与共享后台验收

`client-services::telemetry::backend_sender` 统一 Host API 两条发送分支与桌面独立发送器。显式 `telemetry-loopback` 特性和编译时 `MSIME_TEST_TELEMETRY_ORIGIN` 共同限定验收目标；未启用特性时保持正式发送器。工具与边界见 `tools/crash/README.md`。当前 C ABI 的测试握手在访问队列前校验，错误库或错误目标不能先建目录再失败。工具还先用两个不联网的 `begin` 核对字段确实被解析；忽略未知字段的合成旧 ABI 已验证会在任何 `flush` 前被拒绝，不能用可能联网的 `flush` 探测旧库能力。

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

实际后台自有帧 UUID `034f751d09483bceb6df057f22d20ea7`、偏移 `0x44bcd4` 与本轮 App/dSYM 匹配，`atos` 仅还原到 `main (MetasequoiaImeApp.swift:0)`。这证明构建身份、模块偏移和函数归属，**非零源码行验收仍未通过**；系统 MetricKit 实际投递、键盘扩展故障、真机签名与后台仍未验收。证据为 `target/ios-product-local-evidence/`，远端进程、记录与核对材料在 `target/ios-product-loopback-a03/linked-acceptance-attempt2/`。

准备失败全部保留：模拟器缺少 `sandbox_init` 所需的库，外层 `sandbox-exec` 没有约束被系统启动的子进程，动态注入与额外调试签名的部分启动未获确认或被拒绝，外部标准流路径出现 `Launchd job spawn failed`，最终采用包内必须依赖、App Group 内审计路径及逐进程映射检查。不能将这些失败算通过，也不能仅凭后续成功认定它们各自的唯一原因。首次显式链接专项在发送信号后没有记录而失败，没有建立可单独核验的就绪证据，根因未确认；保留原目录，后续完整重跑退出 0，不放宽故障必须留记录的断言。

验收后已经重新编译无回环特性的 iOS Host API 静态库与原生 App，真实模拟器 ABI 调用拒绝 `test_origin` 且没有创建握手目录；App、键盘扩展均不再依赖护栏，包内护栏文件已删除。正式静态库 SHA-256 为 `c37fab70a07eaab32577a69b6671fb4084073b38b27c1bc3bbe3230a809e88ab`，正式 App 二进制为 `0d0b13471b7795497ef6de9cd93e91eee8ca7ed18c31d937324c4250c16085df`；恢复前后的 12,444 个源码条目相同。产品进程、任务模拟器、临时后台与数据库、内部网络和专用 SSH 转发均已清理，未改动其他模拟器。

### PVE x64 Linux 与 Android 整轮补验

`c713178d8e` 的 x64 Linux 优化构建已完成八个 Python 与十二个原生辅助入口的实际信号/异常、关闭对照、共享 ABI 代收、隔离官方后台与数据库核对，二十条故障保留原身份，没有输入会话；十二份原生实际后台返回帧经独立符号包还原到非零源码行。X11 监听器使用独立 Xvfb；代收/发送由验收程序调用真实 ABI，完整设置宿主的代收重启仍须补验。HTTP worker 的普通队列采集额外中位数约 3–4 ms，64 条、约 908 KB 队列约 6–7 ms；在线真实入口开启/关闭各二十四次均完成 100 ms 总预算。本轮不削弱短命采集或重写传输。恢复正式 Host API 后，九十七项原生测试全部通过，正式库拒绝回环测试握手；12,444 个源码条目前后一致。配置不匹配的首次原生代理测试失败与后台验收程序排序切片错误均保留，原因、范围、符号与清理见[宿主采集笔记](../../implemented/feature/2026-10-27-native-crash-capture-and-symbol-identity.md)。

Native Platform CI `40853458086` 的 `95cc68519e` Android API 35 作业于 `2026-10-29T11:05:39Z` 完整成功：四个共享 crate 的设备单测、实际 APK、崩溃/系统退出/ANR 专项及整套输入冒烟全部通过，语音回复与自动插入用例亦实际通过；本轮通过不证明此前 `voice polish replies (AssertionError)` 的根因已经修复。该运行整体验收仍为失败，因为 Windows x86 系统 WER 的目标退出超时；它已留完整记录，新增诊断的 PVE 两架构专项与完整 quick 通过，仍待新 CI。不能将 Android 整作业通过写成全仓 CI 绿，也不能将旧提交结果写成尚未运行的新 head 验收。

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
