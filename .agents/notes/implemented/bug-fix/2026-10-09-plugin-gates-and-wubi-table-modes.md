# Agent Note: 插件「选了却不生效」：门控开关、五笔的表模式与诊断出口

Status: implemented

## Problem

用户反馈「插件的功能完全没作用」。逐个平台、逐个插件类型追了一遍，没有一处共享代码让所有插件同时失效，失效来自几层叠在一起：

- 选中一个包和打开让它生效的开关是两份偏好。「设为当前音效包」只写 `key_sound.pack`，`key_sound.enabled` 默认关，host-api 在它关着时一个音都不排；音乐包同理看 `music.enabled`。指令表只写 `plugins.command_tables`，而 host-api 只在 `local_modes.command`（默认关）打开时才读指令表；`@` 名单同理看 `local_modes.mention`。插件页却只按「选中」标「使用中 / 已启用」。
- Engine 只在 `SchemeType::opens_local_modes()`（全拼、双拼）下打开 K、`/`、`@`，五笔用户启用的短语表和指令表永远没有入口，界面和文档都不说。
- Windows 从开始菜单打开的原生设置窗口按自己的规则找状态目录，与 Server 用的安装器 `DataDir` 不一致，它写的 `local_modes.*` 到不了输入法。
- 鸿蒙「背单词」的请求缺 host-api 必填的 `resources`，整个功能报错，单词本插件出不来；鸿蒙键盘扩展与设置应用分属两个沙箱，见 [鸿蒙共享沙箱提案](../../proposed/architecture/2026-10-09-harmony-ime-shared-sandbox.md)。
- Linux 托盘和 macOS、Windows 原生设置里的辅助码方案菜单不知道辅助码插件已经替换了方案。
- 声音和插件表的运行期失败只写 stderr，macOS 输入法进程的 stderr 无人接收，排查时什么都看不到。

## Decision

- 选中即使用：共享设置界面 `withPackSelected` 选按键音效包时同时打开 `key_sound.enabled` 并切到按键模式，选旋律包时切到旋律模式并打开，选音乐包时打开 `music.enabled`。`packMarker` 只在门控开关也开着时才标「使用中 / 已启用」，否则标出是哪个开关没开；短语表和指令表还要看当前方案能否打开 K 或 `/`（`PluginTableModes.schemeOpens`，即 `schemeOpensTableModes`），打不开时先于开关标「已启用 · 当前方案打不开 …」，`@` 名单那一行同样先说「当前方案打不开 @ 模式」，因为这时打开开关也没用。
- `/` 指令和 `@` 模式不随插件自动打开：打开它们会改变 `/`、`@` 在所有应用里打出来的字。插件详情和 `@` 名单在模式关着时给出说明和打开它的入口，与短语表看 K 模式的做法一致。
- Engine 新增 `SchemeType::opens_table_modes()`（全拼、双拼、五笔），K、`/`、`@` 看它，其余 Shift+字母模式仍看 `opens_local_modes()`。五笔编码只用小写字母，大写字母、`/`、`@` 不会和编码冲突；代价是五笔空闲时 Shift+K 在 K 模式打开时进入短语模式，不再直接打出大写 K。四码打满后（顶字）按下的大写字母同样先上屏首选：K 模式开着时 Shift+K 接着进入短语模式，引擎不收的大写字母（K 模式关着时的 K 和其他大写）由 input-runtime 跟在首选后面原样上屏，与 `literal_mark` 处理暂存词组后的标点相同；原先这个键被丢掉，宿主却被告知已处理；Windows 在 Engine 不留组合时给 TSF 消费数 5，让 TSF 清掉缓冲里的这个大写字母。宿主侧的同名判断一起跟上：input-runtime 的字面标点路由、Linux `scheme::OpensTableModes`（定义在 `InputSchemeTraits.h`，`SpellingSymbols.h` 用它）、Windows Server 下发给 TSF 的 `command_mode` / `mention_mode`（`apply_local_mode_switches` 按 `common/InputSchemeTraits.h` 的 `scheme::OpensTableModes` / `OpensLocalModes` 决定，两者和其他平台的同名副本一样由 `scripts/test-scheme-traits-parity.py` 对照 Engine，不再是 `server_main.cpp` 里手写的方案名比较）。Linux 两个宿主（IBus `ClientEngine.cpp`、Fcitx5 `showCandidateAnnotations`）原先在五笔下按 `wubi_code_hint`、在拼音下按辅助码 `show_in_candidate_window` 整体隐藏候选注释，现在只在 `local_mode` 是 `command` 或 `mention` 时跳过这两道门控：这两个模式里 Engine 填进 `annotation` 的是指令标题或 `@` 地名的省市，不是编码提示，否则五笔关掉剩余编码后 `/`、`@` 候选会丢掉标题；其余本地模式（超级简拼、快捷短语等）的注释仍是辅助码，照旧受这两个开关控制。界面的 `schemeOpensTableModes` 与它保持同一份方案集合。
- Windows 原生设置窗口的状态目录与 Server 走同一个 `msime::windows::resolve_state_directory()`，并优先用 `runtime-options.json` 里的绝对 `preferences_directory`；`scripts/test-windows-state-dir-parity.py` 把它列入必须调用共享查找的调用方。
- host-api 提供 `msime_client_set_diagnostic_sink`，声音播放器、插件表和辅助码表的运行期失败经它送出，没注册时仍写 stderr。每行是「固定类别: 细节」，类别是不含冒号的 `'static` 短语，包名、路径和错误原文都在细节里。macOS 在启动时注册，诊断日志只记 `host_api: <类别>`、日志关着时不留存，保住「只记类别与错误码、不记路径与错误字符串、关着时什么都不记」的既有隐私约定。
- 辅助码方案菜单（Linux IBus / Fcitx5、macOS 与 Windows 原生设置）在辅助码插件生效时显示插件，从菜单选内置方案即清掉该方案的插件选择，与设置页 `helpcode-page.tsx` 的契约一致。
- 鸿蒙「背单词」请求补上 `resources`；`docs/plugins.md` 加平台支持表，写明 Android、iOS 没有插件页，鸿蒙键盘目前读不到设置应用导入的包。

## Alternatives considered

- 选指令表时顺手打开 `local_modes.command`：用户点「启用」的意图最直接。否掉，因为 `/` 默认打出顿号或斜杠，打开模式会改变所有应用里这个键的行为，应该由用户自己决定；给出就地入口已经足够。
- 只让界面诚实、不改 `withPackSelected`：保留「选来源」和「开关」分离的旧契约。否掉，用户点「设为当前」就是要听到它，再去另一页找开关正是这次「完全没作用」的来源。
- 让五笔打开全部本地模式：`opens_local_modes()` 直接加五笔。否掉，U、J、Y 等模式按拼音查，五笔下没有意义。
- 修鸿蒙共享沙箱：根因在那里，但需要在 AppGallery Connect 申请 data-group-id，留在提案里。

## Consequences

- 收益：桌面三平台上选了包就有声，`/`、`@`、K 在模式关着或方案不支持时界面会直说；五笔用户第一次能用短语表和指令表；运行期失败的类别在 macOS 诊断日志里看得到。
- 代价：五笔空闲时 Shift+K 的行为变了（K 模式默认开）；`withPackSelected` 不再只是选择，测试和文档按新契约更新。
- 未覆盖：iOS、Android 键盘工具栏仍只在拼音方案里提供 K、`/`、`@` 入口；鸿蒙共享沙箱未做；Linux 与 Windows 没有注册诊断出口，仍写 stderr。macOS 日志不含包名与错误原文，会话创建时只报一次的失败（辅助码表包回退、整句模型读不了）在日志打开前发生就看不到。

## Verification

`cargo test -p msime-engine`、`cargo test -p msime-input-runtime`、`cargo test -p msime-host-api` 与对应 clippy；`apps/desktop` 的 `tsc --noEmit` 与 `vitest run tests/settings/plugins-section.test.tsx`；Linux `platforms/linux/build-container.sh`（ctest）；macOS `macos-diagnostic-log` 与 `shortcut` ctest；鸿蒙 `keyboard-logic` 测试与 `scripts/test-harmony-*.py`；`scripts/test-windows-state-dir-parity.py`；`scripts/test-scheme-traits-parity.py`；Windows 的 `windows-tsf-config-frames` 与 `windows-input-scheme-traits` 两个可移植测试在 macOS 上用 `c++ -std=c++20 -DMSIME_EDITION_FULL` 直接编译运行。其余 Windows 原生代码（含 `server_main.cpp`）在本机无法编译，交给 Windows CI。
