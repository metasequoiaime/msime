# Agent Note: 插件门控如实显示、宿主侧五笔表模式与插件失败的诊断出口

Status: implemented

## Problem

用户反馈「插件的功能完全没作用」。逐个平台、逐个插件类型追下来，没有一处共享代码让所有插件同时失效，是几层叠在一起。其中几层已经各自修好：选包同步打开播放见 [插件选择同步打开播放开关](2026-10-10-plugin-selection-playback-gates.md)，Engine 给五笔开放 K、`/`、`@` 见 [五笔开放本地表模式入口](2026-10-10-wubi-table-local-modes.md)，顶字后的大写字母见 [五笔顶字保留后续按键](2026-10-10-wubi-top-commit-tail.md)。剩下的是：

- 插件页只按「选中」标「使用中 / 已启用」。指令表只在 `local_modes.command`（默认关）打开时被 host-api 读取，`@` 名单看 `local_modes.mention`，声音和音乐包在用户之后关掉开关时也照样显示在用；列表不看当前方案能不能打开 K、`/`、`@`。
- 宿主各有一份「哪些方案打开 `/`、`@`」的判断，Engine 放开五笔后它们仍只认全拼、双拼：Linux `SpellingSymbols.h` 把五笔空闲时的 `/`、`@` 当成方案自己的拼写直接交给 Engine，绕开标点路由；Windows Server 下发给 TSF 的 `command_mode` / `mention_mode` 在五笔下恒为假，TSF 根本不把这两个键交给 Engine。
- Windows 顶字路径总让 TSF 消费 4 个字母；顶字后的大写字母现在随首选上屏、Engine 不留组合时，TSF 会把这个字母留在自己的缓冲里重新组字。
- Linux 两个宿主在五笔关掉剩余编码、或拼音关掉候选窗辅助码时整体隐藏候选注释，五笔能进 `/`、`@` 之后，指令标题和地名省市也一起被隐藏。
- Linux 托盘和 macOS、Windows 原生设置的辅助码方案菜单不知道辅助码插件已经替换了方案。
- 声音和插件表的运行期失败只写 stderr，macOS 输入法进程的 stderr 无人接收。

## Decision

- 插件标记只在门控开关也开着、且当前方案能打开对应模式时才标「使用中 / 已启用」，否则写明哪一项不满足。`/` 指令和 `@` 模式不随插件自动打开：打开它们会改变 `/`、`@` 在所有应用里打出来的字；指令表详情和 `@` 名单在模式关着时给出说明和就地打开的入口，与短语表看 K 模式的做法一致。界面的 `schemeOpensTableModes` 与 Engine 的 `opens_table_modes` 是同一份方案集合。只闪烁候选栏的设备（Windows、鸿蒙）上特效说明按实际画法写。
- 宿主侧副本跟上 `opens_table_modes`：Linux `scheme::OpensTableModes`（定义在 `InputSchemeTraits.h`，`SpellingSymbols.h` 用它）；Windows `common/InputSchemeTraits.h` 的 `scheme::OpensTableModes` / `OpensLocalModes`，由 `apply_local_mode_switches` 决定下发给 TSF 的 `command_mode` / `mention_mode` / `expression_mode`。两者都由 `scripts/test-scheme-traits-parity.py` 对照 Engine，不再是手写的方案名比较。
- Windows 的 `AutoCommitAndContinue` 消费数按「按键前 Engine 持有的编码长度，Engine 之后不留组合时再加上这个键」算（`wubi_continue_consumed`）：唯一码第四个字母 3+1，顶字后的小写字母 4，被放走的大写字母 4+1。固定写 5 会在 Engine 不留组合的唯一码路径上吞掉用户提前敲进 TSF 缓冲的下一个字母，所以不用常数。
- Linux 两个宿主只在 `local_mode` 是 `command` 或 `mention` 时跳过两道注释门控：这两个模式里 Engine 填进 `annotation` 的是指令标题或 `@` 地名的省市；其余本地模式的注释仍是辅助码，照旧受开关控制。
- 辅助码方案菜单（Linux IBus / Fcitx5、macOS 与 Windows 原生设置）在辅助码插件生效时显示插件，从菜单选内置方案即清掉该方案的插件选择，与设置页 `helpcode-page.tsx` 的契约一致；macOS 原生设置把缺 `plugins` 或缺键视为没有选插件，与 Rust 省略空值的写法一致。
- host-api 提供 `msime_client_set_diagnostic_sink`，声音播放器、插件表和辅助码表的运行期失败经它送出，没注册时仍写 stderr；已注册的出口在被替换后仍可能被进行中的报告调用，所以必须在进程内一直可调用。macOS 在启动时注册，只记冒号前的类别（不记包名、路径和错误原文），日志关着时不记，守住 `diagnostic.log` 已写明的隐私约定。
- macOS 第一次激活时还没有会话的控制器，在会话建好后补一次背景音乐的占用。
- `docs/plugins.md` 加平台支持表，写明 Android、iOS 没有插件页，鸿蒙键盘目前读不到设置应用导入的包（见 [鸿蒙共享沙箱提案](../../proposed/architecture/2026-10-09-harmony-ime-shared-sandbox.md)）。

## Alternatives considered

- 选指令表时顺手打开 `local_modes.command`：用户点「启用」的意图最直接。否掉，因为 `/` 默认打出顿号或斜杠，打开模式会改变所有应用里这个键的行为，应该由用户自己决定；给出就地入口已经足够。
- 宿主不维护副本、一律问 Engine：Windows TSF 要在把键交给 Server 之前决定，Linux 要在自己的按键绑定之前决定，两者都拿不到同步的 Engine 回答；保留副本并用 parity 脚本钉住是现有做法。
- macOS 日志记下 host-api 原文并在日志关着时暂存：排查更方便。否掉，与 `docs/implementation.md`、`docs/windows-parity.md` 和 MCP 工具说明里「只记类别、关着时什么都不记」的约定冲突。
- 修鸿蒙共享沙箱：根因在那里，但需要在 AppGallery Connect 申请 data-group-id，留在提案里。

## Consequences

- 收益：插件页不再在开关或方案不允许时声称插件在用；五笔的 `/`、`@` 在 Linux 和 Windows 上真正进得去；运行期失败在 macOS 诊断日志里看得到类别。
- 代价：宿主多了两份要与 Engine 同步的判断，靠 parity 脚本兜底；macOS 日志只到类别，具体是哪个包、什么错误仍要另行复现。
- 未覆盖：iOS、Android 键盘工具栏仍只在拼音方案里提供 K、`/`、`@` 入口；鸿蒙共享沙箱未做；Linux 与 Windows 没有注册诊断出口，仍写 stderr。会话创建时只报一次的失败，macOS 在首次会话前就按偏好配置好日志，见 [macOS 首次会话诊断日志时序](2026-10-10-macos-diagnostic-startup.md)；日志关着时它们照样不记。

## Verification

`cargo test -p msime-host-api` 与 clippy；`apps/desktop` 的 `tsc --noEmit` 与 `vitest run tests/settings`；Linux `platforms/linux/build-container.sh`（ctest）；macOS `macos-diagnostic-log` 与 `shortcut` ctest；Windows `reply_composer`、`tsf_config_frames`、`input_scheme_traits` 在本机用 clang 编译运行；`scripts/test-scheme-traits-parity.py`、`scripts/test-windows-state-dir-parity.py`。Windows 原生代码的 MSVC 编译交给 Windows CI。
