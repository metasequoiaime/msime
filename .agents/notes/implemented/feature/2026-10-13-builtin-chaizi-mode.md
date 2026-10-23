# Agent Note: 内置拆字反查（C 模式）

Status: implemented

## Problem

用户遇到不会读的字（焱、犇、拆字里的「斥」）时，拼音输入法打不出它。Rime 的 `radical_pinyin`、fcitx5 拼音默认开的 `ChaiziEnabled`、搜狗 U 模式的部首拆分都提供「按部件的拼音找字」，kfcd/chaizi 有 800 多颗星。本仓库此前没有任何拆字能力：`crates/engine/src/wubi/lookup.rs` 写明字根拆分没有授权数据；五笔的 `z` 临时拼音反查和辅助码都要求先知道读音。维护者决定做成六个平台都有的内置功能，而不是脚本插件（移动端不运行脚本包）。

## Decision

- **入口**：第 10 个 Shift+字母本地模式 `LocalInputMode::Chaizi`，名字 `chaizi`，入口键 Shift+C（C 是「拆」的声母，记法同 J 简拼、Y 英语、R 罗马字）。判定在 Engine 的 `local_mode_for_entry`：受总开关 `shift_letter_entry` 约束，方案用 `opens_table_modes`（全拼、双拼、五笔），和 K 一样在五笔下也能进，因为它查的是编进引擎的表，不经过方案的拼写。移动宿主经同一个 Engine 入口打开（菜单发带 Shift 的 `C`），宿主按键路径不改。
- **默认值按宿主形态区分**：Engine 的 `LocalModeOptions::chaizi`、`EngineOptions::local_chaizi` 默认 `false`。共享偏好是 `local_modes.chaizi: Option<bool>`，`#[serde(default, skip_serializing_if = "Option::is_none")]`，缺省 `None`。host-api 在建会话和偏好重建两处取 `chaizi.unwrap_or(TOUCH_KEYBOARD_HOST)`，`TOUCH_KEYBOARD_HOST = cfg!(any(target_os = "android", target_os = "ios", target_env = "ohos"))`：触屏宿主默认开（屏幕键盘从菜单进入），桌面宿主默认关（打开后句首和中英混打的大写 C 会进模式，见下）。判断只看编译目标，不看运行时的设备形态：Android 的实体键盘（`MSIMEInputService.onKeyDown` 经 `character(unicode, isShiftPressed())` 送字母）和 HarmonyOS 的实体键盘、鸿蒙电脑（`HardwareKeyRouter.normalizeLetterCase` 还原大小写后送 Engine）上 Shift+C 同样进入拆字，「C盘」会上屏 `Cpan`。iOS 键盘扩展收不到硬件键，没有这个问题。只有用户动过开关时文档里才有这个键。与 Windows v0.11.1 `utility.radical_mode`（pin `unicode_mode` 改名）出厂开的差异已记录在 `2026-10-22-windows-factory-default-parity.md`，是否对齐留待产品决策。
- **输入与匹配**：`C` 之后收字母，长度上限 `GENERATED_MODE_INPUT_LIMIT`；`'` 只收在部件之后、不能紧跟 `C`、不能连续，宿主当标点送来时经 `takes_local_separator` 同样当分隔符，光标处插入走同一组规则。双拼方案用得到 `;` 键时（微软双拼的 ing），`;` 和普通组字一样只补完奇数长度的片段，片段从上一个 `'` 或入口字母之后算起（`InputSession::chaizi_takes_symbol`），`sjq;` 查到「清」。输入里有大写字母就不查表（`CPU`、`CEO` 照原样）。比较时去掉 `'`；写了 `'` 时每个 `'` 必须落在键的部件边界上，打头的 `'`（删掉 `C'huo` 之间的部件后留下的）不算分隔符。先列整串精确命中的字，再列前缀补全，补全要求输入已经打完键的前两个部件（以 `hen` 开头的键有几十个，`Chen`、`Chang` 因此只出原样行）。双拼另把输入按当前双拼方案转成全拼再查一次，和 E、M 模式相同。同一级按名次排，最多 36 行。没有命中时只有原样兜底行。
- **候选与注释**：行来源 `Generated`，`pinyin` 记命中的键。注释是这个字的带调读音（Unihan `kMandarin` 首值，焱 → `yàn`）。新加 `LocalInputMode::annotates_rows()`（V、`/`、`@`、拆字为真），替换注释与快照补辅助码两处原来的 `generates_text()` 判断，所以拆字行不补辅助码，133 个没有读音的字注释留空。`generates_text()` 不变，它还管学习和统计。
- **学习与统计**：选词不进任何词库（本地模式下句子学习本来就被拦住）；回车上屏原文（含 `C`），和 U、T、K、E、M、J 一样经 `commit_raw_with_policy` 学成英文词；计入打字统计（字是用户挑出来的，和 E、J 同类）。脚本插件的 `LocalModeClass::Chaizi = 13`，不在可读文本的白名单里。
- **简繁**：拆字行照常走宿主的繁体输出转换。繁体输出开着时（`EngineOptions::chaizi_traditional_output`，host-api 取自 `traditional_chinese_output`），Engine 按表里预先算好的 OpenCC s2t 单字字形去重，保留名次靠前的一行，免得一屏出现两个「議」；关着时简繁两种字形都列出。
- **数据**：kfcd/chaizi 固定提交 `e177ab54ce4edf255315a60593ffecbd8167e96b` 的简繁两表取并集，读音取仓库已锁的 Unihan 17.0.0（`kMandarin` ∪ `kTGHZ2013`，外加生成器里 `ALIASES` 的部首俗称，如 氵 san、辶 zou、灬 huo），名次按《通用规范汉字表》一二三级、它们的繁体形、基本区、扩展 A、辅助平面分层。`scripts/build_chaizi_table.py`（只用标准库）按 `resources/chaizi.lock.json` 校验输入后生成 `crates/engine/src/local/chaizi.tsv`（键 → 字串）和 `chaizi-chars.tsv`（字 → 名次、读音、繁体字形），两表提交进仓库，`include_str!` 编进引擎，第一次查询时建行首偏移表后二分查找。网页引擎（wasm32-unknown-unknown）里是空表。上游的几处异常（「□」占位行、带尾随空格的键、一个私用区字、一个与表内字重复的兼容表意字）的数目写成断言，上游变了就让生成失败。
- **许可**：CC BY 3.0 的署名、改动说明和许可证全文在 `resources/licenses/kfcd-chaizi-CC-BY-3.0.txt`，`docs/third-party.md` 的「编译进共享库的数据」一节有对应的一行。表里的读音、名次和繁体字形取自 Unihan，Unicode License V3 要求随数据附上它的版权与许可声明，所以同样的渠道也带上 `resources/licenses/Unicode-License-V3.txt`（仓库的先例：脚本插件包里编进 Unihan 派生数据时随附同一份文件）。`scripts/test-chaizi-table.py` 的 `NOTICE_CHANNELS` 列出已经带上这两份文件的声明渠道。

## 宿主契约

- 模式名 `chaizi` 出现在 `View.local_mode`、`commit_context.local_mode` 和 `EngineSnapshot::local_mode`；入口字母 `C`，经 `msime_client_character(handle, 'C', shift = true)` 进入。
- `spelling_symbols` 按光标处列出：`'` 在它收得下时（跟在部件后面），`;` 在微软双拼里它是韵母 ing 时；只有入口字母 `C` 时为空。数字照常选词。宿主照这里把这两个键当字符送进来，不再当标点先以高亮行结束组字；Windows Server 的「二三候选」也因此不把它们当选词键（`second_third_candidate_engine_input` 读的就是它）。这样做是因为宿主按整段组字原文数 `;` 的奇偶（IBus、Fcitx5、Windows、HarmonyOS 实体键盘都这样），入口字母 `C` 让奇偶错开一位，`Csjq` 之后的 `;` 原来会上屏 `Csjq；`。runtime 的 `takes_local_separator` 和 Engine 的 `handle_punctuation` 仍然接住当标点送来的 `'` 和 `;`。
- 偏好键 `local_modes.chaizi`：缺键读作「由宿主形态决定」。触屏宿主把缺键读作开，和它们对其他本地模式的读法一致；桌面宿主的原生设置界面要把缺键读作关，开关写回明确的布尔值；共享设置页的 `defaultLocalModes` 不放这个键。
- 脚本插件 ABI：`LOCAL_MODE` 表加 `("chaizi", 13)`，SDK 的 `MSIME_LOCAL_MODE_CHAIZI` 与 `local_mode::CHAIZI` 同步。
- 桌面宿主（macOS、Windows、Linux）的开关：macOS `AppearancePreferences.mm` 的 `localModeEnabled`、Windows 设置窗口的 `default_on = false`、Linux 的 `local_mode_enabled_by_default`、共享设置页的 `chaizi ?? !shiftLetterEntrySwitch` 都把缺键读作关。macOS 账号同步的总布尔 `platform.macos.local_input_modes` 不包含拆字（`MSIMECloudLocalModeKeys`）：默认关的键放进去，会让没动过开关的新用户导出假，导入时也会把拆字和其他模式一起开关。按键路由都不改：`'` 和微软双拼的 `;` 列在 `spelling_symbols` 里时，IBus（`engine_spelling`）、Fcitx5 和 Windows Server（`EditPolicy` 的 `spelled_by_engine`）都把它当字符交给 Engine。三个桌面宿主的声明渠道都带上了许可证文件。
- 移动宿主（iOS、Android、HarmonyOS）的菜单和开关都从各自的模式表生成，表末尾各加一项拆字（入口 `C`、标题「拆字」、偏好键 `chaizi`）：iOS `KeyboardViewController` 的模式表与 `LocalModeSettingsView`，Android `LocalInputMode.CHAIZI`，HarmonyOS `LocalInputMode.ts` 的 `MODES`。三处都把缺键读作开。iOS 按 Engine 实际所在的模式（`currentLocalMode == "chaizi"`）而不是触发字母决定画不画读音注释：拆字模式下任何方案、包括九键都画 Engine 给的读音，VoiceOver 读作「读音 yàn」而不是「还需输入」。接入时修了 iOS 一个所有本地模式都有的问题：`render` 判断是否离开本地模式时读的是上一次快照的值，从菜单打开模式后刚设好的 `localModeTrigger` 立刻被清掉，候选栏标题只剩字母、打字统计不记本地模式、临时日语上屏被转繁体；改为读本次快照的 `snapshot.isInLocalMode`。HarmonyOS 工具面板照旧不按 `local_modes` 过滤磁贴，实体键盘的 `APOSTROPHE_LOCAL_MODES` 也不加拆字：`'` 和 `;` 由 `HardwareKeyRouter` 按 `spelling_symbols` 当输入。Android 和 HarmonyOS 的触屏字母行只在 `local_mode == "none"` 时显示微软双拼的 `;` 键（`MicrosoftShuangpinKeyPolicy.visible`），拆字里要从符号面板打 `;`，经 runtime 的标点路由按 `spelling_symbols` 交给 Engine。iOS 设置 App 的关于页、Android 的 `assets/native-notices`、HarmonyOS 的 `resfile/licenses` 带上了许可证文件。

## Alternatives considered

- **六个平台都默认开**：功能最容易被发现，和 fcitx5 拼音默认开 `ChaiziEnabled` 一致；按 macOS 自带词表实测，19,907 个 c 开头的英文词里只有 6 个整串命中（cheer、crier 等），前缀命中为 0。但中英混打「C+拼音」全部变样：`C盘` 上屏 `Cpan`、`C位` 上屏 `Cwei`，`C语言` 整串命中 6 个字、空格上屏「琰」。仓库对占用原本会作为文字交给应用的键的新模式一律默认关（V，`LocalModeOptions` 上方的注释），[Shift+字母进入快捷模式的总开关](2026-10-11-shift-letter-entry-switch.md) 也拒绝悄悄改变现有用户的习惯，所以桌面默认关。
- **六个平台都默认关**：最保守。但移动宿主把缺键读作开（iOS `?? true`、Android `optBoolean(key, true)`，HarmonyOS 不过滤），要它们改读法才不会出现点了没反应的菜单项，而屏幕键盘从菜单进入没有按键冲突，代价是移动用户要先去设置里打开一个他们不知道存在的功能。
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
  - 每个原生平台的引擎多约 0.6 MB 只读数据（两表 298 KB + 324 KB，因为加了部首俗称，键从 14,453 个增至 17,942 个），首次查询分配约 150 KB 偏移表。iOS 键盘扩展的内存峰值没有测过。
  - 数据质量：8 个部件没有读音（生成器的 `unread_components`，不算「□」占位和私用区部件，它们所在的拆法另行去掉），因此去掉 83 条拆法，连同含「□」或私用区部件的 25 条共 108 条拆法、76 个字查不到；部首变体的叫法靠手写别名补，多音部件按 `kMandarin` ∪ `kTGHZ2013` 展开，仍可能漏掉习惯读法；185 个辅助平面字的字体覆盖没有保证。
  - `deny_unknown_fields`：用户动过拆字开关后，早于它的设置应用或 `msime-mcp` 会拒读文档，和 `shift_letter_entry` 的取舍相同。
  - 繁体去重依赖 host-api 看到的 `traditional_chinese_output`；IBus 没有可写偏好目录时的会话级切换下，繁体输出可能出现重复字形，不会上屏错字。
  - 许可有三处残余不确定，未经法务审阅：CC BY 3.0 与 GPL-3.0 没有权威的兼容声明（结论来自条款本身：第 3(b) 条允许改编、没有相同方式共享的要求，第 4(b) 条的署名要求落在 GPLv3 第 7(b) 条允许的附加条款里）；如果这张表被看作原作的重排而不是改编，第 4(a) 条的技术措施条款就适用于 App Store、AppGallery 的分发，这和仓库已分发的 CC BY 4.0、CC BY-SA 4.0 数据以及加加辅助码表是同一类风险；上游没有说明数据自身的来源。加加辅助码表的部件拆分已经通过 rime-radical-pinyin 带着同一份 kfcd 数据随 iOS 和 HarmonyOS 分发（`resources/helpcodes/NOTICE.md`）。
- Android、HarmonyOS 接实体键盘时默认开带来的「C盘 → `Cpan`」问题没有缓解：两者的设置都不提供「Shift+字母」总开关（宿主能力的 `shift_letter_entry_switch` 只给桌面，两个宿主的原生设置里也没有），用户只能整项关掉拆字。是按运行时形态（例如 HarmonyOS 的 `KeyboardFormFactor.isDesktop()`、Android 接了实体键盘）把缺键读作关，还是给这两个宿主也提供总开关，留给维护者决定。
- **重访信号**：维护者判断许可不可接受时改走资源包；桌面用户普遍打开后抱怨 C 盘之类的回归时，考虑「C 后接已知英文词或单音节就交还」之类的保护规则，而不是改默认值；Android、HarmonyOS 实体键盘用户出现同样的抱怨时按上一条处理。
- **落地范围**：本篇随 Engine、数据、共享偏好、host-api 和契约检查一起落地。六个平台宿主的设置开关、移动端菜单项和声明渠道（照 `scripts/test-language-data-notices.py` 的渠道清单）随后接上，都已列进 `test-chaizi-table.py` 的 `NOTICE_CHANNELS`；以后新增的渠道同样要加进去。

相关笔记：[五笔开放本地表模式入口](../../implemented/bug-fix/2026-10-10-wubi-table-local-modes.md)（`opens_table_modes`）、[插件门控与宿主侧五笔表模式](../../implemented/bug-fix/2026-10-09-plugin-gates-and-wubi-table-modes.md)（移动端不为五笔开本地输入）、[五笔反查](2026-10-11-wubi-reverse-lookup.md)（已知读音时的查码路径，与拆字互补）。

## Verification

- `python3 scripts/build_chaizi_table.py --check`：两张表与锁定输入的生成结果逐字节一致（缓存里没有输入时 skip，设 `MSIME_FETCH=1` 下载）。
- `python3 scripts/test-chaizi-table.py`：表的不变量、样例（焱、好、李、拆）、锁与许可证一致、`NOTICE_CHANNELS` 里的渠道都列出许可证。
- `cargo test -p msime-engine`：`local::chaizi::tests`（匹配、分隔符、ü 写 v、nue/lue、前缀补全、排序、上限、大写、繁体去重、双拼）、`session::tests::chaizi_*`（入口与方案门控、默认关、读音注释、不学习、回车原文、`'` 的两条路、光标处插入、微软双拼的 `;` 和它在 `spelling_symbols` 里的列法、删出来的打头 `'`、`CPU`/`Chen`/`Cpan` 只有原样行）、`host::tests::chaizi_*`（快照不补辅助码、选项映射）。
- 桌面宿主：macOS `local-mode-preferences`（缺键读作关、Shift+C 交还应用、打开后选出焱）、`cloud-appearance-settings`、`bundle-contents`；Windows `windows-session-smoke --local-modes`（`'` 作分隔符）；Linux `linux-spelling-symbols`、`linux-candidate-local-mode-labels`、Fcitx5 选项组项数，以及要锁定词库的 `ibus-engine-smoke --chaizi`（`ctest -R ibus-chaizi`）与 Fcitx5 原生测试 `--local-modes` 里的拆字用例；共享设置页 `local-modes-section.test.tsx`。
- 移动宿主：iOS 模拟器上的 `LocalModeSettingsTests.testChaiziShipsOnAndTurnsOffInTheLiveSession`（默认开、`huohuohuo` 首个候选焱读音 yàn、`nv'zi` 得好、关掉后不再打开）、`NineKeyKeyboardTests.testChaiziReadingsShowOnTheNineKeyStrip` 与 `testLocalModeLayouts`（打开每个模式后标题是模式名，不带 `snapshot.isInLocalMode` 的修正时拆字标题是 `C`）、`ProjectConfigurationTests.py`；Android `LocalInputModeSmoke`；HarmonyOS `keyboard-logic.test.ts`。三个移动平台都没有在真机或真实编辑器里打过拆字。
- `cargo test -p msime-host-api`：`chaizi_follows_the_host_form_until_the_user_flips_it`（桌面构建下缺键不进模式、`Some(true)` 进入、繁体输出去重、`Some(false)` 不进）、`chaizi_traditional_forms_match_the_shared_conversion`（表里每个字的繁体列等于 client-core 的 `simplified_to_traditional`）。
