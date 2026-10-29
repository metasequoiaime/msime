# Agent Note: 全辅音简拼的字面整词排在全拼纠错前面

Status: implemented

## Problem

#9903：全拼打 `qjn` 想要「前几年」一类的三字简拼，打 `zhr` 想要 zh'r 的「正如、整容、招人」，两个纠错开关都开着（出厂默认）时首页全是纠错单字：`qjn` 被邻键纠错读成 qun/qin（j 挨着 u、i），首页是「群 亲 裙 侵」；`zhr` 被读成 zhe（r 挨着 e），首页是「这 者 着 折」。字面简拼行并没有丢，但排在整张纠错列表后面，`qjn` 的「前几年」排在两百多个 qun/qin 单字之后。

根因有两层。`crates/engine/src/pinyin/autocorrect.rs` 的 `looks_like_syllable_with_jianpin_tail` 让全辅音串一律保持可纠错，注释写的前提是「没有多字母简拼，`bqng` 只能是 `bang`」；[全拼简拼保留双字母声母](2026-10-20-quanpin-retroflex-abbreviations.md)之后 zh/ch/sh 也是简拼单位，这个前提早就不成立。`crates/engine/src/quanpin/dictionary.rs` 的 `query_exact_projected` 在 `corrected_input` 时把纠错读法当主键，字面读法只用 `append_unique_words` 接在后面兜底，于是简拼词一条也上不了首页。

## Decision

全辅音输入（不含 a/e/i/o/u/v，也不含 `'`）本身就是一串简拼。纠错分支把结果组装完之后调用 `quanpin::series::lead_literal_abbreviation_phrases`：匹配码折叠后等于原文字母、读满了全部按键的多字字面行（不含整句行）稳定地挪到最前面，纠错读法和只读了一部分按键的字面行按原来的先后跟在后面。纠错行的标记不受影响，`mark_autocorrect_candidates` 按匹配码判定，排序不改匹配码。

原来打头的纠错候选（匹配码不是原文前缀的首行）在词典给出的列表里最多退到首页最后一个座位，即宿主每页候选数减一，最小为 1，词典列表的首位归字面简拼词。这个座位只在词典列表里算：引擎之后插进首页的行（第一行后面的混输英文、首页末尾的快捷短语，见 `ime/queries.rs` 的 `default_quick_phrase_slot`）会把它往后推，`pdf`、`ppt`、`thx` 这类本身也是英文词或缩写的全辅音输入，纠错候选可能落到第二页。首行是只读了一部分按键的字面前缀行，或是匹配码等于原文的整句行时，没有打头的纠错候选，不保座，纠错行全部排在字面整词后面。`zhr` 读满全部按键的 zh'r、z'h'r 词在真实词库里有上百条，全排在前面会把纠错读法挤出前几页，等于对全辅音输入关掉了纠错；保一个首页座位让按偏了键的用户在首页还看得到「这」。这个座位跟随 `first_page_rows`，而 `set_first_page_rows` 本来就会清空系列缓存。

开着模糊音时，`query` 在 `query_exact` 之后还要调 `merge_series_by_coverage` 按匹配字母数重排整张列表，哪怕模糊音一行也没查到。删字纠错的匹配码比原文多一个字母（`shng` → shang 五个字母，字面 s'h'n'g 四个），这一步会把纠错读法整组排回字面简拼整词前面，保座也被冲掉。所以纠错在作用时（`series::has_corrected_cut`，与纠错标记用的是同一个判断），`query` 在模糊合并之后再调一次 `lead_literal_abbreviation_phrases`；没有纠错时不调，模糊音原有的「覆盖更多输入的模糊整词在前」不变。纠错和模糊音同时开着时，字面简拼整词也排在模糊读法的整词前面。

规则只看「全是辅音」和「有没有读满全部按键的字面整词」两件事，不比较权重：

- 字面简拼一条整词都没有时列表原样不动，纠错读法继续打头。
- 带元音的输入不走这条规则。`sahng` 的字面读法 sa'h'n'g 读得出「撒谎那个」，但那是音节加简拼的混合形状，误触的可能比简拼大得多，`sahng → 上` 仍排第一。
- 纠错关掉时不进纠错分支，字面结果和以前一样。

同一处改掉 `looks_like_syllable_with_jianpin_tail` 里过时的注释：全辅音串仍然可纠错，两种读法都保留，先后由词典定。

桌面共享设置页（macOS、Linux 的设置应用，以及 `mobile_settings` 为假的鸿蒙平板）在「高级选项」的模糊音后面加一组纠错开关：邻键纠错写 `quanpin.autocorrect_neighbor`，乱序纠错写 `quanpin.autocorrect_transposition`，默认值仍是开。全拼和双拼都读这两个字段，[双拼纠错整句](../feature/2026-10-09-shuangpin-typo-sentence.md)定下的是一个开关管两种方案，`shuangpin::engine::request_autocorrect_types` 把它们折成双拼的纠错掩码；`SchemeType::supports_autocorrect` 只管学习和抑制，不管纠错跑不跑。所以这组在全拼和双拼下都列出，组名、开关名和说明按当前方案写：全拼是「全拼纠错」，说明里写两个都关时漏打、多打字母的纠错（`request_autocorrect_mask` 的 `MISSING_OR_EXTRA`）也一起关闭；双拼是「双拼纠错」，说明只用于三个音节以上的整句、排在词条和整句后面。五笔不列出，但这不是因为五笔不纠错：五笔的混输拼音（`wubi_mixed_pinyin`）和 z 键拼音反查都经 `ime::apply_request_options` 带着这两个字段查全拼，纠错照样生效。两者是五笔里的附属输入，混输默认关，这次不为它们单列一组；五笔用户要关，得先切到全拼方案关掉，字段不分方案，回到五笔照样生效。其他方案不纠错，不列出。iOS、Android 的 Tauri 设置页也走这个共享页，但它们的原生设置（Android `TypingPage`、iOS `OnboardingView`）已有「拼音纠错」一行，键盘也不画纠错标记，所以这组只在桌面画（`!mobilePlatform`），说明文字也不提 `*` 标记。鸿蒙手机仍用[共享设置页拆分全拼纠错开关](../feature/2026-10-27-shared-autocorrect-switches.md)放在「中文」组里的那两行，不重复。宿主读这两个字段的代码早就有，不用改。

Windows 不在覆盖范围里。Windows 发布的设置窗口是原生的 WinUI 3 程序 `msime-client-settings.exe`（`platforms/windows/settings/main.cpp`，Win7 是另一套原生窗体），不启动 Tauri，不用这个共享页；它的 README 写明「自动纠错」照 macOS 不在窗口里，`platforms/windows/tests/tools/settings_ui.ps1` 还检查输入页里没有「自动纠错」。vitest 里 `renderInputPage("windows")` 渲染的是共享页，证明不了 Windows 产品上有这组开关。macOS 的共享页现在有了，两边不再一致；要补得改 `InputSettingsModel.h`、`main.cpp`、Win7 的 `AdvancedInputForm`、README 和那条 UI 检查，没有 Windows 构建环境验证，这次不做，单独跟进。

## 修复前后

`cargo run --release -p msime-engine --example quanpin_autocorrect_dictionary`，Studio 上的随包词库 `msime-test-resources`，两个纠错开关开、学习关，取候选前八：

| 输入 | 修复前 | 修复后 |
| --- | --- | --- |
| `qjn` | 群 亲 裙 侵 琴 勤 秦 覃 | 前几年 区间内 期间内 去见你 裘佳宁 群 亲 裙 |
| `zhr` | 这 者 着 折 浙 哲 遮 褶 | 正如 注入 阵容 众人 主任 这 张让 只认 |
| `bqng` | 帮 棒 榜 邦 绑 膀 镑 磅 | 并且能够 比去年高 比起那个 帮 棒 榜 邦 绑 |
| `gau`、`hauzh` | 挂 打头；华中在第 3 位 | 不变 |

`bqng` 是原注释拿来论证「只能是纠错」的例子，在真实词库里它的字面简拼有「并且能够」，按这条规则排到前面，「帮」紧跟其后。`shng`（漏了 a）同理：sh'n'g 有「是那个、少年宫」，删字纠错出的「上」排在它们后面，在词典列表里最晚在首页最后一个座位。

## Alternatives considered

- **让全辅音输入完全不进纠错**（在 `resolve_series_query` 的资格判断里加一条）：改动最小，`qjn`、`zhr` 立即干净。没有采用：`bqng → 帮`、`shng → 上` 这类真的打错的输入从此没有纠错候选；维护者要求纠错读法留在后面并带标记。
- **字面整词全部排前、纠错不保座**：最贴近「字面简拼排在纠错前面」的字面意思。没有采用：两字母、三字母的简拼表很密，`zhr` 的纠错候选会落到好几页之后，全辅音输入的纠错实际上等于没有。
- **按权重混排字面整词和纠错读法**：看起来更「公平」。没有采用：两组权重来自不同读音的表，纠错单字的权重普遍远高于简拼多字词，混排就是修复前的样子。
- **补词「请教你」或加「优先短语 / 优先逐字」选项**：issue 的另外两条希望。不做：词库来自锁定的生成流程，不按单条手工补词；用户打一次 qingjiaoni 学到的词条经 `create_word_from_canonical_pinyin` 写入时带 jp `qjn`，纠错开着也能被 `qjn` 查到，这次修复后它和其它字面整词一起排在纠错前面（`a_learned_phrase_leads_the_corrections_of_its_abbreviation`）。排序选项要解决的冲突在这次修复后不存在了，不再加偏好项。
- **设置组只在全拼下列出**：初版这样写，理由是引擎只对全拼纠错。这个理由不成立，双拼也读这两个字段，只在全拼下列出会让双拼用户在桌面共享页上没法关掉正在作用的纠错。

## Consequences

收益：全辅音简拼不再被邻键纠错抢走首页，`qjn`、`zhr` 词典列表的首位是字面简拼词；纠错读法仍在首页，桌面宿主照旧画 `*` 标记。用户学到的简拼词条（带 jp）同样排在纠错读法前面。桌面共享设置页能按当前方案分别关两类纠错。

代价：全辅音的真实误触（`bqng`、`shng`）首位变成字面简拼词，纠错候选退到其后，在词典列表里最晚在首页最后一个座位；想要「帮」「上」的用户多看一眼或多按一个数字键。这个顺序不看权重，排在词典查询之后，选词调频和 `PersonalTypoProfile` 都改变不了它：习惯性漏打、按偏的用户每次 `shng` 都选「上」，「上」也回不到首位，最好停在字面整词后面、最晚在首页最后一个座位。修复前这些输入的首位就是纠错候选，所以对这部分用户是一次学不回来的退步；要让学习能翻过来，得记下用户在这个按键串下选过纠错候选，再在那时不提字面整词，这次没做。纠错候选保座只保原来打头的那一条，其余纠错行仍在全部字面整词后面；保座是软保证，词典之后插入的混输英文和快捷短语会把它推后。

缺口：词典之后的重排器可以把纠错单字再转回首位。任何一行带纠错标记时，`ordering::any_corrected_key` 对全拼撤掉词典首选的豁免，`rerank_pick` 在回答按键的前几行里比模型分，其中包括保座的那条纠错（`zhr` 的「这」）；输入运行时的 `promote_row`（`crates/input-runtime/src/runtime.rs`）和网页引擎（`crates/engine-wasm/src/host.rs`）都会把选中的行提到首位。重排器默认关（`neural_desktop`），开着时语言模型分高的单字可能重新拿到首位，症状和 #9903 相同。重排器只看行，不知道输入是不是全辅音，要堵住得把这个信息带进 `OrderRow`，这次没做；上面的探针也没有接重排器。issue 里 `qjn` 首位的「强健泥」不在修复前引擎给出的完整本地候选列表里（纠错开着，用上面的探针），来源可能在宿主侧的云候选或 AI 候选，这次没有追查，纠错关闭时的本地列表也没有核对。没有在 macOS 或 Linux 的系统输入法里装包验收，共享设置页只跑了 vitest。Windows 原生设置窗口没有这组开关，见 Decision 末段。

## Verification

- `cargo test -p msime-engine`：新增 `literal_abbreviation_phrases_lead_corrected_readings`、`the_leading_correction_keeps_a_first_page_seat_behind_many_phrases`、`corrections_still_lead_without_a_literal_abbreviation_phrase`（`quanpin::dictionary`）和 `literal_abbreviation_phrases_move_ahead_in_their_own_order`、`the_leading_correction_keeps_the_last_first_page_seat`（`quanpin::series`），前两条在修复前失败。`literal_abbreviation_phrases_still_lead_after_the_fuzzy_merge`：开着模糊音（an=ang）时 `shng` 的首页和关着时相同，首位是字面简拼词，删字纠错的「上」在保座上，加 `query` 里那次重排之前失败。`autocorrect_marking` 里 `shng` 的断言随规则改成「撒谎那个」打头、带标记的「上」第二，夹具里「撒谎那个」的简拼正好是 s'h'n'g。
- `cargo clippy -p msime-engine --all-targets -- -D warnings`、`cargo fmt --all -- --check`。
- 真实词库探针 `quanpin_autocorrect_dictionary` 加了 `qjn` 首位是「前几年」、`zhr` 首位是两字词的断言，修复前后的输出见上表。
- `quanpin::dictionary` 的 `a_learned_phrase_leads_the_corrections_of_its_abbreviation`：经 `create_word_from_canonical_pinyin` 存下「请教你」后，两个开关都开时 `qjn` 首位是它，不带纠错标记，「群」在后面且带标记。
- `pnpm --filter @msime/desktop exec vitest run tests/settings/language-card.test.tsx tests/settings/settings.test.tsx`：macOS、Linux、Windows 三个平台的共享页（Windows 产品不用共享页，这一条只说明共享页本身）在高级选项里都有两个全拼开关，点击后只写对应字段；双拼方案下列出「双拼邻键纠错」「双拼乱序纠错」，写回同一对字段；五笔方案下不列出；iOS、Android 的共享页不列出；输入页的分组顺序断言加上「全拼纠错」。
