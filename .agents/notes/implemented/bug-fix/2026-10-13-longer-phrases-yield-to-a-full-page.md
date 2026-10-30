# Agent Note: 拼音打完整时，补全词让位于一页精确同音词

Status: implemented

## Problem

[拼音打完整时，首选不再让给更长的补全词](2026-10-12-exact-reading-keeps-first-seat-over-longer-phrase.md) 把补全词从首位赶走，但全长组里补全词仍整组并入、和同长度行按 `weight` 混排：只要同长度表填不满一页，补全词就会占掉第一页剩下的位置。两头的输入都因此变差：

- 精确同音词本来就多（锁定词库里 `qi'shi` 有 27 行）的输入，第一页仍被「其实是」「企事业单位」占走两格。
- 精确同音词很少（`fa'ban` 只有 法办 4045、发版 100）的输入，第一页 6 格里有 4 格是「发版号 100、发版支持 100、发版要求 87、法半夏 61」。

补全词是给「同长度表只剩冷僻同音词」的输入兜底的（`ping'guo` 的第二位只有 平锅），上面两类输入不是这个理由成立的地方。

## Decision

全长组里有补全词时，精确同音词（`canonical_pinyin` 等于键入切分的行）自己够填首页时，补全词不再按 `weight` 混排，改成接在整组的末尾；不足一页时照旧并入但最多 `LONGER_PHRASE_LIMIT` 条：

1. 页门槛取宿主的每页候选数，不写死：`SessionOptions::first_page_rows` → `QueryRequest::first_page_rows` → `QuanpinDictionary::set_first_page_rows`。host-api 用 `host_page_size(page_size_override.unwrap_or(candidate_page_size))` 填它（与交给 `Runtime` 的页大小同源），网页引擎用自己的 `page_size`；缺省 `DEFAULT_FIRST_PAGE_ROWS = 6`，只留给不设它的调用方（goldens、mcp-server、backend-engine）。判据只看行数，不看权重：够填一页就说明用户要的词大概率就在本表里。
2. 「接在整组末尾」而不是「整组丢掉」：整组本身至少就有 `first_page_rows` 行，所以接在末尾的补全词一定落在首页之后，首屏的座位仍归同音词。丢掉会让「中国人民」（`zhongguo`）、「是的话」（`shide`）这类精确表本来就够一页的输入再也翻不到它们；词库以后给 `ping'guo` 多加几个冷僻同音词，「苹果电脑」也会整个消失。补全词上限从 12 收到 3（`LONGER_PHRASE_LIMIT`），只在不足一页时生效，足够覆盖 `ping'guo` 的「苹果电脑 苹果公司 苹果机」这类相邻短语。
3. `LONGER_PHRASE_EXTRA_SYLLABLES = 3`（最多多三个音节）不变，[首选规则](2026-10-12-exact-reading-keeps-first-seat-over-longer-phrase.md) 不变：首位仍是读音与键入逐字相等的行里最重的那条。

4. 门槛在两个键盘上按同一份值归一：`FirstPageRows::new` 是唯一把 0 当 1 的地方（`QueryRequest` 派生 `Default`，裸 `usize` 会默认成 0，而 0 会让 `exact_rows < 首页行数` 永远不成立），`QuanpinDictionary` 与两个会话的字段都用这个新类型，0 没有别的入口。
5. 宿主在 fcitx5 的「9 个候选」这类菜单里改每页候选数时不重建引擎，所以 `Runtime::set_page_size` 把新值一并交给引擎（`InputEngine::set_first_page_rows`），门槛与分页始终同源。`Runtime::new` 和 `replace_engine` 同样把自己的页大小交给引擎，直接建 runtime 的示例、测试和评测工具不必再在引擎选项里另设一份（见[评测按宿主的每页候选数和本地模式配置](../testing/2026-10-29-eval-page-size-and-local-modes.md)）。

## Alternatives considered

- **加一个偏好开关，关掉补全词。** #7345 报告人的原提案，也是让不想要补全词的人彻底不见它们的唯一办法。没有采用：[首位修复](2026-10-12-exact-reading-keeps-first-seat-over-longer-phrase.md) 已按「问题在首位不在补全词本身」否过一次；排掉首位之后，补全词剩下的问题就是首屏占位，页门槛加数量上限直接解决它，不必为此新增偏好键并在六个平台设置界面各接一遍。真要给「一条都不看」的人开关，仍可单独做。
- **去掉补全词。** 实现最简。`ping'guo` 的第二位会退回「平锅」，这正是当初并入补全词的原因，不选。
- **按权重比例设门槛（补全词不低于精确首词的某个比例）。** 比行数门槛更细。它分不开要解决的两类输入：`fa'ban` 里「发版号」100 对「法办」4045 是 2.5%，而 `ping'guo` 里要留下的「苹果电脑」21495 对「苹果」1143881 只有 1.9%，任何阈值在一类上生效就会在另一类上一起翻转。
- **所有同长度行都排在补全词前面。** 等于把门槛提到「精确词填满列表」，会把「平果」「平锅」抬到「苹果电脑」前面，回到并入补全词前的样子，已否。
- **只留页门槛、不加上限。** 精确同音词只有一两条的输入（`fa'ban` 两条）仍会让补全词占满首页剩余位置，正是要修的一半，不选。
- **写死 6，与 `ALTERNATIVE_SEGMENTATION_FIRST_PAGE_SIZE` 一样不随设置变。** 引擎不必知道宿主的页大小，golden 与测试也不受设置影响；参考实现正是这么处理那个「保护位」的（QD:29-34）。没有采用：那条规则保护的是一个跨输入的固定座位，与「这一页放不放得下」无关；这里问的恰好是后者，用户把每页候选数调大（`candidate_page_size` 1..=10）后仍按 6 算，会在没填满的首页上白丢补全词。

## Consequences

- **收益**：`qi'shi` 第一页回到 其实 骑士 启示 气势 岂是 歧视…，「其实是」「企事业单位」退到首页之后；`shi'de` 的精确同音词已有 9 行以上，补全词整组排在它们后面；`shen'me`、`zhe'ge` 的补全词从 8-9 条收到 3 条。词级评测（`quanpin-words-v1.tsv --limit 3000`，2791 条）top-1 0.781 不变，top-5 0.942 → 0.960，top-9 0.970 → 0.985，MRR 0.847 → 0.853：补全词不再把真正要的词挤出前五位。
- **代价**：精确同音词不足一页时，常用的补全词可能被 3 条上限截掉（`fa'ban` 仍有 发版号/发版支持/发版要求 三条，第四位的 法半夏 被截）；「苹果电脑」「苹果公司」这类相邻短语在精确同音词多于一页时要翻页才看得到。门槛只看本组的精确行，看不见首页上的整句块、AI 位、云候选位和后插入的用户词，所以精确行不满一页时补全词仍可能占进首页——这是已知的不精确，换来的是它们不会被整组丢掉。
- **基线**：`resources/eval/baseline-words.json` 按新排序更新；把「丢掉」改成「接在整组末尾」不动任何一条评测指标，基线不用再改。
- **跟随设置**：同一句拼音在每页候选数 6 和 9 的宿主上候选不同。host-api 每次重建引擎都重新下发，改设置即生效；只在菜单里改页大小的那条路（`msime_client_set_candidate_page_size`）由 `Runtime::set_page_size` 一并下发。九键那条路在打开词库时把同一值交给它的 `QuanpinDictionary`；不过九键随后按 `full_len` 丢掉一切比键入切分更长的行，补全词到不了九键的候选里，这条接线是为将来留的。双拼没有补全词逻辑，不受影响。
- 与 [模糊音下覆盖整个输入的词条排在替补整句之前](2026-10-08-fuzzy-whole-input-before-stand-in-sentence.md) 同向：精确命中优先于猜出来的行。

## Verification

- `crates/engine/src/quanpin/dictionary/tests.rs` 的 `exact_rows_filling_a_page_push_the_longer_phrases_behind_it`：6 条精确同音词 + 一个更重的补全词，前六位是精确行，补全词仍在列表里、位置在首页之后；`longer_phrases_are_capped_when_the_exact_table_is_short`（精确两条 + 五条补全词，补全词只留 3 条）；`a_larger_first_page_rows_keeps_the_longer_phrases`（6 条精确词，`set_first_page_rows(8)` 后补全词按权重并入）。前两个在改动前失败。
- `crates/engine/src/session/tests.rs` 的 `first_page_rows_reaches_the_dictionary_through_the_session_options`：同一夹具在默认 6 下前六位是精确行、`first_page_rows = 8` 时补全词按权重排到第二位；删掉 `session/input.rs` 里那行接线它会变红，钉住 `SessionOptions` → 请求 → 引擎这段通路。`a_zero_first_page_rows_reads_as_one_row` 钉住 0 与 1 走同一条路。
- `crates/input-runtime/src/tests.rs` 的 `resizing_the_page_tells_the_engine_its_first_page_gate`：改页大小会同时改引擎的首屏门槛。该测试目标在 Windows 上编不过，见下。
- `cargo test -p msime-engine --lib quanpin::dictionary::tests`：54 passed；`cargo test -p msime-engine --test golden`：31 passed（脚本场景仍与参考实现逐位一致）。
- `cargo check -p msime-engine --lib --tests`、`-p msime-host-api --lib --tests`、`-p msime-input-runtime --lib`、`-p msime-engine-wasm` 全部通过。
- Windows 上 `cargo test -p msime-engine --lib` 有 17 个失败（`handwriting`/`japanese`/`paths`/`dictionary::hanzi` 计时/`session` 分配计数等），在改动前的 commit 上同样失败；`cargo test -p msime-input-runtime --lib --tests` 在 Windows 上编不过（`#[cfg(unix)]` 的辅助函数被非 unix 测试引用），同样是既有问题。两者都不是本改动引入的。
- 用锁定词库（dict-v2.0.14）经 `msime-input-runtime` 打 `qishi faban youdian shenme zhege shide pingguo dacuozi doumeiren`：结果同上述收益，`pingguo` 仍出 苹果电脑 苹果公司 苹果机，`dacuozi`、`doumeiren` 首位仍是 打错字、都没人。
- Android 真机装机验收：包来自由本改动触发的 `release-android.yml`（`publish=false`，只产 workflow artifact），候选优先级按预期变化、功能正常。其余平台的真机复核没做。
- 补上 review 提到的三处之后复跑：词级、九键、14 键、九键整句、14 键整句五套评测的基线逐位不变（`convert_eval` 全部 exit 0）。`baseline-sentences.json` 在本机这份资源目录下与改动前、与 #7596 合并后的代码都对不上，两次的 `overall` 逐位相同（top1 0.75 / mrr 0.75），说明差异来自资源而不是代码，未动它。
