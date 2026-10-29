# Agent Note: `{` `}` 固定出「」，Windows 与 Linux 跟上 macOS（issue #10047）

Status: implemented

## Problem

issue #10047 要求中文标点下按 `{`（`Shift+[`）出「、按 `}`（`Shift+]`）出」，和雾凇拼音一样；另外希望开放标点自定义。改动前三套语义并存：

- macOS 普通中文方案由宿主在到达引擎前把 `{` `}` 拦成固定的「/」（#9062），已发布。
- 共享契约第 3 版（#9185，见 [第 3 版笔记](2026-10-21-shift-bracket-corner-quotes.md)）在 `alternating` 里让 `{` 交替出「」、`}` 交替出『』：连按两次 `{` 才得到」，`}` 出的是『。引擎、`shared/input/EnglishModeOutput.h` 都照这张表。
- Windows TIP 的本地表 `PunctuationTable` 把 `{` `}` 映射成 ASCII 本身，成对补全补 `{}`、全角补 ｛｝；Linux IBus 与 Fcitx5 在成对补全打开时把 `{` 走 `msime_client_punctuation_ascii` 补 `{}`，关掉时才落到引擎的交替表。

第 3 版笔记把「到底用固定「/」还是交替表」留给 #8998 确认。#8998 的报告者核对过 macOS 系统布局（`Shift+[` `Shift+]` 出「 / 」，整个布局没有『』），#10047 要的也是固定「/」，交替表已经没有依据。#10047 里没有书面的范围决定：维护者 2026-10-28 的回复只确认 macOS 已有实现（#9062），其他平台的范围待定。这里定的范围是 Windows 和 Linux 跟上 macOS，不加开关，不做任意逐键标点自定义，由维护者在 PR 评审时确认。

## Decision

键位语义定为固定「/」，从契约一路落到各宿主：

- 契约升到第 5 版（`shared/contracts/punctuation/policy.json` → `generate.py` → `policy.h`）：`{` → 「、`}` → 」挪进 `simple`，从 `alternating` 和 `styles.cornerQuotes` 删掉。它们是无状态的一对一映射，正好是 `simple` 的语义；`generate.py` 要求 cornerQuotes 与 alternating 键集一致的校验因此不用改。升版本是因为表结构变了（两个键换了段），`punctuation_contract.cpp` 与引擎的 `matches_the_shared_contract` 同步。
- 引擎 `crates/engine/src/punctuation.rs`：`SIMPLE` 末尾加两行，删掉 `{` `}` 的两个交替状态。`{` `}` 走 `SIMPLE` 分支，所以也会关掉引号的退格撤销窗口，与其他固定标点一样。注音大千键位的 `SHIFT_PUNCTUATION`（`{` `}` 出『』）不经过这张表，不变。
- `shared/input/EnglishModeOutput.h`（macOS、IBus、Fcitx5 英文模式下锁中文标点）：`{` `}` 改成固定的两行，删掉两个交替状态字段。
- Windows TIP：`Globals.cpp` 的 `PunctuationTable` 两行改成「/」。`PairedPunctuationHostPolicy.h` 删掉 `{`/`｛` 的成对条目、候选侧对花括号的排除、只为全角花括号存在的 `PairedPunctuationKeyOpening` 和跨过时的全角参数：`{` 出的「按已有的「→」条目补全，`}` 出的」按已有的」分支跨过。`KeyHandler.cpp` 去掉 `fullWidthPair`，并让 `{` `}` 出的「」不武装空格转换。
- Linux：IBus（`ClientEngine.cpp`）与 Fcitx5（`FcitxEngine.cpp`）的 `{` 改走 `msime_client_punctuation`，和 `(` `[` `<` 一样交给引擎中文标点表。`PunctuationPairMode::Brace` 认上屏末尾的「补」；`paired_closing_for_key('}')` 在配对栈顶是」时跨过」。
- macOS：可见行为不变。`MSIMEKeyProducesCornerQuote` 里 `}` 不再算出『』的键（契约里它已不出『』），注释同步。
- iOS：`SymbolLayerLayout.engineTranslatedKeys` 加上 `{` `}`，中文层 `#+=` 上这两个键按键面原样插入，见下文「移动端」。
- Windows 的「`{` `}` 不武装空格转换」写成纯函数 `Global::PunctuationKeyArmsSpaceConversion`，`punctuation_key_policy.cpp` 按行为测它，`paired_punctuation_wiring.cpp` 只查武装处用的是它。

## 专用英文与引擎不处理的情况

Linux 两个宿主在成对补全路线上保留了花括号补全：引擎写 ASCII 标点的方案（专用英文：组字中把 `{` 跟在上屏串后，空闲时交回宿主）出的仍是 `{`，`Brace` 模式照旧补 `}`（全角 ｛｝），空闲时宿主自己补 `{}`。`}` 跨过时按配对栈顶在」和花括号之间挑。这样专用英文的行为与改动前一致，与 macOS 「独立英文模式保留大括号的成对补全」相同。Fcitx5 现在经 `punctuation()` 走带上下文的标点路由，大写锁定英文标点打开时 `{` 走 ASCII 路线，同样落回花括号补全。

## 空格转换

Windows 与 Linux 的空格转换都不认 `{` `}` 出的「」：Linux 的 `is_space_conversion_key('{')` 本来就是假（[宿主表笔记](2026-10-22-punctuation-host-tables-enum-comma-space.md) 决定花括号不进空格转换名单），Windows 的 `SmartPunctuationAsciiFor` 会把「」按引号样式转回 `'`，所以在武装处按键排除。macOS 宿主把 `{` 出的「转回 `{`（#9062 带来的宿主扩展），`}` 出的」仍转回 `'`，这次不动 macOS 的可见行为。`crates/client-core/src/punctuation.rs::ascii_for_chinese_mark` 也不动：移动端触屏的 `{` 在 `#+=` 层按字面插入（见下文「移动端」），不会以 `{` 的身份送来「。

## 不做的部分

任意逐键标点自定义不做，理由沿用 [标点样式笔记](2026-10-13-punctuation-mark-styles.md) 的「任意逐键映射表」一条：六个宿主的补全、跨过、改回前引号、空格转换、英文模式和触屏键面都要改成运行时数据，Windows TIP 还要把整张表过 IPC。也不加开关（第 3 版笔记已否决过，理由不变）。

## 移动端

三端触屏键盘的 `{` `}` 都只在 `#+=` 层，中文层上按键面原样插入 `{` `}`，不经过引擎。Android 设计层（`commitNineKeyLiteral`）和 HarmonyOS（`NumberSymbolLayout.route` 中文层一律 `LITERAL`）本来就是这样。iOS 原先不是：`SymbolLayerLayout.input` 把 `{` `}` 当作引擎原样放行的 ASCII，走 `handleSymbol` → `handlePunctuationWithContext`。第 3 版契约下这两个键就会被引擎换成交替的直角引号，第 5 版后键面是 `{` 打出的却总是「；成对补全默认开，`PairedPunctuationPolicy.completion` 按「补」，而 `PairedPunctuationStack.closings` 没有 `}`，再按 `}` 写出第二个」，得到「」」。所以把 `{` `}` 加进 `engineTranslatedKeys`，和 `[` `\` 一样按键面插入，与另两端一致，也守住中文层「键面与写出的字始终一致」的规则。

Android 实体键盘的可打印键走 `character()`，引擎随契约出「」，但这条路线宿主本来就不做成对补全，没有跨过的问题；没有在设备上验证。

## Alternatives considered

- **保留第 3 版的交替表，撤掉 macOS 的宿主拦截**：最强的理由是交替表能同时打出『』。不用：macOS 系统布局与 #8998、#10047 的报告者都要固定「/」，『』用 `"` 在直角引号样式下就能打；而且 macOS 的固定语义已经发布，撤掉是回退。
- **在契约里另开一段 `cornerBrackets` 放这两键**：最强的理由是让「花括号键」在契约里有名字，宿主可以按段识别。不用：它们就是无状态一对一映射，另开一段要给生成器、引擎和五个宿主各加一种读法，换来的只是名字。
- **Windows 让 `{` 出的「空格转回 `{`（像 macOS 那样）**：最强的理由是给写 LaTeX、代码的用户一个不关中文标点就能拿回花括号的办法。不用：空格转换名单不含花括号是已有决定，Linux 也不认；成对补全默认打开时补出的一对本来就不武装空格转换，这条退路只在关掉成对补全时才存在，收益小于三个宿主再分叉一次。
- **移动端给跨过表加 `}` → 」，让 iOS 的 `{` `}` 照旧走引擎**：最强的理由是改动只在跨过表一行，`{` 键也能直接打出「」。不用：键面画的是 `{`，打出来却是「，违背中文层键面与写出的字一致的规则；Android、HarmonyOS 的同一个键写的是 `{`，三端会分叉。想打「」用 `'` 键在直角引号样式下就能打。
- **Linux 直接删掉 `{}` 补全和 `}` 跨过花括号**：最强的理由是代码更少。不用：专用英文等由引擎写 ASCII 标点的路径仍会把 `{` 交回宿主，删掉会让那里的花括号不再成对，是一次没人要求的行为变化。

## Consequences

- 中文标点下 `{` `}` 在 macOS、Windows、Linux 上一致：固定「/」，成对补全打开时 `{` 出「」并把光标放在中间，`}` 跨过补出的」；全角模式下也是「」。
- 代价：Windows 和 Linux 上在中文标点下打花括号的用户（LaTeX、代码）要切英文标点或英文模式，没有开关；Windows 全角模式的 `{` 不再出 ｛｝。日文方案在 Windows、Linux 上的 `{` `}` 也随契约出「」；macOS 的日文在成对补全打开时仍走宿主的花括号补全，没有跟着改。
- Windows 组字中按 `{` `}` 的路径没改（它们不在 `CommitWithHighlightedCandPunc`，Server 的 `candidate_punctuation` 也不认它们），只是出的字换成「」。
- 验证：`cargo test -p msime-engine` 与 `cargo test -p msime-host-api --lib punctuation` 通过，其中 `punctuation::tests::shifted_brackets_give_corner_quotes` 按新语义重写（旧实现下 `{}{{}` 出「『」「』，断言必然失败）；`python3 shared/contracts/punctuation/generate.py --check` 通过；`punctuation_contract.cpp`、Linux 的 `smart_punctuation_space.cpp` 与 `paired_punctuation.cpp`、Windows TSF 的 `punctuation_key_policy.cpp` 与 `paired_punctuation_wiring.cpp` 用主机编译器单独编译运行通过。iOS 的 `SymbolLayerLayout.input` 用 `swiftc` 单独编译 `KeyboardLayoutTables.swift` 加一个断言脚本验证 `{` `}` 在中文层按字面插入；`NineKeyKeyboardTests` 的同一组断言已改，留给 CI 的 iOS 测试跑。Windows、Linux 宿主没有整体编译，也没有在记事本、IBus、Fcitx5 里实际打字验收，留给 PR 的 CI 与真机。
