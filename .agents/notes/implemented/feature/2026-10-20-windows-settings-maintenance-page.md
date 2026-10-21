# Agent Note: Windows 设置「维护与诊断」页对齐 macOS 并补上重启输入法服务

Status: implemented

## Problem

macOS 的「维护与诊断」页（共享的 `packages/ui/src/settings/pages/developer-page.tsx`）分四组：「输入法服务」、「诊断日志」、「数据目录」和只在 macOS 上有的「卸载」。Windows 原生设置窗口的这一页只有两组：「诊断」里挤着两个日志开关和一行「数据目录」（路径和搬迁说明塞在副标题里），另一组是「设置文件」。缺的两样是实打实的功能：没有重启输入法服务的入口，也没有打开日志目录的那一行；共享应用 `MSIME.exe` 在 Windows 上其实早就能重启（`apps/desktop/src-tauri/src/lib.rs` 的 `restart_input_method`，`crates/host-api/src/tests.rs` 断言 Windows 的 `restart_input_method` 为真），只是原生设置窗口没有接。

## Decision

- 页面依次画「输入法服务」「诊断日志」「数据目录」「设置文件」。前三组的标题、说明和按钮文字照共享页的 Windows 一支（`input-method-service-section.tsx` 非 macOS、非 Linux 的一支，`diagnostic-logs-section.tsx` 的 Windows 一支）；「设置文件」是 Windows 独有的，留在最后不动。
- 「立即重启输入法服务 / 重启」在工作线程上用 TSF 已有的 `tsf/IPC/AuxNamedpipeSender.h`，往 `shared/contracts/windows_ipc.h` 的 `FANY_IME_AUX_NAMED_PIPE`（带本版本后缀）写 UTF-16LE 的 `RestartServer`，不带结尾的 0、不等回答，与 Tauri 外壳 `windows_restart_payload` 的字节完全相同。Server 的 `AuxListener` 只接受本用户完整桌面进程发来的这条消息，收到后以 `watchdog::restart_exit_code` 退出，由看门狗重新拉起。结果照共享页 HostActionButton 的措辞写在页面顶上：「已发送重启请求。」或「重启输入法服务失败，请稍后重试。」；请求在途时再点不重复发送。
- 「诊断日志」是「Server 端日志」开关、「日志文件 / 打开日志目录」、「TSF 端日志」开关，顺序与共享页相同。「打开日志目录」打开数据目录下的 `logs`（`src/system/DiagnosticLog.h` 写 `logs\server.log`），还没写过日志、目录不存在时打开数据目录本身，与共享页「还没有写入时打开它所在的目录」一样。
- 「数据目录」是一句组注加「当前目录」一行：路径照共享页的 `<code>` 用 Consolas、可选中，按钮只有「打开」。共享页在 macOS 和 Linux 上的「选择位置…」不做，搬迁办法（重新运行完整安装包，在「选择数据位置」一步选空文件夹）写进组注，理由见 [Windows 设置文件与本机数据对齐](2026-10-10-windows-settings-file-and-local-data-parity.md)。共享页的「（默认）」标记也不画。macOS 的标记是拿当前目录和 Tauri 的 `app_data_dir` 比较路径（`apps/desktop/src-tauri/src/lib.rs` 的 `is_default`），只有一个对照物；Windows 上有两个互不相同的「默认」：安装器在「选择数据位置」一步建议的是 `%LOCALAPPDATA%\metasequoiaime-<版本>`（`msime_setup.iss` 的 `ResolvePreviousDataDir`，目录名是 `editions.iss` 的 `MyEditionInstallDir`），而 `common/StateDirectory.h` 在既没有环境变量、HKLM 也没有登记 `DataDir` 时退回的是 `%LOCALAPPDATA%\MSIME-Client`（`MSIME_EDITION_STATE_DIRECTORY`）。完整安装包总会写 `DataDir`（`[Registry]` 一节无条件写入），所以装好的机器上生效的是前者，可 C++ 侧没有 `MyEditionInstallDir` 的常量；拿后者比，装好的机器上永远不会出现标记，拿前者比又要在版本表里再抄一份安装器的目录名并加一致性检查。这个窗口不能搬迁目录，标记只是提示，不值得为它多一份要对齐的名字。
- macOS 的「卸载」一组不画，Windows 从系统的「应用」设置卸载，卸载时保留数据与否由安装器问。

## Alternatives considered

- **在 host-api 加一个 C ABI 导出 `msime_client_restart_input_method`，让 Rust 去写管道** — 管道名和消息由 Rust 侧 `quiesce::server::pipe_name` 统一给出，原生窗口与 Tauri 外壳共用同一份实现。但这要改 C ABI 和头文件，而重启的契约只有一个固定的 UTF-16LE 字符串和一个已经在 C++ 头文件里的管道名；TSF 也一直用 `AuxNamedpipeSender.h` 往同一根管道写消息。直接复用这两个现成的头文件，不动 ABI。
- **等 Server 回「OK」再报成功** — 能区分 Server 收到与否。但 Server 对 `RestartServer` 不回答（只有词库维护和终端停用会回「OK」），Tauri 外壳也是写完即算成功；等回答只会在 150 ms 后一律报失败。
- **「数据目录」照共享页给「选择位置…」** — 体验与 macOS 一致；否决理由与原笔记相同：状态根登记在 HKLM 的 `DataDir`，改它要管理员权限和停 Server、看门狗，安装器已经做对了这一套。

## Consequences

- **收益**：Windows 用户在原生设置窗口里就能重启输入法服务和打开日志目录，排查问题时不必去任务管理器或资源管理器里找；页面结构与 macOS 一一对应，帮助与反馈里「诊断日志」那一行跳过来能看到同名的组。
- **代价与已知上限**：重启只说请求发出去了，不保证 Server 真的重新起来（看门狗没在运行时 Server 就停在那里）；Server 没在运行时管道连不上，报「重启输入法服务失败」，此时应该重新登录或重新安装，而不是再点一次。`windows_ipc.h` 和 `AuxNamedpipeSender.h` 从此也是设置窗口的依赖，改管道名或这条消息的格式时要连同设置窗口一起看。

## Verification

- `x86_64-w64-mingw32-g++ -std=c++20 -fsyntax-only` 单独编译一个只含 `windows_ipc.h`、`AuxNamedpipeSender.h` 和重启、打开日志目录两段写法的翻译单元通过。`main.cpp` 依赖 WinUI 头文件，本机编不了，由 PR 的 Windows CI 编译。
- `platforms/windows/tests/tools/settings_ui.ps1` 打开「维护与诊断」页时核对四组和「立即重启输入法服务」「日志文件」「当前目录」各行都在；不点「重启」，免得重启验收机器上真正的 Server。重启本身没有在真机上点过。
