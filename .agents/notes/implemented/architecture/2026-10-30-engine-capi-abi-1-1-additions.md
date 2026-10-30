# Agent Note: 引擎 C ABI 1.1 补上的缺口：调频、混排与本地模式开关、数字选词、词条检查、日志回调

Status: implemented

## Problem

C ABI（`crates/engine-capi`，见 [引擎的 C ABI](2026-10-29-engine-capi.md)）的 golden 回放在 300 个场景里跳过 99 个，跳过表就是 ABI 的缺口清单（[引擎 C ABI 回放 golden 场景](../testing/2026-10-29-engine-capi-golden-replay.md)）。其中几项是第三方前端正常要用、却拿不到的：

- 调频方式不在选项 JSON 里，引擎库的缺省是 `disabled`，采用方根本打不开调频；建好会话之后也没法改。
- 混排 emoji、颜文字、符号（`SessionOptions::expressive`）和各个本地模式的开关（`LocalModeOptions`）不在选项 JSON 里：本地模式按引擎缺省全开（计算、拆字缺省关），前端既关不掉也打不开。
- 引擎自己的数字选词入口 `Session::candidate_key` 和个人词条的检查 `validate_personal_dictionary_entry` 没有 C 函数。
- 引擎的警告日志只能写标准错误。引擎早有可换的出口（`msime_engine::set_log_sink`，见 [引擎库的一套默认值、错误码、日志出口与维护锁归属](2026-10-29-engine-library-defaults-errors-log-and-lock.md)），C 侧没有注册回调的办法。

ABI 1.1（`MSIME_ENGINE_ABI_MINOR = 1`）已经在 develop 上，还没有发布过 1.1 的二进制，所以这些都追加在 1.1 里，不再升次版本。

## Decision

### 收进 1.1 的

- **调频**：选项 JSON 加 `frequency`（`mode` 用名字 `disabled`、`pin`、`halve`、`linear`、`promote`，`trigger_count`、`linear_step` 各 1–10，越界在解析时就报出是哪一键）。运行时经 `msime_engine_set_int` 的三个新编号 `MSIME_ENGINE_OPTION_FREQUENCY_MODE`（11）、`_FREQUENCY_TRIGGER_COUNT`（12）、`_FREQUENCY_LINEAR_STEP`（13），方式的整数取值是新的冻结常量 `MSIME_ENGINE_FREQUENCY_DISABLED` 到 `_PROMOTE`（0–4，按引擎枚举的声明顺序，C 侧用穷举 `match` 映射，不做类型转换）。引擎原来没有运行时设置调频的入口，为此加 `Session::set_frequency_adjustment`（26 键和九宫格一起换，越界按建会话时的检查报 `INVALID_SESSION_OPTIONS`，什么也不改）和读当前值的 `Session::frequency_adjustment`。新方式从下一次选择起生效，眼前的候选不重排，和 `set_first_page_rows` 这类只影响以后按键的设置一样；修订号照样递增。
- **五笔拼音混输的运行时开关** `MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN`（14）：引擎早有 `Session::set_wubi_mixed_pinyin`，组字中切换会重查候选。它和标点、辅助码那几个开关是一类设置，前端的设置页改了不必重建会话。
- **`expressive`**：`emoji_candidates`、`kaomoji_candidates`、`symbol_candidates` 三个都收，缺省沿用引擎的全关。前两个读 `resources/msime-others.db`，第三个的表编进了引擎。
- **`local_modes`**：除了 `/` 指令和 `@` 名单，`LocalModeOptions` 的每个开关都收（`unicode`、`date_time`、`date_time_inline`、`date_time_menu`、`quick_phrase`、`emoji`、`kaomoji`、`super_jianpin`、`temporary_english`、`temporary_japanese`、`expression`、`chaizi`、`symbol`、`shift_letter_entry`），缺省沿用引擎。`command` 和 `mention` 不在 JSON 里，`deny_unknown_fields` 让它们成为 `INVALID_ARGUMENT`，两个模式固定关：`/` 的内置行只有日期时间（T 模式已经有）和要宿主接翻译通道的 `/fy`，`@` 的行要宿主喂名单，这两样都在 README「不在这个 ABI 里」。`expressive` 和 `local_modes` 只在建会话时设：引擎没有它们的运行时入口（`InputSession::set_local_mode_options` 是私有的），前端改设置时重建会话。
- **`msime_engine_candidate_key(session, digit, result)`**：包 `Session::candidate_key`。只收 ASCII 数字；有组字时 `1`–`9` 选整张表的第 `digit - 1` 行（第一页），越南文 VNI 和注音此刻拼写用的数字交给拼写，没有组字时不处理并忘掉上屏的上下文，上屏后联想不接数字。它不带修订号，选的是调用时的候选。它不先试拼写（Unicode、日期、计算模式里也照样选词），所以 README 按键路由第 4 条不变：先 `key_char`，没处理时只显示第一页的前端可以用它代替 `select(result.revision, 数字 - 1)`。msime 自己的宿主不用它（数字经 `character` 加 `select`），它是参照实现的入口，golden 场景驱动它。
- **`msime_engine_user_dict_validate(entry_json, len, &normalized_json)`**：包 `validate_personal_dictionary_entry`，和 `user_dict_edit` 收新词条用同一套规则和同一个 JSON 形状，合法时交回规范化后的词条（编码小写、拼音空格换 `'`、补上缺省权重），不合法时 `INVALID_ARGUMENT`，`last_error` 是引擎的英文原因。不需要路径、不碰文件、会话开着也能调，给编辑框即时提示用。
- **`msime_engine_set_log_callback(callback, user_data)`**，回调类型 `typedef void (*msime_engine_log_callback)(void *user_data, const char *message, size_t message_len)`，NULL 回到标准错误。约定与实现见下一节。

### 日志回调的约定

- C 包装层持一个进程级的 `RwLock<Option<LogTarget>>`（回调加 `user_data`），注册时把引擎的出口换成一个转发函数，NULL 时换回引擎的缺省出口。转发时持读锁调回调，换回调时取写锁：`set_log_callback` 返回时旧回调没有正在执行的调用，以后也不会再被调用，调用方可以立刻释放旧的 `user_data`。引擎的 `warn` 在自己的锁外调出口，换出口时先取 C 侧写锁再换引擎出口，两把锁总按同一顺序，不会互等。
- 一个线程局部标志记着「本线程正在执行回调」：回调里再调 `set_log_callback` 返回 `BUSY`（否则在自己持着的读锁上等写锁会死锁）；回调期间同一线程上再记的日志直接丢掉（不再取读锁，免得写锁在等时读锁重入卡住）。
- 引擎取到转发函数之后、调用之前恰好被换回缺省时，转发函数读到空槽，把这一行按缺省的样子写到标准错误，不丢。
- 交给回调的是 NUL 结尾的 UTF-8（内嵌 NUL 换成 U+FFFD），`message_len` 不含 NUL，只在这次调用期间有效；不带 `msime: ` 前缀和换行。回调可能在任何线程、几个线程上同时被调用，不能抛 C++ 异常或 `longjmp` 出去。不分级别：引擎的每一行都是兜住了的警告。

### 不收的

- `sentence_alternatives` 固定为假：其余读法要前端自己重排和裁剪，这个 ABI 只给引擎原始的顺序（38 个场景因此跳过，这是设计，不是缺口）。
- 在线、云与 AI 候选（11 个场景）、九键与按键网格（10 个）照旧不在 ABI 里，理由见 [引擎的 C ABI](2026-10-29-engine-capi.md) 和 README。
- `balance_paired_punctuation_after_auto_close`（宿主自己补了右括号之后的配对修正）和单个会话的 `reset_cache`：只有一个场景（`qp_initial_expansion_and_toggles`）用到，前者只有替应用自动补右括号的宿主用得上，后者的内存回收由进程级的 `msime_engine_release_caches` 承担。ABI 只追加不撤回，用得上的采用方出现之前不加。
- 候选混输总开关 `SessionOptions::mixed_candidates`：各分项（`english.mixed_candidates`、`expressive.*`、`local_modes.date_time_inline`）都能单独关，总开关只是一次关掉全部的便利。

### 数据文件

README「要带的数据文件」逐行写明每个新开关读什么：emoji 与颜文字（混排和 E、M 模式）读 `resources/msime-others.db`，它不进代次；msime-dictionary 发布的这份里 emoji 数据与颜文字的来源没有记录（`docs/data-formats.md` §21），所以 README 要求再分发前另行确认，或按 §12 的格式用自己有权分发的数据生成。快捷短语（K）、超级简拼（J）读代次里的 `msime-pinyin.db`，临时英文（Y）读 `msime-english.db`，临时日文（R）要 `enabled_schemes` 里有日文和 `msime-japanese.dat`。Unicode、日期时间、计算、拆字、符号的表和规则编进了引擎，许可证随引擎 README 的「许可证」一节走，二进制归档已经带着。

### 回放覆盖

golden 回放补上 `candidate_key`、`set_wubi_mixed_pinyin`、`validate_entry` 三个 op 和选项 JSON 的三个新键之后，300 个场景回放 240 个、两边一致，`REPLAYED_FLOOR` 从 201 升到 240。`personal_dictionary_entries` 虽然有了 `validate_entry`，仍因九键的 op 跳过，`validate_entry` 由单元测试覆盖。真实词库四个集合不再把调频改回缺省，按产品取值（`promote`）回放。

## Alternatives considered

- **日志回调带级别参数（`void (*)(void *, uint32_t level, const char *, size_t)`）** — 最强的理由是 ABI 冻结之后改不了签名，现在带上级别，以后引擎有了别的级别也不必再加一个注册函数。没有采用：引擎的出口不分级别，每一行都是兜住了的警告，现在的级别只能是一个常量，采用方写不出能测到的分支；真有级别时追加一个新的注册函数，旧调用方不受影响，与只追加的规则一致。上面那篇日志出口笔记也把签名留到有级别时再定。
- **回调直接装进引擎的 `set_log_sink`，不在 C 侧另持锁** — 最强的理由是少一层锁和一个线程局部标志，引擎本来就在锁外调出口。没有采用：引擎先克隆出口再在锁外调用，换出口返回时旧出口可能还在别的线程上执行，C 调用方就不能在 `set_log_callback` 返回后释放 `user_data`，只能靠文档里一句「再等一会儿」。C 采用方最常见的写法正是换掉回调后马上释放上下文，生命周期必须由库给出确定的保证。
- **不加 `candidate_key`，只在 README 里讲 `key_char` 加 `select`** — 最强的理由是 msime 自己的宿主不用它，它也替代不了按键路由第 4 条（不先试拼写），多一个入口可能让采用方误以为数字只交它就够了。收进来是因为它是引擎唯一一处把 VNI、注音的拼写数字、空闲时数字打断上屏上下文、联想不接数字这几条规则放在一起的入口，只显示第一页的前端用它更不容易写错；文档写明它不先试拼写，路由规则不变。
- **调频只进选项 JSON，不做运行时选项** — 最强的理由是不用为此给引擎加公开方法。没有采用：调频是设置页上的常见开关，其余同类开关都能运行时改，只有它要重建会话会让前端为一个设置丢掉组字和联想状态。
- **`local_modes` 也收 `command` 和 `mention`** — 最强的理由是 `LocalModeOptions` 原样映射，JSON 与引擎结构一一对应最好维护。没有采用：这两个模式的行要宿主喂指令表、名单和翻译通道，ABI 不收这些表，打开后只剩日期时间的重复行和兜底行，`/`、`@` 两个键还会从直接上屏的字符变成进入模式，对采用方只有坏处。

## Consequences

- **收益**：第三方前端能打开并在会话里切换调频，能按自己的产品决定混排和本地模式，能在编辑框里即时检查个人词条，能把引擎的警告接进自己的日志系统并在换掉回调后安全释放上下文。C ABI 回放从 201 个场景升到 240 个，剩下的 60 个跳过都是有意不进 ABI 的能力。
- **代价**：引擎多两个公开方法（`set_frequency_adjustment`、`frequency_adjustment`），C 侧多三个函数、四个选项编号、五个调频常量和一个回调类型，都是 1.1 的冻结面。日志转发每行多一次 `CString` 分配和一次读锁，日志只在异常路径上记，代价可以忽略。`expressive` 和 `local_modes` 只能建会话时设。`msime-others.db` 的来源没有核实，emoji 与颜文字的混排和模式要采用方自己解决数据。
- **没做的**：事件通知、日志级别；`balance_paired_punctuation_after_auto_close` 与 `reset_cache`；九键、在线候选和整句的其余读法照旧不在 ABI 里。

## Verification

- `cargo test -p msime-engine-capi`：新增五个单元测试覆盖选项 JSON 的新键（拒绝越界与 `command`、`mention`，本地模式开关决定 Shift 加字母进不进模式）、调频的建会话选项与运行时选项、`candidate_key`、`user_dict_validate`、日志回调（收到不带前缀的行、回调里换回调报 `BUSY`、换回 NULL 之后不再收到）；golden 回放 240 个场景两边一致。
- `cargo test -p msime-engine --lib -- frequency`：`the_frequency_mode_changes_without_a_new_session` 核对引擎的运行时调频。
- `bash crates/engine-capi/tests/c-example.sh`：头文件没有漂移，三种链接方式的 C 示例照常运行。

## 相关笔记

按 engine-capi、C ABI、set_log_sink、日志出口、调频、candidate_key、validate_entry 检索活跃笔记：[引擎的 C ABI](2026-10-29-engine-capi.md) 是本篇扩展的对象，部分重叠，互链；[引擎 C ABI 回放 golden 场景](../testing/2026-10-29-engine-capi-golden-replay.md) 的跳过表与回放下限随本篇更新，部分重叠，互链；[引擎库的一套默认值、错误码、日志出口与维护锁归属](2026-10-29-engine-library-defaults-errors-log-and-lock.md) 定下了 `set_log_sink` 并把 C 回调签名留到以后，本篇回答了它，部分重叠，互链。其余命中的是 host-api 的 `msime_client.h` 与各平台的调频设置，与本篇无关。没有被本篇吸收或取代的笔记。
