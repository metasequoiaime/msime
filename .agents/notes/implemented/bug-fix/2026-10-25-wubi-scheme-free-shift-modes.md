# Agent Note: 五笔打开不经拼音拼写的 Shift 模式

Status: implemented

## Problem

#9696：Windows 0.3.0、五笔方案，按 Shift+Y 后打 `superuser`，上屏「格斗家ruser」；按 Shift+T 后打 `date`，上屏「春花秋月」。设置里「五笔四码唯一候选自动上屏」关着。

根因在 Engine 的 `InputSession::local_mode_for_entry`：只有 K 和 C 按 `opens_table_modes`（全拼、双拼、五笔）判断，其余 Shift+字母模式按 `opens_local_modes`（只有全拼、双拼）判断，所以五笔下 Shift+Y、Shift+T 不进模式，后面的字母照常按五笔码组字。这个划分来自[五笔开放本地表模式入口](2026-10-10-wubi-table-local-modes.md)，那篇否掉「让五笔复用所有 Shift 模式」的理由是「表达式、临时英文等模式依赖拼音输入」；对临时英文、日期时间、Unicode 和符号模式不成立，它们的查询（`ImeQueries::local`）根本不读方案：`temporary_english(code)`、`query_date_time(code, now)`、`query_unicode(code)`、`query_symbol_mode(code, …)`。设置页（共享页、Windows 原生设置、macOS）在五笔版和选五笔时照样列出这些模式、默认打开，也没写五笔下不可用。

报告里 Y 和 T 本身也没上屏，这是 Windows TIP 的另一层：TIP 把 Wubi 当全拼归类（`mode_scheme`），没有组字时的 Shift+字母一律当组字输入吃掉；宿主会话答 `handled=false` 时 `_ApplyHostCompositionResult` 返回 `S_FALSE`，延迟按键队列重放 7 次后放弃（server.log 里每次 Shift+Y、Shift+T 都是一次 `test-keydown` 之后 `deferred-replay-retry` × 7、`deferred-replay-abandoned`、`deferred-queue-cleared`），字母丢掉，会话标脏重连。修好入口以后 Y、T 由 Engine 接住，不再走这条路。

「四码唯一自动上屏」与此无关：入口判定在 `local_mode_for_entry`，排在五笔组字之前；`wubi_unique_four_code` 和 `input-runtime` 的 `wubi_four_code_is_complete`（顶字）都要求 `local_mode == none`，开关怎么设都一样。

## Decision

没有组字时，输入不经过当前方案拼写的 Shift+字母模式按 `opens_table_modes` 判断，五笔下也能进入：K（快捷短语）、C（拆字）、U（Unicode）、T（日期时间）、Y（临时英文）、S（符号）。按拼音拼写查的 E、M、J 和切到日文方案的 R 仍按 `opens_local_modes`；V 的数字和运算符要宿主按方案路由（Windows Server 的 `LocalModeTriggersChanged` 按 `scheme::OpensLocalModes` 下发 expression 标志），也仍只在全拼、双拼下进。两个 trait 的取值不变，只是哪个模式归哪个 trait 变了，宿主的 trait 镜像（Windows、Linux、Android）只改注释。

五笔编码只用小写字母，没有组字时 Shift+字母不会挡住任何五笔码；不想要的用户用「Shift+字母进入快捷模式」总开关或各模式自己的开关关掉，和拼音方案一样。

## 各平台

- Windows：TIP 已经把五笔下的 Shift+字母交给宿主会话，Server 和 TIP 两份会话用同一个 Engine，入口判定一起变。U 模式的数字由视图的 `local_mode` 路由（Server `EditPolicy` 的 `unicode` 分支、TIP 的 `KeyCompositionState::unicodeMode`），不看方案。五笔本地模式里由字符键产生的上屏走 `AutoCommitAndContinue`（`commit_context.scheme == 2`），这条路 K、C 早就在用。
- macOS、Linux：宿主把带 Shift 标志的字母直接交给 Engine，答 `handled=false` 时交还应用，不需要改。
- 移动端：触屏工具栏的本地模式入口照旧只在拼音方案下出现（Android `supportsLocalTools`、iOS `supportsLocalTools`），实体键盘的 Shift+字母跟着 Engine 走。

## Alternatives considered

- 让五笔复用所有 Shift 模式（E、M、J、V 一起打开）：最强的理由是设置页对五笔用户列出了全部模式，打开以后不会再有「开关在但按了没用」的模式。没有采用：E、M、J 按拼音拼写查，五笔用户的入口是另一回事（混输关闭时 `z` 引导临时拼音）；V 要同时改 Windows Server 的 expression 标志和 TIP 的路由，超出这个问题的范围。
- 只在设置里注明五笔下不可用、Engine 不动：不改行为。没有采用：Y、T、U、S 在五笔下没有任何冲突，挡住它们只是沿用了一个事实错误的理由；报告者要的是能用。
- 在 Windows TIP 里按方案拦下五笔的 Shift+字母、交给应用：能修好「字母被吞」。没有采用：TIP 不知道 Engine 开了哪些模式，拦下来模式还是进不去；这是另一层问题，见 Consequences。

## Consequences

- 收益：五笔用户 Shift+Y、T、U、S 进入临时英文、日期时间、Unicode 和符号模式，各平台一致；以后加不经方案拼写的模式时，在 `local_mode_for_entry` 的同一个 `matches!` 里加一项。
- 代价：五笔下空闲时 Shift+Y/T/U/S 不再打出大写字母（打大写要关总开关或对应模式）。E、M、J、V 在五笔下仍是「设置里有、按了不进」，设置页没写出这一点。
- 遗留：Windows TIP 吞掉 Engine 不收的空闲大写字母（五笔的 Shift+A 以及 E、M、J、V，拼音总开关关掉时的大写字母），重放后丢弃并重连。这一层没有在本次修改，由 [Windows TIP 把 Engine 不收的空闲字符当上屏写入](2026-10-25-windows-declined-idle-character.md) 处理，拼音的情形在那里用测试确认。

## Verification

- `crates/engine` 新增 `wubi_opens_the_shift_letter_modes_that_spell_no_pinyin`：五笔下 U、T、Y、S 进入各自模式，Shift+Y 后 `superuser` 选首选上屏 `superuser`，Shift+T 后 `date` 列出日期并上屏，E、M、J、V 仍交还宿主。修改前在 U 处失败，修改后通过。
- `platforms/windows/tests/input/wubi_auto_commit.cpp` 新增 `Tdate`、`Uabcd` 两个场景，覆盖 Server（`ReplyComposer::basic_key`）和 TIP（`EngineSessionAdapter`）两份会话，三种预编辑样式、UI-less 与 86/98 组合：组字留在模式里、不走四码自动上屏，空格上屏 `Uabcd` 的首选。修改前 TIP 侧 Shift+T 答 `handled=false`、断言「ordinary TIP result application succeeds」失败，修改后通过（macOS 上由 `scripts/test-windows-native-run.py` 的同一套编译方式运行）。
- 黄金场景 `local_shortcuts_outside_pinyin` 记录的是参考引擎在五笔下拒绝 U、T，按新行为更新（U、T 之后各加一步 `Cancel`，E、M 保留参考断言），说明写在 `tools/engine-golden/README.md`。
- `cargo test -p msime-engine -p msime-input-runtime -p msime-host-api --lib --locked` 全部通过；`scripts/test-scheme-traits-parity.py` 通过。Windows 实机未验证（测试机不可达），原生代码只由可移植测试和 PR CI 覆盖。
