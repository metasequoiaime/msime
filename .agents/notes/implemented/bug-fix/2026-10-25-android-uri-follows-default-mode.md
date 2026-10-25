# Agent Note: Android 网址框按默认中英文进框

Status: implemented

## Problem

#9725：默认用九宫格的用户，点 Firefox Beta 的地址栏、MT 管理器的应用搜索栏，弹出的都是 26 键英文，每次要手动切回九宫格；Via、X 浏览器的地址栏是九宫格。

`EditorPolicy.prefersLatin` 把 `TYPE_TEXT_VARIATION_URI` 和邮箱、密码归为一类，`KeyboardInputContext.englishOverride` 在每个新文档上把模式临时覆盖成英文，而 `displayedTouchLayout` 在英文模式下一律画 26 键，于是九宫格、14 键、18 键用户进网址框都看到 26 键英文。Android 没有 iOS 的 `.webSearch`：iOS 只让纯网址框（`.URL`）进英文，Safari 地址栏这种「网址兼搜索」框按默认模式进框（`KeyboardInputContext.prefersLatin`，测试断言 `.webSearch` 不覆盖）；Android 浏览器的地址栏只能声明成 URI（Chrome 是 `0x80011`），所以被当成纯网址框。浏览器地址栏同时是搜索框，中文用户在这里多半搜中文。[无建议输入框不再强制进英文](2026-10-08-android-no-suggestions-default-mode.md)当时已把「URI 字段是否也该按默认模式进框」列为另一个问题，本篇处理它。

## Decision

- `prefersLatin` 只认两种邮箱 variation 和三种密码 variation，不再认 URI。网址框和只带无建议标志的框一样，按 `default_ime_mode` 和 `ime_mode_scope` 的记忆进框，九宫格、14 键、18 键在中文模式下保持自己的布局。
- 网址框其余的照顾不变：新增 `EditorPolicy.uri`，`englishSuggestionsActive` 另外排除它，用户在网址框里切到英文后仍不显示英文建议、空格和标点也不自动采用纠错；`capitalizationMode` 对 URI 本来就单独返回 `NONE`，不自动大写。邮箱后缀照常给。
- 中文模式下输入网址由引擎的网址模式接管（`crates/engine/src/local/url.rs`：全拼、双拼、五笔组字里键入 `www.`、`http:` 等之后按 ASCII 原样输入），所以纯网址框留在中文也能打网址；九宫格打英文网址仍要先切到英文。
- 设备验收里 14 键五笔、14 键双拼和 18 键全拼的「password and URL fallback」改为：密码框仍回落 26 键，网址框保留各自的布局；英文纠错的编辑器门禁仍断言网址框里不纠错。
- HarmonyOS 不在本次范围：它的 `EditorPolicy.prefersLatin` 和 `prefersFullFace` 同样包含 URI，理由是九宫格靠词典消歧、网址里没法逐字输入；鸿蒙浏览器地址栏声明的是哪种类型没有核实过，改不改另行决定。iOS 已经区分 `.URL` 与 `.webSearch`，不需要改。

## Alternatives considered

- **按 issue 的提议加一个「锁定键盘布局」开关**：完全不动现有规则，只给想要九宫格的人一个出口，默认关闭，对其他用户零影响。没有采用：根因是网址框压过了用户已有的「默认中英文」，再加一个开关等于让用户为同一件事设置两次；开关还要新增共享偏好字段和三个平台的设置项，而打开它的人在纯网址框里照样会遇到九宫格打不出网址的问题。
- **保留网址框进英文，但不把九宫格换成 26 键**：仍然尊重「网址框要英文」的字段要求，九宫格用户也不必看到另一套键盘。没有采用：英文九宫格靠词典消歧，网址、域名这类任意字符串大多打不出来，鸿蒙 `prefersFullFace` 的注释就是这个理由；这样改比现在更难用。
- **只对浏览器地址栏放行（URI 加 `IME_ACTION_GO`/`IME_ACTION_SEARCH`，或按包名）**：纯网址框保持进英文，只放过「网址兼搜索」框。没有采用：Android 上没有可靠的信号区分两者，回车动作和包名都是应用各自的写法，MT 管理器的搜索栏就不是浏览器；猜错的一边仍是现在的问题，而且这份名单要一直维护。

## Consequences

- **收益**：浏览器地址栏和其他声明成网址的搜索框按用户的默认中英文和按应用记忆进框，九宫格、14 键、18 键用户不再每次被切到 26 键英文。
- **代价**：真正只收网址的输入框（设置页里填服务地址这类）现在也按默认模式进框，默认中文的用户要么用中文模式下的网址模式输入，要么手动切英文；切过之后按 `ime_mode_scope` 记住，同一应用的其他输入框也跟着用这个模式。
- **未覆盖**：MT 管理器的搜索栏没有核实是不是 URI 字段；如果它是因为按应用记住了英文才弹 26 键，这次改动管不到，要在 issue 里请报告者用修复后的版本复验。鸿蒙的同类行为见上文，未改。

## Verification

- `platforms/android/check-host.sh`：276 项 Android JVM 冒烟通过，`EditorSmoke` 断言 URI 与 Chrome 地址栏的 `0x80011` 不再 `prefersLatin`、`EditorPolicy.uri` 识别两者且不误认普通文本和数字框，邮箱与密码仍 `prefersLatin`；245 个设备套件源文件编译通过。旧实现下 `!prefersLatin(URI)` 一条失败。
