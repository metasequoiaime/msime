# Agent Note: 内置拆字反查（C 模式）

Status: implemented

## Problem

用户遇到不会读的字（焱、犇、拆字里的「斥」）时，拼音输入法打不出它。Rime 的 `radical_pinyin`、fcitx5 拼音默认开的 `ChaiziEnabled`、搜狗 U 模式的部首拆分都提供「按部件的拼音找字」，kfcd/chaizi 有 800 多颗星。本仓库此前没有任何拆字能力：`crates/engine/src/wubi/lookup.rs` 写明字根拆分没有授权数据；五笔的 `z` 临时拼音反查和辅助码都要求先知道读音。维护者决定做成六个平台都有的内置功能，而不是脚本插件（移动端不运行脚本包）。

## Decision

- **入口**：第 10 个 Shift+字母本地模式 `LocalInputMode::Chaizi`，名字 `chaizi`，入口键 Shift+C（C 是「拆」的声母，记法同 J 简拼、Y 英语、R 罗马字）。判定在 Engine 的 `local_mode_for_entry`：受总开关 `shift_letter_entry` 约束，方案用 `opens_table_modes`（全拼、双拼、五笔），和 K 一样在五笔下也能进，因为它查的是编进引擎的表，不经过方案的拼写。移动宿主经同一个 Engine 入口打开（菜单发带 Shift 的 `C`），宿主按键路径不改。
- **默认值按宿主形态区分**：Engine 的 `LocalModeOptions::chaizi`、`EngineOptions::local_chaizi` 默认 `false`。共享偏好是 `local_modes.chaizi: Option<bool>`，`#[serde(default, skip_serializing_if = "Option::is_none")]`，缺省 `None`。host-api 在建会话和偏好重建两处取 `chaizi.unwrap_or(TOUCH_KEYBOARD_HOST)`，`TOUCH_KEYBOARD_HOST = cfg!(any(target_os = "android", target_os = "ios", target_env = "ohos"))`：触屏宿主默认开（从菜单进入，没有按键冲突），桌面宿主默认关（打开后句首和中英混打的大写 C 会进模式，见下）。只有用户动过开关时文档里才有这个键。
- **输入与匹配**：`C` 之后收字母，长度上限 `GENERATED_MODE_INPUT_LIMIT`；`'` 只收在字母之后、不能紧跟 `C`、不能连续，宿主当标点送来时经 `takes_local_separator` 同样当分隔符，光标处插入走同一组规则。输入里有大写字母就不查表（`CPU`、`CEO` 照原样）。比较时去掉 `'`；写了 `'` 时每个 `'` 必须落在键的部件边界上。先列整串精确命中的字，再列前缀补全，补全要求输入已经打完键的前两个部件（以 `hen` 开头的键有几十个，`Chen`、`Chang` 因此只出原样行）。双拼另把输入按当前双拼方案转成全拼再查一次，和 E、M 模式相同。同一级按名次排，最多 36 行。没有命中时只有原样兜底行。
- **候选与注释**：行来源 `Generated`，`pinyin` 记命中的键。注释是这个字的带调读音（Unihan `kMandarin` 首值，焱 → `yàn`）。新加 `LocalInputMode::annotates_rows()`（V、`/`、`@`、拆字为真），替换注释与快照补辅助码两处原来的 `generates_text()` 判断，所以拆字行不补辅助码，133 个没有读音的字注释留空。`generates_text()` 不变，它还管学习和统计。
- **学习与统计**：选词不进任何词库（本地模式下句子学习本来就被拦住）；回车上屏原文（含 `C`），和 U、T、K、E、M、J 一样经 `commit_raw_with_policy` 学成英文词；计入打字统计（字是用户挑出来的，和 E、J 同类）。脚本插件的 `LocalModeClass::Chaizi = 13`，不在可读文本的白名单里。
- **简繁**：拆字行照常走宿主的繁体输出转换。繁体输出开着时（`EngineOptions::chaizi_traditional_output`，host-api 取自 `traditional_chinese_output`），Engine 按表里预先算好的 OpenCC s2t 单字字形去重，保留名次靠前的一行，免得一屏出现两个「議」；关着时简繁两种字形都列出。
- **数据**：kfcd/chaizi 固定提交 `e177ab54ce4edf255315a60593ffecbd8167e96b` 的简繁两表取并集，读音取仓库已锁的 Unihan 17.0.0（`kMandarin` ∪ `kTGHZ2013`，外加生成器里 `ALIASES` 的部首俗称，如 氵 san、辶 zou、灬 huo），名次按《通用规范汉字表》一二三级、它们的繁体形、基本区、扩展 A、辅助平面分层。`scripts/build_chaizi_table.py`（只用标准库）按 `resources/chaizi.lock.json` 校验输入后生成 `crates/engine/src/local/chaizi.tsv`（键 → 字串）和 `chaizi-chars.tsv`（字 → 名次、读音、繁体字形），两表提交进仓库，`include_str!` 编进引擎，第一次查询时建行首偏移表后二分查找。网页引擎（wasm32-unknown-unknown）里是空表。上游的几处异常（「□」占位行、带尾随空格的键、一个私用区字、一个与表内字重复的兼容表意字）的数目写成断言，上游变了就让生成失败。
- **许可**：CC BY 3.0 的署名、改动说明和许可证全文在 `resources/licenses/kfcd-chaizi-CC-BY-3.0.txt`，`docs/third-party.md` 的「编译进共享库的数据」一节有对应的一行。`scripts/test-chaizi-table.py` 的 `NOTICE_CHANNELS` 列出已经带上这份文件的声明渠道。

## 宿主契约

- 模式名 `chaizi` 出现在 `View.local_mode`、`commit_context.local_mode` 和 `EngineSnapshot::local_mode`；入口字母 `C`，经 `msime_client_character(handle, 'C', shift = true)` 进入。
- `spelling_symbols` 为空：数字照常选词，`'` 由 runtime 先问 `takes_local_separator`。
- 偏好键 `local_modes.chaizi`：缺键读作「由宿主形态决定」。触屏宿主把缺键读作开，和它们对其他本地模式的读法一致；桌面宿主的原生设置界面要把缺键读作关，开关写回明确的布尔值；共享设置页的 `defaultLocalModes` 不放这个键。
- 脚本插件 ABI：`LOCAL_MODE` 表加 `("chaizi", 13)`，SDK 的 `MSIME_LOCAL_MODE_CHAIZI` 与 `local_mode::CHAIZI` 同步。

## Alternatives considered

- **六个平台都默认开**：功能最容易被发现，和 fcitx5 拼音默认开 `ChaiziEnabled` 一致；按 macOS 自带词表实测，19,907 个 c 开头的英文词里只有 6 个整串命中（cheer、crier 等），前缀命中为 0。但中英混打「C+拼音」全部变样：`C盘` 上屏 `Cpan`、`C位` 上屏 `Cwei`，`C语言` 整串命中 6 个字、空格上屏「琰」。仓库对占用原本会作为文字交给应用的键的新模式一律默认关（V，`LocalModeOptions` 上方的注释），[Shift+字母进入快捷模式的总开关](2026-10-11-shift-letter-entry-switch.md) 也拒绝悄悄改变现有用户的习惯，所以桌面默认关。
- **六个平台都默认关**：最保守。但移动宿主把缺键读作开（iOS `?? true`、Android `optBoolean(key, true)`，HarmonyOS 不过滤），要它们改读法才不会出现点了没反应的菜单项，而触屏上从菜单进入根本没有按键冲突，代价是移动用户要先去设置里打开一个他们不知道存在的功能。
- **`/cz` 指令**：不占字母。但 `/` 模式默认关、要求中文标点，Android 没有 `/` 模式，触屏不提供 `/`。
- **小写 `u` 前缀**（搜狗、Rime `radical_pinyin` 的习惯）：其他输入法的用户熟悉。但 `u` 在双拼里是声母键、在五笔里是编码键，九键和 14 键打不出来，全拼里会抢走 `user`、`update`，还要改普通组字路径。
- **普通组字里整串命中时插入拆字行**（fcitx5 `ChaiziEnabled` 的做法）：不用进模式。但要改 `update_mixed_candidates` 和学习列表，会错调词频，回归面是六个平台的全部普通打字。
- **脚本插件**：不动内核。维护者决定做内置，移动端也不运行脚本包。
- **关掉繁体输出下的拆字转换**而不是去重：简单，但用户开着繁体输出时会上屏简体字，和其他候选不一致。
- **数据按资源包单独下载**（同粤拼、注音、笔画）：App Store 包里不含 CC BY 数据，署名义务随下载包走。但 iOS 键盘扩展和 HarmonyOS 键盘要能读到下载目录（`docs/plugins.md` 写明鸿蒙电脑的键盘读不到设置应用导入的包），表只有约 0.6 MB，和已编进引擎的韩语汉字表同一量级；维护者可以在许可判断不接受时改走这条路。
- **rime-radical-pinyin 当数据源**：上游同样含 kfcd 的 CC BY 3.0 数据和 CHISE（GPL-2.0+），署名义务不会消失，体积大约大十倍，格式是 Rime 专用，只作数据质量的对照。

## Consequences

- **收益**：六个平台共用一份 Engine 实现和一份数据，宿主只需在设置和菜单里加一项；生成器和锁让数据可复现，`--check` 和 CI 的 Rust engine 作业（`MSIME_FETCH=1`）挡住手改和过期的表。
- **代价**：
  - 每个原生平台的引擎多约 0.6 MB 只读数据（两表 298 KB + 324 KB，因为加了部首俗称，键从 14,448 个增至 17,942 个），首次查询分配约 150 KB 偏移表。iOS 键盘扩展的内存峰值没有测过。
  - 数据质量：11 个部件没有读音，108 条拆法、76 个字查不到；部首变体的叫法靠手写别名补，多音部件按 `kMandarin` ∪ `kTGHZ2013` 展开，仍可能漏掉习惯读法；186 个辅助平面字的字体覆盖没有保证。
  - `deny_unknown_fields`：用户动过拆字开关后，早于它的设置应用或 `msime-mcp` 会拒读文档，和 `shift_letter_entry` 的取舍相同。
  - 繁体去重依赖 host-api 看到的 `traditional_chinese_output`；IBus 没有可写偏好目录时的会话级切换下，繁体输出可能出现重复字形，不会上屏错字。
  - 许可有三处残余不确定，未经法务审阅：CC BY 3.0 与 GPL-3.0 没有权威的兼容声明（结论来自条款本身：第 3(b) 条允许改编、没有相同方式共享的要求，第 4(b) 条的署名要求落在 GPLv3 第 7(b) 条允许的附加条款里）；如果这张表被看作原作的重排而不是改编，第 4(a) 条的技术措施条款就适用于 App Store、AppGallery 的分发，这和仓库已分发的 CC BY 4.0、CC BY-SA 4.0 数据以及加加辅助码表是同一类风险；上游没有说明数据自身的来源。加加辅助码表的部件拆分已经通过 rime-radical-pinyin 带着同一份 kfcd 数据随 iOS 和 HarmonyOS 分发（`resources/helpcodes/NOTICE.md`）。
- **重访信号**：维护者判断许可不可接受时改走资源包；桌面用户普遍打开后抱怨 C 盘之类的回归时，考虑「C 后接已知英文词或单音节就交还」之类的保护规则，而不是改默认值。
- **落地范围**：本篇随 Engine、数据、共享偏好、host-api 和契约检查一起落地。宿主的设置开关、移动端菜单项、各平台声明渠道（照 `scripts/test-language-data-notices.py` 的渠道清单）由各宿主接上，接上一个渠道就把它加进 `test-chaizi-table.py` 的 `NOTICE_CHANNELS`。

相关笔记：[五笔开放本地表模式入口](../../implemented/bug-fix/2026-10-10-wubi-table-local-modes.md)（`opens_table_modes`）、[插件门控与宿主侧五笔表模式](../../implemented/bug-fix/2026-10-09-plugin-gates-and-wubi-table-modes.md)（移动端不为五笔开本地输入）、[五笔反查](2026-10-11-wubi-reverse-lookup.md)（已知读音时的查码路径，与拆字互补）。

## Verification

- `python3 scripts/build_chaizi_table.py --check`：两张表与锁定输入的生成结果逐字节一致（缓存里没有输入时 skip，设 `MSIME_FETCH=1` 下载）。
- `python3 scripts/test-chaizi-table.py`：表的不变量、样例（焱、好、李、拆）、锁与许可证一致、`NOTICE_CHANNELS` 里的渠道都列出许可证。
- `cargo test -p msime-engine`：`local::chaizi::tests`（匹配、分隔符、ü 写 v、nue/lue、前缀补全、排序、上限、大写、繁体去重、双拼）、`session::tests::chaizi_*`（入口与方案门控、默认关、读音注释、不学习、回车原文、`'` 的两条路、光标处插入、`CPU`/`Chen`/`Cpan` 只有原样行）、`host::tests::chaizi_*`（快照不补辅助码、选项映射）。
- `cargo test -p msime-host-api`：`chaizi_follows_the_host_form_until_the_user_flips_it`（桌面构建下缺键不进模式、`Some(true)` 进入、繁体输出去重、`Some(false)` 不进）、`chaizi_traditional_forms_match_the_shared_conversion`（表里每个字的繁体列等于 client-core 的 `simplified_to_traditional`）。
