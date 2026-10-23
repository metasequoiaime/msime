# Agent Note: 脚本插件在自己的 `/` 指令里出候选：来源 14、按预编辑串缓存的行、只在派发里问插件

Status: implemented

## Problem

脚本插件 v2 的接口只能对内置结果做加工（改写上屏、重排、注释、过滤候选，见 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md)），没有任何一条路能让插件自己给出候选：按键不投递，本地模式不在白名单里时文本一律读不到，插件也就做不出 UUID、符号名转字符这类用户真正缺的东西。

v3 的做法是让插件在 `/` 指令模式里注册自己的触发词：用户打 `/uuid` 时由插件给出几行，用户选中其中一行上屏。这要先在引擎和 runtime 里回答几件事：插件的行用什么来源、和内置指令怎么分、列表重建时行怎么不丢、什么时候问插件（不能把打字停顿和网络时序交给脚本）、这些行上屏之后哪些内置机制不能碰它（学习、统计、AI 上文、释义、全角、失焦上屏）。

这篇记引擎和 runtime 一侧（分支 A）、ABI、清单、作者工具与设置页文案（分支 B，见「ABI」一节），以及 host-api 的投递、授权页的指令列表和作者文档（分支 C，见「投递」一节）。A、B 单独合入时生产路径上没有东西会调用新代码：`EngineOptions::plugin_commands` 默认为空，`CommandProvider` 没有人挂，host-api 不投递 `command` 钩子；B 让用户能导入、授权申请了 `provide_commands` 的包，设置页画得出它，但授权了也不出候选。C 合入之后三个桌面平台上授权了的插件在自己的 `/` 指令里出候选。

## Decision

### 来源 14

`CandidateSource::Plugin = 14`。13 已经是日文混输行，10 是历史空号不复用。它不属于任何学习谓词（`is_sentence_learning`、`is_dictionary`），也不在 `is_editable_source` 的白名单里，所以：

- 本地模式里选中走 `session/commit.rs` 的本地模式分支，上屏 `item.word` 并 `reset_composition`；本地模式的选中照样调 `learn_candidate`，但整句学习被 `can_learn_sentence_candidate` 的本地模式检查挡住，调词频要 `is_dictionary`，两条都走不到。`/` 只在没有组字时打开，`reset_composition` 清掉了 `ranking_candidates`，学习读的是 `local_candidates`，不会把下标错映射到另一个词库词上。
- 不能置顶、固定或删除。
- 指令模式属于 `generates_text`，不计打字统计。

### 引擎：插件指令、匹配和缓存

- **类型**：`PluginCommand { trigger, title }`（`types.rs`，`CommandTableEntry` 旁边）；`EngineOptions::plugin_commands` 和 `SessionOptions::plugin_commands` 默认空表，`Session::set_plugin_commands` 实时替换。
- **哪些插件指令生效**（`local::command::usable_plugin_commands`）：触发词 2–16 个小写 ASCII 字母，标题 1–8 个字符、不含控制字符、首尾不是空白；触发词不等于、也不和任何内置触发词、用户指令表（`usable_command_table` 之后）的触发词、排在前面的插件触发词互为前缀。用户的指令表永远赢，插件之间先到先得。`CandidateQueries` 留着宿主交来的原表，`set_command_table` 之后按新表重新过滤。过滤之后任何两个生效的触发词都没有前缀关系，所以 `plugin_command_source` 不需要最长匹配，一段输入至多属于一条插件指令。client-core 的 `script_commands::resolve` 用同一套规则给授权页算状态（分支 C，见「投递」），内置触发词由 `local::command::builtin_triggers`、`builtin_titles` 公开（`msime_engine::host::builtin_command_triggers`、`builtin_command_titles`），给分支 B 的一致性测试用。
- **匹配**（`plugin_command_source`）：`/` 之后的输入以某个生效的触发词开头、剩下的全是小写字母时，返回（触发词，参数），参数可以为空。收键本来就只收小写字母（`'` 只在 `/fy` 的单词之间收），本地预编辑串最长 `GENERATED_MODE_INPUT_LIMIT = 64` 字节，参数长度由它封顶。
- **引擎自己不列行**：`CandidateQueries::local` 的指令分支在输入属于插件指令时返回空，列表由缓存的插件行或兜底行（原样的 `/uuid`）顶着。打 `/u` 这样的前缀时插件指令不出现在列表里：列表里每一行都必须能上屏，插件指令在用户打完触发词之前给不出行。
- **要问插件的那一次**（`Session::plugin_command_query`）：只在 `/` 模式里、不在专用英文、输入属于某条插件指令、这份预编辑串还没有缓存时返回 `PluginCommandQuery { session_id, preedit, trigger, argument }`。九键会话一律 `None`。
- **写回**（`Session::apply_plugin_command_rows(query, rows)`）：`query` 必须仍等于当前的 `plugin_command_query()`，否则拒绝（会话不同、预编辑串变了、已经缓存过）。`rows` 是（文本，插件注释），注释不进引擎。引擎丢掉空白、含控制字符、超过 `TEXT_UTF16_LIMIT`（199）个码元、和前面同文的行，最多留 `RESULT_LIMIT`（18）行；行的 `pinyin` 存触发词，来源 14，权重按顺序递减。有行时去掉兜底行；0 行也缓存成空表。
- **缓存**（`InputSession::plugin_rows: Option<(预编辑串, 行)>`）：`update_local_candidates` 每次都从头重建 `local_candidates`，调用它的地方很多（按键、退格、光标处编辑、换表），只记一个「已经问过」的标记的话，标记还在、行已经没了，查询又不会再发。所以缓存的是行本身：重建时在 `apply_candidate_positions` 之后、加兜底行之前，预编辑串和缓存一致就把行放回前面（同文的后一行去掉），不一致就清掉缓存。只有一槽，所以 `/uuid` → `/uui` → `/uuid` 会重新问插件、换一个 UUID；方向键、列表重建时行一直在，不再问。`reset_composition`、进入本地模式、`set_plugin_commands`（授权撤销后旧行不能插回来）、`set_command_table`（生效的插件指令可能跟着变）都清掉缓存。
- **引擎注释为空**：`candidate_annotations` 的指令分支对来源 14 返回空串，`command_title` 也不查插件指令。插件的标题和注释只画在插件注释格里（见下），引擎注释格只放引擎的内容。
- **释义**：`host::glosses::candidate_gloss_key` 对来源 14 返回 `None`，经引擎查释义的宿主路径（macOS 的 `candidate_gloss_request`、Windows 的 `msime_client_candidate_gloss_request`）都不给它查。

### runtime：什么时候问插件

- **`CommandProvider`**（`transform.rs` 的 desktop 模块，cfg 同候选变换）：`provide(trigger, argument, out) -> CommandOutcome`。`Ran` 表示插件运行完了，`out` 就是它的行（可以是 0 行）；`NotRun` 表示插件没运行（模块还在编译、已经熔断、本轮配额用完、没有主人），不写缓存，下一次派发再问。`Runtime::set_command_provider` 挂上或去掉它；移动端和网页引擎没有这个 trait，inert 模块里同名的内部方法什么都不做。
- **调用时机**：`refresh` 的第一步、取快照之前（`run_command_provider`）。只有同时满足才问：正在派发类调用里（`dispatch_live` 置的 `live` 标记）、没有暂停（`suspend_candidate_transform`，宿主在 `COMMIT_OBSERVE_ONLY` 时设）、挂着提供者、引擎的 `plugin_command_query()` 是 `Some`。插件运行了就 `apply_plugin_command_rows`，然后照常取快照。行在同一次派发里写进引擎，代次只在派发开头前进那一次，`CandidateId` 当场就能选。旁路调用（`apply_online_*`、`rerank_settled`、`all_candidates`、`apply_translations`、设置类调用）从不问，引擎缓存的行照样显示，所以停顿和网络时序都不交给脚本；高亮移动和翻页在 refresh 之前就返回，也不问。
- **`Runtime::set_plugin_commands`**：交给引擎，照 `set_command_table` 推进代次并刷新。runtime 不另记标题：宿主交来的表可能有重复的触发词、不合规的标题，哪一条生效只有引擎知道（先到先得、标题不合规的丢掉，用户指令表变了还要重新过滤），runtime 另记一张表就会和引擎分叉，后来者的标题画到先来者的行旁边。
- **插件注释**：runtime 记下最近一次写进引擎的行的（文本 → 注释）；标题按行的 `pinyin`（触发词）向引擎要（`InputEngine::plugin_command_title`，引擎侧是 `Session::plugin_command_title`，查的是过滤之后的生效表）。`script_annotation(seat)` 对来源 14 的可见行返回「标题 注释」，没有注释时只有标题，含控制字符的注释不显示，整段截到 `MAX_ANNOTATION_UTF16 = 32` 个 UTF-16 码元（和 script-runtime 的 `limits::MAX_ANNOTATION_UTF16` 同值，在字符边界上截）。来源 14 的行不经过候选变换的注释表。
- **不进 AI 上文**：`transition` 在 `facts.selection` 的来源是 14、结果是 Committed 时跳过 `remember_commit`。runtime 只能按选中行的来源区分，这是来源 14 不能复用 `Generated` 的原因。
- **全角**：`OutputContext::plugin_row`（`#[serde(skip)]`，不上线缆）按同一条件算；host-api 的 `complete_transition` 在它为真时不转全角，写法同网址模式。同一个 `/` 模式里内置指令（`/rq`）的上屏照旧转全角。
- **失焦**：新动作 `Action::FinishOnFocusLoss`，host-api 的命令 20（宿主替失焦结束组字，macOS 的 `commitComposition:`）派发它，生命周期类别仍是 `FocusLoss`（`InputClass::of` 对它返回 `Commit`，命令 20 再覆盖成 `FocusLoss`）。它和 `Finish` 只差一处：高亮行是来源 14 时走 `Command::CommitRaw`，上屏用户打的 `/uuid`，因为这一次用户没在看候选窗。`Finish`（命令 9）和标点结束仍提交高亮行，和 `/rq` 一致：那是用户自己按下的键。

### host-api 和 client-core 的排除项

- `client-core::translation::source_gets_no_gloss` 加上 14：在线释义不问它。
- `msime_client_translation_query` 在当前页有来源 14 的行时整页返回 `null`：不问在线释义、不交给用户的翻译服务、也不查离线释义，免得插件的文字离开本机或经释义写回学到的词库。页面和回答按下标对齐（`Runtime::translation_candidates`），所以不逐行剔除；这时页上只有插件行，至多还有一行原样的兜底行，整页不请求没有损失。`/fy` 的翻译请求走另一条分支，不受影响。

### Windows 选词策略

`CandidateCompletionPolicy.h` 的 `candidate_finishes_composition` 加上 14。不加也照常出声、画特效：本地模式里选中时引擎 `reset_composition`，这次是 Committed 不是 Partial，`lifecycle::held_piece` 为假。加它是让策略表和引擎「选中即整段上屏」的语义一致，和来源 8 同一个理由。

### 汇总

| 项 | 插件指令行（来源 14） |
|---|---|
| 调词频、记整句、连选成词、个人上下文 | 不做（不属于任何学习谓词） |
| 置顶、固定、删除 | 引擎拒绝（`is_editable_source`），Windows 右键菜单白名单本来就不含 14 |
| 打字统计 | 不计（`/` 属于 `generates_text`） |
| AI 和重排上文 | 不进（`transition` 跳过 `remember_commit`） |
| 释义与翻译 | 整页不请求；引擎的释义键为 `None` |
| 全角 | 不转（`OutputContext::plugin_row`） |
| 失焦（命令 20） | 上屏原样的 `/xxx` |
| 引擎注释 | 空；标题和插件注释画在插件注释格 |
| 简繁转换 | 照做，显示和上屏一致 |

### ABI（分支 B）

- **能力 `provide_commands`**（`Capability::ProvideCommands = 7`）：没有依赖、不和任何一项互斥，`reads_text()` 为真（它读用户在它的触发词后面打的字母）。`Capability::ALL` 现在 8 项，正好占满 u8 的 `CapabilitySet`；`abi.rs` 里一条编译期断言 `Capability::ALL.len() <= u8::BITS`，第 9 项加入时编译失败，那时再扩宽（授权文件存的是能力名，扩宽不影响格式）。
- **钩子 `command = 18`**：不是生命周期事件，排在 `candidates = 17` 之后，`Hook::ALL` 14 项；只投递给触发词的主人。`Hook::modifies()` 包含它，但那只是归类：真正的门控在每个 import 的 `modification_allowed(caller, 能力, 钩子)` 里。
- **三个 import**（`IMPORTS` 15 → 18，追加在最后）：
  - `command_text(field, dst, cap)`：能力 `provide_commands`，字段 `abi::command_field::TRIGGER = 0`、`ARGUMENT = 1`，只在 `command` 里有值，别处是 `ABSENT`。它不受本地模式白名单限制：`/` 是本地模式 8，对别的钩子、别的插件照旧屏蔽，只有触发词的主人在 `command` 里读得到自己触发词后面的字母，所以不需要新的 `LocalModeClass`。
  - `command_row(text, tlen, note, nlen)`：能力 `provide_commands`，只在 `command` 里可用，别处 `DENIED`。行暂存在 `Actions::rows`，钩子正常返回才交给宿主，trap、fuel 耗尽、返回负数时整批作废。第 10 行、文本超过 64 个 UTF-16 码元、注释超过 32 个返回 `LIMIT`；文本为空、首尾是空白、含 `is_forbidden_plugin_char` 拦下的字符或者含 `://`、`www.`、`mailto:`（不分大小写，不让插件行成为链接），注释含拦下的字符，返回 `INVALID`（`abi::valid_command_row_text`）。成功返回行号。
  - `random_fill(dst, len)`：不需要能力。每次 `command` 事件 64 字节，按调用顺序取，剩下的不够 `len` 个时返回 `LIMIT`、什么都不写；别的钩子里没有随机数。产品里取自操作系统的 CSPRNG（分支 C），`msime-pack run` 取自夹具。没申请任何权限的插件声明不了 `command` 钩子（见下面的清单规则），所以两处授权页「只能知道发生了什么事件」那句照旧成立，没有改。
- **跑在快照上（审查意见 B1）**：模块声明了 `before_commit` 或 `command` 时在 `msime_init` 之后取快照（`Hook::runs_on_snapshot`），这两个钩子每次都在快照复位出的新实例上运行，持久实例不受影响，`command` trap 了也不让持久实例不可用。所以 `command` 给出的行只取决于触发词、参数、随机数和模块本身：同时申请了 `read_commit` 的插件在 `commit` 里存进内存的上屏文本、全局计数，进不了任何一次 `command`。`CallContext::load` 另外对 `command` 事件不复制组字、上屏和候选，`event_scalar` 的标量只留本地模式（事件类型由钩子给出），宿主误填了上屏来源这类标量插件也读不到，分支 C 构造事件时填多了也带不进行里；在别的钩子里误带了 `CommandEvent` 时触发词、参数和随机数一样不交给插件；触发词和参数只在授予了 `provide_commands` 时复制。因此 `check_rules` 不新增互斥，三处界面的单对互斥写法不动。
- **fuel**：`COMMAND_FUEL = 100_000`，是单次上限，也是每个插件每轮 `command` 的合计，和 `BEFORE_COMMIT_FUEL` 同一个理由单独记：同一轮里 `input` 等钩子先花掉多少，不该决定用户主动打开的指令里有没有候选，它也不扣其余钩子的配额。`ScriptInstance::resume_turn` 多一个参数、新增 `command_fuel_left()`，host-api 的 `Entry::turn` 和 `msime-pack run` 的轮配额元组各多一份。被本轮配额截断时的判定和其余钩子相同（这一次用掉达到单次上限一半就算失败）。一次投递的工作量按构造有界（审查意见 M4）：最多 100 000 fuel、9 行、每行 64 个码元、64 字节随机数。
- **其余限制**（`limits.rs`）：`MAX_COMMAND_ROWS = 9`、`MAX_COMMAND_ROW_UTF16 = 64`、`RANDOM_BYTES_PER_EVENT = 64`、`MAX_COMMAND_ARGUMENT_BYTES = 64`、`MAX_COMMAND_TRIGGER_BYTES = 16`。`command` 事件没带 `CommandEvent`、触发词或参数超长时 `deliver` 不运行，返回 `Outcome::NotSubscribed`：宿主本不该这样投递，引擎的本地输入最多 64 字节，参数本来也到不了这个长度，这里是第二道；分支 C 把它和模块没编译好一样当作 `CommandOutcome::NotRun`。
- **清单 `[[script.commands]]`**（`client-services::plugins::script_pack`，`[script]` 的键 4 → 5）：1 到 4 条，每条只有 `trigger` 和 `title`。触发词 2–16 个小写 ASCII 字母，不等于、也不和任何内置触发词互为前缀，同一清单里不重复、不互为前缀；标题 1–8 个字符，不含 `abi::has_forbidden_plugin_char` 的字符，首尾不是空白，不等于内置标题。指令、`provide_commands`、`command` 钩子三者缺一不可。摘要的 `commands` 只在非空时序列化，旧包的 JSON 不变；内容哈希公式不变，清单字节本来就计入，没有 `commands` 的旧包哈希逐字节不变。清单查不了和别的插件、用户指令表的冲突：那由分支 C 的 `script_commands::resolve` 和引擎的 `usable_plugin_commands` 按同一套规则决定。
- **两份内置指令规则**：client-core 不能依赖引擎（`docs/ARCHITECTURE.md`），`script_pack::BUILTIN_COMMAND_TRIGGERS`、`BUILTIN_COMMAND_TITLES` 复制了引擎 `local::command::builtin_triggers`、`builtin_titles` 的全集；host-api 同时依赖两边，`the_manifest_check_knows_exactly_the_engines_builtin_commands` 核对两份完全相同。清单的标题规则比引擎的下限更严（还拒禁用字符和内置标题），引擎接受清单接受的一切。
- **作者工具**：`msime-pack run` 的夹具写 `{"event":"command","trigger":"uuid","argument":"","random":"<128 位十六进制>"}`：`local_mode` 固定是 `command`，触发词必须是清单声明过的、2–16 个小写字母，参数只收小写字母，`random` 不写是全 0。输出的 `command` 行多一个 `rows`（每行 `[文本, 注释]`，没有正常返回时为空），其余事件没有这个键，旧金样不变。`plugins/script/tools/check_run.py` 的 `command` 上限是 50 000，不授权时 `rows` 必须为空。SDK 两个文件加上三个 import、`MSIME_COMMAND_TRIGGER`/`MSIME_COMMAND_ARGUMENT`（Rust 绑定的 `command` 和 `random` 模块）和 `MSIME_HOOK_COMMAND`，由 `crates/pack-tool/tests/sdk.rs` 按 `abi::imports()` 和 ABI 常量核对。
- **设置页文案（审查意见 M3）**：共享页（`plugin-script-model.ts` 的 `capabilityLabels`）和 Windows 原生页（`PluginPageModel.h` 的 `capability_label`）都加「提供 / 指令」，两处一字不差，分支 C 写 `docs/plugins.md` 时照抄：「你在 / 指令里输入它声明的触发词（例如 /uuid）之后打的字母会交给这个插件，它据此给出候选；给出的候选只取决于这些字母和随机数，带不走你之前打过的内容。你按序号、点选、按空格选中某一行，或者打标点结束输入时，高亮的那一行会输入；在有的系统上，按快捷键或 Tab 等键、用菜单或工具栏结束输入时也会输入这一行。按回车时只输入你打的原文（例如 /uuid），点到别的窗口时插件给出的行不会输入。它看不到你平时打的字、其他指令和别的插件的内容；它给出的文字不进入词库、不计入打字统计、不会作为上文发给 AI 候选，也不会拿去查释义或翻译。」回车那半句逐个宿主核对过：macOS（`InputController.mm` 的 Return 分支）、Windows（`KeyEvent.h` 的 `0x0D`）、IBus（`ClientEngine.cpp`）和 Fcitx5（`FcitxEngine.cpp`）在 `/` 指令里都发 `MSIME_COMMIT_RAW`，只有韩文汉字表、注音表打开时改发选中命令，上屏的是原文。「点到别的窗口」只说插件行不会输入，不说输入原文：macOS 失焦走命令 20 上屏 `/uuid`，Windows Server 和 Linux 的 `Runtime::focus` 在本地模式里丢弃组字，文档里留下什么看框架（见「后果」里失焦那一条）。用键盘切窗口不写进这句：macOS 上 Cmd+反引号这类组合键如果交给了输入法，会走下面的组合键路径，没在设备上核实。快捷键那半句写成「在有的系统上」，因为各宿主不一致：macOS 的 Cmd、Ctrl、Option 组合键（`InputController.mm` 组合键分支）、候选窗不可见时的 Tab，Fcitx5 没被前面处理的非字符键（`FcitxEngine.cpp` 按键处理的末尾），以及 macOS 切方案、切英文、开表情面板、开始语音这些菜单和工具栏动作都发 `MSIME_FINISH_COMPOSITION`，上屏高亮行；Windows Server 的 `translate_key` 对组合键发 `MSIME_CANCEL`。这些都是用户自己的操作，和标点同一类；早先的文案只列了序号、点选、空格、标点，漏了它们（分支 B 的复审意见 1）。钩子的白话名是「你在 / 指令里输入它的触发词时（给出候选）」。共享页的 `ScriptCapability` 和 Windows 的 `script_capabilities` 各多一项；没有这些，导入一个申请了 `provide_commands` 的包会让共享授权页取文案时抛错，Windows 页把它当成不认识的能力丢掉。脚本包摘要的 `commands` 在 TypeScript 里和指令表共用一个字段，类型是 `(PluginCommand | ScriptCommand)[]`。
- **数据文件不做**：设计里给拆字准备的 `[script].data`、MSTB 和三个查表 import，唯一的消费者改成了内置模式，见 [脚本插件的只读数据文件](../../rejected/architecture/2026-10-13-script-plugin-data-files.md)。

### 投递（分支 C）

- **交给引擎的是原样列表**：`ScriptSet::refresh` 用 `client-services::plugins::script_commands::granted_commands_of` 读每个启用、用户授予了 `provide_commands`、清单也申请了它的插件的清单（`load_package_unread`：清单和模块文件的大小，不读模块、不算哈希），把指令记在计划条目上。授权戳变了时整份重读；授权戳没变时逐个看清单的戳，变了的插件重算指令（模块还没拿到时照旧重新登记编译）。只跟授权戳不够：同哈希重新导入时授权文件先写、包目录后换，中间有一瞬包目录不在，这一瞬聚焦会读到空指令，之后授权文件不再变，触发词在这个进程里就再也不交给引擎，模块却照样恢复（复审意见 1）。`ScriptSet::plugin_commands` 按执行顺序把它们排开，`HostSession::refresh_plugin_commands` 在它和 `options.plugin_commands` 不同时经 `Runtime::set_plugin_commands` 交给引擎并记进 `options`（重建的引擎从这里开始），换表失败只报告、不让聚焦失败，规则同 [插件表刷新从不让聚焦失败](../bug-fix/2026-10-11-plugin-table-refresh-never-fails-focus.md)。调用处是聚焦时 `refresh_plugin_tables` 的最后一步，和 `msime_client_set_lifecycle` 刚挂上脚本集合时。没有选择加入生命周期的会话（移动端、设置应用、MCP）、进程开关关着、没有插件根目录时是空表，`/uuid` 只是一段普通的输入。
- **谁生效只由引擎决定一次**：用户指令表赢、插件先到先得的过滤只在 `usable_plugin_commands` 里做；用户指令表变了（聚焦时换表、偏好应用时重建引擎）引擎当场重新过滤，host-api 不跟着重算。client-core 的 `script_commands::resolve` 用同一套规则只给授权页算状态（`ScriptGrantsView::commands`，每条 `active`、`shadowed_by_user_table` 带冲突的触发词、`shadowed_by_plugin` 带冲突的插件和触发词）。client-core 不能依赖引擎，两份写法由 host-api 的 `the_grant_page_and_the_engine_resolve_plugin_commands_alike` 拿 2000 组随机输入对齐（触发词只用 `a`、`b`、`r`、`q`，相等、互为前缀、单字母和撞上内置 `/rq` 的情况都很密）；引擎为此在 `msime_engine::host` 公开了 `usable_command_table`、`usable_plugin_commands`。授权页的用户指令表是偏好里启用的指令表包（`script_commands::user_triggers`，和输入法交给引擎的同一份；桌面外壳读它管着的 `PreferencesStore`，`msime_client_plugins` 读 `state_root` 里的偏好）；`/` 关着时引擎拿到的是空表，授权页仍按启用的表说明，打开之后就是那样。
- **触发词的主人**是计划里第一个声明它的插件。引擎在同一个触发词重复时只留第一条（标题都过了清单校验，不会因为标题被丢），后面同名的那条在引擎里也被前一条挡住，所以引擎问到的触发词和 host-api 找到的主人是同一个插件。别的插件一律不投递。
- **运行条件**同其余钩子（`prepare`）：普通字段、属性已确认、进程开关开着、计划不空。`COMMIT_OBSERVE_ONLY` 时 runtime 暂停插件变换，提供者也不问。不满足时是 `NotRun`，引擎不写缓存，列表里只有原样的兜底行。
- **事件**：`Event::new(Hook::Command)`，标量只填本地模式 `command`，`CommandEvent` 带触发词、参数和这一次的 64 字节随机数（`script_commands::random_bytes`，即 `getrandom`；取不到时 `NotRun`，不拿全 0 顶替）。经 `deliver_one` 投递：查缓存拿模块、需要时实例化、在快照上运行、用 `command` 自己那一份轮配额、记健康。
- **结果**：正常返回是 `Ran`，`Actions::rows` 交给引擎。trap、fuel 耗尽、返回负数也是 `Ran`，只是 0 行：插件运行过了，引擎把这份预编辑串缓存成空表，同一份预编辑串上的下一次派发不再投递，免得一个出错的插件在每次派发里重跑、很快熔断；健康计数和熔断规则照旧。没运行（没有主人、模块还在编译或包对不上哈希、已经熔断、推迟实例化、本轮配额用完、宿主一侧 panic）是 `NotRun`，下一次派发再问。宿主一侧 panic 照旧关掉本进程的全部脚本。
- **模块未就绪**：触发词取自清单，不等模块编译好（审查意见 m2），刚启用的插件在这次聚焦就占住触发词；第一次打 `/uuid` 时模块可能还在后台编译，这时只有兜底行，模块好了之后同一份预编辑串的下一次派发就出行。
- **授权页**：共享页和 Windows 原生页在提供指令的插件详情里多一组「提供的 / 指令」，每条「/uuid UUID」一行，右边是状态：「生效」「与你的指令 /uu 冲突，未生效」「与插件 xx 的 /uui 冲突，未生效」（插件用名字），没启用时「授权并启用后生效」，启用了但没授予这项能力时「没有授予「提供 / 指令」，不会生效」，被输入法停用时照实说，宿主没列出（超过 8 个、清单载不入）时「未生效」。授权对话框列出这些指令，调整授权时带上状态。`/` 指令关着时这一组写「需要先打开 / 指令，并使用中文标点；在全拼、双拼、五笔下没有输入时按 / 再输入触发词。」并给「打开 / 指令」按钮，点了才调 `onLocalMode("command")`（Windows 是 `enable_local_mode(L"command")`），授权本身从不改偏好。两处文案逐字相同（`SCRIPT_COMMAND_MODE_NOTE`、`script_command_mode_note`）。
- **作者文档**：`docs/plugins.md` 的能力表、钩子表、限制表、隐私规则、失败、`msime-pack run` 的夹具表，和新的「/ 指令」一节（触发词规则与冲突、投递时机、事件和行的规则、上屏与排除项、`/` 默认关和一键打开、残余风险）。能力文案和两处设置页一字不差。

## Alternatives considered

- **插件行插进普通组字的主候选窗**（设计里的方案 B：错音提示、行内拆字）。最强的理由是用户价值更高，不用先进 `/`。不用：全拼、双拼、五笔下 `READING` 是空串，插件拿不到读音；插行要同时改 `mixed_candidates` 和 `ranking_candidates`，否则 `ranking_index` 回退成显示下标、`learn_candidate` 会调到另一个词库词的词频；每个按键都要多跑一次脚本、多取一次快照，时延没量过；五笔四码唯一的判断看的是整个列表的长度。`/` 模式复用 `/fy` 的现成路径，只有本地模式受影响。
- **runtime 在 `set_plugin_commands` 里自己记一张（触发词 → 标题）表**（最初的实现）。最强的理由是 `InputEngine` 不用多一个方法，画注释时也不用跨进引擎。不用：那张表从宿主交来的原表直接收集，重复的触发词后写覆盖前写，标题被引擎丢掉的那条也收进来，而引擎是先到先得，`[{uuid,UUID},{uuid,EVIL}]` 时引擎把 `/uuid` 交给第一条，行旁却画「EVIL」；照着引擎的规则再过滤一遍又得跟着用户指令表变，等于第二份过滤。
- **复用来源 `Generated`（8）**。最强的理由是不用改线缆上的来源编号，宿主一行不改。不用：runtime 只能按选中行的来源区分上屏，复用 8 就分不出哪一次上屏来自插件，UUID 会被喂进 AI 上文，也分不出该不该转全角、该不该请求释义。
- **只记「这份预编辑串已经问过」的标记，不缓存行**。最强的理由是状态最少。不用：`update_local_candidates` 每次都从头重建列表，标记还在、行已经没了，又不会再问，插件行就此消失。
- **缓存多槽，或者只在发出新查询时才替换缓存**。最强的理由是 `/uuid` 退一个字母再打回来时不用再跑插件。不用：那样同一份预编辑串永远是同一个 UUID，和「删掉重打换一个」矛盾；一槽、预编辑串一变就清掉，语义最简单。
- **失焦时也提交高亮的插件行**（和 `Finish` 完全一样）。最强的理由是 macOS 的命令 20 少一个分支，行为和 `/rq` 一致。不用：命令 20 是宿主替用户结束组字，用户这时没在看候选窗；插件行要靠用户自己选中、或者自己按下结束键才进文档。
- **只剔除在线释义、保留离线释义**。最强的理由是离线释义不出本机。不用：页面和回答按下标对齐，保留离线释义要给每一行加「可以离开本机」的标志并改三个宿主的释义路径；插件行没有释义价值，整页不请求更简单。
- **最长匹配，允许插件触发词之间互为前缀**。最强的理由是插件作者有更多触发词可选。不用：`czh` 会截走 `cz` 的输入，后装的插件能抢走先装插件的用户输入；禁止互为前缀之后至多一个匹配，谁拥有输入一目了然。
- **把 `/fy` 的 `'` 分隔推广到插件触发词**。最强的理由是插件可以收多个单词。不用：唯一需要它的是拆字，而拆字已经做成内置的 C 模式（见 [内置拆字反查](../feature/2026-10-13-builtin-chaizi-mode.md)）；插件参数只收小写字母。
- **`CapabilitySet` 先从 u8 扩成 u16**。最强的理由是第 8 项能力正好占满 bit 7，再加一项时 `1 << 8` 会出错。不用：现在只需要 8 项，授权文件存的是能力名，以后扩不影响格式；分支 B 加一条 `Capability::ALL.len() <= 8` 的编译期断言，第 9 项加入时编译失败、在那时再扩。
- **`command` 跑在持久实例上，再加一对互斥**（设计原来的写法，审查意见 B1 的另一种修法：申请 `provide_commands` 就不能申请任何读文本的能力）。最强的理由是不用每次复位（dev 构建实测 53–343 µs），插件还能在两次 `command` 之间缓存算好的表。不用：互斥要在 `check_rules` 和三处界面加第二对、改掉单对的写法；而且即使不读别的文本，持久实例也能记住同一个字段里用户先前打过的参数，再夹带进后来的行。快照让「输出只是这一次输入的函数」按构造成立，复位的代价远低于 4 ms 的慢熔断线。
- **在清单解析里先拒绝 `provide_commands`，只在测试特性下放开**（审查意见 M3 的另一种修法）。最强的理由是分支 B 不碰共享页和 Windows 页，改动面更小。不用：测试特性下放开意味着 `msime-pack validate` 和导入按两套规则解析，作者工具接受的包产品拒收；文案反正要写，放在 B 里让设置页从第一天就认得这项能力。
- **参数超长时给 `Outcome` 加一个变体**。最强的理由是语义比借用 `NotSubscribed` 准确。不用：`Outcome` 被 host-api 和 `msime-pack run` 穷举匹配，宿主对这种投递只需要知道「没运行、没花 fuel」，`NotSubscribed` 就是这个意思；宿主本来也不该这样投递。
- **`random_fill` 归到 `provide_commands` 名下**。最强的理由是每个 import 都对应一项能力，更像最小权限。不用：随机数和用户输入无关，又只在 `command` 里有内容，而声明 `command` 钩子必须申请 `provide_commands`，归过去拦不住任何多出来的东西。
- **用 `unicode-general-category` 拦连续的组合附加符号（Mn/Me 超过 2 个）**（设计 5.2）。最强的理由是能挡住把一个字叠得很高、盖住相邻行的文本。不用：它对 script-runtime 是一条新的依赖边，没有获批；`MAX_COMMAND_ROW_UTF16 = 64` 的硬上限已经限制了能叠多少，获批后在 `valid_command_row_text` 里加一条规则即可。
- **host-api 用 `resolve` 算好生效的表再交给引擎**（计划原来的写法）。最强的理由是引擎拿到的就是授权页上显示生效的那几条。不用：用户指令表在偏好应用、重建引擎时也会变，host-api 得在 `apply_pending` 里再重算一次才跟得上，而引擎每次换表本来就重新过滤；两边规则相同，交原样列表只让引擎这一处决定，授权页的状态由一致性测试守着。
- **在 `plugin_tables.rs` 的戳里再加上授权文件**（计划原来的写法）。最强的理由是和指令表、短语表同一种结构。不用：`ScriptSet` 已经用同一个 `key_sound::stamp` 按戳重读授权，再记一份戳就是同一件事的两份状态；`refresh_plugin_commands` 只在列表变了时才交给引擎。
- **触发词等模块编译好才生效**（设计原文）。最强的理由是生效的触发词一定有能运行的模块。不用：编译在后台，第一次聚焦时总还没好，刚启用的插件要再换一次输入框才有触发词（审查意见 m2）；先占住触发词、没就绪时给兜底行不写缓存，用户只是第一次没有行。
- **插件失败时按 `NotRun` 处理，不缓存**。最强的理由是失败可能是偶发的，下一次派发能补上行。不用：同一份预编辑串上的派发（修饰键、刷新）每次都会再跑一遍出错的插件，三次就熔断；失败是插件自己的，改一个字母就会重新问。
- **被熔断或模块不合规而停用的插件让出触发词**（在 `ScriptSet::plugin_commands` 和 `granted_commands` 里跳过 `health.disabled()` 的条目）。最强的理由是排在后面、声明了同一个触发词的插件不会被一个已经不运行的插件挡住，用户打 `/uuid` 还能有行。不用：触发词归谁是用户排执行顺序时做的选择，停用之后悄悄改投给后面的插件，等于把用户在触发词后面打的字母交给一个他没选来处理这个触发词的插件，而且这一切发生在一次输入里、用户看不见；停用状态在两次聚焦之间随时会变，引擎的表和授权页的状态都得跟着它变。现状是停用的插件照旧占着触发词，`/uuid` 只有兜底行，到授权变化或重启为止；授权页对它说「插件已被输入法停用，不会给出候选」、对后面的插件说「与插件 xx 的 /uuid 冲突，未生效」，和实际一致，`docs/plugins.md` 的「和别的指令冲突」写明了这一点（复审意见 3）。
- **随机数取不到时照样投递全 0**。最强的理由是插件总能运行。不用：UUID 这类输出会变成可预测的固定值，用户看不出来；系统随机数源不可用时不投递，列表里是原样的兜底行。
- **授权页从输入法进程拿生效的触发词**（例如健康文件旁再写一份）。最强的理由是显示的就是输入法实际用的那一份。不用：要给输入法加一个写文件的出口，而状态只取决于授权、清单和偏好里的指令表，设置进程自己就读得到；两边规则相同由测试守着。
- **宿主的注释过滤跟着追加新的禁用字符**。见 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md) 里禁用字符那一段：宿主那一道只防断行和方向翻转，runtime 是插件文本的唯一权威。

## Consequences

- **收益**：插件可以在自己的 `/` 指令里给出能选中的行，选中的就是显示的那一行；插件行不学习、不计统计、不进 AI 上文、不请求释义和翻译、不转全角，失焦时不替用户上屏；旁路调用不运行脚本；没有插件指令时引擎和 runtime 的行为不变（有一条测试逐个比较视图）。
- **代价**：`/` 模式多一份缓存和一次匹配；每份预编辑串第一次出现时同步跑一次插件，耗时由 fuel 上限兜住，真机时延要在投递接上之后量。
- **只在 macOS 上验证过失焦路径的来源**：命令 20 只有 macOS 发（`commitComposition:`）。Windows Server 和 Linux 失焦时调 `msime_client_focus(false)`，`Runtime::focus` 在本地模式里丢弃组字、不提交任何行，所以插件行不会被替用户上屏；文档里最后留下什么由各自的框架决定。IBus 对不属于 `commits_on_blur` 的组字用 `IBUS_ENGINE_PREEDIT_CLEAR` 画预编辑（`platforms/linux/src/core/ClientEngine.cpp` 的 `render`），失焦时清掉，什么都不写；Fcitx5 的客户端预编辑是用户打的 `/xxx`，失焦时写不写进文档看 Fcitx5 和客户端，Windows 看 TSF 结束组字时组字范围里的文字，这两处都没有在设备上核实。Fcitx5 在快捷键等路径上发 `MSIME_FINISH_COMPOSITION`，提交的是高亮行，那属于用户主动按键，和标点同一类。
- **触发词规则有两份**：引擎一份，client-core 一份（client-core 不能依赖引擎，见 `docs/ARCHITECTURE.md`）。内置触发词和标题由分支 B 的一致性测试守着；长度和字符的规则两边各写一份常量，分支 C 的有效触发词计算还要再对一次。
- **每个插件每轮最多用三份 fuel 之和**（其余钩子 125 000、`before_commit` 100 000、`command` 100 000）。三份互不挤占，代价是一个插件一轮的最坏工作量比 v2 多了 100 000。
- **B 单独合入时是惰性的**：能导入、能授权、设置页认得新能力，但输入法不投递 `command`，授权了也不出候选，PR 正文要写明。
- **投递（分支 C）的收益和代价**：插件只在自己的 `/` 指令里、只给触发词的主人投递，设置页说明每条指令为什么生效或没生效，`/` 关着时一键打开。代价是授权戳变了的那次聚焦要多读几份清单（每个提供指令的插件一个小文件）；授权页每次读授权都读一次偏好和启用的指令表包；规则仍有两份，靠一致性测试守。包被绕过导入路径换掉、和授权的哈希对不上时，触发词照样占着（取自清单），但模块永远编不出来，`/uuid` 只有兜底行。
- **AI 上文在 host-api 层没有端到端断言**：runtime 不公开 `ai_context`，host-api 的端到端测试只断言上屏的就是显示的插件行和 `commit_context`；「插件行不进 AI 上文」由 input-runtime 的测试守着。
- **重访信号**：真要做「普通组字里插行」（错音、行内拆字）时，上面第一条备选的几个前提都要先解决；需要第 9 项能力时扩 `CapabilitySet`。

## Verification

- `crates/script-runtime/src/tests.rs`：新增的不可见字符逐个经 `rewrite_commit` 和 `candidates_annotate` 得到 `INVALID`，跟在 emoji 后的 VS16 放行。
- `crates/engine/src/local/command.rs` 的 tests：内置触发词和标题的全集；插件触发词和内置、用户表、前面的插件相等或互为前缀时丢掉，长度、字符、标题不合规时丢掉；参数匹配。
- `crates/engine/src/session/tests.rs`：插件行顶替兜底行，0 行时兜底行还在且不再问；方向键和列表重建不丢、不再问，退一个字母再打回来重新问；换插件指令、换指令表清掉缓存，用户表有 `uu` 时 `uuid` 不生效；过期查询被拒；其他指令照旧带标题、从不问插件；开着个人上下文和词频调整时选中插件行只上屏这一行、不带诊断，用户日志行数和词库文件字节不变，不能置顶固定删除；插件行误进 `is_dictionary` 时这条会红（调词频在 `/` 模式下没有键，带回诊断），误进 `is_sentence_learning` 时行为上看不出来（`can_learn_sentence_candidate` 在本地模式一律拒绝），只由谓词单测守着。`types.rs`：`from_u8(14)`、往返覆盖 0–255、不属于任何谓词。`host/tests.rs`：`candidate_gloss_key(_, 14)` 为 `None`。
- `crates/input-runtime/src/tests/plugin_commands.rs`（桌面 cfg，假的提供者）：有插件指令、没有提供者时 `/` 模式的视图和没有插件指令时逐个相同；打完触发词的那次派发里写行，代次只前进一次，ID 可选，引擎注释为空，插件注释是「标题 注释」；表里触发词重复、或前一条标题含控制字符时，行旁的标题是引擎留下的那一条；方向键、翻页、`all_candidates`、`apply_translations`、设置类调用不问；`NotRun` 下次派发再问，0 行不再问；暂停时不问；插件行上屏不进 AI 上文而 `/rq` 照进；注释截到 32 个码元且不切开代理对；`FinishOnFocusLoss` 在高亮插件行时上屏 `/uuid`，`Finish` 和标点上屏高亮行，高亮的不是插件行时和 `Finish` 相同。`tests.rs` 的选词路由表加上 `FinishOnFocusLoss`。
- `crates/host-api/src/tests.rs`：页上有来源 14 时 `msime_client_translation_query` 为 `null`，在线翻译、英文释义、离线释义三种开法都一样，`/rq` 作对照照常请求；全角开着时插件行上屏半角、`plugin_row` 不出现在 JSON 里，同一会话的 `/rq` 照旧转全角。`tests/lifecycle_traces.rs`：命令 20 在插件行高亮时上屏 `/uuid`、记 `commit{FocusLoss}`，命令 9 上屏插件行。`lifecycle/tests.rs`：`FinishOnFocusLoss` 的类别。
- `client-core` 的 `translation` 测试：`source_gets_no_gloss(14)`。
- Windows `tests/input/candidate_completion_policy.cpp` 加 14，只经 CI 的 MinGW 交叉构建编译运行。
- 分支 B，`crates/script-runtime/src/tests.rs`：能力和钩子的名字、编号、`ALL` 的长度和三个 import 的归属；`command_row` 没申请能力不能导入、`random_fill` 不需要能力；没授予时两个指令类 import 是 `DENIED`、别的钩子里 `command_text` 是 `ABSENT`、`command_row` 是 `DENIED`、`random_fill` 是 `LIMIT`（取 0 字节是 0）；触发词和参数的读取、错字段、放不下；`command` 事件里宿主误填的上屏和组字读不到，误填的九个标量都是 `ABSENT`（同一实例在 `commit` 里读得到，作对照）；`input` 事件误带 `CommandEvent` 时结果和不带时一样；没有 `CommandEvent`、参数或触发词超长时不运行；行的各条规则（空串、首尾空白、全角空格、换行、零宽字符、新增的不可见字符、非 UTF-8、`://` 等片段、64 和 65 个码元、按码元算的 emoji、注释的 32 和 33 个码元与禁用字符）；第 10 行 `LIMIT`、行号依次返回；返回负数、trap、越界时暂存的行作废，持久实例照样可用；`random_fill` 按顺序取、不够时不写、每次事件重新给满；B1：三次投递给出同一行，在 `commit` 里存下的上屏文本进不了之后的行（把 `deliver` 里的快照条件临时改回只有 `before_commit`，这条和另外四条测试变红，行从 `2` 变成 `4`，已还原）；同一轮里其余钩子的配额烧到见底后 `command` 仍有完整的 100 000；被本轮配额截断的判定对 `command` 同样成立。
- 分支 B，`crates/client-services/src/plugins/script_tests.rs`：两条指令的包摘要和 JSON；内置触发词相等或互为前缀（`rq`、`fan`、`riqix`、`dates`，以及每个内置触发词本身和加一个字母）、清单内重复和互为前缀、单字母、17 个字母、大写、带数字、标题 9 个字、空标题、等于内置标题（全部五个）、首尾空白、零宽字符、多余的键、缺标题、空表、5 条、不是表的数组、有指令缺能力、有能力缺钩子、有能力没指令、有钩子没指令，各自给出中文原因；没有 `commands` 的旧包摘要里没有这个键，内容哈希等于固定值（另用 Python 按公式独立算过一遍，相同）。`crates/host-api/src/tests/script_plugins.rs`：内置触发词和标题与引擎一致。
- 分支 B，`crates/pack-tool/tests/run.rs`：WAT 写的指令插件的金样（授予时的 `rows` 和随机数换成的字母、不授予时 `denied` 和空 `rows`，其余事件没有 `rows`）；夹具里 `command` 缺触发词、别的事件带触发词、大写触发词、参数带数字、`random` 长度不对、`local_mode` 不是 `command` 的报错；经命令行：`validate` 接受带指令的包、拒绝和 `/fy` 冲突的触发词，`run` 拒绝清单没声明的触发词。`tests/sdk.rs` 的两条一致性测试覆盖新的常量和 import。
- 分支 B，界面：`apps/desktop/tests/settings/script-plugins.test.tsx` 加两条（授权对话框里「提供 / 指令」能勾、没有依赖和互斥提示、交出去的是 `["provide_commands"]`；详情页列出这项能力和 `command` 钩子的白话名），`pnpm --filter @msime/desktop typecheck` 通过。Windows `tests/ui/plugin_page_model.cpp` 加第 8 项能力的排序、依赖、互斥、勾选、文案和钩子名，用 Studio 上的 `clang++ -std=c++17 -Wall -Wextra -Werror` 编过并运行通过；MinGW 交叉构建只在 CI 上跑。

- 分支 C，`client-services::plugins::script_commands` 的 tests：用户指令表赢、插件先到先得（比的是排在前面已经生效的，被用户表挡住的不挡后面的插件）、相等和两个方向的前缀、用户表里引擎不用的触发词不挡、不合规和撞内置的不列出、状态的 JSON 形状、随机数填满。`plugins/script_tests.rs`：只算启用、授予了 `provide_commands`、清单能载入的插件（模块文件被删的不算），用户指令表包里的 `si` 挡住 `sig`、不启用时 `sig` 生效，只算执行顺序上的前 8 个。
- 分支 C，`crates/host-api/src/tests/script_plugins.rs`（经 FFI，WAT 插件放进进程缓存，清单写在插件目录里）：打完 `/uuid` 出两行（「触发词-参数」带注释、8 个随机字母），插件注释是「UUID one」、引擎注释为空，打 `/uui` 时不投递；移动高亮、翻页和 `reset_cache` 都不再投递；改一个字母再改回来重新投递、换了随机数；选中第 1 行上屏的就是它、`commit_context.local_mode` 是 `command`；参数每打一个字母投递一次。SECURE 字段和 `COMMIT_OBSERVE_ONLY` 下只有兜底行、一次都不投递，换回来就有。没授予 `provide_commands` 的不投递；两个插件同触发词时归执行顺序在前的，换顺序之后归另一个，停用之后它的触发词不再生效。用户指令表有 `uu` 时 `/uuid` 不归插件，同一个插件不冲突的 `/guid` 照常。模块还在编译时只有兜底行，模块就绪之后同一份预编辑串的下一次派发（`reset_cache`）就出行。trap 和死循环各三次：没有行，失败计数逐次加一，同一份预编辑串上的下一次派发不再投递，第三次熔断、之后不再投递。页上有真实插件行时释义请求为 `null`，命令 20 上屏 `/uuid`、命令 9 上屏插件行。`msime_client_plugins` 的授权回答列出每条指令的状态，偏好里启用了有 `si` 的指令表后 `sig` 归用户。两份规则对 2000 组随机输入给出相同的生效表。授权文件不变、包目录先不在后回来时，下一次聚焦 `/uuid` 就出行；先聚焦、后选择加入生命周期时，选择加入那一刻就把指令交给引擎；脚本集合正被借着时刷新，`options.plugin_commands` 不变、之后 `/uuid` 照样出行。变异检查：把主人改成计划里最后一个声明的插件、把失败改成 `NotRun`，各有一条测试变红；去掉清单戳变化时的指令重算、去掉 `msime_client_set_lifecycle` 里的 `refresh_plugin_commands`、把借用时的 `None` 改成空表，后加的三条各自变红（都已还原）。
- 分支 C，界面：`apps/desktop/tests/settings/script-plugins.test.tsx` 加三条（状态和两种冲突的说法、没授权和宿主没列出时的说法、`/` 关着时有说明和按钮且经对话框授权不改偏好、点按钮才调 `onLocalMode("command")`）。Windows `tests/ui/plugin_page_model.cpp` 加状态的各种情况、对话框的行和 `/` 的说明，用 Studio 的 `clang++ -std=c++17 -Wall -Wextra -Werror` 编过并运行通过；`main.cpp` 的画法只做了源码审阅，MinGW 交叉构建在 CI 上。Linux 的 Tauri 外壳（`desktop_plugins.rs` 的授权命令）在 `Dockerfile.desktop-check` 的容器里 `cargo check -p msime-desktop --all-targets` 通过。Linux 原生宿主的 C++ 代码没有改动，`bash platforms/linux/build-container.sh` 在固定容器里构建 IBus 引擎、Fcitx5 插件和单测，ctest 83 项全过；这是容器构建这一级，不是 IBus/Fcitx5 里的装机打字验收。
- 分支 C 没有做的：三个桌面平台的装机打字验收（macOS 安装 `.app`、在 TextEdit 里打 `/<触发词>`）要等分支 D 有了 uuid 插件；Windows 原生页和 MinGW 构建只到源码审阅。

分支 C 的当场分类：[脚本插件在输入法进程里](2026-10-12-script-plugins-in-the-input-method-process.md) 部分重叠，就地补了 `command` 的投递路径和聚焦时把清单里的指令交给引擎；[授权页](2026-10-12-script-grants-settings-page-without-keychain.md) 和 [Windows 授权页](2026-10-12-windows-script-grants-page.md) 部分重叠，就地补了指令列表、状态和按钮；[插件表刷新从不让聚焦失败](../bug-fix/2026-10-11-plugin-table-refresh-never-fails-focus.md) 的规则由 `refresh_plugin_commands` 照搬，无冲突；[指令表去重](../testing/2026-10-08-local-command-table-dedup.md) 讲用户指令表本身，无关。

分支 B 的当场分类：[脚本插件进入插件包体系](2026-10-12-script-plugin-kind-and-grants.md)、[作者工具](../feature/2026-10-12-script-plugin-author-tools.md)、[Windows 授权页](2026-10-12-windows-script-grants-page.md) 部分重叠，各自就地更新了键数、轮配额份数、import 数和能力表的事实并链到这里；[沙箱运行时](2026-10-12-script-runtime-sandbox.md) 和 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md) 部分重叠，就地改了能力数、钩子数、`command_fuel_left`、三份轮配额和 `command` 也跑在快照上的事实并链到这里（最初的分类漏了这两篇，复审时补上）；[脚本插件合集](2026-10-12-script-plugin-collection.md) 讲编进模块的数据和 `check_run.py`，部分重叠（`command` 的 fuel 上限），无冲突；新建的 [脚本插件的只读数据文件](../../rejected/architecture/2026-10-13-script-plugin-data-files.md) 记下没做的那一块。

相关笔记的当场分类：[脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md) 部分重叠（禁用字符的事实在那篇就地更新，互链）；[日文混输候选](../feature/2026-10-12-mixed-japanese-candidates.md) 是新来源值不属于任何学习谓词、跳过繁体转换的先例，无冲突；[内置拆字反查](../feature/2026-10-13-builtin-chaizi-mode.md) 解释了拆字为什么不是插件，无冲突；[插件表刷新从不让聚焦失败](../bug-fix/2026-10-11-plugin-table-refresh-never-fails-focus.md) 的规则由分支 C 下发有效触发词时遵守，现在无关。

后续：第一个真实的指令插件是随桌面安装包分发的 uuid（`plugins/script/uuid/`，`/uuid`、`/guid`），它是 `provide_commands` 和 `random_fill` 的参考样板，见 [官方插件包随三个桌面安装包分发](2026-10-13-official-plugin-packs-in-desktop-installers.md)。

共享脚本模型与授权组件的模块边界见[脚本插件模型与授权对话框](../simplification/2026-10-23-script-plugin-modules.md)，本篇的指令文案和投递规则不变。
