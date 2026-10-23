# Agent Note: `Shift+[` `Shift+]` 出直角引号（issue #8998）

Status: implemented

## Problem

中文标点下 `Shift+[`（`{`）原本只出 ASCII 花括号 `{}`（全角 `｛｝`），由 macOS / Linux / Windows 宿主在 `punctuation_ascii` 路由上自己补右半个，不经过引擎标点表；直角引号「」『』只能靠 `quote_style=corner` 整体替换 `'` `"` 的弯引号，两种引号互斥，不能并存。issue #8998 要求 `Shift+[` `Shift+]` 直接出直角引号，`[` `]` 仍出【】，弯引号保持不变。上游 #9062 已先合入一种实现：macOS 宿主在 `handleEvent` 里把 `{`/`}` 拦成固定的「/」（macOS 系统输入法的左右引号键位），并用 `Refs #8998` 关联——issue 建议表格写的是两组交替引号，键位意图确认前上游特意不关 issue。

## Decision

把映射落到共享标点契约与引擎，宿主层保留上游已合入的拦截，两套语义各守一边：

- 契约升到第 3 版（`shared/contracts/punctuation/policy.json` → `generate.py` → `policy.h`）：`alternating` 新增 `{` → 「/」、`}` → 『/』两条，`styles.cornerQuotes` 同步补同键同字的两行（`generate.py` 要求 cornerQuotes 键集与 alternating 完全一致）。升版本是因为这是表结构变化（新增受支持的键），不是第 2 版语义内的改字；`punctuation_contract.cpp` 与引擎的 `matches_the_shared_contract` 断言同步更新。
- 引擎 `crates/engine/src/punctuation.rs` 为 `{` `}` 各存一个交替状态，与 `'` `"` 互不共享；这两个字就是直角引号本身，不吃 `quote_style`，两种样式下相同。`set_marks` 与配对开关都不重置交替。
- macOS 宿主（`platforms/macos/src/input/InputController.mm`）保留上游的 `{`/`}` → 「/」宿主分支和 `{`→`{}`/`｛｝` 旧补全块：普通中文方案（`HostSmartPunctuation`：全拼、双拼、五笔、粤语、笔画）的这两个键在到达引擎前就被拦下，合同第 3 版的引擎映射在那里不会生效——macOS 可见行为与上游一致。真正到达引擎 `typeASCII` 的 `{`/`}` 是日文（用中文标点表但不做宿主智能标点）等未被拦截的路径，它们按契约出「/」/『/』交替。
- 配对面与「后引号改回前引号」的判定从「样式键 `'` `"` 出的字」扩成「出这个字所属组的键」（`MSIMECornerQuoteKey` 换成 `MSIMEIsCornerQuoteMark` + `MSIMEKeyProducesCornerQuote`，按契约分组：非注音方案下 `{` 算出「」的键、`}` 算出『』的键）。这不是泛化洁癖：日文的 `}` 现在经引擎出『，若仍按「只有 `"` 出『』」判，`『` 会被当成「别的键出的直角引号」而不补』，也改不回『。`}` 不算出「」的键是故意的——上游分支里 `}` 上屏的是」，把它算进「」组会让它被改回「。注音的大千键位 `{` `}` 由引擎 `SHIFT_PUNCTUATION` 固定出『』，仍不算。`quoteKey` 例外（长提交尾端的引号键仍按引号键处理）同样补 `{` `}`。
- 空格转换的 ASCII 反查在 macOS 上按实际按下的键定：`noteCommittedChinesePunctuation` 带上按键，`{` 出的「转回 `{`，不再转回样式键 `'`。
- `shared/input/EnglishModeOutput.h`（macOS、IBus、Fcitx5 英文模式下锁中文标点的手抄表）按契约补齐 `{`/`}` 两组交替，状态与弯引号分开存。合同 README 的手抄表清单同步改名。

## 其他平台

契约是全平台共用：Windows TIP 经 `policy.h`、Linux 经同一引擎拿表，所以语义上各平台都会跟着变。但 `{`/`}` 键在多数宿主上仍被引擎之前的拦截挡住，拿不到新功能：Linux IBus/Fcitx5 在 `pairedPunctuation` 里把 `{` 拦成 `Mode::Brace` + `msime_client_punctuation_ascii`，Windows TIP 在 `PairedPunctuationHostPolicy.h`/`Globals.cpp` 把 `{` 查表成 `{`/`｛`。不改不会回归——`{` 继续出 `{}`。待办：

- Linux：把 `{`/`}` 改走 `msime_client_punctuation` + 引号配对面（`normalize_punctuation_pair` 已能按提交尾认「」『』），`paired_closing_for_key('}')` 的 `}`/`｝` 跨过条目退役。
- Windows TIP：`PunctuationTable`、`GetPunctuation`、`SetupPunctuationPair`、`PairedPunctuationHostPolicy.h`、`SmartPunctuationAsciiFor` 同步。
- iOS / Android / HarmonyOS：`PairedPunctuationPolicy` 与符号键面本地表按 README 清单同步。
- `crates/client-core/src/punctuation.rs::ascii_for_chinese_mark` 与 macOS `MSIMEASCIIForSmartChinesePunctuation` 的反查是「字→样式键」，不是唯一映射（「可同时由 `'` 和 `{` 出）；macOS 侧已按按键修正，其余平台沿用样式键，属已知取舍。
- 键位语义的分裂待 issue #8998 的意图确认：上游 #9062 给 macOS 选了固定「/」（见 [标点样式笔记](2026-10-13-punctuation-mark-styles.md) 的「macOS 的独立直角引号键位」一节），契约第 3 版实现的是 issue 建议表格里的两组交替引号。若最终确认用固定「/」，把 `}` 行从 alternating 挪走即可；若确认用交替表，撤掉 macOS 的宿主拦截分支即可全平台对齐。

## Alternatives considered

- **沿用上游 #9062 的宿主侧拦截，不动契约**：最强的理由是它已经合入、零风险。不用的原因：它只在 macOS 生效，`{`/`}` 在其他平台和未被拦截的路径上仍出 ASCII 花括号；issue 的建议表格写的是两组交替引号，契约层实现才能覆盖『』，且为 Linux/Windows/移动端同步留了统一真源。
- **撤掉上游的宿主拦截，让 macOS 也走引擎交替**：本仓库「不回退已合并改动」的约束不允许，且上游明确把键位意图留给 issue 确认——在确认前撤掉语义不同（固定 vs 交替）的上游实现是抢答。
- **加一个默认关闭的独立开关**：最强的理由是 `{`→`{}` 是多年习惯，直接改键位可能影响写 LaTeX/代码片段的用户。不用的原因：成对标点开着时 `{` 已经出的是一对 `｛｝`，习惯的是「按一下出一对」而非 ASCII 花括号本身；开关还会复制 `quote_style` 的样式矩阵再乘一维。若后续有真实反对再退回此方案。

## Consequences

- macOS 可见行为不变：普通中文方案 `Shift+[` 出「、`Shift+]` 出」（上游语义），成对标点开着时自动补对并光标居中；注音 `{` `}` 仍是大千键位的『』；韩文、越南文不走中文标点表，`{` 仍是 ASCII。
- 引擎与英文模式固定表侧：`{`/`}` 到达中文标点处理即出「/」/『/」交替——日文的 `}`、英文模式锁中文标点的 `{`/`}` 立刻生效；Linux/Windows/移动端仍被宿主拦截，行为暂时分裂（见上节待办）。
- 代价：`{`/`}` 键位语义变了，没有开关；`「`/`」` 的字→键反查在非 macOS 宿主上固定答 `'`。
- 验证：`punctuation::tests::shifted_brackets_give_corner_quotes` 先失败后通过；`cargo test -p msime-engine punctuation` 全绿；`cargo fmt --all --check` 通过；`bash scripts/run-checks.sh`（含 `test-punctuation-contract`，校验 `policy.h` 与 `policy.json` 一致）通过；`smart_punctuation_space.cpp` 的英文模式断言覆盖 `english_mode_output` 的 `{`/`}`。macOS 宿主编译与 ShortcutTest 沿用上游 #9062 的覆盖，本 PR 的宿主侧改动是泛化判定与注释，不改变可达路径上的输出。
