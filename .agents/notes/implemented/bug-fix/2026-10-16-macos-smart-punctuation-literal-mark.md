# Agent Note: macOS 智能标点判为 ASCII 时由宿主写出字面标点

Status: implemented

## Problem

一位 macOS 用户在五笔下打版本号 `0.10.1`，智能标点开着，屏幕上却是 `0。10。1`。偏好已经生效（诊断日志有 `preferences_applied`），会话里的 `smart_punctuation`、`smart_punctuation_direct_digit` 也都是开的。

用真会话从 `handleEvent:client:` 一路打进去，全拼和五笔、读得回文档和读不回文档的客户端全部复现，所以这不是五笔或某个应用的问题。共享判断本身是对的：`msime_client_punctuation_with_context` 在前一个字符是数字时选 ASCII 路线（`crates/client-core/src/punctuation.rs` 的 `route`），没有组字时 `Runtime::punctuation_ascii`（`crates/input-runtime/src/runtime.rs`）不出字，回 `handled: false`，意思是「键交回宿主，字面标点由宿主出」。[大写锁定那篇](../feature/2026-10-09-caps-lock-ascii-punctuation.md)把这条约定写过一遍。

`platforms/macos/src/input/InputController.mm` 的 `handleSmartPunctuation:client:` 把这个 `handled: false` 当成「这个键不归我」返回 NO。`handleKeyEvent:client:` 于是接着往下走，到了 `typeASCII:shift:`（`msime_client_character`）这条不看前文的普通标点路线，Engine 照中文标点开关写出 `。`。这个分支从 2026-09-16 移植智能标点起就在，一直没被测到：`SmartPunctuationShadowTest` 和 `SmartPunctuationSpaceTest` 的假会话在 ASCII 路线上回 `handled: true` 并带着字面标点，和真会话的回答不一样。

## Decision

- 没有组字、Engine 回 `handled: false` 时，`handleSmartPunctuation:client:` 自己写出字面标点：在 Engine 的回答上补 `handled: YES` 和 `commit`，经 `apply:` 上屏，返回 YES。全角开着时写全角形式（`MSIMEFullWidthSmartMark`）。走 `apply:` 是为了和组字中的路线一样：影子随这次上屏记下（读不回文档的客户端靠它判断下一个标点），成对标点、输入统计照常处理。iOS（`KeyboardViewController` 里 Engine 不收时 `insertDirectText`）和鸿蒙（`KeyboardSession.ets` 在 `commit` 为空时 `insertEditorTextSync`）本来就是这样做的。
- 这样写出的 ASCII 标点只在标点锁定为「跟随」且中文标点开着时武装连按改中文的手势（`_lastSmartPunctuation`）。Engine 回 `handled: false` 也可能是因为中文标点关着或锁定为英文，那时打出来的本来就是 ASCII，没有可改回的中文标点。条件和 Linux Fcitx5 宿主的 `keptAscii` 相同。
- 连按手势本身不改：两秒内紧接着再按同一个键才算连按，中间按过别的键时 `handleKeyEvent:client:` 已经清掉 `_lastSmartPunctuation`。`0.10.1` 里的第二个 `.` 前面有 `1` 和 `0`，所以不算连按。
- 诊断日志开着时，`, . :` 每次判断记一行：

  ```
  [smart-punctuation] composing=<0|1> shadow=<类别|invalid> preceding=<来源>:<类别> decision=<结果>
  ```

  类别是 `digit`、`letter`、`ascii`、`non_ascii`、`none`，来源是 `shadow`、`document`、`candidate`、`unread`，结果是 `ascii`、`ascii_literal`、`ascii_literal_unarmed`、`engine_commit`、`engine`、`rejected`、`repeat_to_chinese`、`repeat_to_chinese_posted`、`spelling`、`off`、`scheme`、`session_error`。按的是哪个键和字符本身都不写，因为 `DiagnosticLog.h` 规定诊断日志不收按键和输入内容。只看类别和结果，已经分得清是开关没生效（`off`）、影子丢了（`shadow=invalid preceding=document:none`）、被当成拼写（`spelling`），还是判断对了但后面出了问题。

## Alternatives considered

- **`handled: false` 时把键交回应用（`handleKeyEvent:client:` 直接返回 NO）。** Linux Fcitx5 就是这样做的，应用打出的键也会作为直通键进影子。没有选，原因有三个：一是 `handleSmartPunctuation:client:` 的 BOOL 只能表达「已处理」和「继续往下」，要交回应用就得给它加第三种结果，而它的两个测试文件有十几处直接断言这个 BOOL；二是全角开着时交回应用打出的是半角，还得在宿主里另外转；三是成对标点的后半个挂在 marked text 里时，交给应用的键和 marked text 的先后顺序由应用决定，经 `apply:` 写出的就和其它上屏一样排在后半个前面。
- **改 Rust，让没有组字时的 ASCII 路线也带着字面标点回 `handled: true`。** 这样所有宿主都不用再自己写。没有选：Linux 两个宿主、iOS 和鸿蒙都按现在的约定实现了，而且 Fcitx5 依赖 `handled: false` 把键交回应用；改约定就得同时改五个宿主，出错的却只有 macOS 这一个。

## Consequences

- **收益**：数字和字母后面的 `, . :` 在 macOS 上真正按智能标点出字，各方案、各客户端都一样。读不回文档的客户端（Electron、终端）也能连续判断，因为写出的标点进了影子。下一份同类报告可以开诊断日志后直接看判断结果。
- **代价**：这种标点现在由输入法上屏，不再是应用自己处理的按键。应用收到的是一次文本插入，而不是 `.` 这个按键。iOS 和鸿蒙一直是这样，组字中的智能标点在 macOS 上也一直是这样。
- **已知上限**：大写锁定 ASCII 标点开着时（`caps_lock_ascii_punctuation`）同样会武装连按手势，和 Linux 一致；两秒内连按同一个键会把它改成中文标点。
- **其它平台**：根因只在 macOS 宿主，共享 Rust 代码没有改。Windows TSF 在本地 `ResolvePunctuation` 出字并自己上屏，Linux IBus（`ClientEngine.cpp` 自己 `commit_text`）、Fcitx5（把键交回应用）、iOS、鸿蒙都按约定处理 `handled: false`。Linux 两个宿主另有一个缺口：前一个字符只取自 surrounding text，没有 macOS、Windows 那样的影子，不提供 surrounding text 的客户端里数字后的 `.` 仍会是 `。`。这和本问题无关，要单独处理。

## Verification

- `platforms/macos/tests/input/ShortcutTest.mm` 的 `TestSmartPunctuationVersionNumberThroughRealSession` 用真会话和用户的偏好（五笔 86、五笔拼音混输、唯一自动上屏、数字键选词、智能标点和三个子开关全开）从 `handleEvent:client:` 打键，控制器不收的键由测试替应用写进文档。全拼和五笔、读得回和读不回文档的客户端都断言 `0.10.1` 得到 `0.10.1`；单独一个 `.` 仍是 `。`；中文标点关着时 `0..` 得到 `0..`，不武装；读得回文档时 `0..` 得到 `0。`，连按手势还在。它还打开诊断日志，断言三行判断记录的内容。
- 修复之前同一个测试在全拼和五笔、两种客户端上都得到 `0。10。1`，读得回文档时 `0..` 得到 `0。。`。
- `shortcut-test`（含 `--translations`）、`smart-punctuation-space-test`、`smart-punctuation-shadow-test` 全部通过。
