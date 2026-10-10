# Agent Note: Windows 剪贴板采集跳过敏感内容、面板上屏计入打字统计、host-api 诊断出口

Status: implemented

## Problem

三处 Windows 与 macOS 行为不一致，都属于「数据去了不该去的地方，或该去的地方没去」：

- 剪贴板历史：macOS 按 nspasteboard.org 的 Concealed/Transient 标记和 1Password 私有类型拒收样本（`BackendClipboardCapture.swift` 的 `excludedTypes`，Tauri 侧 `crates/host-macos/native/clipboard.mm` 有同一份名单）。Windows 的三个采集点——Server 的 `ClipboardMonitor`、设置应用的 `start_clipboard_monitor`、剪贴板面板「同步」按钮用的 `read_clipboard_text`——只看有没有 `CF_UNICODETEXT`，KeePass、1Password、Bitwarden 复制的密码会原样写进剪贴板历史，开着云同步时还会上传。
- 打字统计：macOS 共享面板（表情/符号、手写、剪贴板）提交的文字在 `commitPendingEmojiForClient` 里按 `TypingSource::Local` 记，原生手写按 `Handwriting` 记。Windows 的 `send_text`、`submit_handwriting_candidate`、`paste_clipboard_text` 注入成功后不记。与此同时，水杉 tip 在目标编辑器里时会把 `SendInput` 注入的 `VK_PACKET` 字符当作直通按键记进 `PassthroughStatisticsQueue`，所以已有的语音面板（`send_voice_text` 自己按 `Voice` 记）在这种情况下其实记了两次。
- 诊断：host-api 把音效包、音乐包、辅助码包载入失败和音频设备打不开这类自行恢复的失败经 `msime_client_set_diagnostic_sink` 送出，没登记出口时写 stderr。macOS 登记了，Windows Server 没有；Server 在 Watchdog 下运行，stderr 没人读，`logs\server.log` 里看不到这些失败，`read_diagnostic_log` 也就查不到。

## Decision

### 剪贴板隐私标记

判定是纯函数，两份实现、同一份名单：Server 侧 `platforms/windows/src/clipboard/ClipboardPrivacyPolicy.h`，设置应用侧 `crates/host-windows/src/clipboard_privacy.rs`。剪贴板上出现以下任一情况，这次样本整条丢弃：

- 注册格式 `ExcludeClipboardContentFromMonitorProcessing` 存在；
- 注册格式 `Clipboard Viewer Ignore` 存在；
- `CanIncludeInClipboardHistory` 存在且 DWORD 为 0；
- `CanUploadToCloudClipboard` 存在且 DWORD 为 0。

许可格式存在但读不到数据或不足 4 字节时按拒绝处理：写下这个格式的程序显然在表态，宁可漏记一条也不记下可能的密码。

- `ClipboardMonitor.cpp` 在已打开剪贴板的作用域里先读标记，命中就不取文本；`sequence_` 照样前移，这次变化不会被重读。
- `read_clipboard_text` 的返回改为 `Result<Option<String>, ()>`，`Ok(None)` 表示带隐私标记、不可采集。设置应用的剪贴板监视线程只收 `Ok(Some)`；`sync_clipboard_history_blocking` 把 `None` 当作「剪贴板上没有可采集的内容」，照常返回当前历史，与 macOS 的 `clipboard_snapshot` 返回 `text: None` 一致。

### 面板上屏计数与注入标记

- `send_text`（表情/符号、云剪贴板、语音面板的普通发送）注入成功后按 `TypingSource::Local` 记，与 macOS 面板提交一致；`submit_handwriting_candidate` 按 `Handwriting` 记，与 Linux 和 macOS 原生手写一致；`paste_clipboard_text` 粘贴成功后按 `Local` 记。写统计在 `spawn_blocking` 里完成（`panel_input::record_windows_panel_typing_statistics`），因为统计文件要拿跨进程的文件锁。
- `crates/host-windows` 的 `unicode_input` 给每个 `KEYEVENTF_UNICODE` 按键带 `dwExtraInfo = 0x4D535053`（`send_input_marker.rs`）。tip 的 `_NotePassthroughStatistics` 先用 `GetMessageExtraInfo()` 比对 `PassthroughStatistics.h` 的 `PanelTextSendInputExtraInfo`，相同就不进直通统计。按键怎么处理不变：这个值刻意不放进 `IsSelfGeneratedSendInputExtraInfo`，那个集合会让 tip 整键放行。
- 粘贴走 Ctrl+V，tip 的直通统计本来就排除 Ctrl 组合，不需要标记。

### host-api 诊断出口

Server 在创建 `DiagnosticLog` 之后、读取状态用的那次 `msime_client_prepare_host` 和首个会话之前登记 `write_host_api_diagnostic`；托管启动首次运行时准备状态的那次 `msime_client_prepare_host`（`prepare_first_run`）更早，那时日志开关还没按偏好打开，即使登记了出口也会丢弃。日志开关在第二次 `prepare_host` 返回偏好后、任何会话创建之前打开，所以会话创建时只报一次的辅助码回退进得了日志。出口只写 `host_api: <category>`（`platforms/windows/src/system/HostApiDiagnosticLine.h`：冒号前的固定类别，没有冒号写 `uncategorized`，整条不超过 192 字节），日志的 server 开关关着时丢弃，空指针整条丢弃，与 macOS `msime_macos_diagnostic_host_line` 同一规则。

host-api 要求登记过的出口在进程余下的时间里一直可调用，音频线程上的报告可能晚于 `main` 返回，所以 Server 的 `DiagnosticLog` 改为分配后不析构，出口经一个只赋值一次的原子指针找到它。

## Alternatives considered

- **只在 Server 的 `ClipboardMonitor` 检查** — Server 是主采集点，改一处最省事；但设置应用自己也有一个监视线程，「同步」按钮也直接读剪贴板，任何一处漏掉，密码照样进历史，所以三处都查。
- **两侧共用一份 C 头文件里的名单** — 只有一份事实来源；但 `crates/host-windows` 不经 bindgen 也不编 C，引一个头文件只为四个字符串不划算。改为各写一份，由 `crates/host-windows/tests/clipboard_privacy_policy.rs` 用 `include_str!` 对照 C++ 头文件里的字面量，名单不一致时测试失败，与 macOS 两份名单靠注释约定相比多了机械检查。
- **tip 对所有 `VK_PACKET` 都不计直通统计** — 不需要新标记；但 Windows 触摸键盘也用 `VK_PACKET` 送字符，那是用户真实的输入，一刀切会把它们漏掉。
- **把面板标记加进 `IsSelfGeneratedSendInputExtraInfo`** — 一个集合管所有自注入；但那会让 tip 对面板文字整键放行，中文模式下面板送来的标点、字母的处理方式随之改变，超出「不重复计数」这一件事。
- **把 `DiagnosticLog` 留在 `main` 栈上，退出前把原子指针清空** — 不留不析构的对象；但清空和另一线程上已经读到指针的报告之间有竞争，读到的对象可能正在析构。日志对象只有路径、互斥量和两个原子量，不析构没有可见代价。
- **tip 也登记诊断出口** — tip 在应用进程里创建会话，辅助码回退会在那里报告；但 tip 是可卸载的 DLL，没有自己的日志文件，只能把行经管道交给 Server，而出口规定不得阻塞。这次不做，tip 进程的这些报告仍写 stderr。

## Consequences

- **收益**：Windows 的剪贴板历史不再收下密码管理器标记过的内容；设置应用面板的上屏计入打字统计，语音面板在水杉 tip 激活时的重复计数也随之消失；`server.log` 能看到 host-api 自行恢复的失败的类别。
- **代价与已知上限**：只认这四个 Windows 约定格式；不按约定标记的密码管理器（或只放 `CF_UNICODETEXT` 的自动输入工具）仍会被采集，macOS 那几个 nspasteboard 私有类型在 Windows 上没有对应物。面板文字被中文模式的 tip 吃进组字（例如面板送出 ASCII 字母）时，组字上屏会再经提交路径计一次，这种用法罕见，没有处理。`GetMessageExtraInfo()` 在 `OnTestKeyDown` 里可用是沿用既有自注入标记的前提，未在真机上单独验证面板标记。tip 进程内的 host-api 报告仍写 stderr。

## Verification

- `platforms/windows/tests/clipboard/clipboard_privacy.cpp`（`windows-clipboard-privacy`）：格式名、DWORD 判定、每种标记都会排除样本。
- `crates/host-windows/tests/clipboard_privacy_policy.rs`：Rust 侧判定，并对照 `ClipboardPrivacyPolicy.h` 的格式名。
- `crates/host-windows/tests/send_input_marker.rs`：面板标记与 `PassthroughStatistics.h` 一致，且不在 `MetasequoiaIME.h` 的自注入标记里。
- `platforms/windows/tsf/tests/passthrough_statistics.cpp`：`IsPanelTextSendInput` 只认面板标记。
- `platforms/windows/tests/runtime/host_api_diagnostic_line.cpp`（`windows-host-api-diagnostic-line`）：只留类别、无冒号归为 `uncategorized`、长度上限不泄露细节。
- 相关笔记：host-api 诊断出口的来历见 [插件门控如实显示、宿主侧五笔表模式与插件失败的诊断出口](../bug-fix/2026-10-09-plugin-gates-and-wubi-table-modes.md)，macOS 首次会话前配置日志见 [macOS 首次会话诊断日志时序](../bug-fix/2026-10-10-macos-diagnostic-startup.md)。
