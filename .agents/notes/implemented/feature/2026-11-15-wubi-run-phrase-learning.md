# Agent Note: 五笔按输入习惯自动造词（连续片段累积窗）与落库位置选项

Status: implemented


## Problem

拼音有四条自动造词路径（分段连词与整句经 `learn_pick_pair` 和 `pick_transitions` 计数落库、词对凑词、回车学英文词），五笔被全部排除：pick 构造的合取里有 `!self.is_wubi()`（`session/commit.rs:392`），「造词中」组字对五笔候选写死 `can_store: false`（`session/composition.rs:566`），五笔侧只有手动加词和调频。五笔用户想连续打出「哔哩（kxkj）+ 哔哩（kxkj）」之后自动记住「哔哩哔哩」，现在做不到。

约束有四条，都不容让步：

- **四码顶字手感绝不能动。** 对照 CyIME（librime 内核的安卓输入法）的上游五笔方案（kingzcheung/rime-wubi，README「常见自定义功能 §6 开启自动造词」）：librime 开五笔造词必须注释掉 `max_code_length`（四码上屏）、关掉 `auto_select`（自动上屏）——句子模式和顶字直出在 librime 里是二选一。本仓的方案把造词做成不打断顶字的后台学习，每次选择上屏本身就是一次「片段关闭」，不需要等句子。
- **学习总开关沿用**「学习（记忆新词）」偏好，不加总门；反查、关闭学习、已有词条、删除记录照旧不学（[五笔反查](../../implemented/feature/2026-10-11-wubi-reverse-lookup.md)已定反查禁学）。
- **新世纪五笔维持「当前方案是新世纪才学」**，不做新世纪的全对称读写：`wubixsj` 日志行的宽松读取还没有覆盖到足够多的用户，降级安装会拒绝整份日志（[词库数据格式 §6.2](../../../docs/data-formats.md)）。
- **自造词不挤占已有同码候选**是既定设计（[词库持续补全与五笔新词学习](../../implemented/feature/2026-10-18-dictionary-completion.md)立的初始权重规则，[个人词库零权重行](../../implemented/bug-fix/2026-10-30-personal-dictionary-zero-weight-rows.md)复核过 0..9 的边界）。

维护者 2026-11-15 确认自造词落库位置做成选项——默认「沉底」（现状），可选「置顶」，默认值等于现状语义，属于新字段而非翻转出厂值。造词时机选项（默认「凑 3 次才记」/ 可选「打 1 次就记」）于 2026-11-19 撤回，造词固定按默认凑三次，理由见「造词时机选项的撤回」一节。

## Decision

五笔自动造词走连续片段累积窗，复用引擎既有的 pick 链与 `pick_transitions` 计数表，不给五笔另起一套存储：

- `Pick` 带 `pieces: Vec<u8>`（每片段字数）。拼音 pick 恒为空；五笔段以 `pieces` 非空 + `canonical_pinyin` 为空双标记，与拼音 pick 共用 `chain.last_pick` 不串扰。
- `commit.rs` 的五笔 early-return 分支（把 `last_pick` 置 None 的那路）在选择满足五笔原生候选、本地模式关、学习开、个人上下文开、全汉字、1..=12 字时调 `note_wubi_pick`，把这次上屏记为一个片段。
- 累积窗：与上一次上屏间隔 ≤10s（沿用 `PICK_PAIR_MAX_GAP_SECONDS`）续段；整段 ≤12 字（`MAX_WUBI_RUN_CHARS`）、≤8 片段（`MAX_WUBI_RUN_PIECES`）；间隔超时先「关闭」旧段再开新段；放不下下一片段（超限）时整段作废（`Pick::overflowed`），后续片段继续并进这个作废段，直到下一个关闭点或 >10s 间隔都不计数（2026-11-25 审查改动：原先在上限处关闭并计数前半段，上限落在句子中间，截出来的是半句话）。
- **只在整段关闭时按整段全文计数**，绝不把「中国」「中国共」这类前缀记成词：写 `pick_transitions` 表，哨兵 `previous_key = previous_value = "wubi-run"`（拼音凑对键全是 `[a-z]` 规范拼音，含连字符的哨兵不冲突；`previous_value` 不留空，空串过不了 `habit_is_storable`，本地备份导出时会整行跳过这些计数，2026-11-24 审查时改正）、`key = value = 整段词`。
- 素材规则：每个片段 = 一次五笔选择上屏的文本（单字或已有词），全汉字；**整段 >4 字时任一片段为单字 → 整段不学**（>4 字的词不允许拿单字当素材，防垃圾组合）；≤4 字不限。单片段段不计数。
- **关闭点不止「下一次五笔上屏」**（2026-11-18 实机验收补齐）：打开的段在组字与会话的各关闭点由 `close_open_wubi_run` 收尾——宿主 flush 组字（`finish_composition` 尾部，焦点移动/宿主自行插入文本）、回车原文（`commit_raw` 与 `handle_command` 的空组字分支——组字已空时 CommitRaw 到不了 `commit_raw`，直接 `chain.reset()`，这是实机上「打完词停手」最常见的路径）、无选中上屏（`commit_selection` 的 `None` 分支）、material-false 早退（早退前取走的段要先放回再收尾；2026-11-24 审查前这里直接清掉，段后接一次非汉字五笔行、表情或快捷短语上屏时整段无声丢失）、光标后编辑（`consume_caret_prefix`）、个人上下文重置（`record_personal_context` 三处）、会话丢弃（`Session::drop`，关段先于刷盘）。五笔段关闭计数（凑满阈值即落库），拼音 pick 原样放回维持既有语义。空组字分支里的退格与向后删除不是关闭点：用户在删刚上屏的字，这段不是要留下的文字，直接丢弃不计数（2026-11-24 审查时改正）。caret prefix 只认拼音单元，五笔会话走不到 `consume_caret_prefix` 的取走路径，接线只为防御。
- 阈值固定 `PICK_PAIR_WORD_THRESHOLD = 3`（与拼音凑对同一个阈值）→ `wubi_learning::learn` 落库 → `engine.reset_cache()` → 清该计数；计数跨会话持久，与拼音凑对同一组诊断码（`PICK_TRANSITION_NOT_PERSISTED`、`PHRASE_NOT_PERSISTED`）。

`wubi_learning::learn` 接收一个新参数：

- 位置（`WubiPhrasePlacement`）：`Bottom`（默认，现状——同码已有行最小权重 −1，clamp 0..=9）/ `Top`——**入库时一次性给「各版表内同码已有行最大权重 + 1」**（clamp 到 `MANAGED_WEIGHT_CEILING`，同码无行时给 10，与手动加词默认 `NEW_WORD_WEIGHT` 同级），86/98 各表独立算。不是永远悬浮第一：之后照常参与调频与置顶操作。垃圾词兜底不做时间衰减（见 Alternatives），靠 ≤9/低权重与删除。

删除与学词对称：学词时几版同时插行（86/98 总是一起学，新世纪只在当前方案是新世纪时学），`wubi_learning.rs` 的 `delete_other_learned_kinds(main_db, user_db, keep, profile, word)` 对三版里除 `keep` 外的版本按各自码表独立推导四码，日志里确有该码的学词 upsert 行才删行并写墓碑（在新世纪下学过、换回 86 再删的词三版都删干净；没用过新世纪的用户不会因删除多出 `wubixsj` 行）（`operation='delete'`），推导失败的版本跳过（该版本本来也学不进）。`remove_candidate` 的五笔分支在 `delete_dictionary_candidate` 成功后调用。

一个新设置键：`input.wubi_phrase_placement`（`bottom`/`top`），缺键按默认读，默认时不写进偏好文档（照 `candidate_font_weight` 的 serde 模式）。管线：`client-core/preferences.rs` → `client-services/account/settings_sync.rs`（同步键）→ `client-core/edition.rs`（设置页分组，与 `input.learning` 同组）→ `host-api`（重建会话选项）→ `engine/src/host/options.rs`（`SessionOptions` 字段）。共享设置页「候选与联想」组、「学习」开关之后一行（文件 `wubi-phrase-learning-section.tsx`），说明收进标题旁「?」，仅五笔方案且学习开启时显示。

**共享设置页（`packages/ui`）进控件是维护者 2026-11-15 的明确授权**，作为 AGENTS.md「界面迁原生、`packages/ui` 只修缺陷不加新设置」的例外：Linux 的原生设置迁移排在最后，维护者主力平台是 Ubuntu（共享 Tauri 设置页），没有界面这个选项就无法使用。例外范围以本次一个控件为限。

### 造词时机选项的撤回（2026-11-19）

维护者要求取消「打 1 次就记」的策略与设置项，造词时机固定为默认凑三次，落库位置选项保留。撤回理由：一次即记的垃圾风险高，默认值虽未翻转出厂值，但多一个设置项增加维护与用户选择成本；三次阈值沿用既有 `PICK_PAIR_WORD_THRESHOLD`，与拼音凑对同一条落库路径，不引入代码分叉。

实现上删掉 `WubiPhraseLearning` 全链（`client-core/preferences.rs` 字段/枚举/迁移、`client-services/account/settings_sync.rs` 上传下载、`client-core/edition.rs` 过滤键、`host-api` u8 编码、`engine` 的 `EngineOptions`/`types.rs` 枚举、`engine-wasm` 选项、共享设置页与桌面测试），引擎固定用 `PICK_PAIR_WORD_THRESHOLD`。旧文档残留的 `input.wubi_phrase_learning` 键读盘后不再被识别、不会作为本版本设置写出；字段未发布，无真实用户数据需要迁移。决策笔记就地更新，不另起新篇。

### CyIME/librime 对照

CyIME 没有内置五笔，五笔方案从市场下载（`SchemaConfigHelper.kt` 指向 kingzcheung/rime-wubi），造词能力来自 librime `table_translator` 的 `enable_encoder` + `encode_commit_history`：上屏历史连缀、**一次提交即造词**、按构词规则编码、userdb c/d/t 模型 dee 时间衰减淘汰不用词。代价是关掉四码自动上屏/顶字（上游 README 明说），且垃圾组合会在候选里短暂出现。本方案保手感、以重复阈值和固定低权重代替衰减；librime 的 userdb 动态权重模型（commits/dee/tick）与本仓个人词库的静态权重 + 日志回放模型不同构，不采。

## Related decisions

- [词库持续补全与五笔新词学习](../../implemented/feature/2026-10-18-dictionary-completion.md)：`wubi_learning::learn` 的来源，本篇扩展它的权重规则与调用方，不推翻它「只补缺失、不覆盖已有、反查禁学」的任何一条。
- [个人词库零权重行](../../implemented/bug-fix/2026-10-30-personal-dictionary-zero-weight-rows.md)：0..9 权重的边界复核，`Bottom` 模式维持它记下的派生规则；`Top` 模式是新加的第二档初始权重，不产生 0 权重行。
- [五笔多字词覆盖](../../implemented/feature/2026-10-18-wubi-vocabulary-coverage.md)与[共用编码流程](../../implemented/simplification/2026-10-12-wubi-supplement-common.md)：来源筛选与取码规则是本篇 `phrase_code` 推导的上游，复用不改。
- [五笔反查](../../implemented/feature/2026-10-11-wubi-reverse-lookup.md)：反查禁学的边界，本篇沿用。
- 部分重叠已核对：[86 五笔原表拆分](../../implemented/feature/2026-11-04-wubi86-table-split.md)改变同码行的权重分布（尾段行按拼音频率越过冷僻极点行），`Top` 模式取「同码最大权重 + 1」在拆分后的表上仍然成立——最大值就是该码当前的第一行；`Bottom` 模式的最小值同理不受影响。无关：learning-sync、粤语造词。

## Testing

- `cargo test -p msime-engine`；`wubi_run_*` 系列测试钉住：凑三次落库（86/98 各自编码）、单字拼长词无前缀、>4 字禁单字素材、间隔分段与 12 字/8 片段上限、学习关与墓碑不复活、置顶模式两表独立高于同码行；**关闭时机（2026-11-18 补齐，11-19 随撤回改为三次语义）**：凑三次模式经宿主 flush（`session.finish(0)`）、回车原文（`Command::CommitRaw`）、会话丢弃（Drop）三条路分段攒满计数后落库，学成即清计数；既有拼音凑对测试全部仍过。
- 对称删除：fixture 学入两版后删除，两版行都消失、两版墓碑都在、重放零失败、`learn` 不复活。
- 偏好管线：落库位置缺键按 `bottom` 读、`top` 往返、非法值按默认读；造词时机字段已撤回（`wubi_phrase_learning_is_not_a_recognized_preference` 钉住文档残留键不再被写成设置）；settings_sync 与 schema 测试、host-api 选项重建测试、edition 版本过滤测试（`wubi_phrase_placement_key_follows_the_wubi_edition`）。
- 设置页：`apps/desktop/tests/settings/wubi-phrase-learning.test.tsx` 钉住非五笔方案或学习关闭时不渲染落库位置、只渲染「五笔自造词位置」一行、切换回调写出正确偏好值。
- 实机验收（Ubuntu 26.10 + iBus，维护者）随 PR 验收：固定凑三次——哔哩哔哩打 3 次后四码直出、排位沉底；切置顶模式——新词出现在同码候选前列；切回沉底确认回到现状。

## Alternatives considered

- **librime 式「一次即造 + dee 时间衰减」**——最强理由：即时性最好（哔哩哔哩打一次就记住），不打自动沉底，不用重复阈值；CyIME 全套五笔体验就是这么来的。否掉理由：本仓个人词库是静态权重 + 日志回放模型，加一套随时间衰减的权重需要新的表结构和查询时衰减计算，与调频、置顶、固定排位互相纠缠，工程量大且收益存疑；维护者 2026-11-15 选定不做衰减，垃圾词兜底靠低权重（沉底模式）与删除（iBus 候选右键/快捷键在配套 PR 里补齐）。「打 1 次就记」曾以阈值 1 的选项形式给到即时性，2026-11-19 按维护者要求撤回（见「造词时机选项的撤回」）。
- **>4 字的词也收单字素材**——最强理由：「中国 + 共产党」这类常见组合只能单字拼，禁单字素材会漏掉它。否掉理由：垃圾风险不对称——随手单字连打的碎片组合会在 >4 字段里攒出大词，而 ≤4 字时单字拼词的垃圾空间小得多；真要「中国共产党」可以用置顶模式手动加词，或等它以 ≤4 字词组合出现。维护者确认禁单字素材。
- **自造词永远首位（持续置顶）**——维护者否决过的方向：自动改写排序会盖过用户手动固定排位，垃圾词也会被顶到第一。本篇的 `Top` 是折中：入库时一次性给高位权重，之后照常参与调频，别的词用得多可以超过它；要钉死第几位用现有的固定排位操作。
- **拼音凑对机制直接给五笔用**——最强理由：零新机制，`learn_pick_pair` 现成。否掉理由：拼音凑对的语义是「两次整句上屏拼一对词」（两次 pick 的规范拼音连起来），五笔片段是任意词/字序列的拼接，没有「两段拼一对」的词法语义；强行复用会把单字上屏也当成凑对素材，前缀污染防不住。累积窗是同一存储上的不同语义，成本只在新函数。
- **新世纪五笔的全对称读写**——降级兼容拒绝（[词库数据格式 §6.2](../../../docs/data-formats.md)：不认识 `wubixsj` 的旧引擎把未知行算失败、整份日志拒绝），维持「当前方案是新世纪才学」的单向写入，等宽松读取版本覆盖了足够多用户再议。2026-11-25 审查本 PR 时又评估过一次「造词三版全写」，维护者否决：拼音造词也经 `wubi_learning::learn` 写五笔表，拼音用户默认算 86，全写会让几乎所有造过词的用户日志里出现 `wubixsj` 行，降级到宽松读取之前的引擎时整份个人词库准备失败；换到新世纪后旧词要重打一遍的代价小得多。

## Consequences

收益：五笔用户连续打出重复片段后自动积累词库，不打断四码顶字；固定凑三次造词，落库位置给一档选择，默认值等于现状语义，不翻转任何出厂值；删除与学词对称，不再留任一版的单侧残行。

代价与风险：

- **前缀污染**的防线只有「只对关闭的整段计数」一条，`close_wubi_run` 的素材规则写错就会把「中国」记成词——测试钉死。
- **`Top` 模式顶掉已有同码候选**：这正是该选项的语义，但用户可能低估影响——「?」说明里写清楚「置顶只对四码全码生效、会排在同码已有词之前」。
- **`pick_transitions` 表增长**：五笔段按整段全文计数，长段键长；12 字上限封住单键尺寸，跨会话计数清理沿用拼音凑对的清理时机（学成即清）。
- **落库位置模式分叉**：位置有两档，若实现成两条路径会腐烂——审查重点钉「同一路径只有参数不同」。
- **新世纪写而不读的墓碑**：对称删除对新世纪推导失败的版本跳过（无行无墓碑），与 `learn` 的跳过条件对称，不产生新世纪方向的孤儿墓碑。
- **降级兼容**：落库位置设置键只影响新引擎；旧引擎读到缺键的旧文档按现状语义走，新文档多出的键被旧引擎的偏好读取忽略（字符串枚举键，不是枚举值新增，无 `wubixsj` 式日志降级问题）。
