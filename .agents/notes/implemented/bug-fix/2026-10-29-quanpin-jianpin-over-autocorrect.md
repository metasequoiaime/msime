# Agent Note: 全辅音简拼的字面整词排在全拼纠错前面

Status: implemented

## Problem

#9903：全拼打 `qjn` 想要「前几年」一类的三字简拼，打 `zhr` 想要 zh'r 的「正如、整容、招人」，两个纠错开关都开着（出厂默认）时首页全是纠错单字：`qjn` 被邻键纠错读成 qun/qin（j 挨着 u、i），首页是「群 亲 裙 侵」；`zhr` 被读成 zhe（r 挨着 e），首页是「这 者 着 折」。字面简拼行并没有丢，但排在整张纠错列表后面，`qjn` 的「前几年」排在两百多个 qun/qin 单字之后。

根因有两层。`crates/engine/src/pinyin/autocorrect.rs` 的 `looks_like_syllable_with_jianpin_tail` 让全辅音串一律保持可纠错，注释写的前提是「没有多字母简拼，`bqng` 只能是 `bang`」；[全拼简拼保留双字母声母](2026-10-20-quanpin-retroflex-abbreviations.md)之后 zh/ch/sh 也是简拼单位，这个前提早就不成立。`crates/engine/src/quanpin/dictionary.rs` 的 `query_exact_projected` 在 `corrected_input` 时把纠错读法当主键，字面读法只用 `append_unique_words` 接在后面兜底，于是简拼词一条也上不了首页。

## Decision

全辅音输入（不含 a/e/i/o/u/v，也不含 `'`）本身就是一串简拼。纠错分支把结果组装完之后调用 `quanpin::series::lead_literal_abbreviation_phrases`：匹配码折叠后等于原文字母、读满了全部按键的多字字面行（不含整句行）稳定地挪到最前面，纠错读法和只读了一部分按键的字面行按原来的先后跟在后面。纠错行的标记不受影响，`mark_autocorrect_candidates` 按匹配码判定，排序不改匹配码。

原来打头的纠错候选（匹配码不是原文前缀的首行）最多退到首页最后一个座位，即宿主每页候选数减一，最小为 1，首位永远归字面简拼词。`zhr` 读满全部按键的 zh'r、z'h'r 词在真实词库里有上百条，全排在前面会把纠错读法挤出前几页，等于对全辅音输入关掉了纠错；保一个首页座位让按偏了键的用户在首页还看得到「这」。这个座位跟随 `first_page_rows`，而 `set_first_page_rows` 本来就会清空系列缓存。

规则只看「全是辅音」和「有没有读满全部按键的字面整词」两件事，不比较权重：

- 字面简拼一条整词都没有时列表原样不动，纠错读法继续打头。
- 带元音的输入不走这条规则。`sahng` 的字面读法 sa'h'n'g 读得出「撒谎那个」，但那是音节加简拼的混合形状，误触的可能比简拼大得多，`sahng → 上` 仍排第一。
- 纠错关掉时不进纠错分支，字面结果和以前一样。

同一处改掉 `looks_like_syllable_with_jianpin_tail` 里过时的注释：全辅音串仍然可纠错，两种读法都保留，先后由词典定。

桌面共享设置页（macOS、Linux 的设置应用，以及其它不是鸿蒙手机的共享页）在「高级选项」的模糊音后面加一组「全拼纠错」，放两个开关：全拼邻键纠错写 `quanpin.autocorrect_neighbor`，全拼乱序纠错写 `quanpin.autocorrect_transposition`，默认值仍是开，只在当前方案为全拼时列出（引擎的 `SchemeType::supports_autocorrect` 只对全拼成立）。说明文字写清楚各自管哪类纠错，以及两个都关时漏打、多打字母的纠错（`request_autocorrect_mask` 的 `MISSING_OR_EXTRA`）也一起关闭。鸿蒙手机仍用[共享设置页拆分全拼纠错开关](../feature/2026-10-27-shared-autocorrect-switches.md)放在「中文」组里的那两行，不重复。宿主读这两个字段的代码早就有，不用改。

## 修复前后

`cargo run --release -p msime-engine --example quanpin_autocorrect_dictionary`，Studio 上的随包词库 `msime-test-resources`，两个纠错开关开、学习关，取候选前八：

| 输入 | 修复前 | 修复后 |
| --- | --- | --- |
| `qjn` | 群 亲 裙 侵 琴 勤 秦 覃 | 前几年 区间内 期间内 去见你 裘佳宁 群 亲 裙 |
| `zhr` | 这 者 着 折 浙 哲 遮 褶 | 正如 注入 阵容 众人 主任 这 张让 只认 |
| `bqng` | 帮 棒 榜 邦 绑 膀 镑 磅 | 并且能够 比去年高 比起那个 帮 棒 榜 邦 绑 |
| `gau`、`hauzh` | 挂 打头；华中在第 3 位 | 不变 |

`bqng` 是原注释拿来论证「只能是纠错」的例子，在真实词库里它的字面简拼有「并且能够」，按这条规则排到前面，「帮」紧跟其后。`shng`（漏了 a）同理：sh'n'g 有「是那个、少年宫」，删字纠错出的「上」排在它们后面，最晚在首页最后一个座位。

## Alternatives considered

- **让全辅音输入完全不进纠错**（在 `resolve_series_query` 的资格判断里加一条）：改动最小，`qjn`、`zhr` 立即干净。没有采用：`bqng → 帮`、`shng → 上` 这类真的打错的输入从此没有纠错候选；维护者要求纠错读法留在后面并带标记。
- **字面整词全部排前、纠错不保座**：最贴近「字面简拼排在纠错前面」的字面意思。没有采用：两字母、三字母的简拼表很密，`zhr` 的纠错候选会落到好几页之后，全辅音输入的纠错实际上等于没有。
- **按权重混排字面整词和纠错读法**：看起来更「公平」。没有采用：两组权重来自不同读音的表，纠错单字的权重普遍远高于简拼多字词，混排就是修复前的样子。
- **补词「请教你」或加「优先短语 / 优先逐字」选项**：issue 的另外两条希望。不做：词库来自锁定的生成流程，不按单条手工补词；用户打一次 qingjiaoni 学到的词条带 jp，本来就能被 `qjn` 查到，这次修复后它和其它字面整词一起排在纠错前面。排序选项要解决的冲突在这次修复后不存在了，不再加偏好项。

## Consequences

收益：全辅音简拼不再被邻键纠错抢走首页，`qjn`、`zhr` 首位是字面简拼词；纠错读法仍在首页、带 `*` 标记。用户学到的简拼词条（带 jp）同样排在纠错读法前面。桌面共享设置页能分别关两类全拼纠错。

代价：全辅音的真实误触（`bqng`、`shng`）首位变成字面简拼词，纠错候选退到其后，最晚在首页最后一个座位；想要「帮」「上」的用户多看一眼或多按一个数字键。纠错候选保座只保原来打头的那一条，其余纠错行仍在全部字面整词后面。

缺口：issue 里 `qjn` 首位的「强健泥」不在修复前引擎给出的完整本地候选列表里（纠错开着，用上面的探针），来源可能在宿主侧的云候选或 AI 候选，这次没有追查，纠错关闭时的本地列表也没有核对。没有在 macOS 或 Linux 的系统输入法里装包验收，共享设置页只跑了 vitest。

## Verification

- `cargo test -p msime-engine`：新增 `literal_abbreviation_phrases_lead_corrected_readings`、`the_leading_correction_keeps_a_first_page_seat_behind_many_phrases`、`corrections_still_lead_without_a_literal_abbreviation_phrase`（`quanpin::dictionary`）和 `literal_abbreviation_phrases_move_ahead_in_their_own_order`、`the_leading_correction_keeps_the_last_first_page_seat`（`quanpin::series`），前两条在修复前失败。`autocorrect_marking` 里 `shng` 的断言随规则改成「撒谎那个」打头、带标记的「上」第二，夹具里「撒谎那个」的简拼正好是 s'h'n'g。
- `cargo clippy -p msime-engine --all-targets -- -D warnings`、`cargo fmt --all -- --check`。
- 真实词库探针 `quanpin_autocorrect_dictionary` 加了 `qjn` 首位是「前几年」、`zhr` 首位是两字词的断言，修复前后的输出见上表。
- `pnpm --filter @msime/desktop exec vitest run tests/settings/language-card.test.tsx tests/settings/settings.test.tsx`：macOS、Linux、Windows 三个平台的共享页在高级选项里都有两个开关，点击后只写对应字段；双拼方案下不列出；输入页的分组顺序断言加上「全拼纠错」。
