# Agent Note: 偏好文档读不了时输入运行时照常运行，枚举列表读入放宽

Status: implemented

## Problem

10-26 到 10-27 有很多人遇到同一个问题：键盘上显示「输入运行时准备失败：text edit button order must contain each of the seven side actio…」，布局是乱的，也打不了字。

- **起因**：#9802（764faf08a9，10-26 14:07 JST）给文本编辑面板加了「选择行」，默认排列从七个动作变成八个。`Preferences::validate` 仍要求排列恰好包含全部动作，错误文案也还写着 seven。以前的版本保存的 preferences.json 只列七个动作，于是整份文档通不过校验。
- **为什么打不了字**：
  - 宿主准备运行时要读偏好（host-api 的 `edition_preferences` 调 `write_first_document`），读失败时准备直接失败。iOS 键盘每次启动都要准备，失败就显示上面那行字，没有会话。HarmonyOS 的 `start()` 也一样。Windows 的 Server 准备失败就退出，#9975 的提交里记着 Win7 v0.3.0 装在旧数据目录上实测复现。
  - 不重新准备的宿主（macOS、Android、Linux）也逃不掉：`create_session` 要校验运行时选项里的偏好副本，副本是以前的版本准备时写下的七个动作。
- **坏窗口**：
  - nightly run 40411840275 构建的是 060b72974d，含 #9802、不含 #9975。iOS 这一版送了 TestFlight 外部测试组（nightly.yml 的 `testflight_external: true`），macOS 更新了 Sparkle 的每日构建通道，HarmonyOS 和 Windows 的产物进了滚动发布的 `nightly` release。Android 和 Linux 的子 workflow 失败了，没有分发。
  - 正式版里带上这个 bug 的只有 Windows 7 v0.3.0（windows7-v0.3.0，10-26 20:56 JST）。
  - #9975（e734968d23，10-27 08:25 JST）在读入时补上缺的动作，下一次 nightly（18926f387c）已经含修复。但 TestFlight 外部测试者要等 Apple 审完修好的那版，期间可能一直停在坏的版本上。
  - 新装用户不受影响，受影响的是坏窗口里升级、而且已经有 preferences.json 的用户。

#9975 只修了这一个字段，同一类问题在 develop 上还在：

1. 一个只管外观、可有可无的偏好字段通不过，整份偏好文档就读不了。
2. 偏好文档读不了，输入运行时就起不来，用户完全打不了字。

具体还剩这些情况：

- 更新的版本写下了本版本不认识的取值，serde 报 `unknown variant`。降级、从每日构建或 TestFlight 退回正式版、macOS 输入法由 Sparkle 单独更新而设置应用没跟上、手工拷贝数据目录，都会遇到。
  - android-v0.6.0、harmony-v0.3.0-beta、macos-v0.55.0、linux-v0.14.0 都有 `touch_text_edit_button_order` 这个字段，却不认识 `select_line`，读到现在的版本写的文档就会整份读不了。
  - 触屏方案的枚举几天里加了十八键、粤语双拼等好几个值，用户启用过的话，没有这些值的版本同样读不了。
- 排列里有重复的动作，也让整份文档被拒。
- 其它约五十个标量枚举字段、`validate` 里的白名单和数值范围，同样是一个键坏了整份读不了。

## Decision

规则分两层：读盘放宽，提交严格；读不了的文件不改写，但运行时不因此停下。

### 1. 枚举列表读入放宽

- **`touch_text_edit_button_order`**：`deserialize_text_edit_button_order` 先把列表读成 JSON 值。
  - 认识的动作保留第一次出现的位置。
  - 不认识的名字、重复的动作、不是字符串的元素都丢掉。
  - 缺的动作按默认先后接在末尾。
  - 只有不是数组时才报错。
  - 丢掉的动作不需要写回：它只决定按钮先后，更新的版本读回去时会把它补在末尾。
- **`touch_keyboard_schemes`**：`StoredTouchKeyboardScheme` 多了一个 untagged 的 `Newer(String)`。
  - 启用列表里不认识的方案丢掉。
  - 只剩退役方案或不认识的方案时，回到全拼 26 键。
  - 选中的是不认识的方案时当作没选，退役的 `thoughtful_reply` 仍迁移到第一个启用的方案。
- **`fuzzy_pinyin.rules`**：经 `deserialize_known_elements` 读入，只留认识的规则。
- **`validate()` 不变**：直接交给它的结构体照旧要求完整、不重复的排列。这与 #9975 的分工一致：反序列化宽松，结构体校验严格。

### 2. 读入时丢掉的取值，保存时接回

只在 `PreferencesStore` 读写磁盘时做。

- **记下**：`newer_list_values` 从盘上文档里找出两类取值，记成 `UnknownPreferenceField { elements: true }`：
  - `NEWER_VALUE_LISTS` 里两个列表（触屏方案的启用列表、模糊音规则）中不认识的字符串；
  - 选中的那个不认识的方案。
- **接回**：`save` 时由 `restore_unknown_fields` 把记下的元素接到本版本写出的同一个列表末尾。选中项按未知键的规则放回：本版本选了别的方案时以本版本为准。
- **不接回的情况**：
  - 退役的取值；
  - 「恢复默认设置」，与未知键的处理相同。
- **为什么只收这两个列表**：
  - 它们是用户的选择，丢了就找不回来；
  - 文档总会写出这两个列表，接回时列表一定在。
  - `multilingual_input.languages` 不收：它所在的段落等于默认值时不写出，接不回去。它交给第 3 节兜底。
- **设置文件导入**：`settings_document::import_document` 遇到这些取值时拒绝整份文件，与它拒绝未知键的理由相同——文件出自更新的版本，只导入认识的部分会悄悄丢掉其余设置。文本编辑排列里不认识的动作照常导入，丢掉的只是按钮位置。

### 3. 偏好读不了时，输入运行时用内存里救下的设置运行

- **`PreferencesStore::load_for_runtime()`**：
  - `load` 读得了就返回它的结果。
  - 读不了、而且不是存储故障时，用 `Preferences::salvaged_from` 逐键救下本版本认得的设置，其余取本存储所属版本的默认值。逐键取舍与 `recover` 相同。
  - 修订号沿用文档里的值，读不出时为 0。
  - 结果放在 `RuntimePreferences.load_error` 里交出，同时写明为什么读不了。
  - 只在内存里，不写盘。
- **host-api 有两处改用它**，所有平台都经过这两处：
  - `edition_preferences`（准备宿主时）：`write_first_document` 失败、又不是 I/O 错误时改用它。没有文件时照旧写第一份。
  - `HostOptions::from_document`：副本解析不了或者通不过 `validate`，都改用实时文档。实时文档也读不了时救它；没有实时文档时救副本自己的；副本本身没问题、是文档别处不对时，原样交回副本。
- **报告**：两处都经 `diagnostics::report` 报告（`preferences document unreadable`、`runtime options preferences unusable`）。
- **文件为什么安全**：
  - 读不了的文件仍然不自动改写，`recover_malformed` 只修不是 JSON 的文件，这条不变。
  - `save` 在锁里会重新严格读一遍，文档读不了时，任何宿主的保存都写不进去。
  - 设置界面经 `load` / `try_load` 读，看到的仍是错误，修不修复由用户在设置页决定（`msime_client_repair_preferences`）。

### 4. 防止再犯的测试

- **已发布版本的样本**：`crates/client-core/tests/fixtures/released-preferences/` 放着六个已发布版本第一次保存时写下的 preferences.json，是在各自的发布标签上运行 `PreferencesStore::save(0, Preferences::default())` 生成的：android-v0.6.0、harmony-v0.3.0-beta、ios-v0.52.0、linux-v0.14.0、macos-v0.55.0、windows7-v0.3.0。`documents_written_by_released_versions_load_and_save` 要求当前版本严格读得了它们、校验通过、改一项后存得下。#9802 那样新版本校验更严的改动会在这里失败。以后发版时补上新的标签。
- **未来取值**：`every_string_setting_with_a_future_value_still_runs` 把默认文档里每一个字符串取值逐个换成 `future_value`，`load_for_runtime` 都必须成功、结果通过校验、文件字节不变。新加一个只认几个取值的字段，会自动被覆盖到。

## Alternatives considered

- **`validate` 改为接受缺项、重复项。** 最省事，一处改动就能让所有旧文档读得进来。但设置页提交残缺排列也会被放过，宿主界面的错误不再暴露。#9975 已经否掉过这个方案，这里沿用它的分工。
- **读不了时自动用救下的设置改写文件。** 用户什么也不用做，下次读取就正常了。但读不了的文档多半出自更新的版本，悄悄改写会丢掉那个版本的设置。`recover_malformed` 正是为此只修不是 JSON 的文件，自动修复和显式修复的区分（`msime_client.h`、`ffi/host.rs`）也是为此。在内存里降级运行，不碰文件，就足以让用户继续打字。
- **给每个枚举加 `#[serde(other)]` 兜底变体。** 粒度最细，每个字段遇到新值都有确定的回退。但约五十个字段都要加一个本版本不可能选中的变体，每个 `match` 都得处理它，新加的枚举还容易漏。第 3 节在整份文档的层面兜底，自动覆盖以后新加的字段，再由未来取值测试把它锁住。
- **多语言列表也放宽并写回。** 它所在的段落等于默认值时不写出（`skip_serializing_if = "MultilingualInputPreferences::is_default"`），丢掉一个新语言后，段落可能正好等于默认值而不写出，写回无处可放，结果就是悄悄丢掉用户的选择。不如让它整份读不了，交给运行时兜底，设置页不能保存，也就不会丢。
- **在 `HostOptions` 里加一个字段，把读不了的原因带给键盘界面。** 键盘能直接显示一行提示。但 `HostOptions` 拒绝未知键，旧版本（macOS 不跟输入法一起更新的设置应用、降级）读到带这个键的 runtime-options.json 会整份拒绝，正好在版本错开、最需要它的时候把问题放大。先只走诊断出口，各平台键盘要不要显示提示另行决定。
- **`touch_text_edit_button_order` 等于默认值时不写出。** 只有这个办法能保护已经发出去的版本：Android 0.6.0、HarmonyOS 0.3.0-beta、macOS 0.55.0 和 Linux 0.14.0 读得了用户没动过排列的新文档。但这是磁盘格式的改动，Android 的 `TextEditButtonOrder.java` 对默认偏好取这个键时，键不在会抛异常，必须同时改；还要先由项目负责人决定。本次不做。

## Consequences

- **收益**：
  - 偏好文档里任何一个键读不了，都不再让用户打不了字：iOS 键盘、HarmonyOS、Windows 的 Server 能准备运行时，macOS、Android、Linux 能建会话。
  - 文本编辑排列、触屏方案、模糊音规则遇到更新的版本写的取值，直接读得进来；方案和规则保存后还在，更新的版本读回时用户的选择不丢。
  - 已发布版本的样本和未来取值测试，会把「新版本校验更严」和「新加只认几个取值的字段」这两类问题拦在 CI 上。
- **代价**：
  - 降级运行时，用户看到的是部分设置回到默认。键盘界面目前没有提示，只有诊断日志和设置页的读取错误能说明原因。
  - 设置页在用户修复或升级之前存不了设置。
  - 救一次偏好要对每个顶层键做一次反序列化和校验，只在失败路径上发生，但在这个状态下每建一次会话都会重做一次。
  - 文本编辑排列里不认识的动作在本版本保存后丢掉位置；导入新版本导出的文件时也一样。
- **已知上限**：
  - 已经发出去的版本照旧严格。降级到 0.6.0 之类的版本时，它们仍读不了现在的文档，只能靠上面那条「不写出默认排列」，或者用户手动修复。
  - 除韩文布局、文本编辑排列、触屏方案和模糊音规则之外，其它字段的新取值仍让整份文档严格读不了，只是不再挡住打字。

与 [偏好文档保留未知键](../architecture/2026-10-15-preferences-keep-unknown-fields.md) 部分重叠：它的 Consequences 里写着「新版本给枚举加了新取值，旧版本照样读不了整份文档，本次不处理」，这件事由本篇接手，那篇的事实已经同步更新。[文本编辑面板选择当前行](../feature/2026-10-26-android-text-edit-select-line.md) 里「重复或未知的动作仍然拒绝」已改为本篇的读法。

## Verification

- 文本编辑排列：
  - `text_edit_button_order_reads_any_list_it_is_given`，从文件读入以下几种排列，结果都是完整排列、文件不被改写：v0.6.0 之前写下的七个动作、现在的八个、带更新版本动作 `select_word` 的、有重复的、空列表、不是字符串的元素。
  - 不是数组时 `load` 报错，`load_for_runtime` 用默认排列运行。
  - `text_edit_button_order_rejects_missing_duplicate_and_unknown_actions` 里结构体提交的严格校验不变。
- 写回：
  - `newer_list_values_survive_a_save`：更新版本的方案、选中项和模糊音规则在本版本保存后接回，退役方案和不认识的动作不接回；本版本选了自己的方案时以本版本为准；恢复默认后全部清掉；启用列表只剩新方案时回到全拼 26 键并接回新方案。
  - `touch_keyboard_scheme_visibility_matches_apple_order_and_fallback_contract` 原来断言 `future_scheme` 读失败，现在断言读得进来、文件不变。
  - `fuzzy_pinyin_preserves_disabled_rules_and_reads_past_unknown_ids`（原 `..._rejects_unknown_ids`）原来断言不认识的规则名让反序列化失败，现在断言它被丢掉，不是字符串的元素仍然报错。
- 运行时降级：
  - `load_for_runtime_keeps_typing_on_unreadable_documents`：更新版本的标量枚举、通不过校验的值、更新的格式版本、不是 JSON 的字节，都能运行。读不了的键取默认值，其余设置照旧，修订号沿用文档里的值。文件不变，`save` 仍被拒绝。
  - `every_string_setting_with_a_future_value_still_runs` 覆盖所有字符串取值。
- 往返：`develop_documents_round_trip_through_the_tolerant_reader` 确认本版本写下的文档经宽松读法往返后取值逐个一致，没有改动的保存不前进修订号、不改写文件。
- 导入：
  - `a_newer_or_damaged_document_is_refused_rather_than_partly_applied` 增加了带更新版本方案、规则的文件，结果是整份拒绝。
  - `a_text_edit_order_from_a_newer_version_is_imported_without_its_unknown_action` 确认排列里的新动作丢掉、其余照常导入。
- 设置同步：`desktop_settings_sync_works_on_a_document_from_a_newer_version`，本机文档带不认识的动作、重复的动作和新方案时照常上传和应用，写回后新方案还在。
- host-api：
  - `preparing_over_an_unreadable_preferences_document_still_types`：分别放入校验更严的值、新的枚举取值、类型不对的列表元素，`msime_client_load_preferences` 都报错，准备宿主都成功，建会话后能打字，文件不变；用户点修复后严格读取恢复正常；事故里那份七个动作的文档直接读得了。
  - `session_reads_live_preferences_when_the_options_copy_is_refused` 改为：没有实时文档时救下副本照常建会话；副本解析得了却通不过校验时同样不再拒绝。
- 命令与结果（在 Mac Studio 上经 `rbuild` 运行）：
  - `cargo test --no-fail-fast -p msime-client-core -p msime-client-services -p msime-host-api`：client-core 和 client-services 全部通过。host-api 有三项失败，都与本改动无关：
    - `listing_reports_every_pack_and_checks_the_state_root`（列表里多了 `pronunciations`）和 `a_command_gets_the_local_time_to_the_minute`，在未改动的 616bd2888e 上单独运行同样失败；
    - `translation_queries_never_clear_a_mixed_japanese_row_for_the_network` 只在整套并行时失败，单独运行通过。
  - `cargo clippy --keep-going -p msime-client-core -p msime-client-services -p msime-host-api --all-targets -- -D warnings`：本次改动没有新的告警。剩下两处 `field_reassign_with_default`（`preferences/tests.rs` 的 `android_keyboard_skin_slots_round_trip_without_changing_legacy_theme`、`account/settings_sync/tests.rs:1338`）在 616bd2888e 上已经存在。
  - `cargo fmt --check`、`bash scripts/run-checks.sh`、`platforms/macos/tests/settings/preference_coverage.py`、`pnpm run verify-notes` 通过。
