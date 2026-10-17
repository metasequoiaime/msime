# Agent Note: 「标点与翻译」页精简：标点合成一个选择，候选释义合成一个标签

Status: implemented

## Problem

macOS 设置精简调研发现，「标点与翻译」页的拥挤主要来自三处：同一个决定被拆成几行、少数人用的选项平铺在主流程里、两个标签控制同一行释义。其中几处还会误导用户：

- 智能标点有三个开关：总开关、数字后直出、字母后直出。macOS 和 Windows 新配置上三个都是关（`smart_punctuation_default`；之后的[智能标点默认开启](../../implemented/feature/2026-10-16-smart-punctuation-default-on.md)把它们在所有平台上改成默认开，并把旧文档一次性迁移）。`client-core` 的 `punctuation::route` 只有在两半之一也打开时才走 ASCII，所以只打开总开关，已经上屏的数字或字母后面打 `,` 仍然是中文标点，和它的说明「字母或数字后的 , . : 自动使用英文标点」相反。只有正在组字的那条路径（macOS `InputController` 的 `punctuationASCII`）绕过了这个检查。
- 智能标点关着时，四个细分开关仍然能点，但都不起作用：重复转中文和空格转换在 `InputController` 里先检查总开关，两种直出在 `route()` 里先检查总开关。
- 「中文标点」开关和「固定标点」三选一描述的是同一件事。`punctuation_lock` 为 `chinese` 或 `english` 时，引擎不看 `chinese_punctuation`：`engine_chinese_punctuation` 和 `route()` 按锁定走，`InputController` 的工具栏切换直接把运行时状态设成锁定值。「三者互斥」的说明和旁边单独的开关也对不上。只有 macOS 的 `runtimeChinesePunctuation` 在没有运行时覆盖（新开会话、从英文模式切回）时回落到 `chinesePunctuation`，于是「锁定英文、起始中文」时菜单勾选、工具栏标题和成对 `{` 显示成中文。
- 翻译服务在新配置上显示「腾讯云机器翻译」，密钥却是空的，因为 `TencentTmtPreferences::default` 是 `enabled: true`。macOS 上没有可用密钥的腾讯云不会被请求：host-api 的 `selected_translation_services` 只在两个密钥都可用时才把腾讯云放进查询，`InputController` 的 `currentOnDeviceGlossRequest` 看到查询里没有服务，就改用系统本机翻译。所以 macOS 上的「关闭」和「没配好的腾讯云」实际都是离线翻译，不是关掉翻译。
- 「候选与释义」标签在 macOS 上只有「显示英文释义」和「显示读音」两行（英文建议只在手机上显示），「候选翻译」标签控制的是同一行释义。

## Decision

### 标点

- 桌面、Android、iOS 共用的 `PunctuationSection` 把主流程减到四行：「标点」选择、引号样式、智能标点、成对标点自动补全。方头括号样式、智能标点的四个细分开关、大写锁定时使用英文标点收进「更多选项」（`MoreOptions`）。（之后的[智能标点默认开启](../../implemented/feature/2026-10-16-smart-punctuation-default-on.md)让智能标点默认开，把总开关也收进「更多选项」，主流程剩三行。）
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
- HarmonyOS 手机的 `PhonePunctuationSection` 保持设计稿布局（「使用英文标点」开关，「更多选项」里的「固定标点」），只修两处 bug：总开关同样连同两半一起写，细分开关同样会变灰。（智能标点总开关之后随[智能标点默认开启](../../implemented/feature/2026-10-16-smart-punctuation-default-on.md)收进「更多选项」。）
- macOS 原生回退窗口（`AppearancePreferences.mm`）的「智能标点」开关同样这样做。`_pendingSmartPunctuationDirect` 记下用户刚切换的值，`sharedPreferencesByMerging:` 每次保存都带上两半，直到 `applySharedInputPreferences:` 载入的文档里两半都等于它才撤销。所以一次保存失败不会把它丢掉，撤销之后也不再写这两个键。回退窗口里的「中文标点」和「固定标点」仍是两个控件，这次只合并 React 页面。
- MCP 的 `update_preferences` 改 `smart_punctuation` 时同样写两半，与它改模糊音时跟设置页保持一致是同一个做法。

### 候选释义

- `expressionTabs` 从四个标签变成三个：标点、候选释义（`gloss`）、翻译服务。表达页通过 `TranslationSettingsContent` 的 `renderGroup`，把「多语言与释义」组和「候选词翻译」组放进同一个 `gloss` 面板。手机不分标签，各组顺序不变。
- 合并的依据是宿主怎样组合两个来源。`candidate_english_gloss` 只打开离线词库释义：`currentGlossRequest` 查英文，`currentTargetGlossRequest` 查其他目标语言。`candidate_translations` 在这些之外再加用户的在线服务、水杉账号或系统本机翻译。`offlineGlossResults` 把所有来源按候选、按目标语言合进同一行，两个开关都按同一个 `translation_target_language` 和第二种语言取值。所以它们是同一行释义的两个来源集合，不是两个独立功能。
- 「显示读音」不再放在「多语言与释义」组里以禁用状态出现。它改为通过 `CandidateTranslationOptionsSection` 的 `beforeMore`，画在目标语言之后，只在任一释义来源打开时出现。已存的值不动。
- `CandidateTranslationOptionsSection` 的默认布局把第二种语言收进「更多选项」，做法与手机布局一致。Android 的「使用水杉账号翻译候选词」留在外面，因为它在 Android 上是唯一的在线翻译来源。

### 翻译服务

- macOS 上（`createCandidateTranslationSettings` 传入的 `offlineFallback: translationOfflineFallback(...)`，macOS 得到 `"system"`），翻译服务的 `none` 选项叫「离线翻译（不联网）」，并说明来源：离线词库；macOS 26 起，词库查不到的候选由系统在本机翻译。腾讯云缺密钥时的警告也改说「填好之前候选仍用离线翻译」，不再说「不会有任何结果」。Windows、Linux、HarmonyOS 得到的 `"dictionary"` 一支见「各平台跟进」。
- `useTranslationSettings` 在 macOS 上把「没有可用密钥的腾讯云」显示为 `none`，与宿主实际行为一致。腾讯云本来就是 `enabled: true`，用户在下拉框里选它不会改文档，所以用窗口内的 `tencentChosen` 状态区分「用户这次选了腾讯云」和「从没选过」。选了就显示腾讯云，凭据输入框随之出现。
- 默认值不改：`TencentTmtPreferences::default` 和 `translation-defaults.ts` 保持原样。Linux 的密钥存在 provider 自己的文件里，偏好里的空密钥不代表没配置，所以 Linux 的显示不变。Windows 和 HarmonyOS 后来核对过，见「各平台跟进」。
- 腾讯云「地域」收进凭据下方的「更多选项」。请求一律发往 `tmt.tencentcloudapi.com`，地域只写进 `X-TC-Region` 头，留空时回退到 `ap-guangzhou`（`client-core/src/translation.rs`）。已存的地域不合规时，折叠区默认展开。HarmonyOS 手机的「翻译服务」整组已经收在「更多选项」里，那里传 `foldRegion={false}`，地域不再套第二层。Linux 的 `LinuxTencentCredentialsSection` 是另一套写凭据文件的表单，后来同样折叠，见「各平台跟进」。

## 各平台跟进

macOS 之后，Windows、Linux、HarmonyOS 逐项按同一个标准跟进：控件在那个平台上存在，macOS 的理由在那里也成立，才照做；平台上存在的 bug 一律修。这一页没有砍掉的项，所以没有能力位要改。

### Windows

原生 WinUI 设置页（`platforms/windows/settings/main.cpp` 的 `build_expression_page`）照共享页改，规则放在不依赖 WinRT 的头文件里单独测试：

- 「中文标点」开关和「固定标点」合成四项的「标点」选择（`PunctuationSettings.h`，推导和补丁与 `punctuation-preferences.ts` 相同，`windows-punctuation-settings` 测试）。理由在 Windows 同样成立：TIP 的 `ResolvePunctuationOpen` 让锁定压过 Ctrl+. 和工具栏，托盘在锁定时禁用「中文标点」。
- 「标点」的说明与共享页不同。TIP 每次激活和每次切换中英文时都按模式重定标点（`CompositionProcessorEngine.cpp` 的 `SetPunctuationMode`，以及开关中英文时的 `CompartmentCallback`），从不读已存的 `chinese_punctuation`，Server 端会话的起始值随后就被 TIP 的每会话覆盖取代。所以不锁定时，「中文标点」「英文标点」在这台电脑上没有持久作用，只决定同一账号其他设备上的起始标点，说明照实写出这一点。TIP 没有改成从已存值起步，理由见「Alternatives considered」。
- 智能标点总开关连同两半写。这个陷阱在 Windows 上最常见：三个键默认都是关（[智能标点默认开启](../../implemented/feature/2026-10-16-smart-punctuation-default-on.md)之后改为默认开，总开关也挪进下面的「更多选项」），TIP 的 `ResolvePunctuation` 要所在一侧的直出也开着才出 ASCII，原来的页面只写总开关，新装后打开智能标点什么都不发生。「总开关开、两半都关」时说明多一句提示，两半的开关切换后重画页面，提示跟着变。
- 方头括号样式和四个细分开关收进「更多选项」，控件用页面里语音页、AI 页已有的 `Expander`，细分开关在总开关关着时变灰（原来就有）。Windows 没有「大写锁定时使用英文标点」一行（`caps_lock_punctuation` 只在 macOS 为真）。
- WinUI 页面没有标签，「多语言与释义」和「候选词翻译」两组本来就挨着，所以没有合并标签这一步，`SettingsNavigation.h` 的页数不变。
- 「显示读音」移到目标语言之后，只在英文释义或候选词翻译打开时出现。除 macOS 外只有 Windows 提供读音（`TranslationWorker` 画读音）。
- 目标语言原来只在候选词翻译打开时可选，可 host-api 的 `msime_client_translation_query` 在只开英文释义时也按目标语言给出 `offline_gloss_languages` 和 `english_gloss`，只开英文释义的人改不了一个仍在生效的值。现在两者之一打开就可选，与共享页的 `candidateGlossLanguagesEnabled` 相同。
- 第二种语言（会上云，只折叠）和腾讯云地域收进「更多选项」。地域的折叠与共享页 `MoreOptions` 的 `defaultOpen` 相同，第一次画出时已存的地域不合规就展开。
- 翻译服务的 `none` 叫「不使用在线服务」，不照搬 macOS 的「离线翻译」：Windows 没有系统翻译，可它也不是「关闭」，host-api 仍把装好的离线词库语言放进查询，`TranslationWorker` 照样填出释义。没有可用密钥的默认腾讯云同样显示成 `none`（Server 从 `msime_client_translation_query` 取查询，host-api 只在两个密钥都可用时放进腾讯云），用户在本窗口选了腾讯云才显示腾讯云和凭据框（`TranslationServiceSettings.h` 的 `tencent_chosen`）。缺凭据的警告改说「填好之前不会请求在线翻译」。
- 共享 React 页在 Windows 平台上也走同样的叫法和显示（`offlineFallback: "dictionary"`，`useTranslationSettings` 的 `tencentChosen` 扩到 Windows），两套界面不分叉。
- 设置窗口里工具栏预览的标点按钮原来只看 `chinese_punctuation`，锁定时画错，而 TIP 从不读这个键。现在照 TIP 的 `ResolvePunctuationOpen` 画：锁定时取锁定值，跟随时随预览里语言按钮的中英文模式（`PunctuationSettings.h` 的 `toolbar_chinese_punctuation`）。悬浮工具栏那一片没有碰这个按钮，所以在这里一并改。

### Linux

Linux 画的是共享 React 页，标点合并、细分开关折叠和变灰、候选释义标签、智能标点写两半都随 macOS 那次生效了。宿主侧核对过理由：fcitx5 的 `toggleChinesePunctuation` 在锁定时直接返回，锁定经 host-api 的 `engine_chinese_punctuation` 生效，四个细分开关在 fcitx5 和 IBus 都有读取方。

- 托盘和状态栏切换智能标点原来只写总开关。设置页关过一次之后两半已是假，从这里再打开的就是不起作用的总开关。新增两个宿主共用的 `core/SmartPunctuationChoice.h`：fcitx5 的状态栏切换把三个键放进同一次保存（`PendingPreferenceSave` 的 `smart_punctuation_choice`，不拆成三个各自读写修订号的任务），IBus 的 `save_menu_preference` 同样写两半。IBus 没有偏好目录时只设会话覆盖，那里没有文档要保持一致，所以不动。
- `LinuxTencentCredentialsSection` 的地域收进「更多选项」：Linux provider 同样只请求 `tmt.tencentcloudapi.com`，地域只进 `X-TC-Region`，留空按广州处理（`msime-linux-online-provider`）。地域仍在表单输入里，「保存」照样带上它。编辑输入、地域回退和保存组装由[腾讯云凭据公共纯模型](2026-10-17-tencent-credential-editor.md)承接。
- 翻译服务的 `none` 叫「不使用在线服务」：fcitx5 和 IBus 同样消费 `offline_gloss_languages`，也没有系统翻译。
- 未配置腾讯云的显示不改：Linux 的密钥在 tencent-provider.json，偏好里的空密钥不代表没配置，host-api 也正靠 `enabled: true` 把查询交给 provider。
- 显示读音、第二种语言、大写锁定标点本来就不在 Linux 上显示。

### HarmonyOS

- 手机和平板保持设计稿的 `PhonePunctuationSection`（「使用英文标点」开关，「更多选项」里的「固定标点」），这是 macOS 那次有意保留的布局，`expression-harmony` 测试钉住；翻译相关的控件在手机上本来就收在「更多选项」里，也没有标签可合。2in1 走桌面分支，已经是合并后的选择和标签。
- 账号同步：HarmonyOS 用自己的 `AccountPreferencePlan.ts`，不走 client-services，所以 macOS 那次在 `apply_input_settings` 里修的陷阱在这里还在：别的设备先关后开智能标点，同步过来就是「总开关开、两半都关」。现在云端值和本机不同时两半跟着写，相同时不动，与 Rust 那边同一条规则。
- 固定标点：`KeyboardSession` 的工具栏标点按钮和功能面板开关原来只看 `chinesePunctuation`，锁定英文时仍可能显示中文；工具栏和 Ctrl+. 的切换在锁定时还会改写上云的 `chinese_punctuation`。现在两处都按 `PunctuationLockPolicy` 显示锁定值，锁定时切换什么都不做，与 Windows TIP、fcitx5 相同。功能面板的「中文标点」磁贴在锁定时置灰（`KeyboardView` 的 `functionTileEnabled`），不让它点了又弹回，与 Windows 托盘在锁定时禁用「中文标点」相同。这是 macOS `runtimeChinesePunctuation` 那处修正在 HarmonyOS 上的同类问题。
- 未配置的默认腾讯云同样显示成 `none`：HarmonyOS 的密钥存在偏好里（它画 `TencentTranslationSection`），`TranslationPolicy.provider` 只认 host-api 放进查询的腾讯云。`none` 的叫法和缺凭据警告同 Windows，手机「更多选项」里的选择器也用同一个叫法。
- 键盘上没有智能标点开关，不需要托盘那一类修正；也没有 MCP。

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
- **把 `TencentTmtPreferences::default` 改成 `enabled: false`。** 最强的理由是显示自然就对了，不需要窗口内状态。没有采用：Linux 的密钥在文件里，靠默认的 `enabled` 生效，改默认值会让 Linux 悄悄停掉在线翻译；Windows 和 HarmonyOS 跟进时也只改显示，同样不动默认值。
- **让 Windows TIP 从已存的 `chinese_punctuation` 起步，而不是只改说明。** 最强的理由是「标点」选择的前两项在 Windows 上就会和其他平台一样生效。没有采用：`docs/windows-parity.md` 记着 Windows 有意保留来源「标点总从中英文模式出发」的行为，只有 macOS 改成从保存值起步；改 TIP 要动激活、中英文切换和每会话覆盖三处，必须真机验证，不该夹在设置页精简里。说明照实写出之后，用户至少知道这两项在这台电脑上管的是什么。
- **Windows、Linux 的 `none` 也叫「离线翻译」。** 最强的理由是各平台叫法一致。没有采用：那两个平台没有系统翻译，英文离线释义还要另开「显示英文释义」，叫「离线翻译」会让人以为不开在线服务也有完整翻译；叫「关闭」又不对，离线词库照样出释义。
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
  - Windows 上不锁定时，「标点」的前两项在这台电脑上没有持久作用（TIP 按中英文模式重定标点），只在说明里写明，没有改 TIP。
  - 三套界面（WinUI、共享 React、手机）对 `none` 有三种叫法：macOS「离线翻译（不联网）」，Windows、Linux、HarmonyOS「不使用在线服务」，Android、iOS 仍是「关闭」（没有核对过）。
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
- 各平台跟进：`windows-punctuation-settings`（四种推导、矛盾旧组合按锁定显示、四种补丁、智能标点写三个键、失效提示、工具栏预览的标点按钮按锁定和中英文模式画）；`windows-translation-service-settings` 改为未配置的默认腾讯云读作 `none`、本窗口选过才是腾讯云；`linux-smart-punctuation-choice` 和 fcitx5 原生测试断言状态栏两次切换后两半跟着变并落盘；HarmonyOS `keyboard-logic` 的同步两例（值变了写两半、没变不动）和 `kb-session` 的锁定两例；vitest 新增 `dictionary` 叫法和说明、Windows 和 HarmonyOS 的缺凭据警告、Windows 和 HarmonyOS 未配置腾讯云显示成 `none`、Linux 地域折叠且「保存」带上它、HarmonyOS 手机选择器的叫法。WinUI、fcitx5、IBus 和 ArkTS 改动本机都无法编译，由 CI 编译运行；两个 Windows 头文件测试和 Linux 头文件测试在本机用 clang 跑过。
- 原生回退窗口新增 `ShortcutTest.mm` 的 `TestSmartPunctuationWritesBothHalves`：切换开关后两半写入、文档未确认前持续带上、确认后不再写、之后别处改的一半不被冲掉。原生 `.mm` 改动和这个用例本机都无法编译，只按周边代码逐行核对了类型、ARC 和调用点，由 CI 编译运行。
