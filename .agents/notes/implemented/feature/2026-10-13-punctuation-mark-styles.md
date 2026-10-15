# Agent Note: 引号样式与方头括号样式（直角引号、空心方头括号）

Status: implemented

## Problem

中文标点表是写死的：`'` 只出 ‘’，`"` 只出 “”，`[` `]` 只出【】。msime-windows 的三条 issue 都要换这几个字。W#474 要把 `'` 映射成「」、`"` 映射成『』，并且成对补全照常工作，英文状态不受影响。W#54 说繁体输出时标点仍是简体规范，要求按《重訂標點符號手冊》用「」『』。W#24 在关闭前留下两项没做：【】与〖〗切换、引号样式切换，维护者把它们转到 W#474 继续跟踪。

繁体输出也帮不上忙：OpenCC 的 `s2t` 数据里，“ ‘ 只出现在注释行，没有映射行，所以打开繁体输出后引号仍是 “”。

这件事不只是引擎换一张表。成对补全、跨过后半个、把后引号改回前引号、空格转回 ASCII、英文模式下的中文标点、触屏键面，各宿主手里都有一张自己的表，每张都只认 “”‘’【】。只改引擎，宿主就会补出错位的一对，例如补全认得「，跨过却不认」，结果打成「内容」」。

## Decision

**偏好。** 共享偏好新增两个枚举键：
- `quote_style`：`curly`（缺省）、`corner`、`corner_when_traditional`；
- `bracket_style`：`solid`（缺省）、`hollow`。

两个键等于缺省时都不写进文档（`skip_serializing_if`）。不认识的取值让整份文档解析失败，与 `punctuation_lock` 相同。没有数据迁移，偏好文档仍是 `format_version 1`。

**按键对应。** `corner` 下 `'` 出「」，`"`（Shift+'）出『』，依据是三处来源一致的按键表：W#474 正文、W#54 作者附的 CSV 的「繁体中文」一节、W#24 的勾选项。设置文案直接写出键位。

**解析只有一处。** `crates/client-core/src/punctuation.rs` 的 `punctuation_marks(&Preferences)` 把偏好算成两种确定的值：
- `quotes`：`Curly` 或 `Corner`。`corner_when_traditional` 在这里按 `traditional_chinese_output` 解析掉。
- `brackets`：`Solid` 或 `Hollow`。

引擎、View 和 Windows Server 都只认解析后的值。Windows Server 读的是偏好 JSON，不经过 Rust 结构，所以在 `ReplyCodec.cpp` 的 `apply_punctuation_marks` 里照抄同一条规则，并有测试。

**契约第 2 版。** `shared/contracts/punctuation/policy.json` 升到 `contractVersion: 2`，新增 `styles` 段：
- `cornerQuotes`：`"` → 『』，`'` → 「」；
- `hollowBrackets`：`[` → 〖，`]` → 〗。

样式是同一个键的另一组字，不是新键：`generate.py` 会拒绝 `styles` 里出现 `alternating` 或 `simple` 之外的键。`policy.h` 随之生成 `corner_quotes`、`hollow_brackets` 两个数组和对应的查找函数。

**引擎。**
- `PunctuationPolicy` 带一个 `PunctuationMarks`，`translate` 按它选字。
- 交替状态按键保存，与选哪组字无关，换样式不重置交替。
- `EngineOptions` 平铺成 `corner_quotes`、`hollow_brackets` 两个布尔，`SessionOptions` 带 `punctuation_marks`。

**host-api 与 View。**
- host-api 在建会话和空闲重建时填这两个选项。繁体开关、样式偏好本来就是偏好，改了都会走空闲重建。
- 同时调 `Runtime::set_punctuation_marks`，View 里多出 `punctuation_marks: {"quotes": "curly"|"corner", "brackets": "solid"|"hollow"}`。
- 宿主画键面、做补全、输出英文模式下的中文标点时都读这个字段，不自己解析偏好。

人民币符号的宽度后处理见[人民币标点跟随全半角状态](../bug-fix/2026-10-15-currency-punctuation-character-width.md)；这里的固定表和样式选择仍成立。

**宿主。** 各宿主的表只做加法。
- macOS：`MSIMEPunctuationPairs` 加『』〖〗；改回前引号认」』，但只在按的是 `'` `"` 时改（`MSIMECornerQuoteKey`）；按键出的「『只在同样的键上补全；空格转换反查加这几个字；`PairedPunctuation.h` 的 `paired_closing_for_key` 按 tracker 栈顶在两组后半个里挑。
- Linux（IBus 与 Fcitx5）：`normalize_punctuation_pair` 按文本末尾实际是哪一组就补哪一组；跨过同 macOS；`SmartPunctuationSpace.h` 的反查加这几个字。
- 三个桌面宿主的英文模式中文标点经 `EnglishModeOutput.h`，由新的 `PunctuationMarks` 参数决定出哪一组。
- Windows TIP：`GetPunctuation` 查表后经 `Global::StyledPunctuation` 换字，交替与嵌套仍由原表维护。样式由 Server 经新的 Worker 帧 `PunctuationMarksChanged`（31，载荷 `q<0|1>b<0|1>`）推来，追加在所有旧帧之后。成对表、跨过、改回前引号（`ReopenedPairedQuote`）和 `SmartPunctuationAsciiFor` 都认新字。
- iOS、Android、HarmonyOS 的键面按 View 的样式画：
  - `'` 画「，`"` 画『，`[` `]` 画〖〗；
  - 中文 123 层原样插入的 “” 两个键在直角样式下换成「」，这两个键代表首选引号，所以不按 ASCII 键位对应；
  - 成对补全、改回前引号和跨过都认新字。鸿蒙的 `PairedPunctuationPolicy.completion` 带上按下的键，只补这个键自己的前半个。

**设置入口。** 两项都紧跟在「中文标点」之后：
- 共享 React 设置页：桌面放在「标点」组，鸿蒙手机收在「更多选项」里；
- Windows 原生设置页：两个 `select_row`；
- iOS：标点设置页页首的两个 Picker；
- Android：表达页标点卡片里的两行 `nav` 加 `OptionSheet`。

**账号同步。**
- 客户端：`input.quote_style`、`input.bracket_style` 由 `settings_sync.rs` 和鸿蒙的 `AccountPreferencePlan.ts` 导出与应用，不认识的取值只跳过该键。
- iOS 不同步任何标点键，这次也不加。
- 服务端：字段表在 msime-cloud 的 `internal/account/preferences_fields.json`，需要那边单独加两个 `"type": "string"` 字段。没加之前，客户端上传前会把这两个键过滤掉，不报错，只是不同步。

**不受影响的部分。**
- 注音的大千键位自己占着 `[` `]` `{` `}`，出「」『』，所以 `bracket_style` 对注音的这几个键不起作用。这几个字和直角引号样式下 `'` `"` 出的字一模一样，宿主只看上屏的字就会认错：`]` 出的」被当成要改回的后引号，结果再开一对；`[` 出的「被补上」，而宿主只在 `'` 上跨过」，再按 `]` 就打成「内容」」。所以 macOS 改回前引号和补全都看按下的键，鸿蒙的补全也看按下的键；iOS、Android 的注音标点在补全之前就提前返回；Linux 本来就按键选补全方式。这也让 macOS 上注音 `[` 不再自动补」，与其他宿主一致。
- 韩文、越南文、藏文写 ASCII，不受影响。
- 词库、快捷短语、剪贴板、语音、AI 和翻译上屏的文字不改写。

## 手抄表清单

所有手抄表的位置都写在 `shared/contracts/punctuation/README.md`。以后再给某个标点键加一组字，按那份清单逐个宿主改，并给每个宿主加一条完整序列的测试：开一对、打内容、按同一键跨过。

## 相关笔记

- [共享标点偏好的叶子类型](../simplification/2026-10-15-punctuation-preferences.md)：部分重叠，记录 React 偏好类型的归属与兼容接口；本篇继续持有样式行为和存储约定。

- [Android 成对标点自动补全](2026-10-08-android-paired-punctuation.md)：部分重叠。那篇定下 Android 补全、跨过和改回前引号的做法，本篇只给它的几张表加了「」『』〖〗。
- [Android 字母角标的中文标点](2026-10-12-android-letter-hint-chinese-punctuation.md)：部分重叠。角标经 `ChineseSymbolFaces` 取键面，本篇让它按样式取。
- [二三候选键](2026-10-09-second-third-candidate-keys.md)：无关，只是共用 Worker 帧的编号序列。那篇的帧 30 不再是 `MaxKnown`，31 是本篇的帧，那篇写的事实仍然成立。
- [大写锁定时使用英文标点](2026-10-09-caps-lock-ascii-punctuation.md)：无关。它只决定走不走 ASCII 路线，不涉及中文标点出哪个字。

## Alternatives considered

- **按语义对应：`'` 和 `"` 都按「首选引号」解释，Shift+' 出「」。** 最强的理由是 W#54 正文写的「以「」替换“”」：“” 是首选引号，「」也是首选引号。没采用，原因有三：
  - 三份具体的按键表都写 `'` 出「」，其中包括 W#54 作者自己后来附的 CSV；
  - 按键对应让不按 Shift 的键出更常用的「」；
  - 这个选择只记在契约 `styles.cornerQuotes` 的两行里，维护者要改成语义对应，只需对调两行，代码不用动。
- **任意逐键映射表（W#474 标题里的「自定义标点映射」）。** 最强的理由是一劳永逸，以后的日文、全角括号等需求都不用再开偏好。没采用：
  - 五条 issue 里所有具体的例子都落在引号和方头括号上；
  - 逐键表要求六个宿主的补全、跨过、空格转换、英文模式和触屏键面都改成运行时数据，Windows TIP 还要让整张表过 IPC；
  - 错位一对的风险会乘上几十个键；
  - 本篇「契约加 View 报告生效样式」的结构，以后可以扩成表驱动。
- **`quote_style` 只做一个布尔。** 最强的理由是简单，已经满足 W#474。没采用：满足不了 W#54「繁体输出时用台湾标点」的核心诉求；而把「繁体自动直角」做成默认，又会改变所有现在开着繁体输出的用户。三值是唯一既满足 W#54、又不动老用户的写法。
- **各宿主自己读偏好、自己解析。** 最强的理由是不用改 View，宿主拿偏好本来就方便。没采用：`corner_when_traditional` 要结合繁体开关解析，抄成五份就会有五份漂移。View 只解析一次，而且宿主本来就在每次转换后读 View，`character_width`、`touch_keyboard_layout` 都是这样拿到的。唯一自己解析的是 Windows Server，因为 TIP 的配置帧一向从偏好 JSON 算。
- **宿主每次按键都通过 FFI 问 host-api 要后半个。** 最强的理由是彻底消除手抄表。没采用：补全、跨过、改回前引号发生在宿主的文档操作里，改成每次按键跨一次 FFI 是重构，不是这个功能；而且 Windows TIP 在另一个进程里，没有 host-api 会话可问。本篇只做加法，并把手抄表的位置写进契约 README。

## Consequences

**收益。**
- 用户可以选 「」『』 和 〖〗，并选择是否跟随繁体输出。
- 缺省值下行为与之前逐字相同：引擎 golden 原样通过，契约测试证明缺省表与第 1 版逐键相同。
- 「改样式不重置交替」「组字中改偏好，等这次组字结束才生效」都有测试钉住。

**代价与已知上限。**
- **降级读不了文档。** 选过非默认值的用户回退到旧版本时，旧版本会拒绝整份偏好文档（`deny_unknown_fields`）。这与 `chinese_english_spacing`、`second_third_candidate` 等新偏好是同样的代价。
- **新 Server 配旧 TIP。** 旧 TIP 丢掉帧 31，空闲时仍出弯引号，而经 Server 结束的组字带出的标点已是直角。安装包同时替换两者，只有开发时手动换单个 DLL 才会碰到。
- **Linux 没有偏好目录时。** 繁体开关只是内存覆盖（`traditional_output_override`），引擎看不到，所以 `corner_when_traditional` 不随它变。这只在开发和测试环境出现。
- **服务端字段表没加时同步静默不生效。** msime-cloud 加字段之前，这两个键不会同步。
- **网页引擎（`crates/engine-wasm`）没有标点设置界面**，保持缺省弯引号。
- **macOS 进程内设置窗口没有这两项。** 设置应用找不到或启动失败时退回的进程内窗口（`desktop_route_*`），「标点与字符」卡片不列引号样式和方头括号样式，与 `caps_lock_ascii_punctuation`、`second_third_candidate`、`chinese_english_spacing` 一样只在设置应用里改。
- **重访信号。** 出现第三组需要切换的字（例如日文方案的 `,` → 、），就该重新评估逐键映射表这条路。W#874 已经把日文标点作为单独的事来做。

## Verification

- 引擎与契约：
  - `rbuild cargo test -p msime-engine --locked`：单测和 `golden` 都通过；
  - `punctuation::tests` 的 `matches_the_shared_contract` 比对 `styles` 段；
  - `timeout 60 python3 shared/contracts/punctuation/generate.py --check`。
- client-core、input-runtime、host-api：`rbuild cargo test -p msime-client-core -p msime-input-runtime -p msime-host-api --locked`，覆盖：
  - 解析函数的四种组合；
  - 默认值不写回文档；
  - 账号同步往返与跳过未知值；
  - View 报告样式；
  - 空闲重建时机；
  - 固定中文标点下的英文模式；
  - 空格转换。
- 各宿主测试：
  - Linux：`rbuild bash platforms/linux/build-container.sh` 的 ctest，用例是 `linux-paired-punctuation` 和 `linux-smart-punctuation-space`；
  - macOS：ctest 的 `paired-punctuation`、`smart-punctuation-space`，以及拉回本机运行的 `shortcut-test`；
  - iOS：模拟器上的 `PairedPunctuationTests`、`PunctuationSettingsTests`、`NineKeyKeyboardTests`；
  - Android：`platforms/android/check-host.sh` 的 JVM 冒烟；
  - HarmonyOS：`platforms/harmony/tests/run.sh`；
  - 共享设置页：`apps/desktop/tests/settings/punctuation-section.test.tsx`。
