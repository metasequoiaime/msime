# Agent Note: Windows 组字细节对齐 macOS：双拼键位提示、双拼预编辑、成对标点、日语空格「変換」

Status: implemented

## Problem

组字过程中的四处细节 macOS 有、Windows 没有。双拼组字时 macOS 在候选窗旁显示当前方案的键位图（「输入时显示双拼键位提示」），开关存在 macOS 本机的 NSUserDefaults（`MSIMEClientShuangpinKeymap`）里，Windows 既没有这块浮窗，也读不到开关。双拼预编辑显示原始按键还是展开拼音（`shuangpin_preedit_uses_raw`）在 Windows 上已经生效，但能力位把设置项藏了起来，用户改不了。成对标点自动补全在 Windows 上只走标点键，不认「，也不补全从符号候选里选出来的左半边，全角模式下 `{` 补出的仍是半角的 `{}`。日语方案下 Windows 的空格直接上屏第一个转换，够不到第二个；回车那一半（上屏假名）此前已经补上。

## Decision

**双拼键位提示是共享偏好，Windows Server 画同一块浮窗。** `Preferences::shuangpin_keymap_hint`（`crates/client-core/src/preferences.rs`）是 `Option<bool>`，没选过时为 `None` 且不写进文档。macOS 的 `-shuangpinKeymap` 先读文档值，文档没有这一项时退回 defaults 里升级前的选择；`-sharedPreferencesByMerging:` 在载入过文档之后总是把当前值写进文档，还没载入过文档时只在本机留下过选择才写，迁移随下一次保存完成；`KeymapKey` 也登记在 `SharedOverrideProperties()` 里，「恢复默认值」清掉文档缓存和 defaults 后写回的是关，文档里的旧值不会在下次载入时回来；云快照导出的 `platform.macos.shuangpin_keymap` 也取这个当前值，套用云快照时丢掉文档缓存，让 defaults 里的新值经下一次保存回到文档。`Option` 是为了让 macOS 分得出「从没选过」和「选了关」，前者才退回旧键。文档带着这一项载入过一次之后，`-applySharedInputPreferences:` 删掉旧键（和应用例外迁移完成后删旧键一样），此后文档再缺这一项——共享设置页「恢复默认设置」、导入不带这一项的设置文件——就是关，升级前的旧选择不会复活。新能力位 `shuangpin_keymap_hint`（macOS、Windows）控制共享设置页「输入」页双拼方案下的开关，改动写共享偏好；macOS 在文档还没有这一项时用 `loadMacosShuangpinKeymap` 读旧值来显示，写本机 defaults 的 `save_macos_shuangpin_keymap` 命令随之去掉。WinUI 设置的「输入方案」组有同一个开关。

Windows 上要不要显示由 `CandidatePresentation::shuangpin_keymap` 随候选快照带出：view 的 scheme 为 1、`local_mode` 为 `none`、不在 Engine 自己的英文模式、有方案名、正在组字，与 macOS 的 `updateKeymapPanel` 相同；高亮 `editing_text` 最后一个字母或分号。Server 主循环在候选窗 `refresh()` 之后按同一帧调用 `ShuangpinKeymapWindow::update`，开关关着或候选窗不可见时收起。键位表经 `msime_client_shuangpin_key_hints` / `msime_client_shuangpin_zero_initials` 取自 Engine 的方案表，不另存一份；几何、文字和摆放在 `src/candidate/ShuangpinKeymapLayout.h`：620×203 DIP 的卡片、三排键帽、底部零声母说明，放在候选窗离光标远的一侧（候选窗在下方就接在它下面，放不下就跨过光标所在行放到上方；候选窗翻到上方时反过来）。摆放用的是候选卡片本身的矩形（`CandidateWindow::card_on_screen`，去掉四周透明的阴影边距和吉祥物那一条），不是窗口外框：外框顶边带着 20 DIP 的阴影边距，比光标锚点还高，按它判断会把下方的候选窗当成翻到了上方，键位图盖住正在输入的几行，还会向左错开 32 DIP、和卡片之间隔出 40 DIP 的空；在哪一侧按卡片的竖直中线和锚点比较。窗口不激活、鼠标穿透，配色与候选窗相同，强调色上的字按相对亮度取黑或白，因为 Windows 深色主题的强调色是浅蓝。

**双拼预编辑在 Windows 上给出设置项。** 候选窗的预编辑行画 view.preedit，TIP 的行内组字在「原始按键」样式下取自己宿主会话的 preedit，在「拼音分词」样式下取 Server 回传的预编辑，两处都来自应用了这个偏好的 Engine，所以 `HostCapabilities::shuangpin_preedit` 加上 Windows，WinUI 设置的「预编辑」组加同一个选择。

**成对标点的覆盖面与 macOS 相同。** 成对表移到 `tsf/Global/PairedPunctuationHostPolicy.h` 的 `PairedPunctuationClosingFor`，加上「」和全角的｛｝，标点键路径和 `_GetPairedPunctuationClosingFor` 共用它。全角模式下 `{` 键开的是 ｛｝，`}` 键跨过 ｝。候选上屏（点选、空格、数字、回车）的整条文字恰好是一个左半边时，`_HandleCandidateFinalize` 补上右半边，上屏后经 `_OpenCandidateCommitPair` 入栈并沿带焦点令牌的路径把光标移回两半之间；宿主会话拥有组字时数字在 `_HandleCandidateWorker` 里向宿主会话选词，不经过 `_HandleCandidateFinalize`，那里同样补全，并先结束组字再移回光标（第一版漏了这条路，数字选中的左半边单独上屏）；花括号只由按键补全，以引号或括号结尾的词句原样上屏，和 macOS 只看整条 commit 的规则一致。

**日语空格「変換」由 Server 执行。** 状态机从 Linux 挪到 `shared/input/JapaneseConversion.h`，Linux 的旧头文件只做转发。候选归 Server，所以 `ReplyComposer::basic_key` 在空格上屏高亮候选之前先问 `japanese_space_applies`（日语、裸空格、正在组字、不是 UILess）：第一次回 NavigationIgnored，不动会话；之后执行 `MSIME_NEXT_CANDIDATE`，过了本页末尾执行 `MSIME_FIRST_CANDIDATE`，回 MoveSelectionNext。回复里组字为空或 `cancel()` 时状态清掉。TIP 不另跑状态机：`_HandleCandidateFinalize` 读到日语空格的导航回执时保留组字，记下组字代次和宿主会话的 editing_text（`tsf/Global/JapaneseConversionPolicy.h`）；同一段组字、同一段读音的裸回车改按候选键处理并带 `CandidateActive`，Server 走已有的「回车选中高亮候选」路径，TIP 读回复上屏。排队按键的投影原本把空格当作结束组字（`FUNCTION_CONVERT` 清空投影），日语的空格是否结束组字要等 Server 回复才知道，所以日语空格不清空投影里的组字，紧跟着的空格和回车仍按组字中分类，不会因为投影以为组字已经结束而漏给应用；没有改成让投影作废、读实际状态，因为那时前面排队的字母可能还没处理。代价是唯一候选为 Fallback、空格实际上屏原文的少见情形里，排在后面的空格被吞掉。

## Alternatives considered

- **双拼键位开关留在各平台本地（Windows 另存一份）。** 不用碰 macOS 和共享偏好。但共享设置页仍然只能在 macOS 上改它，两个平台各说各话；任务要求两边认同一个键，所以提升为共享偏好并迁移 macOS。
- **`shuangpin_keymap_hint` 用普通 `bool`、缺省为关。** 和别的开关一样简单。但 host-api 载入时会补上缺省值，macOS 就分不出「从没选过」和「选了关」，升级前开着的用户会在第一次载入后被关掉。
- **横排候选时 ←/→ 移动候选高亮（照 macOS 当时按候选窗朝向决定的规则）。** 本分支一度这样实现：TIP 的宿主会话每次载入共享偏好时记下 `candidate_layout` 和 `navigation.arrows`，候选窗开着、横排、方向键翻选打开、方案不是行内组字时，立即路径和排队路径都把 ←/→ 归为 Server 候选键并按 ↑/↓ 的键码发出，Server 不用改；还比较过改由 Server 经一条新的 Worker 帧推给 TIP，因为要动共享帧编号（`MaxKnown`）和 TIP 的 Worker 线程而没选。最后整块撤掉，Windows 的方向键保持原有规则：产品决定是桌面三平台统一采用 Windows 原来的规则、不再按候选窗朝向决定——组字时 ↑/↓ 移动高亮（受方向键翻选开关约束），←/→ 移动组字光标，韩文汉字列表或注音列表打开时四个方向键都移动高亮，macOS 和 Linux 已改成这样（#6738「桌面三平台方向键统一，不再按候选窗朝向决定」）。Windows 再按朝向分支只会让三个平台重新分叉。
- **日语转换的状态机放在 TIP。** TIP 自己有宿主会话的 view，回车本来就在 TIP 里就地结束。但候选和高亮归 Server，TIP 要让 Server 移动高亮只能借 ↑/↓，而 ↑/↓ 受「方向键翻选」开关约束，关掉时空格就走不动；回到首条也没有现成的键码。所以由 Server 执行，TIP 只从回执类型知道转换开始了。

## Consequences

- **收益**：Windows 的双拼用户能看到键位图并改预编辑显示；成对标点和日语空格与 macOS 的手感一致。键位表和日语状态机都只有一份。
- **代价与已知上限**：TIP 判断回车是否上屏候选，靠的是自己宿主会话的读音与 Server 的会话一致；两边由同一份偏好驱动，读音分叉时回车退回上屏假名。键位图的摆放用候选窗的屏幕矩形和一行 24 DIP 的行高估计光标所在行，与候选窗翻到上方时用的行高相同。这些行为都没有在 Windows 真机上交互验证，只有交叉构建与主机上的策略用例两级证据。

## Verification

- 主机上可跑：`platforms/windows/tests/candidate/shuangpin_keymap_layout.cpp`、`platforms/windows/tests/input/japanese_space_policy.cpp`、`platforms/windows/tsf/tests/input/{japanese_conversion_policy,japanese_conversion_wiring,punctuation_key_policy,paired_punctuation_wiring}.cpp`，由 `scripts/test-windows-native-run.py` 自动发现；Linux 的 `linux-japanese-conversion` 经转发头仍然通过。
- `crates/client-core` 的 `host_surface` 断言与 `shuangpin_keymap_hint_stays_out_of_the_document_until_chosen`；共享设置页的 `settings.test.tsx`（macOS 显示旧值并写共享偏好、Windows 从共享偏好读写、Linux 不显示）与 `input-scheme-details-section.test.tsx`。
- macOS 的 `ShortcutTest` 的 `TestKeymap` 断言文档值优先、缺这一项时退回 defaults 并写回文档。
