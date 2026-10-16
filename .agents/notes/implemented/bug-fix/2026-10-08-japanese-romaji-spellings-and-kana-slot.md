# Agent Note: 日文罗马字对齐主流输入法，完整读音的平假名留在第一页

Status: implemented

## Problem

用户拿 Rime（雾凇加カギロイ）对比日文输入时暴露出拼法和候选位置不一致，假名反查也需要满足整段读音往返不变：

- `xtu` 打不出促音っ。罗马字表照搬 `romaji_converter.cpp`，只有 `xtsu`/`ltsu`，`xtu` 整段停在待定字母里，候选为空。同一张表还缺微软和 Google 日文输入法都认的 `zya`/`zyu`/`zyo`、`dya`/`dyu`/`dyo`。
- 「`nn` 打出んん」：习惯微软、Google 输入法和 Rime 的人每个ん都打 `nn`。原规则在 `nn` 后跟元音或 `y` 时只把第一个 n 当ん，`sinnyou` 得しんにょう、`kinnenn` 得きんねん；输入途中 `onnn` 还会先显示成おんん。
- `tyou` 打不出ちょう。转换本身没错（`tyou`、`chou` 都转成ちょう），但词库里没有ちょう这个假名词条，平假名被追加在全部汉字和联想之后，排在第 23 个；有假名词条的きょう、にほん本来就在第二位。
- 促音反查把后一个假名的首辅音重复一遍，却没有排除 `n`：`っな` 反查成 `nna`，再转换得到 `んあ`；`っん` 反查成 `nn`，丢了促音。`cycle_last_kana_variant` 会重新转写整段读音，所以输入 `xtsunaha`（っなは）再切换末尾的は，连前面的っな也被改成んあ。

## Decision

- `ROMAJI_TABLE` 补上 `xtu`、`ltu`、`zya`、`zyu`、`zyo`、`dya`、`dyu`、`dyo`。反查 `hiragana_to_romaji` 仍按「最长拼法、再字母序」取一个：っ 还是 `xtsu`，じゃ 还是 `jya`，金样里记录的拼法不变。
- `convert_romaji` 里 `nn` 一律是一个ん，和微软、Google、Rime 相同：`sinnyou` 得しんよう，`kinnenn` 得きんえん；んな要打 `nnna`，こんにちは要打 `konnnichiha`。反查 `hiragana_to_romaji` 在ん后面是元音、や行或な行时写 `nn`，否则写 `n`，反查结果再转换回去仍是同一个读音（`cycle_last_kana_variant` 靠这一点）。
- 促音只在后一个假名的首辅音不是 `n` 时使用重复辅音；な行或ん之前显式写成 `xtsu`，与正向扫描中 `current != b'n'` 的条件一致。`っな` 反查为 `xtsuna`，`っん` 反查为 `xtsun`；片假名先折为平假名，沿用同一规则。
- `JapaneseProvider` 在读音完整（没有待定字母）时，把平假名挪到不晚于第二位（`KANA_SLOT = 1`），其余行保持相对顺序；首位仍是最可能的转换。还挂着半截字母时不挪，那时该先给能拼成的词。片假名位置不变。

## Alternatives considered

- **平假名放第一位**：Google 日文输入法在变换前就是平假名，第一位最接近它。不用是因为水杉的候选条第一位就是空格上屏的那个，放平假名会让 `nihon` 空格上屏にほん而不是日本，改变了所有人的日常输入。
- **按权重重排整张列表**：平假名的 `KANA_WEIGHT` 本来最高，按权重排它会到第一位，问题同上；而且这个列表的顺序是有意按来源拼起来的（见 `query_into` 的注释），整体重排会打乱联想与转换的先后。
- **保留原来按后一个字母判断的 `nn` 规则**：最强的理由是 `konnichiha`、`kanna` 这种只打两个 n 的写法能直接得到こんにちは、かんな，初学者不用知道 `nnn`；它也是 C++ 参考实现的行为。不用是因为日文用户的肌肉记忆来自微软、Google 和 Rime，在那些输入法里 `nn` 就是ん，同一串按键在水杉里得到另一个读音（しんにょう、きんねん），而且看不出为什么。两种写法不能同时成立，跟主流走。
- **只补 `xtu`，其余拼法不动**：改动最小。但 `zya`、`dya` 和 `xtu` 是同一类缺口（主流输入法都认、C++ 表缺），用户换个词就会再撞上。没有补 `xka`/`xke`（小写ヵヶ）和 `lwa`：前者在平假名里没有通用写法，后者会把ゎ的反查从 `xwa` 变成 `lwa`。
- **反查促音一律用 `xtsu`**：可以让全部组合不依赖后一个假名，往返规则更简单。但 `きって` 等正常的重复辅音拼法已有固定契约，全部改写没有必要；仅在 `n` 前使用显式拼法即可修复歧义。

## Consequences

- **收益**：习惯微软、Google、Rime 的用户按原来的手法打字就能得到同样的读音（っ、じゃ行、ぢゃ行、`nn` 的ん）；没有假名词条的读音，平假名也在第一页。
- **代价**：原来只打两个 n 的写法变了：`konnichiha` 现在得こんいちは，`kanna` 得かんあ，要多打一个 n。这偏离了 C++ 参考实现，金样场景里没有受影响的用例（唯一带 `nn` 的 `menn` 在结尾）。完整读音时第二位固定给平假名，原来排第二的转换往后挪一位。
- **没有覆盖**：桌面和各平台宿主上的实际效果只靠引擎测试推断。
- **促音边界**：反查和末尾假名变体切换保留な行、ん之前的っ；这类输入的内部罗马字拼法会变长，常规重复辅音拼法保持。

## Verification

- `cargo test -p msime-engine`：1301 条单测和 31 条金样全部通过。新测试 `microsoft_and_google_spellings`、`a_complete_reading_keeps_its_hiragana_on_the_first_page`（去掉挪位时失败）；`doubled_n_rules` 按新规则改写，`kana_and_romaji_round_trips` 检查ん在元音、や行、な行前的反查能转回原读音。
- 用 `MSIME_EVAL_RESOURCES` 指向的真实日文词库：`tyou`、`chou` 的ちょう在第二位；`real_model_answers_common_readings` 通过。`real_model_loads` 失败是因为它断言 dict-v2.0.1 的词条数（1,284,987），本地词库是更新的版本（1,285,198），与这次改动无关。
- 促音边界回归 `sokuon_before_n_preserves_both_kana_on_round_trip` 与 `kana_variant_cycling_preserves_sokuon_before_n_in_the_prefix` 在修复前分别得到 `んあ`、`んあば` 而失败，修复后通过；覆盖な行、にゃ、ん、已有拨音前缀和片假名反查。2026-10-16 的 `cargo test -p msime-engine --no-fail-fast` 通过 2194 项单测及 31 项金样（34 项需显式启用的词库及性能用例忽略），`cargo clippy -p msime-engine --all-targets -- -D warnings` 通过；未做平台宿主界面验收。

待定假名前缀采用[固定拼法索引](../testing/2026-10-09-japanese-kana-prefix-index.md)借用排序去重结果；转换的规范化和全部别名边界保持。
