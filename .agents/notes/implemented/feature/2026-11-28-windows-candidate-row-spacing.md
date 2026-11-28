# Agent Note: Windows 候选窗消费行高与行距

Status: implemented

## Problem

#11939：Windows 用户觉得候选行太高、行与行之间挤，希望可以自己改行距或行高。此前 Windows 候选窗的行高是固定的上下各 6 DIP（`CandidateCardMetrics::row_padding = 12.0`，和 macOS 的 `MSIMECandidateRowPadding` 相同），行距由皮肤的 `item_gap` 或它自己的 0 决定，没有任何设置项能改。macOS 早已消费共享偏好里的 `candidate_item_padding_vertical` 与 `candidate_item_spacing`，Windows 既不读也不出控件。

## Decision

复用共享偏好里已有的两个字段，不新增设置键、不改默认值：

- 共享 Rust（`crates/client-core/src/preferences.rs`）里 `candidate_item_padding_vertical` 与 `candidate_item_spacing` 本来就是 0–16 的 `Option<u8>`，macOS 的设置页与 `CandidatePanel.mm` 已经在用；Windows 这次只是接上读、应用和出控件，字段语义不变：行高是候选文字上下各留的 DIP（0 时贴着文字），行距是行与行之间、横排时列与列之间的空。缺字段或 `null` 都表示跟随皮肤与窗口布局，0 是一个真实的值而不是「跟随」。
- `CandidateWindowStyle` 加两个 `std::optional<unsigned>`，`valid()` 各限 0–16，纳入 `operator==`，让改值重新排版（`set_style` 走 `invalidate_geometry()`）。
- `CandidateWindowStyleSettings.h` 投影两个键；类型错、越界或整份文档不合法时照旧整份拒绝，候选窗保留之前的样式。
- `CandidateCardSize.h` 新增 `CandidateRowStyle`（两个 `optional<double>`）并作为 `candidate_card_metrics` 的最后一个参数，`CandidateCardInput`、`candidate_card_size`、`candidate_expand_card_size` 一路带下去。设置了行高时 `row_padding = 2 × 值`、`candidate_row = line + row_padding`，压过皮肤的 `item_extra_height`；设置了行距时 `item_gap = 值`、`column_gap` 在皮肤原本的 6 DIP（`fit_content` 时是 0）之上再加值、间距大于 0 时高亮铺满整行（`selection_inset = 0`）。命中区、卷轴展开的步进、卡片高度都从同一份 metrics 来，所以点击区与画面不会错位。
- 设置界面：Windows 11 原生设置的「窗口样式」标签加「行高」「行距」两行（滑块 0–16，旁边「跟随皮肤」把键写回 `null`，说明随数值就地改写）；预览按用户值画行内上下留白与列表间距。Windows 7 兼容设置的候选外观页把两个键加进可选数字字段（第一项「跟随皮肤」写回空值）。这两个键在设置同步字段表里本来就有，账号同步不需要改。

只做 Windows：issue 是 `platform/windows`；其他平台按界面迁移顺序各自接入，共享字段与校验没有动。

## Alternatives considered

- **新增一个「候选行距」专用键。** 最强理由是名字更直白、范围可以只给 0–8。但共享层已有两个语义相同的字段且 macOS 在消费，新键会让同一种外观在两个平台由不同的键驱动，设置同步与导入导出也要多一套；复用现有字段是同一件事的更小改动。
- **只做行距、不动行高。** 用户说的是「行距太大」，而行高才是决定行有多高的那一项；只做行距，压缩出的空会被行高占回去，用户仍会觉得行挨不上。两行都在共享层的既有字段里，一起出控件成本几乎相同。
- **把默认行高从上下各 6 DIP 调小。** 这会直接改变所有 Windows 老用户的手感，属于改出厂默认值，需要维护者单独授权；这次只给「能改」，默认仍是 6 / 0。
- **在 Windows 设置里自己算一份行高而不走 `candidate_card_metrics`。** 预览、命中区和卡片尺寸必须用同一份几何，分开算会在缩放的边界上出现「看着宽、点不上」。

## Historical audit

按 `candidate_item_padding`、`candidate_item_spacing`、`行高`、`行距` 与「窗口样式」检索活跃笔记：前两个键没有任何既有决定；[候选字号、字体和窗口样式共用叶子契约](../../implemented/simplification/2026-10-15-candidate-preferences.md)持有共享类型的归属，[macOS 预览 token](../../implemented/bug-fix/2026-10-11-macos-candidate-preview-native-tokens.md)与[内边距下限](../../implemented/bug-fix/2026-10-15-macos-candidate-padding-below-2pt.md)持有 macOS 的运行时几何，均独立保留，本次只补 Windows 的应用侧。

## Consequences

- 默认设置下画面与改动前完全相同：行高仍是上下各 6 DIP、行距 0，没有默认手感变化。
- 想更紧凑的用户可以把行高压到 0–5、行距保持 0；想更松的可以各自加。两个值都随「整体大小」一起缩放，150% 下 8 DIP 的行高是 12 DIP 像素。
- 竖排行距、横排列距由同一个「行距」控制，和共享设置页的说明一致；横排的列距在皮肤版式下是「原本 6 DIP + 用户值」，在不熟悉皮肤的机器上是「用户值」本身。
- 皮肤写了 `item_extra_height` 时用户的行高压过它；用户只设行距时行高仍跟随皮肤。
- macOS、Android、iOS、HarmonyOS、Linux 的候选窗不变；这些字段在它们上面仍由各自的设置页控制或保持未接入。
- Windows 7 兼容设置页能读写这两个键，但它的预览是固定样例，不随行高变化；Windows 11 设置页的预览会跟着动。

## Testing

- `platforms/windows/tests/ui/candidate_card_size.cpp` 新增一组断言：设了行高与行距后卡片高度按 `line + 2 × 行高 + 行距 × (行数 - 1)` 变化、`candidate_row_bounds` 的下一行从上一行底部再加行距开始、横排列宽按列距变化、皮肤 `item_extra_height`/`item_gap` 被用户值压过而只给一个值时另一个仍跟随、两个空表示跟随、0 是贴边的真实值。由 `python3 scripts/test-windows-native-run.py` 在本机真跑。
- `platforms/windows/tests/ui/candidate_window_style.cpp` 覆盖投影：两个键的默认、`null`、上下界 0 与 16、越界 17、负数、小数、字符串与布尔整份拒绝，以及「跟随时与原样式相等」。
- 两个改动的 Windows 翻译单元（`CandidateWindow.cpp`）用本机 MinGW 以 `-fsyntax-only` 编过；`CandidateExpandPanel.h` 经它一并编到。Windows 11 设置工程（WinRT）与 Windows 7 兼容设置由 CI 的 `Windows native settings` 作业用 MSVC 编译，并跑该作业里的 ctest（含 `windows7_settings` 新增的两条断言）。
- 实际画面需要 Windows 实机截图：本机没有 Windows 环境，按 #11929 的先例在 PR 的「截图」一节写明原因，请在实机上确认行高 0/6/16 与行距 0/4/16 的候选窗与预览一致。
