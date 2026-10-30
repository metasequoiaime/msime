# Agent Note: 引擎 C ABI 回放 golden 场景，逐步对照核心 API

Status: implemented

## Problem

`crates/engine-capi`（`msime-engine-capi`）把核心层 `msime_engine::Session` 包成 C 函数，但 msime 自己的六个平台宿主都不链接它，它们用 host-api 的 `msime_client.h`。所以这个 ABI 只有 `src/tests.rs` 的单元测试和 `tests/c-example.sh` 的一个 C 示例在跑：一句全拼、选第一行、读回个人词典。引擎每天在变，C 包装层里有好几处要跟着引擎走的映射，任何一处走偏都不会有测试变红：

- `msime_engine_state` 读的是 `display_snapshot`（全拼 `preedit` 按音节分开），不是参考回放用的 `snapshot`；
- 方案、本地模式、候选来源、命令的编号是 ABI 冻结的常量，C 包装层用 `as u8` 直接转引擎的枚举，引擎枚举一改序号就悄悄换了含义；
- 选项 JSON 逐键映射到 `SessionOptions`，映射错一键，会话照样建得起来；
- 状态与候选借自会话里缓存的快照，缓存只在「改变状态的调用」开头清掉，哪个调用漏了清，C 侧读到的就是旧状态；
- 个人词典三个函数走引擎自己取锁的版本，会话开着时报 `BUSY`。

引擎的 golden 场景（`crates/engine/tests/golden/scenarios`，300 个）已经覆盖了这些路径在核心层上的行为，只是从来没有经过 C 函数。

## Decision

### 测试的位置与运行

`crates/engine-capi/tests/golden_replay.rs` 是这个 crate 的集成测试，经 rlib 直接调用导出的 `extern "C"` 函数。`cargo test -p msime-engine-capi` 就会跑它，所以 CI 不用另接：`ci-platforms.yml` 的 `engine-capi` 作业在 Linux、macOS、Windows 上各跑这条命令，路径过滤 `crates/engine*` 同时覆盖引擎源码、golden 场景和 C 包装层。工作区 `default-members` 也包含这个 crate，裸的 `cargo test` 同样会跑。

### 与 golden 共用同一套回放

测试用 `#[path]` 把 `crates/engine/tests/golden_support/mod.rs` 原样引入，不复制。为此 golden 这边只做了不改行为的两处调整：`golden_dir()` 从编译它的 crate 目录的上一级找 `engine/tests/golden`（两个 crate 都在 `crates/` 下）；`replay_scenario` 拆出 `replay_scenario_observed`，在打开会话后和每一步之后把会话交给一个回调，回放本身和录制文档的形状不变。几个辅助函数和常量（`keeps_page`、`moved_page`、`read_json`、`panic_message`、`root_spellings`、三个语言词库文件名）改成 `pub`。golden_support 自己的单元测试因此也在 engine-capi 的测试二进制里跑一遍。

### 两边怎么跑

- **核心侧**就是 golden 的回放：`replay_scenario_observed` 按 `crates/engine/tests/golden.rs` 的方式暂存夹具、`prepare_runtime_paths`、用 `reference_options()` 加场景选项建会话、执行每一步；回调里读 `display_snapshot` 和 `takes_local_separator`。
- **C 侧**在自己的根目录暂存同一份夹具，用 `msime_engine_prepare_runtime` 准备代次，把同一份 `SessionOptions` 译成版本 1 的选项 JSON 交给 `msime_engine_session_new`，每个 op 都经导出函数执行：`type` 逐字节 `key_char`，`char` 带 `MSIME_ENGINE_KEY_SHIFT_ONLY`，选择类函数带 `state_get` 读到的修订号，开关类经 `set_int`，`set_helpcode_schema` 经 `set_string`（`OK` 记为接受，`FAILED` 与 `INVALID_ARGUMENT` 记为不接受），`reopen` 与 `new_generation` 是释放会话、冲刷学习、重新准备代次再建会话，`dump_journal` 与 `query` 先经 `msime_engine_flush_learning` 再读文件，`dict_list` 经 `user_dict_list`。
- **`dict_edit`** 在 C 侧是释放会话、`user_dict_edit`、重新打开：C 会话持共享租约，维护要独占租约，宿主必须先释放会话（README「生命周期」）。核心侧在会话开着时直接改，因为核心 `Session` 不取租约。
- 两边**先后**跑，不交错。个人学习的延迟写入和连接缓存是进程级的，golden 的 `close` 会冲刷所有会话排着的学习，两个会话交错时一边的冲刷会改变另一边看到的状态。

### 比什么

每一步之后两边都化成 C ABI 能看到的样子再比：`msime_engine_state` 的全部字段（方案、本地模式、四个标志位、光标字节、改字焦点、`preedit`、`editing_text`、`conversion`、`spelling_symbols`、候选数）、每一行 `msime_engine_candidate`（词、编码、注释、纠错原文、来源、方案、固定位置、三个标志位）、这一步的 `msime_engine_result`（按录制文档的形状：`handled`、`commit`、`diagnostic`，路径换成 `$ROOT`），以及场景读出的日志表、SQL 结果和个人词典分页。差异用 golden 的 `diff_documents` 报告，每行带场景名、步号、op、字段路径和两边的值（expected 是核心，got 是 C）。

- 核心侧的方案、本地模式、来源、命令用穷举 `match` 映射到 `MSIME_ENGINE_*` 常量，不做类型转换：引擎给这些枚举加变体时这个测试编译不过，改了序号时 C 侧的 `as u8` 与常量对不上，显示为差异。
- 修订号不跨两边比：C 包装层在调用引擎之前就拒绝不认识的辅助码表名，引擎侧的失败调用却会递增修订号，两边的绝对值合理地不同。C 侧另核对自己的一致性：每个结果的 `revision` 等于随后 `state_get` 的 `revision`，状态与候选的标志位不出 ABI 定义的位。

### 选项与跳过

`SessionOptions` 译成 JSON 时，JSON 里有的键全部写出，不依赖库的缺省值，这样解析器的每一键都被用到。JSON 不承载的字段必须等于 `SessionOptions::new` 的值（C 库从它开始），否则这个场景跳过，原因写成第一个不同的顶层字段名（从两份 `{:#?}` 输出里读出，引擎以后加的字段不用在测试里登记）。没有 C 函数的 op、C 函数按约定拒绝的参数（`key_char` 只收可打印 ASCII 和 Tab，`key_punct` 只收 ASCII 标点）同样跳过并记原因。跳过从不静默：每次运行打印「回放了 N 个、跳过 K 个」和按原因分组的场景名；不带 `MSIME_GOLDEN_SCENARIO` 过滤的完整运行少于 `REPLAYED_FLOOR` 个时失败。这个下限只能随 ABI 覆盖面扩大而上调，不能为了让失败通过而下调。

落地时 300 个场景回放 201 个，下限定为 201。ABI 1.1 补上调频、`expressive`、`local_modes`、`candidate_key`、`validate_entry` 和五笔混输的运行时开关之后（[引擎 C ABI 1.1 补上的缺口](../architecture/2026-10-30-engine-capi-abi-1-1-additions.md)），回放 240 个，下限升到 240。现在跳过的 60 个都是有意不进 ABI 的能力，每个场景只按第一个原因计：

| 原因 | 个数 |
| --- | --- |
| `sentence_alternatives` 为真（C ABI 固定为假） | 38 |
| 在线候选 `apply_online_candidates` | 11 |
| 九键与按键网格（含同时用到 `validate_entry` 的 `personal_dictionary_entries`） | 10 |
| `reset_cache`（只有进程级的 `msime_engine_release_caches`；同一个场景还用到 `balance_paired_punctuation_after_auto_close`） | 1 |

`candidate_key` 经 `msime_engine_candidate_key`、`set_wubi_mixed_pinyin` 经 `MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN`、`validate_entry` 经 `msime_engine_user_dict_validate` 回放；选项 JSON 写出 `frequency`、`expressive` 和除 `command`、`mention` 以外的全部 `local_modes`，这两个模式开着的场景按 `local_modes` 跳过。

### 真实词库

真实词库的四个集合（`golden/real/*.jsonl`，4477 行）只在设了 `MSIME_EVAL_RESOURCES` 时跑，与引擎的 golden 测试用同一个变量，不设时打印跳过原因。选项取 `real::product_options` 的产品取值，再去掉 JSON 不承载的 `sentence_alternatives`（置假）；调频按产品取值（`promote`）一起交给两边。学习关着，所以核心会话和 C 会话共用 C 侧准备的同一个代次，逐行同时打字、比整张候选表，比完各自 `Cancel` 与 `reset_context`。为防资源目录给错时两边都是空表而「一致」，有候选的行不过半就失败。

## Alternatives considered

- **C 侧直接对照 golden 的期望文件，不跑核心侧** — 最强的理由是少跑一遍回放，期望文件本来就是唯一的真值。否掉是因为期望文件记的是 `snapshot()` 的 `preedit` 和一堆 C ABI 不暴露的字段，真实集合按 `sentence_alternatives = true` 录制而 C ABI 做不到；而且核心一旦偏离期望文件，这里也会报错，看起来却像 C 包装层的问题。
- **把 golden_support 复制进 engine-capi 的测试** — 最强的理由是两个 crate 的测试互不牵连，golden 那边改什么都不会碰到这里。否掉是因为夹具暂存、选项映射、日志转储是回放里最容易悄悄改动的部分，两份拷贝会各自漂移，这个测试要防的正是漂移。
- **把 golden_support 抽成工作区里的独立 dev crate** — 最强的理由是共享方式最正规，不靠 `#[path]` 和目录相邻的假设。否掉是因为只为测试代码在工作区里加一个成员、一份清单和发布排除规则，代价大于两处 `pub` 和一个回调。
- **两边会话交错着逐步执行** — 最强的理由是只需在内存里放一步的状态，第一处不同就能停下。否掉是因为个人学习的延迟写入是进程级的，golden 的 `close` 冲刷所有会话，交错时两边互相影响；先后跑再逐步比，差异同样定位到步。真实集合例外，那里学习关着。
- **按 README 按键路由第 4 条在 C 侧模拟 `candidate_key`** — 最强的理由是能多回放 9 个场景。否掉是因为那测的是宿主的路由，不是 API 的等价：数字先交 `key_char`，引擎处理了就不会再选词，两边的结果本来就可以不同。
- **跳过带 `dict_edit` 的场景** — 最强的理由是 C 侧不用模拟「释放、编辑、重开」。否掉是因为那样 `user_dict_edit` 只剩单元测试，而 `personal_dictionary_lifecycle` 正好覆盖了新增、替换、删除、幂等的 `request_id` 和新代次。

## Consequences

- **收益**：C 包装层与引擎之间的每一处映射（快照、编号、选项 JSON、快照缓存、学习、个人词典）都在 201 个场景的每一步上被核对，引擎改动只要让 C ABI 看到的东西不同，三个桌面平台的 CI 就会变红。落地时没有发现差异；把 C 侧的状态改成读 `snapshot()`、把 `learning` 映射取反之后，76 个场景报出差异，报告直接指到 `steps[1] type "nihao": view.preedit: expected "ni'hao", got "nihao"` 这样的位置。真实词库四个集合在 Studio 上 4477 行没有差异，调试构建约 3 分钟。
- **代价与已知上限**：golden_support 的单元测试在两个测试二进制里各跑一次；`golden_dir()` 假设两个 crate 在 `crates/` 下相邻；完整回放在 `engine-capi` 作业的 Linux 和 macOS 上约 7 秒，Windows 上约 2 分钟（CI 实测）。CI 没有真实词库，真实集合的对照只在设了 `MSIME_EVAL_RESOURCES` 的机器上跑。下限要人手上调，ABI 新增选项或函数之后不调，新覆盖的场景退回跳过时测试不会发现。
- **跳过表也是 ABI 的缺口清单**：落地时的调频方式、混排 emoji 与颜文字、本地模式开关、`candidate_key`、`validate_entry` 和五笔混输的运行时开关已在 ABI 1.1 补上并上调了下限；剩下的四类是有意不进 ABI 的能力。以后再补一项，就把它从跳过表里拿掉并上调下限。

## Verification

- `cargo test -p msime-engine-capi --test golden_replay -- --nocapture`：打印覆盖摘要，201 个场景两边一致。
- `MSIME_EVAL_RESOURCES=<词库目录> cargo test -p msime-engine-capi --test golden_replay real_sets -- --nocapture`：四个真实集合逐行一致，每个集合打印行数、有候选的行数和差异数。
- `cargo test -p msime-engine --test golden`：golden 本身不受共用改动影响。

## 相关笔记

按 engine-capi、C ABI、golden、golden_support 检索活跃笔记：[引擎 C ABI 1.1 补上的缺口](../architecture/2026-10-30-engine-capi-abi-1-1-additions.md) 补上了落地时跳过表里的几类缺口，部分重叠，互链；[引擎的 C ABI](../architecture/2026-10-29-engine-capi.md) 是被测的对象，部分重叠，互链，它的「测试与 CI」一节指向本篇；[引擎库的一套默认值、错误码、日志出口与维护锁归属](../architecture/2026-10-29-engine-library-defaults-errors-log-and-lock.md) 定下了核心 `Session` 不取租约、golden 补回辅助码参照值，本篇的 `dict_edit` 处理与 `reference_options` 都建立在它上面，部分重叠；[msime-engine 作为可嵌入的输入引擎库](../../implemented/architecture/2026-10-29-embeddable-engine-library.md) 是 C ABI 的提案，无重叠。其余命中的是 host-api 的 `msime_client.h` 或各自功能的 golden 场景，与本篇无关。没有被本篇吸收或取代的笔记。
