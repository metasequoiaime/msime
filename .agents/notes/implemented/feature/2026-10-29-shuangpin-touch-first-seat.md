# Agent Note: 双拼触点纠错坐首选的规则

Status: implemented

## Problem

#10018 的第二项交付要让落点支持、明显比字面整句好的双拼纠错整句坐首选。[双拼纠错整句](2026-10-09-shuangpin-typo-sentence.md)按 #6034 的产品决定一律不抢首选：`typo_sentence_seat` 至少让出第一位，`ShuangpinEngine::query` 返回前由 `keep_typo_sentence_off_first_seat` 把坐到首位的纠错行挪开，`ordering::corrections_may_lead` 让重排器不拿双拼纠错行比较。[触点计价](2026-10-27-shuangpin-touch-correction.md)和[两键码学习](2026-10-28-shuangpin-typo-learning.md)之后，纠错整句已经按落点计价、被拒绝的改法也有记录，剩下的问题是：在什么条件下首选可以换成纠错整句，而不把打对的输入改坏。

首选被空格直接上屏，改错了用户要退格重打，所以这条规则要先在评测上证明精度够，才开。产品决定：规则挂在现有的「拼音纠错」开关下，开关默认开着；达不到门槛时 m_sp 取无穷大，代码留着、功能不起作用，报告数字。被首选改过的那一行在三端要显式标出（强调色和读屏的「已纠正」），引擎和宿主之间的契约是视图里候选的可选字段 `confident_correction`。

约束：桌面宿主和实体键盘不送触点，结果逐字节不变（G0）；没有触点、没过规则时，三处守卫的行为与以前相同。

## Decision

纠错整句只有同时满足下面六条才坐第一位，并在 `WordItem::confident_correction` 上标出；三处守卫只豁免带这个标记、回答整串按键的那一行；视图只给真正坐在第一位的那一行标 `confident_correction`。引擎默认的 m_sp（`TouchTuning::lead_margin`）是无穷大：在评测上没有一组参数同时达到精度和净收益两道门槛，规则现在不起作用。

### 首选规则

- (a) 请求的每个键都带触点，几何有效（`ShuangpinEngine` 的 `confident_correction_allowed` 看 `key_touches` 全为 `Some`、与 `raw_input` 等长、有几何；词典再看这段 `rows_key` 的触点全在）。撤回选词后重打的那一段、光标前缀查询、无障碍点击拿不到触点，天然不会被首选改掉。
- (b) `TypoSentence::leads(m_sp)`：纠错整句（已扣罚分）比字面整句高出每条纠错边 m_sp。m_sp 是双拼触点专用的 `TouchTuning::lead_margin`，不复用全拼值为无穷大的 `typo_lead_margin`，只在 Rust 侧可调（`host::Session::set_touch_tuning`、`convert_eval --touch-tuning lead=…`），宿主经 FFI 设不了，不面向用户。nonleading 模式取 64 条 n-best 里第一条带纠错边的路径，它就是分数最高的纠错路径，前提是 n-best 按分数降序：`lattice::decode::tests::the_nonleading_typo_sentence_is_the_best_scoring_typo_path` 钉住两者。
- (c) 改动的每一键在落点下改过去的原价都不超过 P_LEAD（`TouchTuning::lead_penalty`，`typo_edges::touch_leads_correction`）。按不打学习折扣的原价算：接受过的改法分数更高、更容易满足 (b)，但按在键心的字母改到邻键的原价接近 B + s，永远挡在线外。对调和没有触点的位置照平面表付 20。
- (d) 字面没有全码词条（`whole_sentence_insert_position(..) == 0`），而且第一行就是字面整句块：坐上去以后字面整句紧跟在第二位。
- (e) 改动的每一对 (按到的码 → 想按的码) 都没被拒绝过（`ShuangpinDictionary::correction_rejected`，键码对与学习时同一个算法 `touch_correction_pairs`，对不上的当作否决）。被抑制的输入根本不带触点（`apply_shuangpin_autocorrect_suppression`），自然不坐首选。自定义方案不学，没有拒绝记录可查，一律不坐首选。
- (f) 不是辅助码：末尾段、句中约束、大写、末尾直接辅助码（`active_helpcode_query`）、手打的 `'` 都不允许；不是本地模式、专用英文（会话层触点不进方案，宿主快照在这两种状态下不给这一列）。
- 另外它一定也是落点支持、留在座位上的那一行（`touch_supported`，`show_margin`、`show_penalty` 两道闸），所以 runtime 的 `runner_up_order_with` 不把它当整句读法挪走。P_LEAD 超过 `show_penalty`（14）时 (c) 不再更严。

### 座位与守卫

- 满足时词典把它插到第 0 位而不是整句块之后；`typo_sentence_seat` 本身不改（它的 `.max(1)` 只对没标记的行起作用）。
- `keep_typo_sentence_off_first_seat(rows, keys)` 只豁免首位带标记、覆盖整串按键的那一行；上一键缓存下来、这一键成了前缀的答案照旧让开。`settle_confident_correction` 在 `query` 返回前清掉不在第一位、或不回答整串按键的行的标记；清掉标记的行如果还排在字面整句前面，挪回字面整句块之后，也就是没有首选规则时 `typo_sentence_seat` 给它的座位。会走到这一步的是模糊音：坐上首选以后桶里第一行是纠错整句（`sentence_association`），(d) 又保证没有非模糊的全码词条，`sort_by_coverage` 就把全码的模糊词条排到它前面，只清标记的话它会以没有标记的改正排在字面整句前面。全码的模糊词条和 (d) 的字面全码词条一样，让纠错整句坐回原来的座位；这时首选不是纠错行，用户选别的行也不记拒绝，没坐过首选的改法本来就不该被否决。
- runtime 不改排序：首行是双拼纠错行时 `rerank_needs_model` 为假（`corrections_may_lead`），模型不跑；它带 `touch_supported`，`runner_up_order_with` 不挪它，字面整句在第二位，前三条字面读法照旧。
- 请求允许首选（`lead_allowed`）而且 m_sp 不是无穷大时，触点签名末尾多一个标记，允许和不允许时的答案各存各的缓存：末尾辅助码的基础部分读不到坐了首选的整句答案，与没有这条规则时逐字节相同。m_sp 是无穷大时 `ShuangpinEngine` 连请求也不看。

### 视图契约

- 引擎快照 `EngineSnapshot::candidate_confident_correction` 与候选同步排列，一行都没有时为空（桌面永远为空），本地模式和专用英文下也为空。runtime 在重排、插件插入和移除时和 `candidate_touch_supported` 一样同步它。
- 视图 `Candidate::confident_correction`：只在第一位、而且那一行带标记时为真；为假时 `skip_serializing_if`，JSON 里没有这个字段，默认视图和快照逐字节不变（golden 快照只记 `corrected_from` 和 `sentence_association`）。插件或固定位置把它挪离第一位以后，它只是一条普通的纠错行。
- 宿主读它：给这一行画平台已有的强调色，读屏标签后面接「，已纠正」；`annotation` 里的原始按键（`corrected_from`）照旧。三端宿主的接线不在这篇。

### 学习

- 拒绝的记账沿用[两键码学习](2026-10-28-shuangpin-typo-learning.md)：显示出来的第一行是纠错行、用户选了别的行，记 `rejected++`。现在纠错行只有按首选规则才坐得上第一行，这一票就是 (e) 的否决。接受照旧。

## 扫参

资源是 `desktop-dictionary.lock.json`（dict-v2.0.39）那 12 个文件，逐个按锁核对过 sha256，目录里没有 `sentence-model-desktop.safetensors`（每次 stderr 都打出 `no settled model at …`）；几何是 Android 两份导出（`android-portrait`、`-microsoft`），种子 20261027，其余计价参数是引擎默认值（B=8、s=12、`show_margin` 6、`show_penalty` 14）。门槛按方案第八节：clean 集首选被改率 σ=0 为 0、总体不超过 0.1%、每个方案不超过 0.2%；realistic 净 top-1 比 PR1b（m_sp 无穷大）至少高 3 个点。

### 调参集

sentences-v1、particles-v1、v2（455 条能编码），小鹤和微软，clean 和 realistic，σ∈{0.25, 0.35} × b∈{0, 0.1}（clean 共 3640 条）。「净 top-1」是 8 组 realistic 的 top1 减去同条件 m_sp 无穷大的平均（百分点），「改对」是 realistic 里首选被换成纠错行且正好是金标准的条数 / 换成纠错行的条数。σ=0 在这一步不跑（σ=0 时落点在键心，(c) 挡掉一切，后面的整句族验证了这一点）。

| m_sp | P_LEAD | clean 首选被改 | 小鹤 / 微软 | 净 top-1 | 8 组范围 | 改对 |
|---|---|---|---|---|---|---|
| 0 | 8 | 1 / 3640（0.03%） | 0 / 1 | +0.49 | +0.0 至 +1.1 | 17 / 50 |
| 0 | 9 | 6 / 3640（0.16%） | 2 / 4 | +1.26 | +0.3 至 +2.2 | 46 / 136 |
| 0 | 10 | 8 / 3640（0.22%） | 2 / 6 | +1.99 | +1.5 至 +2.9 | 74 / 211 |
| 2 | 10 | 7 / 3640（0.19%） | 2 / 5 | +1.85 | +1.1 至 +2.7 | 68 / 194 |
| 4 | 10 | 6 / 3640（0.16%） | 2 / 4 | +1.75 | +0.9 至 +2.5 | 63 / 184 |
| 0 | 11 | 12 / 3640（0.33%） | 4 / 8 | +2.75 | +2.0 至 +3.6 | 101 / 282 |
| 0 | 12 | 16 / 3640（0.44%） | 7 / 9 | +3.40 | +2.7 至 +4.4 | 125 / 361 |
| 1 | 12 | 14 / 3640（0.38%） | 5 / 9 | +3.15 | +2.2 至 +3.8 | 115 / 342 |
| 2 | 12 | 11 / 3640（0.30%） | 4 / 7 | +2.98 | +1.8 至 +3.7 | 109 / 330 |
| 4 | 12 | 10 / 3640（0.27%） | 4 / 6 | +2.76 | +1.6 至 +3.8 | 100 / 311 |
| 6 | 12 | 9 / 3640（0.25%） | 3 / 6 | +2.59 | +1.3 至 +3.5 | 94 / 290 |
| 0 | 14 | 24 / 3640（0.66%） | 10 / 14 | +3.70 | +3.1 至 +5.3 | 136 / 409 |
| 2 | 14 | 18 / 3640（0.49%） | 6 / 12 | +3.26 | +2.4 至 +4.0 | 119 / 376 |
| 4 | 14 | 16 / 3640（0.44%） | 6 / 10 | +2.97 | +2.0 至 +4.0 | 108 / 355 |
| 6 | 14 | 15 / 3640（0.41%） | 5 / 10 | +2.82 | +1.8 至 +3.7 | 102 / 330 |
| 8 | 14 | 13 / 3640（0.36%） | 4 / 9 | +2.64 | +1.8 至 +3.6 | 95 / 313 |
| 8 | 28 | 与 8 / 14 逐项相同 | | | | |

- 代价和收益几乎按同一个比例涨落：P_LEAD 每放宽一档，clean 被改的条数和净 top-1 一起涨；在同一个 P_LEAD 上抬高 m_sp，两者也一起降，而且降得比单收 P_LEAD 更不划算（m_sp 4、P_LEAD 14 与 m_sp 0、P_LEAD 12 被改的条数相同，净 top-1 低 0.4 个点）。
- 坐上首选的纠错行只有约三分之一正好是金标准；净收益为正，是因为改错的那些大多本来字面首选也不对。clean 集被改的用例里有少数其实改对了（小鹤 σ=0.25 `mwwftijngewo`，字面首选「没问题叫个我」，改成金标准「没问题交给我」），门槛按「首选变了」计，一并算作代价。其余是语言模型更偏爱改过的读法：「要是把围墙拆了」改成「于是把围墙拆了」，「在两个分站赛中收获冠军」改成「在两个奋战袋中收获冠军」，「重建舜帝庙」改成「重睑术地面」，「踏春行人入职」改成「踏春商人入职」。这类错误靠落点的闸挡不住：落点贴着键边时，打对和打错的输入在落点上看起来一样。
- P_LEAD 到 14 以后再放宽没有作用：坐首选的那一行同时要过 `show_penalty` 14。

### 留出集

sentences-neutral-v1（1064 条能编码），小鹤和微软，σ∈{0.15, 0.25, 0.35} × b∈{0, 0.1}（clean 共 12768 条），净 top-1 按 σ 分开平均：

| m_sp | P_LEAD | clean 首选被改 | 小鹤 / 微软 | 净 top-1 σ=0.15 / 0.25 / 0.35 |
|---|---|---|---|---|
| 0 | 9 | 37 / 12768（0.29%） | 20 / 17 | +0.13 / +2.20 / +0.83 |
| 0 | 10 | 54 / 12768（0.42%） | 28 / 26 | +0.13 / +3.10 / +1.20 |
| 0 | 12 | 102 / 12768（0.80%） | 51 / 51 | +0.13 / +3.95 / +1.93 |

留出集上代价比调参集高一倍左右（m_sp 0、P_LEAD 9 从 0.16% 到 0.29%），收益更低。

### 整句族

最接近门槛的一组（m_sp 0、P_LEAD 9）按 README 的完整组合跑了一遍：五个方案 × clean、noisy、realistic × σ∈{0, 0.15, 0.25, 0.35} × b∈{0, 0.1}（clean 共 60760 条），对照是这一期重新生成的基线（m_sp 无穷大）。

- clean 首选被改 82 / 60760（0.135%），超过 0.1% 的总门槛；按方案是小鹤 23、自然码 17、手心 16、微软 11、智能 ABC 15（每个方案 12152 条，最高 0.189%，在 0.2% 之内）；按 σ 是 0、1、33、48。σ=0 一条都没变，(c) 在键心落点上挡住了全部改动。82 条里只有 1 条改成了金标准（小鹤 σ=0.25 `mwwftijngewo`），其余都是语言模型更偏爱的错读法，例如「政府令中指出」改成「政府瓶中指出」、「我正在贵州旅行」改成「我正在贵州铝壶」、「让荆州古城日益绽放出新的迷人魅力」在三个方案上都改成「糖精周谷城日益绽放出新的迷人美丽」。逐条清单见下面的[首选变了的 clean 用例](#首选变了的-clean-用例)。
- realistic 净 top-1：σ=0 为 0，σ=0.15 平均 +0.13 个点，σ=0.25 平均 +2.06（各组 +1.4 至 +2.8），σ=0.35 平均 +0.79，所有组平均 +0.74，离 +3 个点很远。realistic 里首选换成纠错行的 1437 次中 462 次是金标准。
- noisy 集 top-1 升得多（σ=0.15 平均 +13.5 个点，0.25 为 +5.3，0.35 为 +1.1）：按错之后纠错行坐上首选的那部分正是收益，只是 realistic 里按错的用例占少数，摊下来不够。

### 结论

没有一组 (m_sp, P_LEAD) 同时过两道门槛：净 top-1 要到 +3 个点，调参集上要 P_LEAD 12（clean 被改 0.30%–0.44%，σ≥0.25 那几组），留出集上同一组是 0.80%；clean 被改最接近门槛的 m_sp 0、P_LEAD 9 在完整的整句族上是 0.135%（仍超过 0.1%），净 top-1 只有平均 +0.74、σ=0.25 也只有 +2.06 个点。`lead_margin` 默认取无穷大，`lead_penalty` 默认 10（无穷大的 m_sp 下不起作用，留作下次扫参的起点），`touch::tests::the_default_tuning_is_the_swept_one` 钉住这组默认值。typo_beam 没有扫：它只决定纠错路径找不找得到，代价和收益同样会一起涨落，不改变上面的结论。

### 首选变了的 clean 用例

m_sp 0、P_LEAD 9 这一点上，clean 集里实际首选与不送触点时不同的每一条，供人工复查。三份清单从 Studio 上的报告（`/Volumes/Workspace/rbuild/evidence/sp-first-seat-20261029/out/` 的 `full-m0-p9-sentences.json`、`tune-m0-p9.json`、`hold-m0-p9.json`，每组 clean 的 `first_seat.changed`）逐条抄出，报告本身没有提交。「坐了首选」是现首选是不是按首选规则坐上去的纠错行：三份清单全部是，没有一条首选变化来自别的原因。

<details>
<summary>整句族，82 条（小鹤 23、自然码 17、手心 16、微软 11、智能 ABC 15；改成金标准的 1 条）</summary>

| 方案 | 几何 | σ | b | id | 按键 | 金标准 | 原首选 | 现首选 | 坐了首选 |
|---|---|---|---|---|---|---|---|---|---|
| xiaohe | android-portrait | 0.25 | 0.0 | `s-056` | `mwwftijngewo` | 没问题交给我 | 没问题叫个我 | 没问题交给我 | 是 |
| xiaohe | android-portrait | 0.25 | 0.0 | `h-0130` | `vgfulkvsviiu` | 政府令中指出 | 政府令中指出 | 政府瓶中指出 | 是 |
| xiaohe | android-portrait | 0.25 | 0.0 | `h-0383` | `dzbusinvzisoys` | 都不似女子所用 | 都不死女子所用 | 都不四女自作用 | 是 |
| xiaohe | android-portrait | 0.25 | 0.0 | `h-0462` | `rugonbyexlhxfwuc` | 如果您也想花费少 | 如果您也想花费少 | 如果您也像话费神 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0202` | `dzzdvelijuhh` | 都在这里举行 | 都在这理据行 | 都在这里出航 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0208` | `svhzfjdjdcle` | 随后反弹到了 | 随后反但到了 | 随后返抵到了 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0309` | `wovgzdgvvzlvhh` | 我正在贵州旅行 | 我正在贵州旅行 | 我正在贵州铝壶 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0322` | `riyixlnscymjyj` | 日益向农村蔓延 | 日一项农村蔓延 | 如意巷农村蔓延 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0402` | `buychfnidelcuile` | 不要恨你的老师了 | 不要很你的老实了 | 不要很难得老实了 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0506` | `whvetdyhdvzijiuo` | 望着太阳对自己说 | 王者太阳对自己说 | 王者太阳队自己是 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0833` | `yenglkhljbuzuicvcjruxb` | 也能令黄金首饰璀璨如新 | 也能领黄金首饰璀璨如新 | 也能灵光金首饰璀璨如新 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `s-031` | `womfykgdzcyidmqu` | 我们应该早一点去 | 我们应该早一点去 | 我们应该早已先去 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `h-0123` | `dakdkduiyyhh` | 打开开始运行 | 打开开始运行 | 打开开始宇航 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `h-0203` | `dzngkdqicimf` | 都能开启此门 | 都能开其次门 | 都能开启之门 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `h-0479` | `xuhllimazdmfqixx` | 徐晃立马在门旗下 | 徐晃立马在门旗下 | 虚晃立马塞门旗下 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `h-0904` | `duiagszojljmiiwftidcxl` | 督察工作将坚持问题导向 | 督查工作将坚持问题导向 | 督查工作将建是问题导向 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `p073` | `viuiya` | 只是呀 | 只是呀 | 只是用 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0395` | `pmdeuibprfdeqm` | 骗的是别人的钱 | 骗的是别人的钱 | 练的是别人的钱 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0438` | `qpuibcvgvrhuyyhh` | 切实保证专户运行 | 切实保证专户云行 | 妾室保证专户宇航 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0462` | `rugonbyexlhxfwuc` | 如果您也想花费少 | 如果您也想花费少 | 如果您也像话匪巢 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0679` | `whziluiuughzhfmntn` | 王子露出生后很苗条 | 王子路出生后很苗条 | 王子路出生后海苗条 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-1033` | `gurjxmzdvuuhlebcxmjmgudexbju` | 固然现在住上了保险坚固的新居 | 固然现在住上了保险坚固的新剧 | 固然现在长上了保险坚固的新剧 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `p-0018` | `mkvjiemjdclu` | 命战车满道路 | 名战车慢道路 | 铭感车迷道路 | 是 |
| ziranma | android-portrait | 0.25 | 0.0 | `h-0123` | `daklkluiyphh` | 打开开始运行 | 打开开始运行 | 打开开始晕乎 | 是 |
| ziranma | android-portrait | 0.25 | 0.0 | `h-0435` | `jtdyuiwzgowzmnde` | 决定是为国为民的 | 决定是为国为民的 | 决定是为果味您的 | 是 |
| ziranma | android-portrait | 0.25 | 0.0 | `h-0449` | `labaivqilejdjply` | 喇叭吹起了将军令 | 喇叭吹起了将军令 | 喇叭吹气可将军令 | 是 |
| ziranma | android-portrait | 0.25 | 0.0 | `h-0676` | `qqkjhjgomjhwdewhvi` | 求看韩国漫画的网址 | 求看韩国漫画的网址 | 求满汉国漫画的网址 | 是 |
| ziranma | android-portrait | 0.25 | 0.0 | `h-0757` | `vitydethwumfybbzgruh` | 只听的堂屋门又被关上 | 只听的堂屋门又被关上 | 只听段塘屋门又被关上 | 是 |
| ziranma | android-portrait | 0.25 | 0.0 | `p-0169` | `yndobickckdajp` | 因躲避曹操大军 | 吲哚比曹操大军 | 阴道壁曹操大军 | 是 |
| ziranma | android-portrait | 0.25 | 0.1 | `h-0888` | `ualenackrfyigecoubbuji` | 杀了那曹仁一个措手不及 | 啥了那曹仁一个措手不及 | 傻了吗草人一个措手不及 | 是 |
| ziranma | android-portrait | 0.25 | 0.1 | `p-0036` | `ruhekfxq` | 如何肯休 | 如何肯修 | 如何宽袖 | 是 |
| ziranma | android-portrait | 0.35 | 0.0 | `h-0208` | `svhbfjdjdkle` | 随后反弹到了 | 随后反但到了 | 随后反倒到了 | 是 |
| ziranma | android-portrait | 0.35 | 0.0 | `h-0700` | `quvbdkukyhdehoiepc` | 衢州到邵阳的火车票 | 衢州到邵阳的火车票 | 衢州盗号羊的火车票 | 是 |
| ziranma | android-portrait | 0.35 | 0.0 | `h-0712` | `rrjmyphhdesuduhfmj` | 软件运行的速度很慢 | 软件运行的速度很慢 | 软件晕乎的速度很慢 | 是 |
| ziranma | android-portrait | 0.35 | 0.1 | `h-0035` | `bmpgdkljlude` | 便碰到拦路的 | 便碰到拦路的 | 变冷捣烂录的 | 是 |
| ziranma | android-portrait | 0.35 | 0.1 | `h-0198` | `veuiyibfiitj` | 这是一本侈谈 | 这是一本侈谈 | 这是一本吃饭 | 是 |
| ziranma | android-portrait | 0.35 | 0.1 | `h-0599` | `jntmwomfjuhhzotjhv` | 今天我们举行座谈会 | 今天我们局行座谈会 | 今天我们菊虎座谈会 | 是 |
| ziranma | android-portrait | 0.35 | 0.1 | `h-0888` | `ualenackrfyigecoubbuji` | 杀了那曹仁一个措手不及 | 啥了那曹仁一个措手不及 | 傻了吧草人一个措手不及 | 是 |
| ziranma | android-portrait | 0.35 | 0.1 | `h-0908` | `gzlehzujjpsskbqidejihv` | 给了黑山军松口气的机会 | 给了黑山君松口气的机会 | 给了黑山纯送口气的机会 | 是 |
| ziranma | android-portrait | 0.35 | 0.1 | `h-1103` | `rhjyvbguigriyivjfhiuxndemirfmzli` | 让荆州古城日益绽放出新的迷人魅力 | 让荆州古城日益绽放出新的迷人魅力 | 糖精周谷城日益绽放出新的迷人美丽 | 是 |
| shoudao | android-portrait | 0.25 | 0.0 | `h-0111` | `zheixcjiqqkg` | 总是心急求快 | 总是心急求快 | 总是新机秋景 | 是 |
| shoudao | android-portrait | 0.25 | 0.1 | `h-0203` | `dsnfklqicimk` | 都能开启此门 | 都能开其次门 | 都能开启西门 | 是 |
| shoudao | android-portrait | 0.25 | 0.1 | `h-0816` | `gktheihejqddlgiklxdn` | 跟同事喝酒到凌晨两点 | 跟同时喝酒到凌晨两点 | 国通适合就到凌晨两点 | 是 |
| shoudao | android-portrait | 0.35 | 0.0 | `h-0035` | `bnpfddljlude` | 便碰到拦路的 | 便碰到拦路的 | 变频捣烂录的 | 是 |
| shoudao | android-portrait | 0.35 | 0.0 | `h-0130` | `vffulgvhviiu` | 政府令中指出 | 政府令中指出 | 政府领涨指出 | 是 |
| shoudao | android-portrait | 0.35 | 0.0 | `h-0151` | `gygueisjlnyy` | 港股十三连阳 | 港股是三连养 | 港股十三炼狱 | 是 |
| shoudao | android-portrait | 0.35 | 0.0 | `h-0203` | `dsnfklqicimk` | 都能开启此门 | 都能开其次门 | 都能开启西门 | 是 |
| shoudao | android-portrait | 0.35 | 0.0 | `h-0318` | `moqgleqiyedieu` | 摸清了企业底数 | 摸清了企业底数 | 抹去了企业抵数 | 是 |
| shoudao | android-portrait | 0.35 | 0.0 | `h-0780` | `vweianqthtbdefijghzo` | 抓实安全环保生产工作 | 抓实安全环保生产工作 | 这是安全环保生产工作 | 是 |
| shoudao | android-portrait | 0.35 | 0.0 | `h-0816` | `gktheihejqddlgiklxdn` | 跟同事喝酒到凌晨两点 | 跟同时喝酒到凌晨两点 | 很痛适合就到凌晨两点 | 是 |
| shoudao | android-portrait | 0.35 | 0.0 | `h-1103` | `ryjgvsguifriyivjfyiuxcdemirkmmli` | 让荆州古城日益绽放出新的迷人魅力 | 让荆州古城日益绽放出新的迷人魅力 | 糖精周谷城日益绽放出新的迷人美丽 | 是 |
| shoudao | android-portrait | 0.35 | 0.1 | `s-031` | `womkygglzdyidnqu` | 我们应该早一点去 | 我们应该早一点去 | 我们应该早已得去 | 是 |
| shoudao | android-portrait | 0.35 | 0.1 | `h-0045` | `bgitblnagehd` | 冰川白哪个好 | 冰川白那个好 | 冰川百纳挂号 | 是 |
| shoudao | android-portrait | 0.35 | 0.1 | `h-0352` | `qpnilfvegeyyzi` | 瞧你冷这个样子 | 瞧你冷这个样子 | 瞧你愣着这样子 | 是 |
| shoudao | android-portrait | 0.35 | 0.1 | `h-0525` | `qqmibndaghbnkjqq` | 球迷边打工边看球 | 球迷边打工边看球 | 球迷变大干煸看球 | 是 |
| shoudao | android-portrait | 0.35 | 0.1 | `h-0599` | `jctnwomkjuhyzotjhv` | 今天我们举行座谈会 | 今天我们局行座谈会 | 今天我们菊虎座谈会 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.0 | `h-0134` | `j;sevebmduhk` | 景色这边独好 | 景色这边都好 | 景色这边符号 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.0 | `h-0310` | `vauitvjnuvugtl` | 扎实推进水生态 | 扎实推进水生态 | 炸鸡腿进水生态 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.0 | `h-0896` | `rjhbysuhyaiidivuxwzvip` | 然后用上牙齿抵住下嘴唇 | 然后用上牙齿地主下嘴唇 | 然后用上牙齿地虎侠嘴唇 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.0 | `p-0187` | `zldmjwuplixwjdhb` | 在电价顺利下降后 | 在店家顺利下降后 | 在电话顺利下降后 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.1 | `h-0712` | `rrjmyphhdesuduhfmj` | 软件运行的速度很慢 | 软件运行的速度很慢 | 软件云冈的速度很慢 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0417` | `ufmefgbaniivllle` | 什么风把你吹来了 | 什么风把你吹来了 | 什么风把击锤来了 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0749` | `xmrexxfztghbrelzy;kd` | 先热血沸腾后热泪盈眶 | 先热血沸腾后热泪盈眶 | 先热谢飞滕州热泪盈眶 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-1103` | `rhj;vbguigriyivjfhiuxndemirfmzli` | 让荆州古城日益绽放出新的迷人魅力 | 让荆州古城日益绽放出新的迷人魅力 | 糖精周谷城日益绽放出新的迷人美丽 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `p095` | `uujula` | 数据啦 | 数据啦 | 数据流 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.1 | `h-0123` | `daklkluiyphh` | 打开开始运行 | 打开开始运行 | 打开开始云冈 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.1 | `h-0739` | `geyavbyihkxdmuzljmvs` | 个亚洲一号项目在建中 | 戈雅周毅浩项目在建中 | 戈雅周一好像木作减重 | 是 |
| smart_abc | android-portrait | 0.15 | 0.1 | `h-1031` | `zoklfaderfdmdwnkpqaiykqrbudi` | 做开发的人对电脑配置要求不低 | 做开发的人堆电脑配置要求不低 | 早开发的人堆电脑配置要求不敌 | 是 |
| smart_abc | android-portrait | 0.25 | 0.0 | `h-0462` | `rugoncyexthdfqvk` | 如果您也想花费少 | 如果您也想花费少 | 如果您也像话非说 | 是 |
| smart_abc | android-portrait | 0.25 | 0.0 | `h-0734` | `vhvultbumftsgkfaeuhb` | 上述两部门通告发出后 | 上述两部门通告发出后 | 尚书令部门通告发出后 | 是 |
| smart_abc | android-portrait | 0.25 | 0.0 | `h-1103` | `rhjyabguegriyiajfheuxcdemirfmqli` | 让荆州古城日益绽放出新的迷人魅力 | 让荆州古城日益绽放出新的迷人魅力 | 柔静周谷城日益绽放出新的迷人美丽 | 是 |
| smart_abc | android-portrait | 0.25 | 0.0 | `p-0053` | `dhmwlctufayiwlhojiby` | 当面临突发意外或疾病 | 当面临突发意外或疾病 | 当面淋浴法医外活疾病 | 是 |
| smart_abc | android-portrait | 0.25 | 0.1 | `h-0529` | `deeaxtbwvnaehuzm` | 的茶香便顺着壶嘴 | 德差想变顺着糊嘴 | 的哇想变顺着壶嘴 | 是 |
| smart_abc | android-portrait | 0.25 | 0.1 | `h-0757` | `aitydethwumfybbqgpvh` | 只听的堂屋门又被关上 | 只听的堂屋门又被关上 | 只听德方屋门又被关上 | 是 |
| smart_abc | android-portrait | 0.35 | 0.0 | `h-0558` | `zbdedmclngzbdeyp` | 走得对才能走得远 | 走的对才能走得远 | 走得到才能走得远 | 是 |
| smart_abc | android-portrait | 0.35 | 0.0 | `h-0803` | `dgqrjzlebxrfzlqujxjm` | 等求教了别人再去解决 | 等求教了别人再去解决 | 橙球叫了别人再去解决 | 是 |
| smart_abc | android-portrait | 0.35 | 0.0 | `p-0248` | `dathrfxtlldsdeaioftubk` | 大唐人向来懂得之恩图报 | 大唐人向来懂得知恩图报 | 大唐人像来大的知恩图报 | 是 |
| smart_abc | android-portrait | 0.35 | 0.1 | `h-0202` | `dbzlaelijuhh` | 都在这里举行 | 都在这理据行 | 都在这里护航 | 是 |
| smart_abc | android-portrait | 0.35 | 0.1 | `h-0762` | `datmhbcehewlcerbtldo` | 大腿后侧和外侧肉太多 | 大腿后侧和外侧肉太多 | 大腿后侧河外侧头太多 | 是 |
| smart_abc | android-portrait | 0.35 | 0.1 | `h-0888` | `valenackrfyigecovbbuji` | 杀了那曹仁一个措手不及 | 啥了那曹仁一个措手不及 | 傻脸娜草人一个措手不及 | 是 |
| smart_abc | android-portrait | 0.35 | 0.1 | `p-0007` | `fhchyiypviyilzehso` | 方舱医院是医疗场所 | 方舱医院是医疗场所 | 方舱医院十一瞭望所 | 是 |
| smart_abc | android-portrait | 0.35 | 0.1 | `p-0012` | `taenhhrfruai` | 踏春行人如织 | 踏春行人入职 | 踏春后人入职 | 是 |

</details>

<details>
<summary>调参集，6 条（改成金标准的 1 条）</summary>

| 方案 | 几何 | σ | b | id | 按键 | 金标准 | 原首选 | 现首选 | 坐了首选 |
|---|---|---|---|---|---|---|---|---|---|
| xiaohe | android-portrait | 0.25 | 0.0 | `s-056` | `mwwftijngewo` | 没问题交给我 | 没问题叫个我 | 没问题交给我 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `s-031` | `womfykgdzcyidmqu` | 我们应该早一点去 | 我们应该早一点去 | 我们应该早已先去 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.0 | `p-0189` | `zlldgeffvjslvsubhogrjp` | 在两个分站赛中收获冠军 | 在两个分站赛中收获冠军 | 在两个奋战袋中收获冠军 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.1 | `p-0012` | `taiphhrfruvi` | 踏春行人如织 | 踏春行人入职 | 踏春汉人入职 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `p-0007` | `fhchyiyruiyilcihso` | 方舱医院是医疗场所 | 方舱医院是医疗场所 | 方舱一月十医疗场所 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.1 | `p-0288` | `vbdpyidgliuirfwu` | 周敦颐等历史人物 | 周敦颐等历史人物 | 周迅一等历史人物 | 是 |

</details>

<details>
<summary>留出集，37 条（改成金标准的 0 条）</summary>

| 方案 | 几何 | σ | b | id | 按键 | 金标准 | 原首选 | 现首选 | 坐了首选 |
|---|---|---|---|---|---|---|---|---|---|
| xiaohe | android-portrait | 0.15 | 0.1 | `h-0734` | `uhuullbumftsgcfaiuhz` | 上述两部门通告发出后 | 上述两部门通告发出后 | 上述两部门童高发除垢 | 是 |
| xiaohe | android-portrait | 0.25 | 0.0 | `h-0151` | `ghguuisjlmyh` | 港股十三连阳 | 港股是三连养 | 港股十三廉航 | 是 |
| xiaohe | android-portrait | 0.25 | 0.0 | `h-0749` | `xmrexpfwtghzrelwykkl` | 先热血沸腾后热泪盈眶 | 先热血沸腾后热泪盈眶 | 先热谢飞退后热泪盈眶 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0035` | `bmpgdcljlude` | 便碰到拦路的 | 便碰到拦路的 | 变冷捣烂录的 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0120` | `wodetsvojnjy` | 我的同桌叫军 | 我的同桌叫均 | 我的同桌进军 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0123` | `dakdkduiyyhh` | 打开开始运行 | 打开开始运行 | 打开开始宇航 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0438` | `qpuibcvgvrhuyyhh` | 切实保证专户运行 | 切实保证专户云行 | 妾室保证专户宇航 | 是 |
| xiaohe | android-portrait | 0.25 | 0.1 | `h-0700` | `quvzdcucyhdehoiepn` | 衢州到邵阳的火车票 | 衢州到邵阳的火车票 | 衢州倒水羊的火车票 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `h-0470` | `jltviuiydmdsiexk` | 将推出纯电动车型 | 将退出纯电动车型 | 江涛初春电动车型 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `h-0700` | `quvzdcucyhdehoiepn` | 衢州到邵阳的火车票 | 衢州到邵阳的火车票 | 衢州刀身羊的火车票 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `h-0712` | `rrjmyyhhdesuduhfmj` | 软件运行的速度很慢 | 软件运行的速度很慢 | 软件蕴含的速度很慢 | 是 |
| xiaohe | android-portrait | 0.35 | 0.0 | `h-0888` | `ualenaccrfyigecouzbuji` | 杀了那曹仁一个措手不及 | 啥了那曹仁一个措手不及 | 傻了吗草人一个措手不及 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0035` | `bmpgdcljlude` | 便碰到拦路的 | 便碰到拦路的 | 变胖捣烂录的 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0120` | `wodetsvojnjy` | 我的同桌叫军 | 我的同桌叫均 | 我的同桌建军 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0281` | `rugoxrzeciuevi` | 如果选择此设置 | 如果选择此设置 | 如果选择只设置 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0419` | `csbolivjdcuhzzgo` | 从玻璃栈道上走过 | 从玻璃站到上走过 | 擦玻璃占到上走过 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0435` | `jtdkuiwwgowwmbde` | 决定是为国为民的 | 决定是为国为民的 | 决定是为果味喵的 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0582` | `buhviuxmiugvdexmxl` | 不会出现出轨的现象 | 不会出现出轨的现象 | 不会出现常规的现象 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0739` | `geyavzyihcxlmuzdjmvs` | 个亚洲一号项目在建中 | 戈雅周毅浩项目在建中 | 戈雅周一好像木塞减重 | 是 |
| xiaohe | android-portrait | 0.35 | 0.1 | `h-0888` | `ualenaccrfyigecouzbuji` | 杀了那曹仁一个措手不及 | 啥了那曹仁一个措手不及 | 啥了纳粹任一个措手不及 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.0 | `h-0130` | `vgful;vsviiu` | 政府令中指出 | 政府令中指出 | 政府令状指出 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.0 | `h-0640` | `dlwol;lvsijidebmhr` | 带我领略四季的变换 | 带我领略司机的变换 | 带我领略低级的变换 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.0 | `h-0734` | `uhuuldbumftsgkfaiuhb` | 上述两部门通告发出后 | 上述两部门通告发出后 | 上述两部门童高发滁州 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.0 | `h-0833` | `yengl;hdjnubuicvcjruxn` | 也能令黄金首饰璀璨如新 | 也能领黄金首饰璀璨如新 | 也能灵光金首饰璀璨如新 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.1 | `h-0740` | `gerfbuffzlgszinzkbjc` | 个人部分在工资内扣缴 | 个人部分在工资内口角 | 个人部分鳃弓自内口角 | 是 |
| microsoft | android-portrait-microsoft | 0.25 | 0.1 | `h-0816` | `gftsuihejqdkl;iflddm` | 跟同事喝酒到凌晨两点 | 跟同时喝酒到凌晨两点 | 镇痛适合就到凌晨两点 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0123` | `daklkluiyphh` | 打开开始运行 | 打开开始运行 | 打开开始云冈 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0151` | `ghguuisjlmyh` | 港股十三连阳 | 港股是三连养 | 港股十三廉航 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0202` | `dbzlvelijuhh` | 都在这里举行 | 都在这理据行 | 都在这里护航 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0277` | `rujnwosjuijisv` | 如今我三十几岁 | 如今我三十几岁 | 如今我玩十几岁 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0309` | `wovgzlgvvblyhh` | 我正在贵州旅行 | 我正在贵州旅行 | 我正在贵州旅港 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0599` | `jntmwomfjuhhzotjhv` | 今天我们举行座谈会 | 今天我们局行座谈会 | 今天我们巨痒座谈会 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0731` | `fzxdzlmjtmrvxtvivs` | 飞翔在漫天瑞雪之中 | 飞翔在满天瑞雪之中 | 飞翔在蓝天瑞雪之中 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.0 | `h-0925` | `yelimaojfuqilehrvedeq;xu` | 也立马安抚起了患者的情绪 | 也立马安抚起了患者的情绪 | 夜里马鞍泛起了患者的情绪 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.1 | `h-0552` | `uebzueuiyphhvgih` | 设备设施运行正常 | 设备设施运行正常 | 设备设施云阳正常 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.1 | `h-0749` | `xmrexxfztghbrelzy;kd` | 先热血沸腾后热泪盈眶 | 先热血沸腾后热泪盈眶 | 先热谢飞烫后热泪盈眶 | 是 |
| microsoft | android-portrait-microsoft | 0.35 | 0.1 | `h-1033` | `gurjxmzlvuuhlebkxmjmgudexnju` | 固然现在住上了保险坚固的新居 | 固然现在住上了保险坚固的新剧 | 固然现在沾上了保险坚固的新剧 | 是 |

</details>

## Alternatives considered

- **门槛差一点，取 m_sp 0、P_LEAD 9 先上线**：留出集净 top-1 在 σ=0.25 上有 +2.2 个点，代价看着不大。但 clean 被改在留出集上已经是 0.29%，超过 0.1% 的总门槛将近三倍；首选被空格直接上屏，按产品决定达不到门槛就不上线。
- **规则只在 m_sp 上做文章（不加 P_LEAD）**：方案的 (b) 本身就有边距。扫参表里同一个 P_LEAD 上抬 m_sp，代价和收益一起降，换算下来比收 P_LEAD 更贵；而且只看分数时按在键心的字母也可能被改，(c) 是保护正确输入的主闸。
- **(c) 用打过学习折扣的价**：与规划纠错边、`touch_supported` 用的是同一份变体，不用再计价一次。但用户接受过多次的码对折扣最多 12，键心落点的价从约 20 降到 8，会过 P_LEAD，「按在键心的字母永远不会被首选改掉」就不成立了。改用原价，学习只通过分数影响 (b)。
- **不加触点签名的标记，在引擎层把不允许时的纠错行挪回整句块之后**：缓存键不变。但缓存里的答案已经把纠错行放在第 0 位，挪回去要猜它原来的座位，中间夹着云候选（第 1 位）时猜不对；末尾辅助码的基础部分正是从这份缓存里取的。加标记后不允许的请求各算各的，结果与没有规则时逐字节相同，代价是辅助码在单个尾字母上来回时基础部分多一次缓存未命中（只在带触点、m_sp 有限时）。
- **把 `candidate_touch_supported` 和新的首选标记合成一列逐行标志**（[触点计价笔记](2026-10-27-shuangpin-touch-correction.md)里评审提过、当时推到这一期）：少一列平行数组。但合列要改 `ordering::runner_up_order_with` 的签名和网页引擎、runtime 每个同步点的类型；新列照抄 `candidate_touch_supported` 的写法（为空表示没有、长度对不上整列作废），改动可以逐行对照。合列仍可以以后单独做。
- **视图在任意位置都标出带标记的行**：宿主不必关心位置。但契约是「坐了首选的那一行」：插件或固定位置把它挪开以后，首选已经不是它，再画强调色就误导了。

## Consequences

- **收益**：规则、标记、守卫豁免、视图字段和学习否决都已实现并有测试，三端宿主可以按契约接线；评测工具能直接量首选规则（每组 `first_seat` 一行、clean 集逐条列出首选变了的用例、`--assert-first-seat-gate`、`--touch-tuning lead=…,lead_pen=…`）。以后有了真机落点分布或更好的语言模型，只要重扫两个参数。
- **代价**：默认参数下没有收益。代码多一条只在 m_sp 有限时才走的路径；快照多一列平行数组（一行都没有时为空，桌面不多分配）；视图多一个可选字段。
- **行为**：m_sp 是无穷大时，有触点和没有触点的结果都与上一期逐字节相同（整句族基线除新加的 `first_seat` 一行外逐字节相同，none 模式和 13 组全拼评测与 develop 逐字节相同）。
- **已知上限**：合成落点是各向同性高斯，不模拟拇指的系统性偏移，也不模拟同一音节两键都错；clean 集被改的用例大多是语言模型更偏爱改过的读法，这是落点的闸挡不住的，要靠更好的模型或句子级的置信度。
- **重访信号**：有了真机盲打的落点分布、换了句子模型、或者整句模型给出了句子级置信度时，按这里的流程重扫 (m_sp, P_LEAD)；某一组过了门槛就改 `TouchTuning::default()` 和这篇的结论，README 的基线命令随之改用 `--assert-first-seat-gate`。

## Verification

全部在 Mac Studio 上经 `rbuild` 跑；资源目录 `/Volumes/Workspace/rbuild/msime-test-resources` 的 12 个文件逐个按本分支 `desktop-dictionary.lock.json` 核对 sha256 全部相符，没有 `sentence-model-desktop.safetensors`。评测的原始输出（报告、stderr、退出码、跑法脚本）在 Studio 的 `/Volumes/Workspace/rbuild/evidence/sp-first-seat-20261029/`：`out/` 是扫参、留出集和整句族，`g0/` 是 G0，`lat/` 是延迟。扫参进行到一半时 Studio 重启过一次（重启前负载 800 以上，原因不明），没跑完的那几组删掉输出后重跑，已经跑完的几组的报告没有动。

- **单元和集成测试**（最终提交上）：`cargo test -p msime-engine --lib` 2798 项通过（57 项本来就忽略）；`--test golden` 31 项；`cargo test -p msime-input-runtime --lib --example convert_eval --example rerank_latency` 408、32、26 项；`cargo test -p msime-host-api --no-fail-fast` 库测试 1055 项通过（5 项忽略），其余目标全部通过。Studio 重启后 shell 的打开文件数上限回到 256，第一遍并行跑引擎和 host-api 的测试时 300 来项在建夹具时失败（打开 SQLite 失败），`ulimit -n 10240` 以后全部通过，单独跑其中一项也通过；`cargo clippy -p msime-engine -p msime-input-runtime -p msime-host-api --all-targets -- -D warnings` 无警告。新增的：
  - `shuangpin::tests`：规则全部满足时坐首选、字面整句在第二位、命中缓存也一样，默认参数下不坐（`a_clearly_better_supported_correction_takes_the_first_seat`，小鹤和自然码）；键心落点在 m_sp 放到 −1000、P_LEAD 19 时也不坐，两键正中的落点要 P_LEAD 到 12 才坐（`a_key_centre_touch_never_puts_a_correction_first`）；分数差的线高于这次的分数差就不坐（`a_correction_leads_only_by_the_margin`）；任何一键没有触点不坐（`a_correction_leads_only_when_every_key_was_touched`）；字面有全码词条不坐（`a_literal_dictionary_entry_keeps_the_first_seat`）；这一对被拒绝过不坐、别的方案和别的码对的拒绝不算、接受不否决（`a_rejected_pair_vetoes_the_first_seat`）；末尾单辅助码时不坐，结果与没有规则的新引擎逐行相同（`an_active_helpcode_keeps_the_correction_off_the_first_seat`）；自定义方案不坐（`a_custom_profile_never_puts_a_correction_first`）；模糊音把全码的模糊词条排到它前面时不再带标记、回到字面整句之后，与没有首选规则的新引擎逐行相同（`a_fuzzy_word_ahead_sends_the_correction_back_behind_the_literal_sentence`，n/l 模糊音）。
  - `shuangpin::engine::tests`：守卫的两半（带标记、回答整串按键的留在首位，只回答前缀的和没标记的让开，挪离首位的清掉标记，还排在字面整句前面的挪回字面整句块之后）和请求层面的每一条允许条件；把 `active_helpcode_query` 那一条去掉，请求层面的测试失败。`shuangpin::dictionary::tests::the_first_seat_rule_keys_its_own_cached_answers` 钉住触点签名的标记。
  - `lattice::decode::tests::the_nonleading_typo_sentence_is_the_best_scoring_typo_path`（n-best 降序、nonleading 取到最好的纠错路径）；`ordering::tests::a_shuangpin_correction_the_engine_put_first_stays_first`（重排器不跑模型，`runner_up_order_with` 不挪它，字面整句在第二位）；`session::tests::rejecting_a_confident_correction_vetoes_its_first_seat`（选了字面整句记一次拒绝，之后不再坐首选，选中它照样记接受）和 `the_session_puts_a_correction_first_only_with_every_key_touched`（键心、缺一键触点、平面表、超级简拼本地模式、按原样上屏被抑制以后都不坐）；复用候选行的几处（韩文汉字、日文、英文词典、页复制、会话候选复制）同时清掉或复制这个标记。
  - `msime-input-runtime` 的 `a_confident_shuangpin_correction_leads_the_view_and_only_it_is_marked`：真实会话、模型偏爱字面整句时纠错整句仍在首位，字面整句在前三，视图只有第一行带 `confident_correction`、JSON 里也只有它有这个字段，默认参数下一行都没有。`msime-host-api` 的 `touched_views_carry_no_confident_correction_by_default`：经 FFI 带触点打字，视图 JSON 里没有这个字段。
  - `convert_eval`：`first_seat` 的计数和清单、门槛判定（恰在门槛上通过、超过时报出方案和总体）、`lead`、`lead_pen` 的解析。
- **G0**：本分支和 develop（`c455167a18`，与本分支的父提交相同）各用自己的镜像和 target 目录编一份 release 的 `convert_eval`，同样的参数各跑一遍：`verify-local.sh` 的 13 组全拼评测报告逐字节相同，也都与仓库里的基线相同；dump 只有 nine-key、fourteen-key 里 `shijian` 那一行的时钟候选不同（两边相差一秒），把时刻换成占位后逐字节相同。双拼 none 模式五个方案的整句族（四份集合）和词级族（`--max-syllables 2 --limit 3000`）报告和 dump 都逐字节相同。
- **基线**：两份基线用默认参数按 README 重新生成（整句族 `--jobs 5`，词级族同），clean 集首选被改 0、首选规则坐上的纠错行 0；去掉新加的 `first_seat` 一行以后与原来提交的两份基线逐字节相同。
- **延迟**：`rerank_latency --touch-arms`（sentences-v1 加 particles，σ=0.25，两份 Android 导出几何，默认参数），小鹤、微软、自然码各 3 轮，每轮紧接一次 A/A（`--no-touch-points`）。Studio 负载 300–900，两臂 p95 之差的中位数小鹤 −0.22 ms（−2.09 至 −0.10）、微软 +0.06 ms（−0.30 至 +0.50）、自然码 +0.67 ms（−0.06 至 +3.19）；A/A 的同一个量是 +0.51、−0.43、+0.42 ms（−1.44 至 +1.18）。自然码的中位数超过 0.5 ms，但落在 A/A 的摆动之内；负载最高的两轮两臂的 p95 都过了 16 ms（小鹤第三轮不送触点那一臂也是 19.3 ms）。这只能说明没看到超出噪声的增量，比 PR1a 的 5 轮少，没有在空闲的机器上复测。默认参数下这一期在每键路径上只多几次布尔判断（m_sp 是无穷大时 `ShuangpinEngine` 不看请求，词典的规则第一条就不成立），没有单独比较本分支和 develop 的延迟。
- **没有做的**：`scripts/verify-local.sh` 整套没跑（只按它的参数跑了 13 组评测）；`cargo check --manifest-path fuzz/Cargo.toml` 没跑；没有在真机或模拟器上验证；三端宿主读 `confident_correction` 画强调色、读屏念「已纠正」不在这一篇的改动里；typo_beam 没有扫。

1–2 音节的完整词补查及其词级对照见[短词触点纠错](2026-10-31-shuangpin-touch-short-words.md)；沿用本篇的既有边界，不启用纠错首选或改变默认参数。
