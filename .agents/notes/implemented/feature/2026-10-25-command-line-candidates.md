# Agent Note: 命令行查询候选在本进程里建输入会话

Status: implemented

## Problem

#9663 要一个命令行工具：传入拼音或按键（连同光标在其中的位置），把应有的候选以 JSON 写到 stdout；不传时从 stdin 逐行读、逐行答。命令行上不带任何方案配置，结果要与用户打字时一致。只做 Windows、macOS、Linux。候选来自用户词库，所以默认关闭，由用户在「维护与诊断」里打开；关着时提示并以非 0 退出，每种失败要有独立的退出码。issue 建议经进程间通信（Windows 上的命名管道）从正在运行的输入法取候选。

已有的 `msime expand` 和 MCP 工具 `lookup_candidates` 不满足「与打字一致」：`crates/host-api/src/dictionary.rs` 的 `lookup_candidates` 直接建 `msime_engine::host::Session` 和 `Runtime`，不经 `HostSession`，并且关掉了调频（`frequency_mode = "disabled"`）和快捷短语、表情、颜文字、超级简拼、临时英文等全部本地来源，不跑停顿重排，不做繁体转换，没有宿主额外请求的 `quick_phrase_candidates`，也不能指定光标；它是为了解释「某个候选为什么排在这里」，而且接受 `--scheme` 临时换方案。

## Decision

`msime-mcp`（macOS、Linux 上也叫 `msime`）新增子命令 `candidates [<keys>] [--limit <n>]`，实现在 `crates/mcp-server/src/candidates.rs`，不经 MCP 服务器。候选由 `crates/host-api/src/candidate_preview.rs` 的 `preview_candidates` 给出，它走桌面宿主建会话、打字的同一条路：

- 输入是 `Config::read_host_options()` 给的运行时选项，其中的偏好已换成状态目录里现行的 preferences.json，和宿主轮询偏好后应用的是同一份；再加上本平台宿主建会话时另外请求的 `phrase_preedit`，以及 `HostCapabilities::quick_phrase_candidates` 为真的平台（macOS、Linux）请求的 `quick_phrase_candidates`。
- 会话由 `ffi::session::create_session`（`msime_client_create` 的本体）建成，用途是 `SessionUse::Preview`：不调用 `dictionary_updates::record_loaded`（否则设置页会以为输入法已经载入新下载的词库代次），`HostSession::preview` 让 `private_session()` 为真，不记选词位置和上屏效率。文档里的 `preferences.learning` 改成 `false`，选词和上屏都不调词频、不造词、不记上下文。
- 聚焦后逐键经 `dispatch_value`（`dispatch_call` 的本体）送 `Action::Character`，大写字母带 Shift；`handled` 为假的键报 `invalid_input`。上屏的文字收进 `committed`。光标靠 `Command::MoveLeft` 从末尾移回去，移不到要的位置就拒绝。之后 `ffi::settled::settle_now` 当场跑一次停顿重排（宿主在停顿 150 ms 后做同样的事），最后取 `Runtime::all_candidates()`，去掉 `CloudPending`、`AiPending` 占位行，按 `traditional_chinese_output` 与 `View::script_conversion` 用 `convert_to_traditional` 转繁体，和宿主画候选时一样。
- 每次查询新建会话、用完即毁，不在两次查询之间持有 `DictionaryAccess` 的共享租约。

开关是偏好 `diagnostic_log.command_line_candidates`，缺省 `false`。放在 `diagnostic_log` 下，现有的 `settings_document::LOCAL_SECTIONS` 和云同步的 `LOCAL_ONLY_FRAGMENTS` 已经按这个名字把整组当作本机设置，不随设置文件和账号同步走；MCP 的 `update_preferences` 不提供这一项，助手不能自己打开它。界面在共享设置页（macOS、Linux，以及直接运行 `MSIME.exe` 时的 Windows 一支）、Windows WinUI 3 原生设置和 Win7 设置的「维护与诊断」→「命令行工具」。命令每次查询前都现读这一项。

输出、退出码和与打字时的差别写在 [docs/command-line-candidates.md](../../../../docs/command-line-candidates.md)。退出码：0 成功，1 意料之外的失败，2 命令行写错，3 `disabled`，4 `not_set_up`，5 `busy`（词库维护中），6 `unavailable`（词库打不开），7 `invalid_input`。stdout 每次查询一行 JSON，失败也是一行带 `error.code` 的 JSON，原因同时写 stderr。逐行读 stdin 时 `busy`、`unavailable`、`invalid_input` 只影响那一行，`disabled`、`not_set_up` 让命令停下。

## 为什么不连到正在运行的输入法

决定前核对了运行中的输入法有哪些候选相关的状态不在磁盘上：

- **偏好**：三个桌面宿主建会话用的是运行时选项文档加 preferences.json，偏好改了由宿主轮询后应用。Linux 输入法菜单里改的调频、繁体输出等选项在有 `preferences_directory` 时经 `save_menu_preference` 写回偏好文件，只有没有状态目录的开发环境才停在进程内的 `*_override` 上（`platforms/linux/src/core/ClientEngine.cpp`）。
- **学到的词频和用户词**：选词调频在选中时同步写进 journal（失败时报 `FREQUENCY_NOT_PERSISTED`），造词同理。只有个人上下文的转移延迟约 2 秒写盘（`msime_engine::flush_personal_learning` 的说明），而上下文重排只在同一会话里前面有刚上屏的词时才起作用，一串全新的按键用不到它。
- **宿主额外请求的能力**：macOS `MSIMESessionOptions`、Linux 两个宿主和 Windows `PhrasePreeditOptions.h` 在文档之外只加 `phrase_preedit`、`quick_phrase_candidates`、音效包目录和 macOS 的辅助码表回退目录，前两项本命令照加。
- **停顿重排**：macOS、IBus 和 Windows 的宿主都跑，本命令同步跑一次。Fcitx5 宿主不跑，是已知差别。
- **只属于某个输入框的状态**：前文、中英模式、大写锁定。命令问的是「这串键给出什么」，相当于在新输入框的中文状态下打这串键，这些本来就不该带进来。

所以在本进程里用同一份磁盘状态、同一个 `HostSession` 建会话，与运行中的输入法给出的候选相同，差别只剩上面列出的几处。实测 macOS 上的 release 构建，一次查询（含进程启动）约 0.1 秒，逐行读时每行约 60 毫秒。

## Alternatives considered

- **经 IPC 从运行中的输入法取候选（issue 的建议）** — 最强的理由是它天然包含进程内的一切状态，用的一定是正在打字的那个引擎版本（macOS 的输入法本体由 Sparkle 单独更新，`MSIME.app` 和其中的 `msime-mcp` 不跟着更新，见[偏好保留未知字段](../architecture/2026-10-15-preferences-keep-unknown-fields.md)，两边的引擎可能不是同一版），「输入法没在运行」也能成为一种明确的错误。不用它，因为：三个平台要各写一个服务端，macOS 的 IMK 进程、Linux 的 IBus engine 和 Fcitx5 插件（两个不同的进程）、Windows 的 Server 都没有现成的查询通道，Windows 的 TSF 与 Server 协议还是 ARCHITECTURE.md 里不能顺手改的边界；服务端也不能往用户正在用的会话里打字，只能另建一个会话，而另建的会话读的正是本命令读的同一份磁盘状态；另外它在每个输入法进程里多开一个任何本地程序都能连的口子。上面核对过的差别都不需要输入法进程才能补上。
- **扩展 `expand` 或 `lookup_candidates`** — 复用现有命令，没有第二个相近的命令。不用它，因为这两者的用途是解释排名（来源、权重），关掉调频和本地来源正是为了这个，`--scheme` 也是它们的功能；改成和打字一致会改变已有用户和助手看到的结果。`candidates` 另起一个命令，两者在帮助里分开写。
- **做成新的 MCP 工具，再由命令行调用** — 和 `cli.rs` 里「命令行不重新实现工具」的做法一致。不用它，因为开关是给命令行的，做成工具还要再定一套助手侧的授权，而 issue 要的是给程序用的稳定 JSON 和退出码，不是 MCP 的结果格式。
- **流式读 stdin 时一直开着一个会话** — 省掉每行建会话的时间。不用它，因为会话持有 `DictionaryAccess` 的共享租约，维护（导入、恢复、换代次）用 `try_maintenance` 不等待，一个开着的流会让设置页的词库导入一直报忙；每行重建也让偏好、词库和学到的词频的改动在下一行就生效，实测每行约 60 毫秒，代价可以接受。
- **另做一个可执行文件** — 名字更直观。不用它，因为 `msime-mcp` 已经随三个桌面平台的安装包分发、已经链接成 `msime`，并且已经有找到运行时选项和状态目录的全部逻辑（含各版本的身份）。

## Existing notes audit

- [偏好保留未知字段](../architecture/2026-10-15-preferences-keep-unknown-fields.md)：部分相关，新字段嵌在 `diagnostic_log` 里，旧版本的输入法或设置应用读写时按那篇的规则忽略并保留它；那篇记下的「macOS 输入法与设置应用分开更新」也是本命令在 macOS 上的版本差别的来源。
- [MCP 命令展示模型](../simplification/2026-10-24-mcp-command-model.md)、[MCP 独立设置页](../architecture/2026-10-18-mcp-independent-settings-page.md)：无关，那是设置页里「连接 AI 助手」的命令展示，本命令不经 MCP。
- [维护页 AI 助手标签](2026-10-18-maintenance-assistant-tabs.md)、[Windows 维护页](2026-10-20-windows-settings-maintenance-page.md)：部分相关，本篇在同一页加了「命令行工具」组，那两篇的分组和标签规则不变。

## Consequences

- **收益**：测试和演示程序不改源码就能拿到与打字一致的候选，覆盖方案、双拼键位、模糊音、快捷短语、插件、学到的词频、繁体输出和停顿重排；不需要输入法正在运行，也不在任何输入法进程里新开通道。查询不改用户数据：不学习、不计统计、不报告词库代次。
- **代价与已知上限**：候选出自 `msime-mcp` 自己编进去的引擎，macOS 上输入法本体单独更新到另一个版本时，两边的排序规则可能不同，要以同一版本的整套安装为准；不含云候选和 AI 候选；一串键中途上屏之后没有上下文重排；Fcitx5 下停顿重排的顺序可能不同；macOS 词库目录缺内置辅助码表时，命令不知道输入法包里的回退表在哪里。开关守的是用户意图而不是安全边界：能以用户身份运行程序的软件可以直接读词库文件，也可以用 `--state-dir` 指向另一份偏好。新增能影响候选的宿主请求（像 `quick_phrase_candidates` 那样写进会话文档的字段）时，要同步加进 `candidate_preview::with_host_requests`，否则这里会悄悄与打字不一致；宿主若改成在进程内保留不写盘的候选状态，本决定要重访。

## Verification

- `cargo test -p msime-host-api --lib candidate_preview`：8 项，覆盖候选与编码、光标只转换前面一段、繁体输出、快捷短语按平台能力出现、选词上屏后词序不变且不计统计、不写 `dictionary-updates/loaded.json`、按键与光标与候选数的拒绝、词库维护中报 `Busy`、坏文档与打不开词库的区分。去掉 `learning = false` 和 `preview` 后「选词上屏后词序不变」一项失败。
- `cargo test -p msime-mcp-server`：`config.rs` 的解析测试和 `candidates.rs` 的单测（光标标记、退出码互不相同、开关只认 `true`、逐行读的长度与编码限制），以及 `tests/stdio.rs` 用编好的二进制跑的四项：默认关闭（退出码 3，allow 开关和 `config set` 都打不开）、单次查询的 JSON、逐行读 stdin、每种失败的退出码（2、4、5、6、7）。
- `cargo test -p msime-client-core --lib preferences` 覆盖缺省关闭和旧文档读出为关闭。
- 共享设置页的 vitest、Win7 设置的 `windows7_settings.cpp` 和 WinUI 的 `settings_ui.ps1` 覆盖开关的显示、缺省值、保存时保留 `server` 与 `tsf`、以及不进设置文件。
