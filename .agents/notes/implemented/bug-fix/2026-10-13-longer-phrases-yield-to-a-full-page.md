# Agent Note: 拼音打完整时，补全词让位于一页精确同音词

Status: implemented

## Problem

[拼音打完整时，首选不再让给更长的补全词](2026-10-12-exact-reading-keeps-first-seat-over-longer-phrase.md) 把补全词从首位赶走，但全长组里补全词仍整组并入、和同长度行按 `weight` 混排：只要同长度表填不满一页，补全词就会占掉第一页剩下的位置。两头的输入都因此变差：

- 精确同音词本来就多（锁定词库里 `qi'shi` 有 27 行）的输入，第一页仍被「其实是」「企事业单位」占走两格。
- 精确同音词很少（`fa'ban` 只有 法办 4045、发版 100）的输入，第一页 6 格里有 4 格是「发版号 100、发版支持 100、发版要求 87、法半夏 61」。

补全词是给「同长度表只剩冷僻同音词」的输入兜底的（`ping'guo` 的第二位只有 平锅），上面两类输入不是这个理由成立的地方。

## Decision

全长组里有补全词时，精确同音词（`canonical_pinyin` 等于键入切分的行）自己够填一页就不再并入补全词，不足一页时并入但最多 `LONGER_PHRASE_LIMIT` 条：

1. 页门槛取 `EXACT_ROWS_THAT_FILL_A_PAGE = 6`，即默认页大小，与 `ALTERNATIVE_SEGMENTATION_FIRST_PAGE_SIZE` 同源、同样不随页大小设置变（QD:29-34）。判据只看行数，不看权重：够填一页就说明用户要的词大概率就在本表里。
2. 补全词上限从 12 收到 3（`LONGER_PHRASE_LIMIT`）。只有同长度表不足一页时才走到这里，3 条足够覆盖 `ping'guo` 的「苹果电脑 苹果公司 苹果机」这类相邻短语。
3. `LONGER_PHRASE_EXTRA_SYLLABLES = 3`（最多多三个音节）不变，[首选规则](2026-10-12-exact-reading-keeps-first-seat-over-longer-phrase.md) 不变：首位仍是读音与键入逐字相等的行里最重的那条。

## Alternatives considered

- **加一个偏好开关，关掉补全词。** #7345 报告人的原提案，也是让不想要补全词的人彻底不见它们的唯一办法。没有采用：[首位修复](2026-10-12-exact-reading-keeps-first-seat-over-longer-phrase.md) 已按「问题在首位不在补全词本身」否过一次；排掉首位之后，补全词剩下的问题就是首屏占位，页门槛加数量上限直接解决它，不必为此新增偏好键并在六个平台设置界面各接一遍。真要给「一条都不看」的人开关，仍可单独做。
- **去掉补全词。** 实现最简。`ping'guo` 的第二位会退回「平锅」，这正是当初并入补全词的原因，不选。
- **按权重比例设门槛（补全词不低于精确首词的某个比例）。** 比行数门槛更细。它分不开要解决的两类输入：`fa'ban` 里「发版号」100 对「法办」4045 是 2.5%，而 `ping'guo` 里要留下的「苹果电脑」21495 对「苹果」1143881 只有 1.9%，任何阈值在一类上生效就会在另一类上一起翻转。
- **所有同长度行都排在补全词前面。** 等于把门槛提到「精确词填满列表」，会把「平果」「平锅」抬到「苹果电脑」前面，回到并入补全词前的样子，已否。
- **只留页门槛、不加上限。** 精确同音词只有一两条的输入（`fa'ban` 两条）仍会让补全词占满首页剩余位置，正是要修的一半，不选。

## Consequences

- **收益**：`qi'shi` 第一页回到 其实 骑士 启示 气势 岂是 歧视…，「其实是」「企事业单位」不再出现；`shi'de` 的精确同音词已有 9 行以上，补全词整组不进；`shen'me`、`zhe'ge` 的补全词从 8-9 条收到 3 条。词级评测（`quanpin-words-v1.tsv --limit 3000`，2791 条）top-1 0.781 不变，top-5 0.942 → 0.960，top-9 0.970 → 0.985，MRR 0.847 → 0.853：补全词不再把真正要的词挤出前五位。
- **代价**：精确同音词不足一页时，常用的补全词可能被 3 条上限截掉（`fa'ban` 仍有 发版号/发版支持/发版要求 三条，第四位的 法半夏 被截）；「苹果电脑」「苹果公司」这类相邻短语只在精确同音词少于一页时才出现。
- **基线**：`resources/eval/baseline-words.json` 按新排序更新。
- 与 [模糊音下覆盖整个输入的词条排在替补整句之前](2026-10-08-fuzzy-whole-input-before-stand-in-sentence.md) 同向：精确命中优先于猜出来的行。

## Verification

- `crates/engine/src/quanpin/dictionary/tests.rs` 新增两个合成夹具测试：`exact_rows_filling_a_page_drop_the_longer_phrases`（6 条精确同音词 + 一个更重的补全词，补全词不入，前六位是精确行）；`longer_phrases_are_capped_when_the_exact_table_is_short`（精确两条 + 五条补全词，补全词只留 3 条，前五位是 法办 发版 发版号 发版支持 发版要求）。两者在改动前都失败。
- `cargo test -p msime-engine --lib quanpin::dictionary::tests`：53 passed。
- `cargo clippy -p msime-engine --lib --tests -- -D warnings` 干净。Windows 上 `--all-targets` 会因 `examples/ninekey_dictionary.rs` 里的 `std::os::unix` 编译失败，那是平台既有问题，与本改动无关。
- 用锁定词库（dict-v2.0.14）经 `msime-input-runtime` 打 `qishi faban youdian shenme zhege shide pingguo dacuozi doumeiren`：结果同上述收益，`pingguo` 仍出 苹果电脑 苹果公司 苹果机，`dacuozi`、`doumeiren` 首位仍是 打错字、都没人。
