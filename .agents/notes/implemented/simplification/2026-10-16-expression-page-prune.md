# Agent Note: 「标点与翻译」页精简：标点合成一个选择，候选释义合成一个标签

Status: implemented

## Problem

macOS 设置精简调研发现，「标点与翻译」页的拥挤主要来自三处：同一个决定被拆成几行、少数人用的选项平铺在主流程里、两个标签控制同一行释义。其中几处还会误导用户：

- 智能标点有三个开关：总开关、数字后直出、字母后直出。macOS 和 Windows 新配置上三个都是关（`smart_punctuation_default`）。`client-core` 的 `punctuation::route` 只有在两半之一也打开时才走 ASCII，所以只打开总开关，已经上屏的数字或字母后面打 `,` 仍然是中文标点，和它的说明「字母或数字后的 , . : 自动使用英文标点」相反。只有正在组字的那条路径（macOS `InputController` 的 `punctuationASCII`）绕过了这个检查。
- 智能标点关着时，四个细分开关仍然能点，但都不起作用：重复转中文和空格转换在 `InputController` 里先检查总开关，两种直出在 `route()` 里先检查总开关。
- 「中文标点」开关和「固定标点」三选一描述的是同一件事。`punctuation_lock` 为 `chinese` 或 `english` 时，引擎不看 `chinese_punctuation`：`engine_chinese_punctuation` 和 `route()` 按锁定走，`InputController` 的工具栏切换直接把运行时状态设成锁定值。「三者互斥」的说明和旁边单独的开关也对不上。只有 macOS 的 `runtimeChinesePunctuation` 在没有运行时覆盖（新开会话、从英文模式切回）时回落到 `chinesePunctuation`，于是「锁定英文、起始中文」时菜单勾选、工具栏标题和成对 `{` 显示成中文。
- 翻译服务在新配置上显示「腾讯云机器翻译」，密钥却是空的，因为 `TencentTmtPreferences::default` 是 `enabled: true`。macOS 上没有可用密钥的腾讯云不会被请求：host-api 的 `selected_translation_services` 只在两个密钥都可用时才把腾讯云放进查询，`InputController` 的 `currentOnDeviceGlossRequest` 看到查询里没有服务，就改用系统本机翻译。所以 macOS 上的「关闭」和「没配好的腾讯云」实际都是离线翻译，不是关掉翻译。
- 「候选与释义」标签在 macOS 上只有「显示英文释义」和「显示读音」两行（英文建议只在手机上显示），「候选翻译」标签控制的是同一行释义。

## Decision

### 标点

- 桌面、Android、iOS 共用的 `PunctuationSection` 把主流程减到四行：「标点」选择、引号样式、智能标点、成对标点自动补全。方头括号样式、智能标点的四个细分开关、大写锁定时使用英文标点收进「更多选项」（`MoreOptions`）。
- 「标点」选择有四项。显示值由 `punctuationMode` 从两个已存的键推出，锁定优先；打开页面不改写文档：
  - `chinese`：`chinese_punctuation = true`，`punctuation_lock = follow`
  - `english`：`chinese_punctuation = false`，`follow`
  - `always_chinese`：`punctuation_lock = chinese`，`chinese_punctuation` 不动
  - `always_english`：`punctuation_lock = english`，`chinese_punctuation` 不动
  用户选定一项时由 `punctuationModePatch` 写入。前两项把两个键一起写；「始终」两项只写锁定，因为 `chinese_punctuation` 会上云（`input.chinese_punctuation`）而 `punctuation_lock` 不上云，顺带写起始标点会把用户其他设备的起始标点也改掉。原来的「固定标点」也只写锁定，这样合并后没有新的跨设备副作用。
- 为此 macOS 的 `runtimeChinesePunctuation` 在锁定为 `chinese` 或 `english` 时直接返回锁定值，不再回落到起始标点。菜单勾选、工具栏标题和成对 `{` 都读它，锁定和起始标点不一致时也和引擎的实际输出一致。
- 智能标点总开关被用户切换时，`smartPunctuationPatch` 把 `smart_punctuation_direct_digit` 和 `smart_punctuation_direct_letter` 一起写成同一个值。打开页面、切换其他开关都不碰这两个键，单独在「更多选项」里调过的值，只有用户再次切换总开关时才会被覆盖。
- 账号同步只映射 `input.smart_punctuation`，两半不上云。`client-services` 的 `apply_input_settings` 在云端值和本机不同时把两半写成同一个值，等于别的设备替用户切了一次总开关；值相同就不动，本机单独调过的一半不会每次同步都被冲掉。不这样做的话，在 Linux 或另一台设备打开智能标点后同步到 macOS，这里会是「总开关开、两半都关」，又回到开了不起作用。
- 以前只写过总开关的存量文档仍是「总开关开、两半都关」。这里不改写存储，`SmartPunctuationRow` 按已存的值在说明后面加一句「数字后直出和字母后直出都已关闭，这一条暂不生效」，并指向「更多选项」或重新切换总开关。
- 四个细分开关（`PunctuationSwitchRow` 新增 `disabled`）在总开关关着时变灰，值不变。大写锁定标点不依赖智能标点（`route()` 在直出检查之前就处理了它），所以收进「更多选项」但不变灰。
- HarmonyOS 手机的 `PhonePunctuationSection` 保持设计稿布局（「使用英文标点」开关，「更多选项」里的「固定标点」），只修两处 bug：总开关同样连同两半一起写，细分开关同样会变灰。
- macOS 原生回退窗口（`AppearancePreferences.mm`）的「智能标点」开关同样这样做。`_pendingSmartPunctuationDirect` 记下用户刚切换的值，`sharedPreferencesByMerging:` 每次保存都带上两半，直到 `applySharedInputPreferences:` 载入的文档里两半都等于它才撤销。所以一次保存失败不会把它丢掉，撤销之后也不再写这两个键。回退窗口里的「中文标点」和「固定标点」仍是两个控件，这次只合并 React 页面。
- MCP 的 `update_preferences` 改 `smart_punctuation` 时同样写两半，与它改模糊音时跟设置页保持一致是同一个做法。

### 候选释义

- `expressionTabs` 从四个标签变成三个：标点、候选释义（`gloss`）、翻译服务。表达页通过 `TranslationSettingsContent` 的 `renderGroup`，把「多语言与释义」组和「候选词翻译」组放进同一个 `gloss` 面板。手机不分标签，各组顺序不变。
- 合并的依据是宿主怎样组合两个来源。`candidate_english_gloss` 只打开离线词库释义：`currentGlossRequest` 查英文，`currentTargetGlossRequest` 查其他目标语言。`candidate_translations` 在这些之外再加用户的在线服务、水杉账号或系统本机翻译。`offlineGlossResults` 把所有来源按候选、按目标语言合进同一行，两个开关都按同一个 `translation_target_language` 和第二种语言取值。所以它们是同一行释义的两个来源集合，不是两个独立功能。
- 「显示读音」不再放在「多语言与释义」组里以禁用状态出现。它改为通过 `CandidateTranslationOptionsSection` 的 `beforeMore`，画在目标语言之后，只在任一释义来源打开时出现。已存的值不动。
- `CandidateTranslationOptionsSection` 的默认布局把第二种语言收进「更多选项」，做法与手机布局一致。Android 的「使用水杉账号翻译候选词」留在外面，因为它在 Android 上是唯一的在线翻译来源。

### 翻译服务

- macOS 上（`createCandidateTranslationSettings` 的 `offlineFallback: macos`），翻译服务的 `none` 选项叫「离线翻译（不联网）」，并说明来源：离线词库；macOS 26 起，词库查不到的候选由系统在本机翻译。腾讯云缺密钥时的警告也改说「填好之前候选仍用离线翻译」，不再说「不会有任何结果」。
- `useTranslationSettings` 在 macOS 上把「没有可用密钥的腾讯云」显示为 `none`，与宿主实际行为一致。腾讯云本来就是 `enabled: true`，用户在下拉框里选它不会改文档，所以用窗口内的 `tencentChosen` 状态区分「用户这次选了腾讯云」和「从没选过」。选了就显示腾讯云，凭据输入框随之出现。
- 默认值不改：`TencentTmtPreferences::default`、`translation-defaults.ts` 和 Windows、Linux 的显示都保持原样。Linux 的密钥存在 provider 自己的文件里，偏好里的空密钥不代表没配置。Windows 上「关闭」实际是什么，没有在这次核对。
- 腾讯云「地域」收进凭据下方的「更多选项」。请求一律发往 `tmt.tencentcloudapi.com`，地域只写进 `X-TC-Region` 头，留空时回退到 `ap-guangzhou`（`client-core/src/translation.rs`）。已存的地域不合规时，折叠区默认展开。HarmonyOS 手机的「翻译服务」整组已经收在「更多选项」里，那里传 `foldRegion={false}`，地域不再套第二层。Linux 的 `LinuxTencentCredentialsSection` 是另一套写凭据文件的表单，没有改。

## Historical audit

按 `expressionTabs`、`punctuation_lock`、`smart_punctuation_direct`、`translation provider`、`腾讯云地域` 检索活跃笔记：

- [顶部功能标签](../../implemented/feature/2026-10-13-desktop-input-expression-voice-tabs.md)：部分被取代。表达页的标签列表已按本篇就地更新，隐藏面板保留挂载、手机直列的决定不变。
- [表达页标签模型](../../implemented/simplification/2026-10-16-expression-tabs.md)：模型模块的决定仍然成立，只是数组内容变了。
- [共享标点偏好的叶子类型](../../implemented/simplification/2026-10-15-punctuation-preferences.md)：部分重叠。「单字段补丁」不再适用于「标点」选择和智能标点总开关，已在那篇注明。
- [引号与括号样式](../../implemented/feature/2026-10-13-punctuation-mark-styles.md)：持有样式行为，本篇只把括号样式挪进折叠区。
- [大写锁定标点](../../implemented/feature/2026-10-09-caps-lock-ascii-punctuation.md)：持有 ASCII 路线行为，本篇只挪位置。
- [macOS 翻译占位与下载提示](../../implemented/bug-fix/2026-10-15-macos-translation-placeholder-download-notice.md)：持有系统翻译语言提示，本篇沿用它的 `onDeviceTranslationInUse` 判定。

## Alternatives considered

- **「标点」只给三项：中文标点 / 英文标点 / 中英文模式都用中文标点（调研报告的提案）。** 最强的理由是更短，而且 `lock = english` 和「英文标点」看起来几乎一样。没有采用：两者差在工具栏和 Ctrl+. 能不能临时切回中文标点（`InputController` 的 `punctuation.enabled`）。三项方案会把选了「始终使用英文标点」的人悄悄变成可切换，`english` 还会在显示上吞掉这个区别，用户再选一次就改写了存储。四项把参考窗口两个设置能表达的每种有效组合都保留下来，仍然把两行合成了一行。
- **把细分开关只在智能标点打开时显示，不收进「更多选项」。** 最强的理由是关着时页面更短。没有采用：打开总开关时一下冒出四行，主流程又变长了；而手机布局已经把它们收在「更多选项」里，桌面照做，两边一致。
- **改 `route()`，让「总开关开、两半从未设过」也算开。** 最强的理由是不需要写两半，旧配置也会自动生效。没有采用：Rust 无法区分「从未设过」和「用户关掉」，这两个键始终会序列化进文档。而且这会改变所有宿主读同一份文档的含义，正是 `smart_punctuation_default` 那段注释反对的「已存值被重新解释」。
- **把 `TencentTmtPreferences::default` 改成 `enabled: false`。** 最强的理由是显示自然就对了，不需要窗口内状态。没有采用：这会改变 Linux（密钥在文件里，靠默认的 `enabled` 生效）和 Windows 的默认行为，而这次只处理 macOS。
- **候选释义改成「一个释义开关 + 来源选择」（离线词库 / 在线服务）。** 最强的理由是更少的行。没有采用：`candidate_english_gloss` 也出现在首次运行引导和多个宿主里，改成来源选择要动偏好语义；合并标签已经让两个开关在同一屏。
- **腾讯云地域直接砍掉，固定 `ap-guangzhou`。** 最强的理由是请求地址写死，地域几乎不影响结果，还可能因为填错而让翻译失败。没有采用：需要境外数据驻留（ap-hongkong、ap-singapore）的用户会失去唯一的入口，按负责人的决定收进「更多选项」。
- **方头括号样式、重复标点转中文、空格转换、大写锁定标点、第二种语言直接固定成默认值。** 没有采用：括号样式和第二种语言会上云（`settings_sync.rs` 的 `input.bracket_style`），只在 macOS 藏起来会出现别的设备设了、Mac 上看不见却生效的值；其余几项都是 issue 明确要求加的（#6370、#3081 和参考产品的 `repeatPunctuation`），砍掉等于删功能。

## Consequences

- **收益**：
  - macOS「标点」标签的主流程从约 11 行降到 4 行加一个折叠区，标签从 4 个减到 3 个。
  - 只打开「智能标点」现在就会按说明工作，从别的设备同步过来的打开也一样；存量的失效状态会在界面上说出来。
  - 一个选择表达标点的全部有效组合；锁定和起始标点不一致的旧文档在页面和 macOS 运行时都按锁定显示。
  - macOS 上翻译服务的显示和宿主实际行为一致。
- **代价**：
  - 存储里锁定和起始标点仍可能不一致（例如锁定英文、起始中文）。这是有意的：起始标点是跨设备的值，锁定是本机的值，只在用户选「中文标点 / 英文标点」时才统一写。
  - 只打开过总开关的存量用户，要在「更多选项」里打开两半或把总开关关掉再打开一次才生效，页面只提示，不自动改写。
  - 单独只开过一半直出的用户，下次切换总开关时这一半会被统一。
  - `tencentChosen` 只活在设置窗口里：选了腾讯云却没填密钥就关掉窗口，下次打开又显示「离线翻译」。这与实际行为一致，只是凭据框需要重新选一次才出现。
  - 手机布局仍保留「使用英文标点 + 固定标点」两个控件。
  - Windows 原生 WinUI 设置页的标点和翻译服务没有跟进，智能标点同样只写总开关，留待 Windows 对齐时处理。
- `scripts/test-reference-config-coverage.py`：所有键仍在页面上出现，不需要新的 `PLATFORM_LOCAL` 豁免。
- `platforms/macos/tests/settings/preference_coverage.py`：不受影响，没有砍掉宿主读取的值。

## Verification

- `apps/desktop` 的 vitest 新增和改写了以下用例：
  - 「标点」选择四种取值的推导、矛盾旧组合按锁定显示且不改写、四种写回补丁（「始终」只写锁定）。
  - 智能标点总开关连同两半写入，细分开关在总开关关着时变灰、打开后单字段写入；手机分组同样的两处修复。
  - 括号样式和大写锁定标点进入折叠区。
  - 候选释义标签同时含两个释义开关。
  - 显示读音只在有释义来源时出现、画在目标语言之后。
  - 第二种语言折叠、账号开关不折叠。
  - macOS「离线翻译」叫法和来源说明、未配置腾讯云显示为离线翻译直到用户选择、Windows 和 Linux 不变。
  - 地域折叠及不合规时展开、macOS 缺凭据警告。
- 参考窗口分区表和选项表改为「标点」并记下原因。
- `msime-mcp-server` 新增 `smart_punctuation_writes_both_halves_with_the_master_switch`。
- `msime-client-services` 新增 `settings_sync_smart_punctuation_change_writes_both_halves`（云端值不同时两半跟着写）和 `settings_sync_unchanged_smart_punctuation_leaves_the_halves_alone`（相同时不动），`settings_sync_round_trips_every_exported_key` 的期望值随之带上两半。
- vitest 另有：「始终」两项只写锁定；总开关开、两半都关时出现提示且不写；HarmonyOS 手机「翻译服务」只有一层「更多选项」，地域就在里面。
- `ShortcutTest.mm` 的 `TestPairedPunctuationPreferences` 断言锁定英文时运行时标点为英文（起始仍是中文）、锁定中文时为中文、跟随时等于起始值。
- 原生回退窗口新增 `ShortcutTest.mm` 的 `TestSmartPunctuationWritesBothHalves`：切换开关后两半写入、文档未确认前持续带上、确认后不再写、之后别处改的一半不被冲掉。原生 `.mm` 改动和这个用例本机都无法编译，只按周边代码逐行核对了类型、ARC 和调用点，由 CI 编译运行。
