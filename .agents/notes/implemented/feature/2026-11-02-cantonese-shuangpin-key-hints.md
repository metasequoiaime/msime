# Agent Note: 粤语双拼键位提示

Status: implemented

## Problem

#10438 希望粤语双拼也像普通话双拼一样，在字母键底部显示这个键代表的粤拼声母和韵母，方便初学者。触屏的[双拼键位提示](2026-10-09-touch-shuangpin-key-hints.md)由引擎按普通话双拼方案表生成（`shuangpin::hints::shuangpin_key_hints`），只认 `ShuangpinProfileKind` 的方案名；iOS 的 `shuangpinKeyHints()` 只在会话方案是双拼（`scheme == 1`）时去取。粤语双拼跑在粤拼方案下（`scheme == 5`，`force_cantonese_shuangpin`），键位只写在 `cantonese::shuangpin::encode` 的一段 `match` 里，没有可以拿来生成提示的表，所以键面一直是空的。

## Decision

- `cantonese/shuangpin.rs` 把 `encode` 的声母分支和韵母 `match` 抽成三张常量表：`MOVED_INITIALS`（`gw`→I、`kw`→U、`ng`→V、`j`→Y，按最长匹配排列）、`PLAIN_INITIALS`（在自己字母键上的单字母声母）和 `FINALS`（52 个韵母各自的键）。`encode` 改为查表，行为不变；测试里原样留着抽表前的 `match` 写法，对所有声母和韵母的组合以及一批不是韵母的尾巴逐个比对。
- 同一文件新增 `key_hints()`，从这两张表生成提示，格式和普通话双拼完全相同：按 `QWERTYUIOPASDFGHJKLZXCVBNM;` 排序，`"声母 / 韵母"`，同一边的单位按字节序用空格连接。声母只写不在自己字母键上的 `gw`、`kw`、`ng`、`j`，和普通话只写 `zh`、`ch`、`sh` 一样；零声母键 `o` 以及 `m`、`ng` 两个鼻音音节的码 `om`、`ov` 不写。每个键列出它上面的全部韵母，包括 `-p`、`-t`、`-k` 入声韵，例如 P 是 `eon un ung ut`、S 是 `ok on ong ot`、V 是 `ng / ui yu`、`;` 是 `ik ing`，最长 13 个字符。
- 对外复用已有的 C ABI `msime_client_shuangpin_key_hints`，加一个保留名 `cantonese`（`host::CANTONESE_SHUANGPIN_HINTS`）：`host::shuangpin_key_hints("cantonese")` 返回粤语双拼的键面，其余名字照旧。头文件、JNI 和 ArkTS 绑定都不用改。`cantonese` 不是 `ShuangpinProfileKind` 的名字，所以 `shuangpin_zero_initials`、`encode_shuangpin`、`shuangpin_uses_semicolon` 和桌面的双拼键位面板都不认它，不会撞名。
- iOS：桥接层加 `cantoneseShuangpinKeyHints()`，只在会话方案是粤拼（`scheme == 5`）时用 `cantonese` 去取，否则给空表，和 `shuangpinKeyHints()` 只给跑着双拼的会话一样，免得缺了 `msime-cantonese.db`、运行时留在上一个中文方案时把键标错。`updateSchemeButton` 在 `inputScheme == .cantoneseShuangpin` 时取这张表，否则照旧。普通粤拼和粤语双拼是同一个引擎方案，只有键盘知道选的是哪一个；普通粤拼完整拼写，不画提示。
- 仍由「双拼键位提示」（`touch_shuangpin_key_hints`，缺省关）这一个开关控制，不新增开关，默认值不变。iOS 设置页这一行在本版本有双拼或粤语双拼时列出，说明里加上粤语双拼。
- Android 和 HarmonyOS 这次不动：两端还没有粤语双拼方案（#10523 正在移植）。它们接入时把 `cantonese` 传给已有的 `NativeClient.shuangpinKeyHints` 或 HarmonyOS 的同一入口即可，提示文字已经在引擎里。

## Alternatives considered

- **新开一个 C ABI，比如 `msime_client_cantonese_shuangpin_key_hints`**：语义最清楚，不用在方案名空间里留保留名。但三端都要各加一份绑定（iOS `@_silgen_name`、Android JNI、HarmonyOS NAPI），返回的形状和现有函数完全一样，多出来的只有绑定代码。保留名不会和普通话方案冲突，因为方案名集合由 `ShuangpinProfileKind::from_name` 固定。
- **在 iOS 宿主里写一张粤语双拼提示表**：改动最小。不采用，这正是[双拼键位提示](2026-10-09-touch-shuangpin-key-hints.md)要避免的事：宿主自己的那份表曾经丢了小鹤 K 上的 `uai`。提示和 `encode` 读同一张表，`key_hints_agree_with_encode` 还会反过来用 `encode` 核对每个提示单位。
- **把入声韵缩写成 `aam/p` 这类形式让键面更短**：P 键会从 `eon un ung ut` 缩到四五个字符。但初学者不一定知道 `-m`、`-p` 成对，缩写要先懂规则才看得懂，而提示本来是给不熟悉键位的人看的。所以照样列全，交给 iOS 提示行已有的自动缩小（9pt，最小缩放 0.6）。
- **把和键名相同的声母（`b`、`p`、`m` 等）也写上**：信息更完整，但每个键都多一个和键面重复的字母，普通话提示也不这样写。

## Consequences

- **收益**：粤语双拼初学者打开「双拼键位提示」就能看到每个键的韵母和挪了位置的声母；键位改动只改一张表，提示和编码一起变。Android、HarmonyOS 接入粤语双拼时不用再做提示。
- **代价**：`encode` 从 `match` 变成线性查表，`decode`、`initial_prefixes` 和 `key_spans` 每次扫库存时多一点比较，量级是库存音节数乘以五十多个韵母，仍在微秒级；`msime_client_shuangpin_key_hints` 的方案名空间多了一个保留名，以后新增普通话方案不能叫 `cantonese`。
- **待确认**：iPhone 窄键宽下 P、S、R、G 这几个较长的提示缩小后是否还看得清，没有在模拟器或真机上截过图。

## Verification

- `rbuild cargo test -p msime-engine --lib -- cantonese::shuangpin shuangpin_key_hints`：13 项通过，含抽表前后 `encode` 逐个比对、每个键的提示文本、提示与 `encode` 互相核对、保留名不被普通话方案函数认出。
- `rbuild cargo test -p msime-engine --lib -- cantonese`：66 项通过。
- `rbuild cargo test -p msime-host-api --lib -- shuangpin_key_hint`：FFI 传 `cantonese` 返回 27 个键，`cantonese_shuangpin` 等名字仍为空。
- `rbuild cargo clippy -p msime-engine -p msime-host-api --all-targets -- -D warnings`、`rbuild cargo fmt --all -- --check`：通过。
- iOS：在编译机上暂存了锁定的 `msime-cantonese.db`，iOS 27.0 模拟器跑 `xcodebuild -scheme MSIMEClientTests -only-testing:MSIMEKeyboardTests/LanguageSchemesTests` 加两条 `NineKeyKeyboardTests`，35 项通过、没有跳过。其中 `LanguageSchemesTests.testCantoneseShuangpinLabelsItsKeysWhenTheHintSwitchIsOn` 核对粤语双拼开关打开时 I、V、P 的提示和读屏，开关关闭和普通粤拼都不画（测试宿主没有暂存词库时跳过）；`NineKeyKeyboardTests.testShuangpinKeyHintsComeFromTheProfileTheSessionRuns` 核对全拼和小鹤会话拿不到粤语双拼键面。
