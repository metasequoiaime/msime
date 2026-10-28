# Agent Note: 全拼纠错补上读不出来的两字母错键

Status: implemented

## Problem

#10124：全拼 26 键打 `wiaini`（想打 `woaini`，i 和 o 是邻键），邻键纠错和乱序纠错都开着，本地候选却是空的；Android 上唯一的一条「我i爱逆」来自 Google 云候选（`inputtools.google.com` 对 `wiaini` 返回的正是它，注音 `wo i ai ni`），「逆」排在「你」前面也是它的结果。

根因在纠错表。`crates/engine/src/pinyin/autocorrect.rs` 的 `generate_table` 照参考生成器只收三到六个字母的错键，理由是两字母串属于简拼（`nh`、`zh`、`wd`）。`wi` 是 `wo` 的邻键变体，只有两个字母，永远不是错键；而 `i` 不能开始任何音节，`wi` 也不是任何音节的前缀，整段 `wiaini` 一个读法都没有：普通切分失败，`autocorrect_cut_kbest` 走不到末尾，词网格的音节纠错边只作用于合法音节。

用锁定词库（develop 的 dict-v2.0.39 和 0.6.0 打包的 dict-v2.0.24）复现，`wiaini`、`wiaino`、`wiai`、`wiaiwojia` 的本地候选在纠错开、关时都是空的。0.6.0 到 develop 之间全拼纠错的代码没有变过；Android 宿主照偏好下发两个开关，26 键全拼不送键位几何，`PersonalTypoProfile` 的抑制也只作用于已经产生的纠错，都不是原因。

## Decision

另加一张两字母错键表 `two_letter_table`，只有邻键和乱序两类：每个两字母音节的变体，若既不是音节、又按简拼和音节前缀都读不出来（整串不是前缀，两个字母也不都是音节开头，见 `two_letters_readable`），就记一行 `(错键, 音节)`。`wi → wo/wu/ai/ei/qi/si`、`hi → hu/yi/ji/ni/bi`、`ia → ai`、`in → ni` 这类都在里面，`nh`、`zh`、`ba` 不在。它在 `correction_index` 里跟在同类的参考表后面并入，两类键互不重叠，每个键的目标仍按权重排序；参考表的四张表、行数和夹具都不变。删字和插字不扩到两字母：去掉声母后剩下的两字母串（`ia` 来自 xia、jia、qia……）一键有十几个目标，没有证据说值得。

两个字母本身证据太少，所以在 `Search::extend` 到达末尾时多一道检查（`short_keys_unsupported`）：一条读法用到了两字母错键、却没有一个照原样打对的音节，就不要它。`wiaini` 有 `ai`、`ni` 打对，读成 wo'ai'ni；单独的 `wi`、`hi`，以及整串都得改的 `wifi`、`hihi` 保持原样，混输英文和表情行照旧。检查和既有的「不收全合法读法」一样放在到达末尾时，前面各位置的列表定稿后不变，沿前驱链查一次即可。

两字母错键落在输入末尾、第二个字母又能开始音节时也不用（`hauz` 的 `uz`）：那个字母更像下一个音节的简拼开头，`resolve_series_query` 的「纠错头 + 简拼尾」重试会把它读成 hua'z、`hauzh` 读成 hua'zh。完整读法的搜索排在重试前面，不挡的话 ha'zu 会先找到，把这两条既有的读法顶掉（`autocorrect_switch_matrix` 和 `resolution_composes_a_corrected_head_with_a_jianpin_tail` 当场就红了）。末尾能留下的两字母键只剩以 `i`、`u`、`v` 结尾的（`niwi`），而一个字母的简拼尾不可能是这三个，两条路不会再争同一段输入。

## 修复前后

同一个 release 构建，产品默认偏好（`msime-host-api` 的 `prepare_host`），锁定的 dict-v2.0.39，纠错开：

| 输入 | 修复前 | 修复后 |
| --- | --- | --- |
| `wiaini` | 空 | 我爱你、哎爱你、唉爱你 |
| `wiai` | 空 | 我爱、无碍、雾霭 |
| `wiaiwojia` | 空 | 我爱我家 |
| `iahao` | 空 | 爱好、哎好 |
| `wiaino` | 空 | 外你哦（wia→wai、in→ni 两处纠错） |
| `niwi` | 空 | 你我、你爱 |
| `hauz`、`hauzh` | 华中在候选里 | 不变（纠错头 + 简拼尾，hua'z、hua'zh） |
| `wi`、`hi`、`wifi` | 表情 / hi / WiFi | 不变 |
| `woaini`、`nihap`、`zhr`、`xiexir`、`woaimi` | — | 不变 |

`xiexir` 仍出不了谢谢：xie'xi'r 是「合法音节加一个尾声母」的简拼形状，`looks_like_syllable_with_jianpin_tail` 按设计不纠。`woaimi` 每个音节都合法，纠错搜索不跑，属于词网格纠错整句的范围。

## 评测

- 提交的 11 份基线（纠错关）用修复后的二进制全部 at baseline。
- 纠错开时，`sentences`、`harvested`、`neutral`、`particles`、`words --limit 3000` 五份正确输入的集合修复前后报告逐字节相同：干净输入都读得出来，两字母键碰不到它们。
- 合成误触集（一次性生成，不入库）：从 `quanpin-words-v1` 和三份整句集里取音节切法唯一的用例，把一个两字母音节换成读不出来的邻键串。词级 11114 条：found 0.009 → 0.801，top-1 0.002 → 0.269，top-3 0.002 → 0.432，top-5 0.003 → 0.580，候选为空 0.651 → 0.006。整句 159 条：found 0 → 0.145，top-1 0 → 0.082，候选为空 0.654 → 0.044。

## 验证

- `cargo test -p msime-engine`：单元 2730 通过、57 忽略，golden 31 通过；`cargo clippy -p msime-engine --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过。新增 `two_letter_keys_cover_only_strings_without_a_reading`、`a_two_letter_typo_reads_with_the_syllables_typed_right`（`pinyin::autocorrect`）和 `a_two_letter_typo_without_a_reading_is_corrected`（`quanpin::dictionary`，`wiaini` 的「我爱你」在前三、纠错关时不出现）。
- 真实词库探针 `cargo run --release -p msime-engine --example quanpin_autocorrect_dictionary <资源>`：`gau` 首选「挂」，`hauzh` 的「华中」在第 3 位。
- 没有在 Android 真机或模拟器上装包验收；改动只在引擎的纠错搜索，不碰文件、权限或安卓资源。

## 已知缺口

同成本的读法里谁当主切分按表的行序定（`wi` 的目标按字母序是 ai、ei、qi、si、wo、wu），词库频率只在整词行里和主切分比。整词能盖住全部输入时（`wiaini` 的「我爱你」、`wiai` 的「我爱」）高频的那个会排上来；长句的整句行只按主切分解码，于是 `wimenmingtiankaishi` 的首选是「挨门明天开始」，整句集 top-1 只有 0.082。这和三字母以上同成本错键（`ahan → shan/zhan`）是同一条既有规则，要改得让主切分按词频选或对同成本读法分别解码，会动纠错解析不依赖词库的前提（解析备忘在词库变化时不清空）和每键延迟，不在这次范围内。

## Alternatives considered

- **直接把参考生成器的长度下限从 3 改成 2**：最简单，邻键、乱序、删字、插字一起覆盖。没有采用：两字母串大多是简拼（`nh`、`wd`、`zh`），收进去会和简拼抢读法；参考表的行数和夹具也要一起改。
- **不加「要有打对的音节」那道检查**：单独打 `wi` 也能出「我」。没有采用：`hi`、`wifi` 这种整串读不出来的输入多半是英文，加上纠错后中文会排到英文词前面，空格上屏的就变了；只有旁边有打对的音节时才说明这一段是拼音打错。
- **只在「纠错头 + 简拼尾」的重试里不用两字母键**：先试过，`hauzh` 回来了，`hauz` 仍被完整读法的搜索读成 ha'zu，因为那条搜索排在重试前面；而且把两字母键放到最后再试要多跑一遍搜索。按末尾字母挡住只看一个字母，两条路的分工也写得清楚。
- **在宿主或云候选上处理**：云候选是否开启是用户的选择，关掉云候选时本地照样是空的，问题在引擎。

## Consequences

收益：原本整段没有读法、本地候选为空的两字母邻键或乱序误触（`wi`、`hi`、`ia`、`in` 这类）现在能纠回来，正确输入的结果和基线完全不变。

代价：读不出来的中间态会出现以前没有的纠错候选，例如 `wiaino` 显示「外你哦」，以前是空的；长句里同成本读法的首选可能选错（见上文缺口）。改动只在引擎，六个平台一起生效，宿主不用改。

和[相邻重复键纠错](2026-10-15-quanpin-repeat-key-autocorrect.md)一样，这次也只在查询端补纠错的范围，不改参考表的生成和开关语义。
