# Agent Note: msime 自己的产品 crate 只经文档化的引擎 API 使用引擎

Status: implemented

## Problem

可嵌入引擎库的提案（`implemented/architecture/2026-10-29-embeddable-engine-library.md`，PR #10167）把 P7 定为「msime 宿主自用」：input-runtime、host-api、engine-wasm、backend-engine 只用文档化的公开 API，`host` 门面只剩产品胶水。P3 之后核心 `Session` 已经自己收尾、维护候选修订号、只有一套默认值，但：

- 核心层自己的签名用到的类型只能经 `host` 拿到：`SessionOptions::helpcode_table` 与 `Session::set_helpcode_table` 收 `SharedKeymap`，`set_command_table`、`set_mention_entries`、`set_quick_phrase_table` 与对应的 `SessionOptions` 字段收 `CommandTableEntry`、`MentionEntry`、`QuickPhraseEntry`，而这几个类型只在 `host` 里重新导出。第三方照文档用核心层，连选项都填不出来。
- v1 对第三方承诺的自定义是 `ShuangpinCustomTable`，它的校验（`validate_shuangpin_custom_profile` 等）只在 `host` 里；根上导出了 `KeyGrid::NineKeyShuangpin` 要的 `NineKeyShuangpin`，造它的函数却也只在 `host` 里。
- `host` 还把根上已有的十来个类型再导出一遍，产品 crate 经 `host::` 用它们，分不清哪些是门面自己的东西。
- `host::Session` 在核心会话旁边另存了一份核心已经有的状态：组码网格、双拼方案名、「微软双拼」标志，以及一份辅助码表——构造时按同样的参数又读了一遍文件，只为给引擎合成的整句补注释。
- 网页引擎 `WebHost` 直接用核心 `Session`，却自己数「上次排序之后改了几次引擎状态」（`engine_changes`），并靠每个改状态的调用记得 `invalidate()` 来作废快照，与核心的 `candidate_revision` 重复。

## Decision

### 文档化 API 与产品胶水的边界

文档化的 API 是 crate 根上的核心层，加上本来就公开的模块（`assets`、`diagnostics`、`touch`、`multilingual` 等）。核心层签名用到的类型、第三方用核心选项必需的函数，都放在根上：

- 辅助码表：`HelpcodeKeymap`（补了文档）、`SharedKeymap`、`load_helpcode_keymap`。
- `/`、`@`、K 模式的表项：`CommandTableEntry`、`MentionEntry`、`QuickPhraseEntry`。
- 26 键自定义双拼表的校验：`validate_shuangpin_custom_profile`、`ShuangpinCustomProfileError`、`ShuangpinCustomTablePart`。
- 九键、14 键、18 键双拼表的校验与取表：`NineKeyShuangpin` 连同 `validate_nine_key_shuangpin_table`、`resolve_nine_key_shuangpin`、`resolve_fourteen_key_shuangpin`、`resolve_eighteen_key_shuangpin`、`NineKeyShuangpinError`，以及 `NineKeyShuangpin::key_hints` 返回的 `NineKeyShuangpinKeyHint`。
- 新增 `Session::key_grid()`，读回上一次 `set_key_grid` 设下的网格。

设置页编辑这两种表用的辅助函数不提升：内置方案写成表的样子（`shuangpin_builtin_table`、`nine_key_shuangpin_builtin_table`）、编辑中不留存的校验（`check_shuangpin_custom_profile`、`check_nine_key_shuangpin_table`）、预览编辑中九键表的键面（`nine_key_shuangpin_table_key_hints`）。它们只为 msime 的设置页存在，第三方把表交给会话用不上，留在 `host` 里，和 26 键的 `shuangpin_key_hints` 一样算产品胶水。

上面这些名字从 `host` 里移走，`host` 也不再重新导出根上已有的类型（`CandidateEdge`、`PluginCommand`、`SentenceAssociationOptions`、`DictionaryStateRecord`、`close_cached_databases` 等）。产品 crate 一律从根上导入它们，编译器因此保证它们不会再经 `host` 绕过去。只为桥接返回 `Result` 的 `host::handwriting_order_candidates` 删掉，input-runtime 直接用根上的 `order_handwriting_candidates`。

留在门面里的是产品胶水，各模块的模块文档写明这一点：

- `host`：`Session`、`EngineOptions` 与产品默认值、翻译侧车、宿主命令编号（回车学英文词的策略和 `CommitRawWithoutLearning` 后来搬进核心，门面只传产品取值，见 [回车学词策略进核心](2026-10-29-engine-core-raw-commit-policy-and-scheme-data-sets.md)）、扁平快照与辅助码注释规则、词库管理与同步（`dictionary_*`、学习习惯）、释义与音标、emoji 目录、`/` 模式表和自定义双拼表、九键双拼表的设置页辅助函数、文本工具。`DictionaryKind`、`DictionaryEntry`、`DictionaryPage` 是桥接沿用的名字，就是根上的 `PersonalDictionary*`，host-api 有上百处在用，保留为别名并注明。
- `ordering`：msime 的候选排序，input-runtime 与网页引擎共用；第三方拿到的是引擎原始顺序。
- `backend`：msime-cloud 的查询协议，只给 `msime-backend-engine`。
- `web`：网页引擎的浏览器存储。

### `host::Session` 只留产品选项

`host::Session` 只有核心会话、一份 `EngineOptions` 和线程绑定标记。组码网格读核心的 `key_grid()`，双拼方案名读核心会话建立时定下的双拼表（crate 内的 `shuangpin_layout()`），「微软双拼」由构造方案和同一张表现算，补注释用的辅助码表读核心过滤用的那一份（crate 内的 `helpcode_table()`），辅助码开关和显示开关读 `EngineOptions`。构造时不再第二次读辅助码表文件。方案名不照抄核心快照的 `shuangpin_profile`：核心快照在九宫格（九键、14 键、18 键）组字时这一项留空，金样例按此冻结，而门面的契约是任何时候都给出会话跑的方案名，HarmonyOS 拿它算键盘外观的缓存键，iOS 拿它画双拼键面，组字途中变空会让键盘重建、键面提示消失。`host/tests.rs` 用 14 键双拼组字钉住这一点。线程绑定（`PhantomData<Rc<()>>`）保留：那是 host-api 的线程模型，核心会话本身可以跨线程。

### 网页引擎按核心修订号判断过期

`WebHost` 记下取快照时的修订号，修订号变了快照就过期；`composing_hint` 连同得知它时的修订号一起记，修订号再变就不作数；排序记下当时的修订号，「两次排序之间引擎改过不止一次」按修订号的差判断。`invalidate()` 和 `engine_changes` 删掉，改状态的调用不必再各自记得作废缓存。`order_valid` 只管网页自己的输入（模型开关、上文）。会话重建（`reset`）时新会话的修订号从头算，所以那里显式作废快照和排序。

核心修订号说的是候选「可能」变了：引擎不收的键（`character` 没处理、也没上屏）照样让它前进。只按修订号判断时，组字中每个引擎不收的数字（选第几格之前先问引擎）、`;`、`'` 和字母都会让排序重做一次，开着模型时就是一次整句重排，耗时计进 `rerank_ms` 和慢帧熔断；同一批里两个这样的键还会被当成换了一次组字。所以这些键之后，若进这一键之前的快照和排序都对得上当时的修订号，就重取快照比一比，没变就把两者挪到新修订号上，这和 `Runtime::refresh` 每个键都比对前后快照、组字和候选都没变就留住高亮是同一个判断；快照本来就过期时没有可比的，照常按修订号过期。发给页面的 `Frame::candidate_revision` 仍是排序后列表的版本，不是核心修订号：模型开关和上文会改顺序而核心修订号不动，引擎不收的键会让核心修订号前进而列表不变。

翻页、高亮、数字键选第几格和「走到最后一页才展开截留的候选」留在网页引擎里。提案的 P7 写了「engine-wasm 去掉自己的分页」，但同一份提案的 P3 硬点表定的是「分页留给宿主，`candidate_get` 按下标随取」：引擎不知道页大小、高亮和页面怎么画，网页引擎就是一个宿主。它也不能改用 input-runtime 的分页：那要把整个 `Runtime`（在线候选、本地模式、供应方进程）拉进 wasm。所以这一条按 P3 的结论处理，不做。

## Alternatives considered

- **给 `host`、`ordering`、`backend`、`web` 加 `#[doc(hidden)]`，产品 crate 照旧经 `host` 用一切** —— 改动最小，P6 的打包分支已经这样隐藏了这几个模块。只这样做时，核心签名要的类型仍然只能经隐藏的模块拿到，第三方照文档用不了 `helpcode_table` 这类选项；产品 crate 也照旧能经 `host` 用根上的东西，看不出哪些依赖是产品胶水。所以隐藏归 P6，这里把边界本身划清。
- **把产品 crate 用到的 `host` 函数全部提升到根上** —— 产品 crate 就真的只用根上的 API。但那等于把词库同步、释义、emoji 目录、设置页辅助函数、msime-cloud 协议都变成库的 semver 面，与提案「不进 ABI」的清单相反，第三方也用不上它们。
- **把 `DictionaryEntry`、`DictionaryKind` 等别名也去掉，host-api 改用 `PersonalDictionary*`** —— 名字更统一。host-api 里有一百多处，只是改名，类型本身已经是根上的文档化类型；留作注明的门面别名更省事，以后单独改名也不影响边界。
- **网页引擎的 `Frame::candidate_revision` 直接用核心修订号** —— 少一个计数。不行：页面拿它核对点选的候选，排序（模型开关、上文变化）会在核心修订号不动时改变座位，而引擎不收的键会让核心修订号前进、列表却没变，页面会无故拒绝点选。

## Consequences

- **收益**：第三方照根上的文档就能填全核心选项、校验自定义双拼表、造九键双拼网格；msime 自己的产品 crate 走同一条路，根上的 API 少了什么会先在自己身上暴露。`host::Session` 少存五份状态，建会话少读一次辅助码表文件、少一份表的内存；网页引擎不会再因为某个改状态的调用忘了 `invalidate()` 而用旧快照。
- **行为变化**：网页引擎在一次排序之间核心修订号前进超过一次时按「换过组字」处理。引擎不收的键在快照对得上时不计入（见上），所以和原来按 `invalidate()` 次数数的差别只剩两种：`set_helpcode` 换表和开关各让修订号前进一次，之后的那一帧高亮回到首位、展开标记清掉，而引擎这时本来就按新设置重新查询、截留的候选又被截掉，清掉是对的；一批按键里先有一个引擎收下的键、后面又有引擎不收的键（快照这时已过期，没有可比的），而批末的组字原文和候选与批首相同时，高亮也回到首位。后一种要求那个收下的键没有改变组字，实际很少见。引擎不收的键在快照对得上时要多取一次快照来比，比原来多一次快照拷贝，但省掉了一次排序。
- **代价**：根上多了二十来个公开名字，0.x 之后它们是 semver 面；`host` 里少了的再导出会让仍按旧路径写的下游代码编译失败（仓库里已全部改完，引擎自己的示例和 input-runtime 的 `convert_eval`、`typing_sim` 示例也一并使用根上的类型）。
- **没做的**：网页引擎的分页（理由见上）；回车学英文词的策略和 `CommitRawWithoutLearning` 当时没有变成核心层的选项，后来由 [回车学词策略进核心](2026-10-29-engine-core-raw-commit-policy-and-scheme-data-sets.md) 搬进核心；`#[doc(hidden)]` 由 P6 的打包分支加。

相关笔记审计：[核心会话收尾与修订号](2026-10-29-core-session-lifecycle-and-journal-threads.md) 提供本篇依赖的 `candidate_revision` 与核心收尾，部分重叠、互链；[引擎库默认值、错误码、日志与维护锁](2026-10-29-engine-library-defaults-errors-log-and-lock.md) 和 [引擎 C ABI](2026-10-29-engine-capi.md) 是同一提案的 P3、P4，无重叠；[宿主快照直接接收拥有型候选注释](../testing/2026-10-15-host-annotation-move.md) 的冻结快照测试改为读核心的网格与辅助码表，构造与断言不变，无需修改那篇；[宿主辅助码快照复用注释结果](../testing/2026-10-15-host-helpcode-reuse.md) 讲注释只算一次，本篇让门面连表也不再另读一份，部分重叠。

## Verification

- `cargo clippy -p msime-engine -p msime-input-runtime -p msime-host-api -p msime-engine-wasm -p msime-backend-engine -p msime-client-core --all-targets -- -D warnings`。
- `cargo test -p msime-engine`（含 golden）、`-p msime-input-runtime`、`-p msime-host-api --lib`、`-p msime-engine-wasm`（原生目标）、`-p msime-backend-engine`，以及 `cargo check -p msime-engine-wasm --target wasm32-unknown-unknown`。门面快照的网格、双拼方案名（含 14 键双拼组字途中）、微软双拼和辅助码注释由 `host/tests.rs` 与冻结快照测试覆盖；网页引擎的按键路由、高亮保留、展开、引擎不收的键不触发重排、`set_helpcode` 后列表重来由 `routing.rs` 覆盖。`routing.rs` 不挂句子模型。
- 挂真实词库和句子模型（`MSIME_EVAL_RESOURCES` 指向 Studio 上安装的 `MSIME.app/Contents/Resources/EngineResources`，release）：`rerank_routing.rs` 通过；`parity.rs` 在 26,596 例里有 2,212 例首页与桌面 `Runtime` 不同，而本篇的基线分支（#10201）在同一份资源上得到完全相同的 2,212 例和相同的差异内容，所以这些差异早于本篇、与修订号改写无关（列出的例子都是 `Runtime` 把正好拼完整个输入的词排在更长的补全词（如 anli 的「案例分析」「安理会」）之前，网页引擎把它们夹在中间）。`parity.rs` 因此不能当作本篇排序一致的证据，它的既有差异另行处理。后来查明这些差异出在测试的 runtime 一侧配置：引擎的首屏门槛 `first_page_rows` 留在 `prepare_options` 的默认 6，而 runtime 分页和网页引擎都是 9（产品宿主两处同源，见[补全词让位于一页精确同音词](../bug-fix/2026-10-13-longer-phrases-yield-to-a-full-page.md)），另有行内日期时间与日期菜单没关、网页没有这两项。按锁定资源（dict-v2.0.39）在本篇之前的 `2e851f3c0f` 与之后的 develop 上都是同样的 2,257 例，补上这两处后为 0。
