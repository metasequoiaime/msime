# Agent Note: 智能标点默认开启，数字后的点直接出英文

Status: implemented

## Problem

中文模式下打 `0.10.1`，Windows 和 macOS 的新用户得到的是 `0。10。1`。智能标点能把数字、字母后的 `,` `.` `:` 留成英文，可它在这两个平台上默认关，得先在设置里找到它才有这个行为；版本号、小数、时间、网址这类输入几乎人人都会遇到，靠用户自己发现开关是不对的。

默认关的决定来自 `2418bee87f`（2026-09-20）和 `97d7734ab4`：来源产品的安装模板 `config.default.toml` 把智能标点整族五个开关全设为关，运行中的宿主读的却是共享偏好文档，于是 `smart_punctuation_default()` 改成 `!cfg!(any(windows, target_os = "macos"))`，macOS 作为那套桌面产品的移植跟着走；Linux、Android、iOS、HarmonyOS 一直默认开，理由是「让偏好在老用户脚下变掉比按平台不同更糟」。那次决定只写在代码注释、提交说明和 `docs/windows-parity.md` 里，没有决策笔记，本篇取代它，并把它的理由一并记在这里。

只改默认值不够：旧的 macOS、Windows 偏好文档把当时的默认 `false` 明明白白存了下来（`Preferences` 每个字段都序列化），从没碰过开关的用户和有意关掉的用户在文档里一模一样，`serde` 默认只对缺键生效，所以老用户升级后仍是 `0。10。1`。

## Decision

### 默认值

- `crates/client-core/src/preferences.rs` 的 `smart_punctuation_default()` 在所有平台上返回 `true`，它是 `smart_punctuation`、`smart_punctuation_repeat`、`smart_punctuation_direct_digit`、`smart_punctuation_direct_letter` 的 `Default` 值和缺键时的 serde 默认。
- `smart_punctuation_space_convert` 仍处处默认关。它把刚上屏的中文标点按空格改回 ASCII，改写的是用户已经看着落下去的字符，而且没有反悔手势（再按一次空格只是空格），默认关的理由对它仍然成立。
- Host API 在保存快照和执行空格转换时都检查已生效的 `smart_punctuation` 与 `smart_punctuation_space_convert`。快照归宿主保存，偏好热更新不会撤回旧快照，所以关闭任一开关后，旧快照也必须停止改写文字；执行入口过滤快照，继续复用共享的编辑器代次、前文与组字校验。
- 重复标点转中文在保存和执行快照时都检查实际生效的输入方案，共用 `scheme_allows_repeat_punctuation`：韩文、越南文、藏文和注音不启用，日文保留原有支持。方案热更新后宿主可能仍持有全拼时的快照，不能只在保存时排除；也不能复用排除了日文的前文直出谓词 `SchemeType::host_smart_punctuation`，两种手势的适用范围不同。
- 跟着改的副本：Windows 安装模板 `platforms/windows/installer/config.default.toml`（四项 `true`，空格转换 `false`）；共享设置页 `punctuationControlState` 的缺键显示值（`packages/ui/src/settings/punctuation-preferences.ts`）；Windows 原生设置页和 Server 读偏好时的回退值（`platforms/windows/settings/main.cpp`、`platforms/windows/src/entrypoints/server_main.cpp`）；macOS 原生 `AppearancePreferences.mm` 在共享文档载入前的回退值和 `CloudAppearanceSettings.h` 的云端快照缺省值。Linux、Android、iOS、HarmonyOS 自己的副本本来就是开，不用改。
- Windows TIP 在 Server 送来已存设置之前保持关（`platforms/windows/tsf/IPC/Ipc.h`），这不是默认值而是「还不知道用户选了什么」的安全态，不改。

### 一次性迁移

- `Preferences` 新增内部字段 `smart_punctuation_migrated`（`#[serde(default, skip_serializing_if = "is_false")]`），不是设置：缺键读作 `false`，表示「默认改变之前写下的文档」；`Preferences::default()` 里是 `true`，新文档从第一份起就带着它。
- `PreferencesStore` 读文档时（`read_locked_keeping_unknown`，`load`、`try_load`、`save` 前的读取、恢复默认设置都经过它）调用 `Preferences::settle_smart_punctuation_default(SMART_PUNCTUATION_WAS_OFF_BY_DEFAULT)`：没有标记时，在 macOS 和 Windows（`cfg!(any(windows, target_os = "macos"))`）上把上面四项打开；不论平台都标上标记。其余平台旧文档里的 `false` 本来就是用户自己关的，只标记不改。空格转换不动。
- 迁移只改读出来的这一份，不为它写盘、不动修订号。每个读者（宿主、设置应用、MCP、同步）读到的都是同一个结果；文件等下一次真正的保存才带上新值和标记，在那之前每次读都重复同样的迁移。这与 `retire_unavailable_asr_model` 的「读入时改，下一次保存落盘」相同。
- 标记只增不减，做法照 `fuzzy_pinyin.seeded`：`save_locked` 里 `current` 带着标记就给要写的偏好也标上，写入方的偏好里没有这个键（设置页、原生宿主、MCP 拼出的 JSON）也不会把它丢掉；`restored_to_defaults_for` 从 `Default` 出发，自然带着；`settings_document::import_document` 取本机和文件标记的或；`salvage_preferences` 修复损坏文档时，文档里没有标记就从垫底值里也去掉，修复结果照旧文档迁移一次（与 `touch_keyboard_schemes` 的处理相同）。
- 迁移之后用户关掉智能标点：保存的文档带着标记和 `false`，之后的读取不再改它。
- 不升 `format_version`：`read_locked_keeping_unknown` 遇到不认识的 `format_version` 整份拒绝，降级或设置应用和输入法版本不一致时会读不了偏好。新字段走[未知字段保留](../../implemented/architecture/2026-10-15-preferences-keep-unknown-fields.md)，旧版本读写时原样留着它。

### 设置同步与导入

- 账号同步只映射 `input.smart_punctuation`（`crates/client-services/src/account/settings_sync.rs`），桌面上上传和应用都是用户手动做的（Windows 的「设置同步」、macOS 的预览加确认）。上传读的是迁移后的值（`store.load()`），所以迁移后的设备上传「开」。默认改变之前上传的快照里可能是旧默认的「关」，用户手动应用它时照快照生效，迁移标记留在本机，之后不会再被改回开：同步和导入是用户自己挑来覆盖本机的值，一份旧快照或旧设置文件里的 `false` 也可能是在 Linux、手机上有意关掉的，分不出来，所以不迁移。
- macOS 原生的云端快照（`platform.macos.smart_punctuation`、`platform.macos.smart_punctuation_repeat`）以前取 NSUserDefaults 里的值，那里可能还留着以前写下的「关」，而宿主实际用的是共享文档里迁移后的「开」。现在 `cloudSettingsSnapshot` 导出 `self.smartPunctuation`、`self.smartPunctuationRepeatToChinese`，即宿主实际在用的值，新上传的快照不会把迁移冲掉。

### 界面

- 共享 `PunctuationSection` 的主流程剩三行：「标点」选择、引号样式、成对标点自动补全；智能标点总开关挪进「更多选项」，排在方头括号样式之后、四个细分开关之前。它默认开，用户不必先找到它，放在折叠区里和细分开关挨在一起。总开关连同两半写、关着时细分开关变灰的规则不变。
- HarmonyOS 手机的 `PhonePunctuationSection` 同样把智能标点收进「更多选项」，主流程只剩「使用英文标点」和「自动补全成对标点」。
- Windows 原生设置页 `build_expression_page` 把「智能标点」行挪进同一个 `Expander`；搜索落在「智能标点」时展开它（`focus_row_ == L"智能标点"`），与折叠区里其他行一样。
- 搜索：共享设置页的搜索只按页名过滤（`shownSidebarGroups`），搜「标点」照样到「标点与翻译」页，挪进折叠区不改变能否搜到；Windows 原生页的搜索按行建索引（`record`），挪进折叠区后照样搜得到并自动展开。
- 「总开关开、两半都关」时的提示改说「打开下面其中一项，或关掉再打开智能标点」，因为总开关和两半现在在同一个折叠区里。

## 为什么默认开

默认关最强的理由有两条，都来自来源产品的考虑：

1. 它改写一个用户已经看见落下去的字符（或者说，按下 `.` 的结果取决于前一个字符，用户可能没预料到）。
2. 句末恰好是数字时（「共计 100。」），句号也会变成英文的 `.`。

回答是重复标点转中文：输出 ASCII 之后两秒内再按同一个键，就把它换成中文标点（`punctuation::should_replace_repeat`，各宿主的 `msime_client_smart_punctuation_arm`/`decide` 或原生实现）。「100。」只多按一下句点；它和智能标点一起默认开，所以退路和行为同时到位。反过来，默认关时数字后的 `.` 被写成「。」，用户除了退格重打、切英文模式或找到开关之外没有退路，而 `0.10.1`、`3.14`、`12:30`、`v2.0` 这类输入比句末数字常见得多。

## Historical audit

按 `smart_punctuation`、`智能标点`、`config.default.toml`、`smart_punctuation_default` 检索活跃笔记：

- [「标点与翻译」页精简](../../implemented/simplification/2026-10-16-expression-page-prune.md)：部分被取代。它把智能标点总开关留在主流程（四行），并以「macOS 和 Windows 新配置上三个都是关」为背景；总开关的位置和默认值由本篇接管，已在那篇注明。总开关连同两半写、细分开关变灰、同步时两半跟着写等决定仍然成立。
- [共享标点偏好的叶子类型](../../implemented/simplification/2026-10-15-punctuation-preferences.md)：事实更新。缺键时默认开启的可选开关多了智能标点、重复标点转中文和两种直出，已就地改。
- [偏好文档保留未知字段](../../implemented/architecture/2026-10-15-preferences-keep-unknown-fields.md)：部分重叠。迁移标记靠它在旧版本保存时不丢失。
- [大写锁定标点](../../implemented/feature/2026-10-09-caps-lock-ascii-punctuation.md)、[Fcitx5 会话偏好](../../implemented/bug-fix/2026-10-10-fcitx5-session-preferences-from-store.md)、[Windows 数字选词与 HUD](../../implemented/bug-fix/2026-10-11-windows-digit-commit-and-hud-anchor.md)、[HarmonyOS 删除光标前文字](../../implemented/bug-fix/2026-10-09-harmony-delete-before-caret.md)、[中英文之间加空格](../../implemented/feature/2026-10-12-chinese-english-spacing.md)、[货币标点宽度](../../implemented/bug-fix/2026-10-15-currency-punctuation-character-width.md)、[引号与括号样式](../../implemented/feature/2026-10-13-punctuation-mark-styles.md)、[Android 字母提示中文标点](../../implemented/feature/2026-10-12-android-letter-hint-chinese-punctuation.md)、[九键符号键](../../implemented/feature/2026-10-15-nine-key-symbol-key-and-quick-marks.md)：只用到智能标点的路线或开关，与默认值无关。
- 「默认关、跟随来源」的决定没有自己的笔记（见 Problem），由本篇取代。

## Alternatives considered

- **保持默认关（跟随来源和安装模板）。** 最强的理由是上面那两条：改写刚落下的字符，以及「100。」。没有采用：重复标点转中文让「100。」只多按一下，而默认关时数字后的点被写成「。」没有对应的退路；而且这是用户最常遇到的场景之一，不该以找到一个开关为前提。来源的模板是 Windows 安装包随附的样板，运行中的宿主从不读它，本仓的模板与共享默认一起改。
- **只改默认值，不迁移旧文档。** 最强的理由是「已存的值从不重新解释」，这是旧注释和[「标点与翻译」页精简](../../implemented/simplification/2026-10-16-expression-page-prune.md)反复坚持的原则，迁移会打开一部分有意关掉它的用户的开关。没有采用：旧文档里的 `false` 绝大多数是从没动过的默认值，不迁移的话老用户升级后仍是 `0。10。1`，这次改动只对新装生效；有意关掉的人再关一次就永久生效。
- **升 `format_version` 做迁移。** 最强的理由是版本号就是为格式迁移准备的，不需要新字段。没有采用：`read_locked_keeping_unknown` 遇到不认识的版本整份拒绝，设置应用、MCP 或降级后的输入法比文档旧时会读不了偏好；一个只给一个开关用的字段经未知字段保留就能跨版本存活。
- **迁移时立即写盘。** 最强的理由是文件马上就是迁移后的样子，同步、导出、外部工具看到的都一样。没有采用：读取入口（`load`、`try_load`）不该写盘，宿主每秒轮询读文档；写盘要动修订号，所有宿主随之重新载入；而外部读者本来就都经过这同一个读取入口。读入时迁移、下次保存落盘与 `retire_unavailable_asr_model` 一致。
- **在 `route()` 里把「总开关开、两半从未设过」当作开。** [「标点与翻译」页精简](../../implemented/simplification/2026-10-16-expression-page-prune.md)已经否过：Rust 分不出「从未设过」和「用户关掉」，现在仍然如此。迁移标记正是为这个区分补上的唯一信息，而且只用一次。
- **同步应用旧快照时也迁移（快照里 `input.smart_punctuation` 为假就忽略）。** 最强的理由是一份迁移前上传的快照会把刚迁移的设备拉回关。没有采用：账号文档没有版本或平台标记，分不出旧默认和有意关掉，忽略它会让在别的设备上真关掉的人同步不过来；桌面上的应用是带预览的手动操作，用户看得到这一项。
- **空格转换也默认开。** 没有采用：它改写已落下的字符，又没有反悔手势，默认关的理由对它完全成立。
- **由各宿主在偏好热更新时清除空格转换快照。** 可以让宿主立即释放旧状态，但要求每个调用方都补一份相同规则，遗漏时仍会改写已经关闭的功能。Host API 自己持有生效偏好，在执行入口统一拒绝旧快照，宿主继续只负责编辑器上下文。
- **由宿主在切换方案时清除重复标点快照。** 可以阻止旧方案的手势流入新方案，但宿主需要识别共享核心实际采用的方案（请求可能因词库缺失而回退），并且所有调用方都要各守一遍。Host API 在执行时直接检查实际方案，保存与执行共用一处谓词，避免两条路径再次分歧。

## Consequences

- **收益**：
  - 所有平台的新装和 macOS、Windows 上没动过开关的老用户，数字和字母后的 `,` `.` `:` 直接是英文，`0.10.1` 打出来就是 `0.10.1`。
  - 句末数字只多按一次句点就得到「。」。
  - 设置页主流程少一行，智能标点和它的细分开关在同一个折叠区里。
- **代价与已知上限**：
  - macOS、Windows 上默认改变之前有意关掉智能标点的用户，升级后会被打开一次，需要再关一次；之后一直关着。
  - 迁移前上传的账号快照或导出的设置文件里如果是旧默认的「关」，用户手动应用或导入时会关掉智能标点，迁移不会再来；只有一次手动操作的距离，但界面不提示这一点。
  - 文件在下一次真正的保存之前仍是旧样子，直接读 `preferences.json` 的外部工具看到的是迁移前的值；仓库里的读者都经过 `PreferencesStore`。
  - Android、iOS 原生设置页的智能标点总开关仍在主流程，本篇只改了共享页、HarmonyOS 手机分组和 Windows 原生页。
  - 再有平台改智能标点默认值时，迁移标记已经用掉，要另立标记。

## Verification

- `msime-host-api::smart_punctuation_space_decision_rechecks_live_switches`：真实上屏「。」并保存快照，经公开偏好更新接口分别保持开启、关闭总开关、关闭空格转换开关，再传回同一份有效快照。旧实现关闭总开关后仍返回 ASCII `46`；修复后只有两项都开启时允许转换。
- `msime-host-api::the_repeat_gesture_rechecks_the_scheme_after_a_preferences_update`：全拼数字后的句点交给宿主按 ASCII 上屏并保存快照，公开偏好更新接口切换到全拼、双拼、五笔、日文、韩文、注音、越南文、藏文后传回同一快照。注音使用合成词库，逐项断言实际方案与更新非延后；旧实现切到韩文仍返回「。」，修复后仅原本支持重复手势的方案允许替换。
- `msime-client-core`（Studio，macOS，迁移会翻转的平台）：`preferences::tests::smart_punctuation_defaults_on_everywhere`、`smart_punctuation_settle_flips_only_where_the_old_default_was_off`（两种平台参数都测）、`smart_punctuation_migration_runs_once_and_a_later_off_sticks`（旧文档读入时打开、文件不变、写入方漏掉标记照样落盘、关掉后再读仍关、恢复默认回到开）、`smart_punctuation_turned_off_on_a_fresh_profile_stays_off`、`smart_punctuation_recovery_migrates_only_a_legacy_document`；`punctuation::tests::the_shipped_defaults_do_what_the_switch_says_they_do`（默认偏好下数字和字母后的 `,` `.` `:` 走 ASCII，中文、空格、中文标点和无前文后归 Engine，重复按句点换回「。」）；`settings_document::tests::an_old_file_keeps_its_smart_punctuation_and_this_machine_s_marker`。全部 395 项通过。
- `msime-host-api`：`default_preferences_keep_ascii_after_a_digit_and_repeat_turns_it_chinese` 经 `msime_client_punctuation_with_context` 和 `msime_client_smart_punctuation_arm`/`decide` 验证默认偏好下数字后三个标点交给宿主出 ASCII、「计」后出「，。：」、ASCII 句点后再按句点换成「。」；`a_legacy_preferences_document_loads_with_smart_punctuation_settled` 经 `msime_client_load_preferences` 验证旧文档读出时迁移。645 项通过。
- `msime-client-services`：`settings_sync_after_the_smart_punctuation_migration` 验证迁移后上传「开」、手动应用旧快照后关掉且再读不被改回。873 项通过。`msime-mcp-server` 全部通过。四个 crate 的 clippy `-D warnings` 无告警。
- `apps/desktop` vitest：智能标点默认显示为开、总开关和细分开关同在「更多选项」、主流程顺序、关掉时细分开关变灰、手机分组同样收进折叠区、整页保存写入关闭的补丁。
- `scripts/test-default-config-parity.py` 对照安装模板和 Rust 默认；`scripts/test-preferences-field-parity.py` 和 `platforms/macos/tests/settings/preference_coverage.py` 记下迁移标记不是设置。
- macOS 原生：`shortcut`（默认开、回到未存状态后控制器按默认走带前文的标点入口，数字后的句点前文取自客户端）、`smart-punctuation-space`、`cloud-appearance-settings`。
- `TestRealSessionComposition` 验证普通中文标点开关时，会先提交原文 `U4e2d`；智能标点默认开启后，末尾字母使逗号正确走 ASCII，原来期待中文逗号的断言因夹具缺少显式偏好而失败。该用例在会话与控制器中都显式关闭智能标点，保留中文标点开关的两条断言；默认开启、数字与字母后的 ASCII 路线继续由独立的智能标点用例覆盖。
- 整合夹具修正后，macOS 本机 `ctest --test-dir target/macos-isolated -R '^(shortcut|smart-punctuation-space|smart-punctuation-shadow)$' --output-on-failure` 三项全部通过，`bash scripts/verify-local.sh --quick` 通过；未安装替换系统输入法。
- WinUI 设置页和 Windows Server 的改动本机无法编译，由 CI 编译；没有在真实输入法里装机验证。
