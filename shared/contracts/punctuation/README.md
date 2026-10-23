# Punctuation contract

`policy.json` is the authoritative list of ASCII punctuation keys handled by Chinese punctuation mode. `generate.py` emits `policy.h`; consumers use the generated header or, in Rust, `crates/engine/src/punctuation.rs`, whose `matches_the_shared_contract` test compares its table with both files. Nobody copies the mapping into a platform-specific table.

The contract holds stateless simple mappings plus the opening/closing pairs for alternating quotes and nested book-title marks. The quote toggles and nesting depth are session state, so each input session owns that state; the contract provides no shared mutable state.

## 顿号键（第 4 版）

`enumerationComma` 列出中文标点下哪些 ASCII 键可以出顿号「、」：`\` 和 `/`。和 `simple` 不同，这一段是「可被偏好关掉的键」——共享偏好 `enumeration_comma_backslash`（默认开）与 `enumeration_comma_slash`（默认关）各自控制一个键，没打开的键不是中文标点，按普通字符交回宿主；两个键至少保留一个（都关时按「`\` 开」读）。`\` 同时留在 `simple` 里：它从第 1 版起就是无条件映射，`is_supported` 与 `simple` 的读者看到的行为不变；`/` 只在这一段，不打开偏好时 `is_supported('/')` 为假。`enumeration_comma_output` 给出某个键启用时出的字，宿主是否启用要读 View 的 `punctuation_marks`，不自己解析偏好。

## `{` `}` 与样式（第 3 版）

第 3 版在 `alternating` 里加了 `{` → 「」和 `}` → 『』：`Shift+[` `Shift+]` 直接出直角引号，与 `'` `"` 的弯引号（或 corner 样式下的同一组直角引号）并存，两组交替状态按键独立保存。`styles.cornerQuotes` 里 `{` `}` 两行与 `alternating` 相同——这两键出的本来就是直角引号，换样式不换它们。

## 样式（第 2 版）

`styles` 是同一个键的另一组字，不是新键：`cornerQuotes` 给 `"` 和 `'` 换成『』和「」（按键对应，`'` 出「」，Shift+' 出『』），`hollowBrackets` 给 `[` `]` 换成〖〗。`generate.py` 拒绝 `styles` 里出现 `alternating` 或 `simple` 之外的键。用哪一组由偏好 `quote_style`、`bracket_style` 决定：`crates/client-core/src/punctuation.rs` 的 `punctuation_marks` 把偏好（含按繁体输出开关解析的 `corner_when_traditional`）算成两种确定的值，host-api 交给引擎，并在 View 的 `punctuation_marks` 里报告给宿主。交替和嵌套规则对两组字一样，换样式不重置交替。

宿主在文档里做的几步——成对补全、跨过后半个、把后引号改回前引号、空格转换、英文模式下的中文标点、触屏键面——各自有一张本地表，加字或改字时这些表都要跟着改，否则会出现错位的一对（补出了「却不认识」，于是打出「内容」」）。补全和改回前引号不能只看上屏的字：注音的大千键位 `[` `]` `{` `}` 不经过这份契约，自己出「」『』（引擎的 `SHIFT_PUNCTUATION`），把它们当成直角引号样式下 `'` `"` 出的字，`]` 会被改回「再补一对，`[` 补出的」也跨不过去。macOS、HarmonyOS 和 Linux 按按下的键判断，Windows TIP 的字本来就由按键查本地表得出；iOS 和 Android 的注音标点在补全之前就提前返回，不走这条路。目前的位置：

- `shared/input/EnglishModeOutput.h`（macOS、IBus、Fcitx5 英文模式下的中文标点，读 View 的样式）
- Windows TIP：`platforms/windows/tsf/Global/Globals.cpp` 的 `PunctuationTable`、`CompositionProcessorEngine.cpp` 的 `GetPunctuation` 与 `SetupPunctuationPair`（查表后经 `Global::StyledPunctuation` 换字）、`Global/PairedPunctuationHostPolicy.h`（补全、跨过、`ReopenedPairedQuote`）、`Composition/Composition.cpp` 的 `SmartPunctuationAsciiFor`；样式经 Worker 帧 `PunctuationMarksChanged` 从 Server 推来，Server 按与 client-core 相同的规则从偏好 JSON 算（`platforms/windows/src/ipc/ReplyCodec.cpp` 的 `apply_punctuation_marks`）
- macOS：`platforms/macos/src/input/InputController.mm` 的 `MSIMEPunctuationPairs`、`MSIMEASCIIForSmartChinesePunctuation`、`MSIMEIsCornerQuoteMark`/`MSIMEKeyProducesCornerQuote` 与改回前引号的那一段，`platforms/macos/src/core/PairedPunctuation.h`
- Linux：`platforms/linux/src/candidates/PairedPunctuation.h`、`platforms/linux/src/core/SmartPunctuationSpace.h`
- iOS：`platforms/ios/KeyboardExtension/Sources/input/PairedPunctuationPolicy.swift`、`KeyboardViewController.swift` 的 `chineseSymbolFaces` / `chineseSymbolFace`（符号键面和注音符号面板）、`KeyboardLayoutTables.swift`（123 层）
- Android：`keyboard/PairedPunctuationPolicy.java`、`keyboard/ChineseSymbolFaces.java`（符号键面，字母角标经 `LetterHintTable` 也用它）、`keyboard/KeyboardLayout.java`（123 层）
- HarmonyOS：`keyboard/input/PairedPunctuationPolicy.ts`、`keyboard/input/SmartPunctuationSpacePolicy.ts`、`keyboard/input/ChineseSymbolFaces.ts`、`keyboard/input/NumberSymbolLayout.ts`（123 层）
- `crates/client-core/src/punctuation.rs` 的 `ascii_for_chinese_mark`（host-api 的空格转换）

After editing `policy.json`, run `python3 shared/contracts/punctuation/generate.py` from the repository root. `scripts/test-punctuation-contract.py` runs it with `--check` in every `scripts/run-checks.sh`, so a header that does not match the JSON fails the gate. The C++ contract test (`shared/contracts/tests/punctuation_contract.cpp`) is built by the Windows project.
