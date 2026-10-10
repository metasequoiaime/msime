# Agent Note: 双拼的纠错整句

Status: implemented

## Problem

双拼完全没有按键纠错（#6034）。纠错在引擎里只覆盖全拼，有两条路：一是 `pinyin/autocorrect.rs` 按非法拼写生成的纠错表；二是词网格里的纠错整句，`quanpin/typo_edges.rs` 在合法音节之间加替换边，让语言模型比较。双拼的 `ShuangpinDictionary::merge_sentences` 给 `merge_lattice_candidates` 传的纠错来源一直是 `None`，`ShuangpinEngine` 也不读请求里的纠错开关。

第一条路天然用不到双拼上：双拼每个音节两键，按错一个键多半仍是合法编码（小鹤 `hc` 是 hao，按成 `hv` 就是 hui），不会变成非法拼写。能纠的只有第二条路。

与此同时，Android、iOS 和共享设置页的「拼音纠错」开关在双拼方案下照样显示、默认开着，却不起作用。

## Decision

- **只做纠错整句，复用全拼的规划**：`quanpin/typo_edges.rs` 的规划和查词抽成 `collect_span_typo_edges`。它接两个闭包：`typos_at(i)` 给出第 i 个音节可能想打的音节（便宜的类型在前），`accepted(i, typo)` 给出用户接受过这条纠错的次数。全拼的 `collect_typo_edges` 照旧按音节拼写取变体、按纠错档案打折，行为不变。弱位置优先、2 到 3 个音节的跨度、96 个键的总预算、每键 4 行、罚分，这些全拼和双拼共用一套。
- **变体按当前方案的两键编码生成**：`shuangpin/typo_edges.rs` 为每个两键编码生成变体，有两种改法。一是把一个键换成 `pinyin::typos::keys_adjacent` 判定的 QWERTY 邻键，二是两键对调。改完用同一方案的 `cvt_single_sp_to_pinyin` 解码，再规范成词库拼写（`lve`、`ju`），和原音节不同的才算变体。同一个音节由两种改法得到时，只留便宜的那种。每个方案第一次用到时，把 27×27 个编码（含微软的 `;`）整表算好（`typo_table`），按方案的地址分开存，不按 `profile.kind`：自定义方案（#6647）的 `kind` 都是 `Custom`，键位却各不相同，按 `kind` 存会让第二张自定义表拿到第一张表算出的变体。`custom::custom_profile` 按表的内容只留一份方案，同一张表总是同一个地址，改了键位就是新地址，所以每张表只算一次，改键位后也不会用旧变体。变体表和自定义方案一样留到进程结束，数量不超过进程里实际用来打字的方案数。漏键和多键不做：它们会让后面每一对键都错位。
- **开关沿用会话的「拼音纠错」**：不新增设置项，默认值就是这个开关原来的默认值。`QueryRequest` 的 `enable_quanpin_autocorrect_transposition/neighbor` 本来就由 `apply_request_options` 写进每个请求，名字带 quanpin 是历史原因；`ShuangpinEngine::query` 把它们折成纠错掩码，只取对调和邻键两位，交给 `ShuangpinDictionary::set_autocorrect_types`。掩码一变，就清掉整句答案和由它们得出的辅助码答案。原因是双拼的 series 缓存键里没有掩码，旧答案是按另一个设置组出来的。网页引擎仍然 `autocorrect_types = 0`。
- **准入**：掩码要开着；这次答案的缓存键里不能有用户手打的 `'`（和全拼一样，按字面切分）；切分出的每一段都必须恰好两键，段数和音节数相同，这样才知道该换哪个键。准入只看缓存键和切分，所以命中缓存的答案和重新计算的结果一致。辅助码路径的基础部分原本按去掉 `'` 的字母缓存（SD:458），手打的 `'` 在那里会丢掉、纠错照给；现在 `generate_with_helpcodes` 改按保留 `'` 的基础输入（`trim_trailing_letters_preserve_delimiters`）缓存和准入，和直接打这段基础输入得到同一份答案，没有 `'` 时它就是原来的字母键。纠错整句还要求至少 3 个音节，这是 `merge_lattice_candidates` 的既有规则。
- **排序不抢首选**：纠错整句一律排在全码词条和整句块之后，不看 `typo_lead_margin`。全拼按那个边距判断，默认是无穷大，效果相同；双拼这里写死，是为了不让以后调全拼边距的人顺带改了双拼。整句块可能是空的（词网格关着、键盘重排器又没给出一条），这时那个位置是第 0 位，所以 `typo_sentence_seat` 至少让出第一位，表是空的就不给纠错行。开着模糊音时，`ShuangpinEngine::append_fuzzy` 会把整张表按覆盖的按键长度稳定排序；整句块为空、全码又没有词条时，纠错行是唯一覆盖全部按键的一行，会被排回第 0 位，所以排序之后 `sort_by_coverage` 发现首位是纠错行就把它和第二行对调。
- **重排器也不把它提上来**：引擎之外还有一层排序，`msime-input-runtime` 的句子模型重排（含桌面的停顿重排）和网页引擎都调 `ordering::rerank_pick`。那里原本对所有方案一视同仁：任何一行带纠错标记，整张表就失去词典命中的豁免，纠错行也和别的行一起让模型比较，可以被转到首位、被空格上屏。`ordering::corrections_may_lead` 对双拼返回 false，双拼的纠错行在重排时不算回答了按键，也不撤豁免，重排结果和没有纠错行时一样。全拼的纠错行本来就允许领先，照旧。行的 `pinyin` 是输入的按键，`canonical_pinyin` 是改正后的全拼读音，`corrected_from` 是输入的按键（宿主把它显示在注释位，和全拼一样）。
- **不打折、不学习、不抑制**：`collect_shuangpin_typo_edges` 传给 `accepted` 的恒为 0。纠错档案是按全拼字母误触统计的，套到双拼键位上没有意义。`SchemeType::supports_autocorrect` 对双拼仍是 false，所以 `learn_accepted_typos` 和 `learn_rejected_correction` 不会把双拼的选择写进档案或抑制表；`ime/mod.rs` 的纠错抑制也仍只认全拼。

## Alternatives considered

- **新增 `shuangpin.autocorrect_neighbor` 设置项，各平台 UI 按方案读写**（分诊建议的做法）：好处是全拼和双拼可以分开开关，旧客户端也会忽略新字段。产品决定是沿用现有开关、按当前方案作用：一个开关覆盖两种方案，用户不用分辨；同步字段表、三端设置页和旧配置兼容都不用动。代价是想只关双拼纠错的人做不到。
- **在 `shuangpin/typo_edges.rs` 里另写一份规划**：不必改全拼文件，但弱位置、预算、去重、查词缓存会有两份。ARCHITECTURE.md 写过两份实现会漂移，`make_sentence_lattice_options` 当初就是因为排序修复只到了一边才合成一处（engine PR #156）。抽闭包后全拼只多一层转发，分配预算测试 `planning_reuses_the_variant_buffer_across_positions` 不变。
- **双拼也按纠错档案打折、也学习接受过的纠错**：可以让常犯的误触越来越容易被纠回来。但档案的键是全拼音节对，全拼和双拼的同一对音节来自完全不同的按键，混在一张表里会互相干扰。要做就得给档案加方案维度，这超出了这次的范围。
- **把切分对不齐的输入也纠**（按错的键让某一对不再是合法编码、切分错位）：这种误触同样常见，但字面切分里已经有单键段，`merge_lattice_candidates` 根本不解码。要纠就得先按偶数位强行配对再生成变体，等于换一套切分，这次没做。

## Consequences

- **收益**：所有平台的双拼都有纠错整句，宿主不复制算法；Android、iOS 和共享设置页上的「拼音纠错」在双拼下终于名副其实。正确输入的首选和整句块不受影响：纠错整句只追加在它们之后。
- **代价**：在真实词库（dict-v2.0.14，校验过的资源目录）上，用 sentences-v1 和 neutral 两个评测集每隔一条抽一条，共 583 句，编成小鹤码逐键查询。每句开始前清空引擎缓存，属于偏悲观的冷缓存测法。开纠错比关纠错每键平均多 0.20 ms，p95 从 2.13 ms 到 2.70 ms，p99 从 22.4 ms 到 26.4 ms。同一批输入走全拼时，现有纠错让 p95 从 1.20 ms 到 2.70 ms、p99 从 28.9 ms 到 48.4 ms。双拼新增的开销没有超过全拼已经付出的。
- **已知上限**：召回保守。在上面的 583 句里，把中间音节的韵母键按成邻键（仍是合法编码）造 573 条误触，纠错整句给出原句的有 38 条，都在第 2 位。全拼用同样方式造的 538 条误触，纠回 57 条。正确输入里有 36 句会在整句块之后多一行纠错整句，首选一次都没变。要提高召回，得调罚分，或者给双拼加学习，两者都要拿评测集来定。
- **微软双拼的 `;` 没有邻键**：`pinyin::typos::keys_adjacent` 只认字母键，所以 `;`（ing）既不会被换成邻键，按成 l 或 p 时也纠不回 `;`，只有两键对调能碰到含 `;` 的编码。要补得改全拼也在用的键盘布局，这次没动。
- **重访信号**：用户反馈双拼纠错行太吵或太少；全拼调整 `typo_lead_margin` 或罚分时，要决定双拼是否跟随；纠错档案加了方案维度之后，双拼可以接上打折和学习；有人想让双拼纠错也能领先时，要同时改 `typo_sentence_seat` 和 `ordering::corrections_may_lead`。macOS 设置页的两个开关仍然叫「全拼乱序纠错」「全拼邻键纠错」，但那张卡片在设置页上是隐藏的（见 `AppearancePreferences.mm` 的注释），用户看不到；Windows 默认配置 `config.default.toml` 里那两项的注释已改成全拼和双拼共用。

## Verification

- `cargo test -p msime-engine`：`shuangpin::typo_edges::tests` 核对各方案的变体表，`a_custom_profile_gets_variants_for_its_own_layout` 用两张不同的自定义表，断言变体各按自己的键位算；`shuangpin::tests` 的 `xiaohe_neighbour_key_typo_is_corrected`（`mwgfxi` 纠回没关系）和 `ziranma_neighbour_key_typo_is_corrected`（`mzgrci`）都断言纠错行在字面整句之后；`correct_input_keeps_its_first_candidate` 断言正确输入开关前后候选相同，开关关着或手打 `'` 时不纠；`the_switch_is_read_on_every_query` 断言开关来回切换时不读旧缓存；`each_profile_corrects_its_own_codes` 让两个方案交替查询，断言各自只按自己的编码纠错。会话层的 `shuangpin_offers_the_typo_sentence_under_the_autocorrect_switch` 选中纠错行后上屏没关系，`a_rebuilt_session_corrects_with_its_new_profile` 按宿主换方案的方式重建会话（小鹤、自然码、再回小鹤），断言每次都按新方案的编码纠错。把准入改成恒不准入时，这些测试全部失败。
- 不抢首选的三处守卫各有测试，把对应修正撤回时各自失败：`ordering::tests::only_shuangpin_corrections_stay_behind` 和 `msime-input-runtime` 的 `a_shuangpin_correction_is_never_promoted_and_keeps_the_dictionary_exemption`（用合成句子模型重排一张带纠错行的表：全拼下纠错行被提到首位、词典命中失去豁免，双拼下顺序不动）；`shuangpin::dictionary::tests::the_typo_sentence_never_takes_the_first_seat`（整句块为空时纠错行坐第 1 位，表空时不给）和 `shuangpin::engine::tests::sorting_by_coverage_keeps_the_typo_sentence_off_the_first_seat`（按长度排序后纠错行仍不在首位）；`a_manual_delimiter_disables_correction_with_a_helpcode`（`mw'gfxim` 带单辅助码时不纠，`mwgfxim` 照纠，同一引擎来回查不串缓存）。
- 上面的延迟和召回数字来自一次性的测量，没有提交测量代码。仓库的 `convert_eval` 和 `rerank_latency` 只跑全拼，而且 `prepare_options` 默认关纠错，所以它们量不到这条路径。
- 全拼没有被改坏：在同一台机器、同一份按锁文件安装的资源目录上（加上 `settled-model.lock.json` 的 `sentence-model-desktop.safetensors`，所以停顿重排也在测），`convert_eval` 的 sentences、harvested、neutral、words、nine-key 五组输出与改动前的提交 75473cdbf 逐字节相同。五组都和仓库里提交的基线有出入，改动前的提交也一样，所以那是基线本身落后于 develop，不是这次改动。
- `rerank_latency` 的 16 ms 预算受机器负载左右，这台机器同时跑 CI，负载在 70 到 160 之间。负载相近时把改动前后的二进制背靠背交替跑了两轮，p95 分别是 17.47 ms 对 19.49 ms、15.75 ms 对 16.23 ms（前者是改动后），没有退化。
