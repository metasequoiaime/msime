# Agent Note: 偏好文档容忍并保留本版本不认识的键

Status: implemented

## Problem

偏好文档 `preferences.json` 的结构（`Preferences` 及其下 26 个段落结构）都带 `deny_unknown_fields`，读到不认识的键就整份拒绝。以往加新顶层字段时的取舍是「正式包里输入法和设置应用版本一致」（见 [logo 开关](../feature/2026-10-08-macos-app-logo-toggle.md) 及之后十几篇沿用它的笔记），这个前提不成立：macOS 的输入法本体由 Sparkle 单独更新（`platforms/macos/package-release.sh`），设置应用 `MSIME.app` 和其中的 `msime-mcp` 是另一个 bundle，按 bundle id 找到（`DesktopSettingsLauncher.h`），不跟着更新。

实际发生过：输入法更新到写 `show_candidate_expand_button` 的版本后，0.53.0 的设置应用读不进偏好文档，设置页改什么都存不下来，界面上看不出原因；`msime-mcp` 的 `get_preferences` 直接报 `unknown field`。反方向（旧输入法读新设置应用写的文档）会让输入法开不了会话。Windows 宿主写了一个设置页不认识的键时，整个设置窗口同样一项都存不了（`smart_punctuation_sub_switches_survive_a_save` 的注释记着那次事故）。

## Decision

- 偏好文档里的结构不再带 `deny_unknown_fields`，读到不认识的键时忽略它。外层信封 `PreferencesSnapshot`（`format_version`、`revision`、`preferences`）仍然严格，信封格式变化由 `format_version` 管。
- `PreferencesStore` 读文档时用 `serde_ignored`（`deserialize_keeping_unknown`）记下 `preferences` 之下本版本不认识的键路径和原值（`UnknownPreferenceField`）；`save` 在同一把锁里读出它们，写入时由 `restore_unknown_fields` 放回原处。于是旧版本保存不会抹掉新版本的设置。
- 放回的规则：
  - 只按对象键定位，经过数组元素的路径不收集，因为数组整个属于一个已知字段。
  - 父对象这次因取默认值没写出时不放，不凭一个未知键重建半个已知对象。
  - 本版本已写出同名键时以本版本为准。
- 「恢复默认设置」（`restore_defaults`）不放回未知键：用户要的是整份回到默认。第一次写文件（`write_first_document`）没有未知键。
- Host API 的 `HostOptions::from_document` 不变：运行时选项里的偏好副本读得进就用副本，读不进才由实时的 `preferences.json` 顶替。带退役键的副本（#2830 的 `autocorrect`）现在自己就读得进，不再走顶替。
- 设置文件导入（`settings_document::import_document`）仍然拒绝带未知设置的文件：它出自更新的版本，只导入认识的部分会悄悄丢掉其余设置，所以返回 `settings_document_unsupported`，行为与以前相同。

### 保存失败要看得见

同一次事故里，设置页的保存一直失败，用户却只觉得「没有保存按钮，也没有自动保存」：保存状态只写在表单最后一行的 12px 灰色小字里（`SettingsActionsFooter`），这一行不随滚动固定，页面一长就要滚到底才看得见。现在自动保存失败时由页面顶部的 `SettingsStatusMessages` 显示「设置没有保存：原因」和「重试」，与读取失败的提示在同一处；表单最后一行在失败时不再重复同一个错误和按钮，只保留「重新读取」，「正在保存…」「已保存」仍留在那里。

## Alternatives considered

- **保持严格，靠版本一致。** 最强的理由是任何拼写错误、残留键都会立刻暴露，文档永远干净。但 Sparkle 只更新输入法，正式用户的版本本来就会错开，失败表现是设置页静默存不下来，比多留一个键严重得多。
- **每个结构加 `#[serde(flatten)] extra: Map` 收集未知键。** 未知键跟着 `Preferences` 走，任何读写路径都自动保留。但 26 个结构里有 10 个是 `Copy`（`KeybindingPreferences`、`LocalModePreferences` 等），加 map 字段要改掉整个工作区里按值复制它们的地方，每个结构字面量也要补字段；存储层只在 `save` 一处需要这些键，在那里处理影响面小得多。
- **把表单最后一行改成固定在窗口底部。** 保存状态无论滚到哪里都看得见，但六个平台、手机和桌面的布局都要跟着改，正常保存时也多一条常驻的底栏；失败才需要打断用户，放在页面顶部已有的错误区就够了。
- **不加依赖，拿原始 JSON 和本版本序列化结果比对找未知键。** 很多字段取默认值时不写出（`skip_serializing_if`），比对会把它们误判成未知键并一直保留，用户再也恢复不了默认值；`serde_ignored` 在反序列化时直接拿到被忽略的路径，没有这个歧义。

## Consequences

[可选罗马字布局](../feature/2026-10-23-korean-romaja-layout.md)保留未知偏好字段，仅在磁盘读取时把未知布局字符串回退到两套式；设置提交仍严格校验。

- **收益**：输入法和设置应用、`msime-mcp`、各平台宿主之间版本错开时，新增键不再让任何一方整份拒读，旧版本保存也不丢新版本的设置。以后新增偏好字段，不必再为旧版本拒读而权衡放在哪一层、默认时写不写出。
- **代价与已知上限**：
  - 拼错或退役的键不再报错，会作为未知键一直留在文档里。
  - 新版本给枚举加了新取值（例如新的输入方案名、新的键位名）时，旧版本照样读不了整份文档，本次不处理。
  - 带自定义反序列化、先读成别的中间形状的字段，内部的未知键是否被记下，取决于中间形状是否经过同一个反序列化器；`#[serde(from = ...)]` 会经过。
  - 只有经由 `PreferencesStore` 的写入保留未知键：Linux 的 `runtime-options.json` 副本、导出的设置文件都只含本版本认识的键。
  - 0.53.0 及更早已发出的版本仍然严格，只有带上这次改动的版本之间才互相容忍。

与 [logo 开关](../feature/2026-10-08-macos-app-logo-toggle.md) 部分重叠：它记的「早于这个字段的设置应用会拒读」对带上本次改动的版本不再成立。[Windows 设置文件](../feature/2026-10-10-windows-settings-file-and-local-data-parity.md) 的导入规则不变。

## Verification

- `settings_from_a_newer_build_are_read_and_kept_through_a_save`：在默认文档的每个对象里试放一个新键，任何结构报 `unknown field` 即失败（它找出了漏掉的 `StoredTouchKeyboardSchemePreferences`）；把所有结构里都放上新键后能读、改一项再保存后每个新键原样还在，恢复默认后全部清掉。`an_unknown_key_goes_back_only_into_a_written_section` 覆盖放回规则。
- 原来断言拒绝未知键的测试改为断言记为未知键；同一测试里拒绝未知取值的断言保留。导入拒绝更新版本文件的 `a_newer_or_damaged_document_is_refused_rather_than_partly_applied` 不改并通过。
- `cargo test -p msime-client-core`：1436 项通过；`recover_suffixes_a_backup_name_that_is_taken` 在整套并行、机器满载时失败一次，它要两次备份落在同一秒内才撞名，单独连跑 3 次都通过，与本改动无关。`cargo test -p msime-host-api -p msime-mcp-server` 715 项通过，其中 `session_reads_live_preferences_when_the_options_copy_is_refused` 改为：带退役键的副本自己能建会话，类型不对的副本才由实时偏好顶替。client-core 与 host-api 的 Clippy（全部 target、警告视为错误）和 Rustfmt 通过。`msime-desktop` 没有在本地编译测试，交给 CI。
- 合约脚本：`test-default-config-parity.py` 的正则不再要求 `FuzzyPinyinPreferences` 上有 `#[serde(deny_unknown_fields)]`；`test-preferences-field-parity.py` 仍检查设置页能写而 Rust 没有字段的键，后果改述为「留在文档里但永远不生效」。`bash scripts/run-checks.sh` 全部通过。
- 设置页：新增 `settings-save-failure-visible.test.tsx`，保存被拒后顶部出现原因和「重试」，位置在表单之前，点「重试」再次保存；保存成功时顶部不出现。原有 `settings.test.tsx` 里失败重试和反复冲突两项改为只有一处「重试」、一处提示后通过。`apps/desktop` 的 `tsc --noEmit`、全部 3591 项 vitest、`format:check` 与 `lint` 通过。
