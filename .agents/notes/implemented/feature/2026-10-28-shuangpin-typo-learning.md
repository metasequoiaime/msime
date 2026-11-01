# Agent Note: 双拼触点纠错按两键码学习接受、拒绝与抑制

Status: implemented

## Problem

#10018 的[双拼触点纠错](2026-10-27-shuangpin-touch-correction.md)让触屏宿主送来的落点给纠错整句计价，但每个用户的手型不一样：同一个人反复把 r 按成 f，引擎每次都按同样的价算，纠回来的那一行排位一次也不会变。用户按 Enter 原样上屏、明确不要这次改正，下次同一串输入还会出现同一行纠错。下一期（PR3）要让落点支持的纠错坐首选，坐首选之前必须先有两样东西：被拒绝过的改法能一票否决首选；用户拒绝过的输入不再纠。

[双拼纠错整句](2026-10-09-shuangpin-typo-sentence.md)当初没有学习，理由是全拼的纠错档案（`pinyin_typo_counts`）按全拼音节对记，套到双拼键位上没有意义，要做得先给档案加方案维度。

约束：桌面宿主和实体键盘的请求永远不带触点，它们的候选和以前逐字节相同（G0），日志里有没有学习记录都一样；全拼的学习路径不变。

## Decision

双拼另起两张日志表，按内置方案和两键码记接受、拒绝和抑制；只有带触点的请求读写它们，接受次数只给触点路径打折。

### 存储

- `journal.rs` 的 `SCHEMA_SQL` 加两张表，都是 `CREATE TABLE IF NOT EXISTS`，旧引擎打开新日志时不认识它们、也不碰它们：
  - `shuangpin_typo_counts(profile, typed, intended, accepted, rejected, updated_at)`，主键 `(profile, typed, intended)`。`profile` 是内置方案名（`xiaohe`、`ziranma`、`shoudao`、`microsoft`、`smart_abc`），`typed`、`intended` 是两个小写字母或 `;` 组成的两键码，两者只差一个键或互为对调。两个计数都封顶 1000。
  - `shuangpin_autocorrect_suppressions(profile, input, commits, updated_at)`，主键 `(profile, input)`。`input` 是带触点那串输入（小写字母和 `;`，不含 `'`，最长 64）。
- 写入整批校验，有一对不合格就什么也不写、连日志都不开。写完按全拼两张表同样的规则修剪到 2048 行：计数表按接受加拒绝、抑制表按上屏次数，先删最少、最旧的，这一批刚写的不删。读的时候只认内置方案、合法的码和一处改动，坏行跳过；行数超过上限或有一行类型不对（比如 `typed` 是 BLOB、`accepted` 是 2.5）时只作废双拼这一部分：`load_pinyin_typo_state` 照样交出全拼两张表的内容，`shuangpin_loaded` 为假，档案换上新读的全拼部分、保留原来的双拼部分（自己的双拼写入补在上面），不记下这次的文件戳，下次查询再试。全拼部分读不出来时仍和加表之前一样整次失败。全拼学习因此不受这两张表连累：外部写入方、文件损坏或以后改表结构弄坏了双拼表，全拼的接受和抑制照样跟着日志走。
- 都由进程共用的 `PersonalTypoProfile`（每份日志一份）读写：`shuangpin_counts`、`shuangpin_suppressed` 查询不分配；`record_shuangpin`（接受或拒绝）、`record_shuangpin_suppression` 写完重读，代次前进。
- 自定义方案一律不学：键位随时会改，按旧键位记下的码对换了键位就是别的意思。
- 不进 `habits.ndjson` 备份：`habits.rs` 只导出原来那六张表。旧版 Android 的 `inspect_habits` 读到不认识的 `type` 会把整份备份判为损坏（`LocalBackup` 检查备份时调它），加一种记录会让新备份在旧版上恢复不了。代价是换手机后双拼的纠错习惯要重新学。
- 重置学习数据换一份空日志，两张表随之清空，档案下次刷新就读不到旧记录。

### 接受与拒绝（`session/learning.rs`）

只在当前请求带触点、开着纠错、内置方案、普通组字（不是本地模式和专用英文）时学，`learn_candidate` 里与调词频、整句学习并列：

- **接受**：选中的行是双拼纠错行——带 `corrected_from` 的整句纠错行（Generated、`sentence_association`），或带 `corrected_from` 的词库行（PR4 的 1–2 音节短输入纠错会是这种）。`supports_autocorrect` 对双拼仍是 false，全拼的 `learn_accepted_typos` 不碰双拼。
- **拒绝**：显示出来的第一行是纠错行、用户选了别的行。只在选中行覆盖到、读音不是纠正后音节的那些位置各记一次拒绝；只覆盖前一个音节的词没对后面的改动表态，不记。纠错行只有按首选规则才坐得上首选，这条路要等首选规则起作用才走得到；首选规则读 `rejected` 一票否决首选，纠错行仍可排在后面，见[双拼触点纠错坐首选的规则](2026-10-29-shuangpin-touch-first-seat.md)。
- **记哪一对**（`shuangpin::typo_edges::touch_correction_pairs`）：纠错行回答的按键每两键切一段，与纠正后的读音逐音节对齐；改动过的音节取本次变体集里解码到纠正后音节、罚分最低的那个两键码，不从音节反推（零声母音节在同一方案里可能有几个码）。读音和按键对不上，或某个改动不在这个码的变体里（模糊音的替代拼写），整条不记。

### 抑制

- 记录（`learn_rejected_correction` 的双拼分支）：带触点的输入有落点支持的纠错可选（`touch_supported` 的整句纠错行，或带 `corrected_from` 的词库行），用户按 Enter 原样上屏，记一次。沉到表尾、落点不支持的纠错行用户多半没看到，不算。
- 生效（`apply_autocorrect_suppression` 的双拼分支）：带触点的请求，输入在当前方案下被抑制过，就把两个纠错开关都关掉、清掉触点和几何，请求与没有触点、没开纠错时相同，按字面回答。改一个键就是另一串输入，不受抑制。不带触点的请求不查，桌面开着纠错时照样给平面表的纠错整句。

### 生效：只给触点路径打折

- `priced_code_variants` 给每个变体两键码计价之后，按这一对的接受次数 n 减去 `learned_discount(n) = NEIGHBOR_COST − discounted(NEIGHBOR_COST, n)`，与全拼按接受次数打折是同一条曲线：n=1 约 2.8，n=3 约 5.5，最多 12。不低于 `P_MIN`（本来就低于它的不往上抬）。平面表外的键先打折再和 `P_CUT` 比，接受过的斜邻键因此也能进来。对调和没有触点的位置在触点路径上同样打折。
- 同一个折扣也进 `touch_supports_correction`（`show_penalty` 闸）：每次查询的变体只由 `priced_shuangpin_variants` 算一次（折扣在这里打），规划纠错边和这道闸共用这一份，所以接受过的改法更容易留在座位上，这是「排位上升」的来源。
- 没有触点时走 `collect_shuangpin_typo_edges`，那条路传给规划的仍是恒为 0 的接受次数，也不读档案。档案里一条双拼记录都没有时（`LearnedTypos::new` 先看 `has_shuangpin_counts`），触点路径也不逐个变体去查。
- `ShuangpinDictionary` 持有共用的档案，在带触点的请求进来时（`set_key_touches`）刷新档案，代次变了就清掉整句答案和由它们得出的辅助码答案，做法同全拼的 `QuanpinDictionary`；不带触点时不刷新也不清，桌面的缓存行为不变。变体跨度行的缓存不含罚分，不用清。

## Alternatives considered

- **复用全拼的 `pinyin_typo_counts`，加一列方案**：一张表、一套修剪和备份。但两键码会和全拼音节撞名（`ai`、`an`、`en`），全拼的校验只收不超过 6 个小写字母的音节，`;` 过不了；改它的主键要迁移已有用户的日志，还会让旧版 Android 的备份读到新形状的 `typo` 记录。另起表最干净，旧引擎直接忽略。
- **按音节对记（typed 音节 → intended 音节）**：与全拼档案同构，计数也更不稀疏。但误触是按键层面的事，同一对音节在不同方案里来自完全不同的按键；零声母音节一个对应几个码，按音节记会把不同的按键错误合在一起。按两键码记更稀疏（方案文档列为风险），效果不够时再考虑按物理键对聚合。
- **接受时从纠正后的音节反编出想按的码**：最简单。但零声母音节在同一方案里有几个码，反编取到的未必是这次变体集实际解出来的那个，下次打折就打到了另一对上。改为在本次变体集里找解到这个音节、罚分最低的码。
- **抑制只清掉触点、不关纠错开关**：被抑制的输入退回平面表，纠错整句照旧生成、沉到表尾，切换时不清缓存（纠错类型不变）。但用户按 Enter 原样上屏已经明确不要改正，全拼的抑制也是把两个开关都关掉；退回平面表还会在开了模糊音时留下纠错行。两样都做：关开关，也清触点，请求与字面输入完全相同。
- **抑制和接受也作用于不带触点的请求**：双拼纠错在桌面也开着（Windows 默认开），学到的东西本可以让桌面的平面表也受益。但桌面的每条新行为都会让 G0 失守，平面表的纠错行本来就沉在表尾；学习记录也只在触屏上产生。
- **把双拼学习也写进 `habits.ndjson`**：换手机后纠错习惯能带过去。但旧版 Android 遇到新类型会判整份备份损坏，连词库都恢复不了；要做得先给习惯文件升版本、让旧版跳过不认识的类型，另作一期。
- **评测回放经真实的选词路径接受**：最贴近用户。但选中纠错整句的同时会把整句存成用户词、记下整句联想的上下文，下一遍金标准排位上升大半来自这两样，量不出计数本身的作用。回放改为把计数直接写进评测自己的日志，选词时记哪一对由会话测试钉住。

## Consequences

- **收益**：同一对接受 3 次后，这一键改过去的价少约 5.5（小鹤 gf → gr，f 落在 f 和 r 正中时从 12 降到约 6.5），纠错整句的分随之升高；原来过不了 `show_penalty` 的改正留在座位上。评测回放见 Verification。
- **代价**：带触点的请求每进一次双拼词典（`ShuangpinEngine` 的查询、首字母展开、在线候选写入缓存三处都经 `set_key_touches`）和每做一次抑制检查（`apply_shuangpin_autocorrect_suppression`，构造光标前缀的请求时也做）都刷新一次档案，每次刷新 stat 日志和它的 `-wal` 两个文件。一次普通的带触点查询是抑制检查加词典查询两次刷新、4 次 stat，赶上首字母展开或在线候选时每处再加 2 到 4 次；文件没变时不读日志。不带触点的请求一次也不刷新。档案有双拼记录时每个变体多一次哈希查找。日志多两张表，各不超过 2048 行。
- **已知上限**：
  - 按两键码记很稀疏，同一对要反复出现才有作用；回放用的是固定偏置的合成落点，真机上同一对重复的频率要等宿主接入后再看。
  - 拒绝一票否决首选由[双拼触点纠错坐首选的规则](2026-10-29-shuangpin-touch-first-seat.md)实现；引擎默认的 m_sp 是无穷大，规则不起作用时纠错行坐不上首选，`rejected` 实际上仍只记不用。
  - 换手机或恢复备份后双拼纠错习惯从零开始。
  - 只学内置方案；14/18 键、九键双拼没有触点，不学。
- **隐私**：只记方案名和两键码对、被拒绝的按键串，不记坐标，不进统计、不上传、不进备份。
- **重访信号**：首选规则读 `rejected`（[双拼触点纠错坐首选的规则](2026-10-29-shuangpin-touch-first-seat.md)）；宿主接入真机几何后重量回放；用户反馈学得太慢时考虑按物理键对聚合；习惯文件升版本时决定双拼两张表要不要进备份。

## Verification

下面的数字都是分支变基到 develop `0117616dc0`（含全拼纠错补两字母错键的 `89763ce33d`）之后、在 Mac Studio 上重跑的。每一步的完整输出、报告、dump、stderr 和退出码都留在 Studio 的 `/Volumes/Workspace/rbuild/evidence/sp-touch-learning-v2-g0-20261028/`：`tests/` 是测试和 clippy 的日志，`out/` 是评测，`out/summary.txt` 是逐组比较的结论，`out/provenance.txt` 记下两份源码的提交、两个二进制的 sha256 和资源目录的文件清单，跑法就是同目录的 `g0.sh` 和 `tests.sh`。

- **单元和集成测试**：
  - `cargo test -p msime-engine --lib`：2762 项通过（57 项本来就标了 ignore）。新增的：
    - `typo_profile`：整批校验（自定义方案、空批、相同的码、两键都换、大写、`'`、一批里混一对坏的都整批拒绝且不开日志）、接受和拒绝分列计数并封顶、按方案分开；两张表修剪到 2048 行且刚写的不删；双拼表超过上限或有一行类型不对时只作废双拼部分、全拼照样读出（`a_broken_shuangpin_table_leaves_the_quanpin_load_alone`），档案此时全拼跟上别的写入方和自己的写入且只算一次、双拼保留原状态并补上自己的写入、表修好后重新按日志读（`a_broken_shuangpin_table_keeps_the_held_shuangpin_state_and_follows_quanpin`）；读的时候跳过自定义方案和坏行；档案的读写、代次前进、别的写入方、查询不分配。
    - `journal` 的表名测试加上两张新表；`habits` 测试钉住两张表不进备份、计数不变；`reset` 测试钉住重置后两张表为空、档案刷新后读不到旧记录。
    - `shuangpin::typo_edges`：接受过的那一对按 `learned_discount(n)` 降价、别的变体不动、自定义方案不打折、不低于 P_MIN；平面表外的键打折后能过 P_CUT；`touch_correction_pairs` 对每个方案码、每个变体音节都给出解得出它、价等于变体集给它的那个码，对不上或解释不了时整条不记。
    - `shuangpin::dictionary`：代次变了，带触点的查询前清掉整句和辅助码答案，不带触点的不清（把这段清缓存去掉，这个测试会失败；`shuangpin::tests` 那个端到端测试单靠它测不出来，因为日志一写，个人整句模型的版本也变了，系列缓存里打过个人分的条目会被那条路先清掉）。
    - `shuangpin::tests::accepted_shuangpin_typos_discount_only_the_touch_path`：接受 3 次后纠错整句的分正好高出 `learned_discount(3)`，原来过不了 `show_penalty` 10 的改正留在座位上；平面表新旧引擎逐行相同；微软双拼不受小鹤的记录影响。
    - `session::tests`：带触点选中纠错整句记 (xiaohe, gf, gr) 接受，关学习和平面表不记；Enter 原样上屏记抑制（落点不支持时不记），之后带触点按字面回答、请求不带触点，改一个键不受抑制，平面表会话照旧给纠错（把抑制分支去掉，这个测试会失败）；把纠错行挪到首位后选别的整句记拒绝、选只覆盖第一个音节的词不记、选纠错行本身记接受；有接受、拒绝和抑制记录的日志下不带触点的会话快照与空日志逐字节相同。
  - `cargo test -p msime-engine --test golden`：31 项通过。
  - `cargo test -p msime-input-runtime --lib --example convert_eval --example rerank_latency`：405、29、26 项通过。
  - `cargo test -p msime-host-api --no-fail-fast`：库测试 1044 项通过（5 项 ignore），其余目标全部通过。变基前那次第一遍有 1 项失败（`dictionary::tests::a_lookup_keeps_the_users_japanese_mixing`，日文混输的查词，与本分支无关），单独跑和整套重跑都通过，这次一遍就过。[触点纠错笔记](2026-10-27-shuangpin-touch-correction.md)记下的两项旧失败（`listing_reports_every_pack_and_checks_the_state_root`、`a_command_gets_the_local_time_to_the_minute`）在现在的 develop 上已经修好，这次都通过。
  - `cargo clippy -p msime-engine -p msime-input-runtime -p msime-host-api --all-targets -- -D warnings` 无警告。
- **G0**：分支的父提交（develop `0117616dc0`，已合入 PR1a #10083 及其评审修正）和本分支各经 `rbuild` 编一份 release 的 `convert_eval`（两棵工作树各有各的镜像和 target 目录，见下），资源目录按 `desktop-dictionary.lock.json`（dict-v2.0.39）逐个校验 sha256、不放 settled 模型（每一次运行的 stderr 都打出 `no settled model at ...`），同样的参数各跑一遍逐字节比较报告、dump 和 stdout：
  - `verify-local.sh` 的 13 组全拼评测（sentences、harvested、neutral、words、nine-key、nine-key-prefix、nine-key-sentences、fourteen-key、fourteen-key-prefix、fourteen-key-sentences、particles、particles-nine-key、particles-fourteen-key）：报告全部相同，两边也都与仓库里提交的基线相同（`--baseline` 退出码为 0）；dump 只有 words、nine-key、fourteen-key 里 `shijian` 那几行的时间候选秒数不同（两边相差一秒，随墙上时钟变），其余相同。
  - 双拼 none 模式，五个方案，整句族（四份集合）和词级族（`--max-syllables 2 --limit 3000`）：报告和 dump 都逐字节相同。
  - 送落点、日志里没有学习记录：词级族的完整组合（5 个方案 × 4 种模式 × 4 个 σ × 2 个偏置，带 `--assert-top1-unchanged`）和学习回放那组参数去掉 `--learn-replay`（整句三份集合，小鹤、微软，σ=0.25，偏置 0 和 0.1）两边的报告和 dump 都逐字节相同，`--assert-top1-unchanged` 也都过了。词级族两边的报告与仓库里的 `baseline-shuangpin-touch-words.json` 都对不上，连 none 模式也不同（小鹤 top1 0.697 对基线的 0.733）：那份基线是 #10082 在词库换成 dict-v2.0.39 之前生成的，develop 上本来就对不上，与本分支无关，这里没有改它。整句族的完整组合要几个小时，没有重跑。
  - 一个教训：起初为省盘让 develop 那份共用分支的 target 目录，cargo 给两棵树里同名的工作区 crate 算出相同的产物名，分支那次 release 编译把 develop 编好的引擎当成新鲜的直接链接了，编出的二进制回放毫无作用。两份各用各的 target 目录重编之后才是上面的结果。
- **学习回放**（`--learn-replay 3`，sentences-v1、particles、v2 共 455 条能编码的用例，`android-portrait` 两份几何，σ=0.25，引擎默认计价参数；每组把 noisy 集里只错一键或两键对调的码对记成接受 3 次）：

  | 方案 | 偏置 | 记的码对 | noisy top9 | noisy found | noisy MRR | realistic top9 | clean 被挤 | 首选被改 |
  |---|---|---|---|---|---|---|---|---|
  | 小鹤 | 0 | 403 | 0.174 → 0.257 | 0.266 → 0.301 | 0.047 → 0.068 | 0.620 → 0.626 | 0 → 2 | 不变 |
  | 小鹤 | 0.1 | 385 | 0.163 → 0.255 | 0.262 → 0.299 | 0.041 → 0.064 | 0.587 → 0.598 | 1 → 1 | 不变 |
  | 微软 | 0 | 414 | 0.156 → 0.268 | 0.273 → 0.292 | 0.044 → 0.072 | 0.626 → 0.637 | 0 → 2 | 不变 |
  | 微软 | 0.1 | 396 | 0.174 → 0.253 | 0.259 → 0.288 | 0.048 → 0.067 | 0.593 → 0.613 | 1 → 1 | 不变 |

  - 回放的报告里正式那一遍的 12 行与上面 develop 不加 `--learn-replay` 的报告逐行相同，表里「→」左边就是这些行。
  - noisy 集第一页召回升 7.9–11.2 个点；三个集合的 top1、top3 一个都没变（这一期不抢首选），按金标准算的 `top1_changed` 也一条没变。回放这一遍不跑 `--assert-top1-unchanged` 那道比实际首选文本的闸。
  - 代价：clean 集（打对的输入）455 条里被往后挤的在偏置 0 下多了 2 条（两个方案都是 0 → 2），偏置 0.1 下不变；realistic 集 gold_demoted 在小鹤偏置 0、微软偏置 0 和 0.1 下各多 1 条，小鹤偏置 0.1 下 found 从 0.664 降到 0.662。打折让接受过的改法更容易过 `show_penalty`，打对的输入里偶尔也有一条这样的纠错整句留在座位上。
  - 这是样本内的回放：记的码对就来自被量的那批用例，而且每一对都记满 3 次，等于一个偏置固定、把每次纠错都接受了的用户，数字是学习作用的上限。真实用户按两键码记很稀疏，同一对未必重复 3 次。
  - 2026-10-28 移植到合入后的 PR1a 上之后在 dict-v2.0.39 上重跑：资源换了，回放前的数字与移植前那份笔记的也不同，两边都按这一份看。变基到 `0117616dc0` 之后再跑一次，表里的数字一个没变。
- **没有做的**：`scripts/verify-local.sh` 整套没跑（只按它的参数跑了 13 组评测）；送落点的双拼整句族完整组合（几个小时）没有重跑，词级族那份过时的基线也没有更新；没有在任何真机或模拟器上验证，宿主还没有接触点（PR1b）；PR3 的首选否决还不存在，拒绝计数只验证了记账。

1–2 音节的完整词补查及其词级对照见[短词触点纠错](2026-10-31-shuangpin-touch-short-words.md)；沿用本篇的既有边界，不启用纠错首选或改变默认参数。
