# Agent Note: 日语成对标点漏补右半且光标误退

Status: implemented

## Problem

日语方案下在 Linux 输入 `[` 或 `{`，IBus 只上屏左半 `「` / `『`，光标却退到它之前；再按 `]` 或 `}` 时，又在刚补的右半后面插入一个。中文方案的同类按键和 Fcitx5 没有这个现象。

两处独立原因：

- 宿主的补全表 `paired_punctuation_closing()` 只收 `【】`、`〖〗`、`《》`、`〈〉`，`Brace` 模式还只硬编码判断 `「`；日语契约表的 `「→」`、`『→』` 从未进入这张表。`paired_closing_for_key()` 的 `]` / `}` 跨过条目也只有中文的 `】` / `〖` 和 `｝` / `}`，认不出日语方案刚落地的 `」` / `』`。
- IBus 的 `apply()` 只把「命令已被处理」返回给调用处；调用处把 `handled` 当成「宿主补进了右半」，无条件转发一个 Left。引擎实际没补成时（例如 `<` 在日文表里没有映射、全角回退只上屏一个 `＜`），光标仍被退回，于是出现「只上左半、光标在前」的观感。Fcitx5 早已有 `pair_inserted_` 守卫，只对真正补进右半的情况退光标。

这是 Linux 宿主侧的补全表和光标守卫缺陷，不改设置默认值、不给日文契约表加新映射。日文表的 15 项与键盘写法见[日语标点键面与上屏统一到契约日文表](2026-11-19-japanese-punctuation-keyface-consistency.md)；上游的光标扫描码、按下/松开配对和 Wayland 延迟见 [IBus 成对标点补全后光标没有退回两半之间](2026-11-20-linux-ibus-paired-caret-keycode.md)，本篇只处理「是否真的补了右半」这个前置判断。

## Decision

- `PairedPunctuation.h` 的补全表加入 `「→」`、`『→』`；`normalize_punctuation_pair()` 的 `Brace` 分支改为查这张通用表，`Bracket` 分支也走同一函数，宿主按提交末尾实际落地的字补右半，不再各写一份字面量。
- `paired_closing_for_key()` 的 `]` 在配对栈顶是 `」` 时返回 `」`，`}` 在栈顶是 `』` 时返回 `』`，跨过刚补的右半而不重复插入；中文的 `】` / `〖` 和 `｝` / `}` 分支原样保留。
- `ClientEngine.cpp` 的 `apply()` 增加 `bool *pair_inserted` 出参，由 `normalize_punctuation_pair()` 的返回值填充。IBus 的引号分支和括号／尖括号分支都用局部 `bool` 接住它，只在 `handled && pair_inserted` 时调用 `forward_paired_caret()`；`<` 平衡通知同样受这个守卫约束。专有英文模式下宿主自己补 `{}` 的兜底路径把 `pair_inserted` 置真，原来的退光标行为不变。消耗出参的调用点传 `nullptr`，签名变化不外溢。
- `《》` 不在日文契约 15 项里，按用户确认的键面事实，`Shift+<` / `Shift+>` 打出的就是 `<` / `>`，本次不新增映射，也不把 `《》` 当日文标点处理。
- 测试：`platforms/linux/tests/input/paired_punctuation.cpp` 按行为断言日语补全与跨过；`platforms/linux/tests/runtime/engine_smoke.cpp` 新增 `--japanese-paired` 场景，覆盖 `[` / `{` 补全并退一次 Left、`]` / `}` 跨过且不再上屏、`＜` 不误退光标；CMake 注册 `ibus-japanese-paired`。

## Alternatives considered

- 在 IBus 调用处比较处理前后的 commit 文本长度来判断有没有补进右半：能覆盖引擎和宿主兜底两条路，但智能标点、空格转换等路径也会改 commit 文本，长度变化不等于补了配对；用引擎的显式出参判定更准，也不会把别的联动算进来。
- 在补全表里再塞 `【】`、`〖〗` 之外的全部成对符号：用户明确本次只修日文契约表已有的对，`【】` 保持原行为，避免顺带改变现有键位手感。
- 在 `ClientEngine.cpp` 另写一条日文专用补全分支：改动更局部，但会与 `PairedPunctuation.h` 的表分叉，契约表再调整时容易漏；改成查同一张表后，IBus 和 Fcitx5 共用一份事实。

## Verification

- 红—绿：先只加测试，`paired_punctuation` 单测与 `--japanese-paired` 场景都失败（缺 `「」` 补全、`]` 没跨过、`<` 误退光标）；恢复生产改动后两者通过。
- 本地门禁：CMake 全量构建通过；`target/paired-test/paired_punctuation` 通过；`ibus-engine-smoke --japanese-paired` 通过；`ctest -E '^ibus-|linux-candidate-native-canvas'` 117/117 通过。两个 `linux-candidate-native-canvas*` 在完全撤销本次改动的基线工作树上同样失败（`auxiliary_ink > 10`、`[0x808080] > 10` 断言），主 `ibus-engine-smoke` 场景在基线上也失败于「IBus properties were not registered in the macOS menu order」，均为本机环境既有失败，未纳入本次通过数。
- 资源目录为合成副本；没有在真实编辑器和报告者环境里验收，PR 正文写明未采集对比图，不把本地 smoke 当成真机结论。

## Consequences

收益：日语方案下 `[` `{` 的右半能补上，光标停在两半之间；`]` `}` 只跨过不重复插入；日文表没有的键不再误退光标。Fcitx5 早已按 `pair_inserted_` 守卫，补全表扩展对它同样生效。代价：`apply()` 多一个出参和两处局部状态，日文补全表现在与中文表共用函数，后续改契约表时两套宿主会一起变化。其他平台、设置字段和缺字段读法均未触及。
