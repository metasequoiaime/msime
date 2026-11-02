# Agent Note: 中英文之间加空格

Status: implemented

## Problem

#7172（macOS）：中文文档里每次都要手动在中文和英文、数字之间补空格。报告者引用的是 rime-ice 的 `lua/cn_en_spacer.lua`：一个候选过滤器，只处理「同一个候选里既有汉字又有字母或数字」的情况（`VIP中P` 变成 `VIP 中 P`），不管两次上屏之间的空格。

产品里原来没有这个功能。唯一相关的是脚本插件模板 `docs/script-plugin-template/src/lib.rs` 的 pangu-spacing，它挂在 `before_commit` 上，而宿主当前不投递这个钩子、改写一律 `DENIED`，脚本又只在桌面开发构建里运行（见[脚本插件的作者工具](2026-10-12-script-plugin-author-tools.md)和[输入法进程里的脚本插件](../architecture/2026-10-12-script-plugins-in-the-input-method-process.md)），用户拿不到。

维护者的决定：只处理单次上屏文本内部（`cn_en_spacer` 的语义），数字算英文一侧；快捷短语、云候选、AI 候选这类现成上屏内容也处理；默认关；只改上屏文本，不改候选窗显示；排在全角转换之后；做成共享偏好和共享 host-api 后处理，六个平台的设置面都有开关；跨上屏的空格不做。

跨次边界的独立能力见[macOS 跨次上屏的中英文空格](2026-10-24-cross-commit-spacing.md)。本篇继续持有单次文本内部的规则及原开关，两者独立。

## Decision

**共享偏好 `Preferences::chinese_english_spacing`**（`crates/client-core/src/preferences.rs`），默认 `false`，`skip_serializing_if = "is_false"`：关着时不写进文档。

**变换在 host-api 的 `HostSession::complete_transition` 里，紧跟全角转换。** 模块 `crates/host-api/src/chinese_english_spacing.rs` 有两个函数：

- `applies(commit_context)`：只看上屏前记下的 `commit_context`，为 `None` 时不处理。方案要满足 `SchemeType::is_chinese()`（全拼、双拼、五笔、粤拼、注音、笔画），`local_mode` 只放行 `none`、`quick_phrase`、`super_jianpin`。
- `space(text)`：在汉字与 ASCII `[A-Za-z0-9]` 相邻处各插一个 U+0020，没有这样的相邻处时返回 `None`，原文不动、不分配。已经隔着空格的不再插，首尾不加。

开关读最新请求的偏好（`requested`，没有时取 `applied`），和 `wubi_auto_commit_unique` 一样：这是上屏后处理，不是 Engine 状态，不必等组字结束、Engine 重建后才生效。

`complete_transition` 是派发（`dispatch_call`）和失焦上屏（`ffi/session.rs`）共同经过的地方，所以六个宿主不写一行转换代码就都有这个行为。

### 顺序

全角转换 → 中英文之间加空格 → `before_commit` 发出点 → 宿主拿到上屏文本 → 宿主的简繁转换（`msime_client_simplified_to_traditional`，看 `commit_context.script_conversion`）。

- 必须在全角之后：先加的半角空格会被全角转换变成 U+3000。全角打开时字母已经是全角，`space` 本来就不会触发。
- 简繁转换在宿主里、在这之后：OpenCC 只换汉字，不动空格。
- 生命周期的 `before_commit` 和以后第 4 阶段的脚本改写看到的是加过空格的文本。pangu 模板对已经隔着空格的位置不会再插。

### 放行的本地模式

单次上屏里同时有汉字和 ASCII 字母数字的来源很少：拼音个人词必须全是汉字（`host-api/src/dictionary.rs`），英文词库只收纯 ASCII（`dict-builder/src/english.rs`），拼音词库源里没有这样的词条。实际会触发的是：

- `none`：云候选、AI 候选，五笔顶字带出的字面字母尾巴（`input-runtime/src/runtime.rs`），暂存的部分选词加 Shift 原样上屏的字母；
- `quick_phrase`：K 模式快捷短语，它的 `local_mode` 是 `quick_phrase` 而不是 `none`，不显式放行就恰好把最主要的来源排除在外；
- `super_jianpin`：J 模式选出的中文词。

网址、日期时间、Unicode、emoji、颜文字、`V` 计算、`/` 指令、`@` 提及、临时英文和临时日文的上屏是用户要原样得到的内容，一律不动。

### 汉字的判定

复用 `msime_client_services::dictionary::is_han_character`（基本区、扩展 A 到 F、兼容表意文字），host-api 的个人词校验已经在用它。Engine 的 `text::is_han` 多覆盖 U+3007「〇」和扩展 G、H，但它在私有模块里；为这个功能把它公开或再抄一份范围，都不如复用现成的公开函数。代价是「〇」和扩展 G、H 的字旁边不加空格。

### 和生命周期 `rewritable()` 的区别

`lifecycle.rs` 的 `rewritable()` 决定第三方脚本能不能改写一次上屏，这里有意用不同的规则，两者不要合并：

- `rewritable()` 只放行 `local_mode == "none"`；这里还放行 `quick_phrase` 和 `super_jianpin`，因为快捷短语正是用户要的触发源。脚本是第三方代码，范围从严；这个开关是用户自己打开的内置行为。
- `rewritable()` 排除以词定字（`SelectionHow::Edge`）。以词定字上屏的是一个汉字，旁边没有 ASCII，这里不需要特判。
- `rewritable()` 排除 `typing_statistics == false` 的引擎生成文本（计算、指令、提及）；这里按 `local_mode` 排除了同一批。
- `rewritable()` 要求字段属性已知且普通，并排除 `COMMIT_OBSERVE_ONLY`。这里不看：内置变换不涉及把输入交给第三方。`COMMIT_OBSERVE_ONLY` 下文档里落的是宿主自己写的文字（见下），加没加空格都不影响落下的内容。

### 设置入口

- 共享 React 输入页（macOS、Linux、HarmonyOS）：`packages/ui/src/settings/chinese-english-spacing-section.tsx`，放在「输出」组繁体输出之后。鸿蒙手机把所有分组折进「更多」，「输出」组以前在那里只在有全角那一行时出现，现在总是出现。
- Windows 原生设置窗口：`platforms/windows/settings/main.cpp` 的「输出」组。
- Android：`InputFeatureToggle.CHINESE_ENGLISH_SPACING`（`Group.OUTPUT`），输入页「中文」组中文字符集之后。
- iOS：原生 SwiftUI 的标点页 `PunctuationSettingsView.swift`，全角输入之后单独一节；键加进 `MetasequoiaInputSessionBridge.appEditedKeys`，活着的键盘扩展下一次出现就交给会话。

## Alternatives considered

- **照 rime-ice 做候选过滤器，候选窗显示和上屏一起改。** 最强的理由是所见即所得，和报告者引用的实现完全一致。不用：维护者要求只改上屏文本；候选文本是选择、学习和释义查询的依据，在 Engine 里加空格会让学习记录和词库里出现带空格的词；六个宿主的候选窗按文本量宽度，显示也要跟着变。
- **把 pangu-spacing 脚本插件作为内置插件发出去。** 算法现成，以后还能扩展。不用：宿主还不投递 `before_commit`，改写一律 `DENIED`，脚本只在桌面开发构建里运行，移动端恒为关；等第 4 阶段接通再发，这个开关在 release 构建和手机上都没有效果。
- **同时做跨上屏的空格（「用」→「Rust」→「写」）。** 这是 issue 标题「中英混输」最常见的场景：中英混输选出的英文候选是纯 ASCII，和前后的中文词各是一次上屏。各宿主其实都有前一个字符的来源（macOS、Windows 的智能标点 shadow，IBus、Fcitx5 的 surrounding text，移动端传给 `msime_client_punctuation_with_context` 的前文）。不做：维护者决定这次只做单次上屏内；跨上屏要一条把前文交给上屏变换的新 ABI 和六个宿主各自的接线，反方向（中文上屏之后直通输入英文）还要宿主在直通前插空格，另行设计。`space` 因此不带前一个字符的参数，接通时再加。
- **每个宿主在拿到上屏文本后自己加。** 宿主各自控制得最细。不用：六份实现会各自漂移；全角转换和网址豁免已经在共享层，加空格必须排在全角之后，放进宿主就要每个宿主都记住这个顺序。
- **ASCII 标点两侧也加空格。** issue 正文写的是「中文和英文、数字、符号」。不用：`cn_en_spacer` 不处理符号；「你好,」这类组合是智能标点有意留下的，标点前后加空格会和智能标点、成对标点的「前一个字符」判断冲突。

## Consequences

- **收益**：一个开关、一处实现，六个宿主同时有这个行为；默认关，关着时上屏一字不改、不分配，文档也不变。
- **触发面窄**：单次上屏内同时有汉字和 ASCII 字母数字的情况主要来自快捷短语、云候选和 AI 候选、五笔顶字的字母尾巴。报告者「切到英文打完再切回来要手动补空格」的主诉（跨上屏）这次没有解决。
- **不经过 `complete_transition` 的上屏不受影响**：macOS 和 Windows 的 Ctrl+Enter 译文和释义列、Windows 以词定字退回首尾字，都由宿主自己合成；移动端直接写入的常用语、剪贴板、语音结果和符号。六个平台的设置描述用同一句话写明这些不受影响：「两次上屏之间不加；网址、日期等快捷模式和全角输入不受影响，译文和释义上屏、常用语、剪贴板和语音结果也不加」，各平台没有的那几类照写，免得各平台说法分叉；以词定字的回退只写在笔记里。iOS 的设置搜索加了关键词「空格」，「更多标点设置」的副标题也提到中英文空格，因为这个开关在表达页下两层。
- **Windows 的两处差异**：
  - `HOST_HOLDS_PIECES` 下部分选词由 Server 暂存，每一段在 host-api 里是单独一次上屏，段与段的交界处不加空格；其他宿主由 runtime 的 `phrase_prefix` 攒成一次上屏，交界处会加。
  - `COMMIT_OBSERVE_ONLY` 的那些派发（韩文、注音、越南文、藏文、整句改字和没有候选时的本地回车），文字由 TIP 从自己的会话写进文档，host-api 加的空格不会落下；其中只有注音和整句改字是中文。
- **HarmonyOS**：API 12 及以上输入法扩展用独立沙箱，设置页写的 `preferences.json` 键盘看不到（[鸿蒙输入法共享沙箱](../../implemented/architecture/2026-10-09-harmony-ime-shared-sandbox.md)），共享设置页上的开关在真机上不会传到键盘，和其他设置一样等那篇提案落地。
- **跨版本**：`Preferences` 带 `deny_unknown_fields`，打开开关后键写进文档，更旧的输入法、设置应用或 `msime-mcp` 读这份文档会整份失败。默认关加 `skip_serializing_if` 把影响限制在主动打开的用户身上。
- **没有接入**：账号设置同步（`settings_sync.rs` 和鸿蒙的 `AccountPreferencePlan.ts`（#8201 以后是 client-services 的 `account::harmony_settings`），要等服务端字段表收录这个键）、MCP 的 `update_preferences` 和 `crates/engine-wasm`。
- **打字统计不受影响**：字数由宿主拿上屏文本记，`typing_statistics.rs` 跳过纯空白的字素，插入的空格不计数。

## Verification

- `client-core`：`preferences::tests::chinese_english_spacing_defaults_off_is_omitted_and_roundtrips` 覆盖默认关、关着时不写键、打开后保存读回。
- `host-api`：`chinese_english_spacing::tests::*` 覆盖插入位置、没有边界时返回 `None`、只认汉字和 ASCII 字母数字（假名、谚文、全角字母、ASCII 标点不算，扩展 B 算）、首尾不加；`tests::chinese_english_spacing_follows_the_switch_and_the_commit_mode` 用合成的 `Transition` 覆盖开关、放行和排除的本地模式、日文和韩文方案、全角在前不出 U+3000；`tests::chinese_english_spacing_reaches_a_quick_phrase_commit` 经完整派发链路把 K 模式短语「用Rust写」上屏为「用 Rust 写」，关掉后原样。`url_commits_stay_half_width_in_full_width_mode` 回归通过。
- `apps/desktop/tests/settings/chinese-english-spacing-section.test.tsx` 和 `settings.test.tsx`：开关默认关、点开写入 `chinese_english_spacing: true`，输入页标题和顺序列表包含这一行。
- Android：`InputFeatureToggleSmoke` 核对默认关和分组，`scripts/test-android-preference-keys.py` 核对共享默认值和读取方。
- 没有在任何平台的真实输入法里装机验证；iOS 的 SwiftUI 只做了语法解析，Windows 设置窗口没有在本机交叉编译。

提交时序的修正见[提交后校验方案](2026-10-24-cross-commit-spacing-post-commit.md)。

快捷短语的 `${cursor}` 展开与上屏后缀契约见[光标定位笔记](../../implemented/feature/2026-10-31-quick-phrase-cursor.md)；不改变本文原有决定。
