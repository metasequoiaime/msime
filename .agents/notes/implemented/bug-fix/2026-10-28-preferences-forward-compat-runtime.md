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

规则分两层：读盘放宽，提交严格；读不了的文件不改写，但运行时不因此停下。另外，本版本写下的文档尽量让已经发出去的旧版本也读得了。

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
- **提交来的 JSON 不放宽**：宽松读法写在 serde 上，经 FFI 或 Tauri 提交保存的 JSON 也走同一套反序列化，不另加一层的话，重复、缺失、不认识的动作会被悄悄整理成合法排列，不认识的方案和规则会被悄悄丢掉，接口照样返回成功。提交方出自本版本，名字和 Rust 对不上是它的毛病，要从「保存失败」暴露出来，不能变成「改了却不生效」。所以新增 `Preferences::from_submission`：
  - `newer_list_values` 在提交内容里找到任何不认识的方案、选中项或规则，就按 JSON 错误拒绝；
  - 排列先按 `Vec<TextEditButton>` 严格解析，再和宽松读法的结果比较，不一致（重复、缺项）就是 `InvalidTextEditButtonOrder`；
  - 用在 `msime_client_save_preferences`（共享设置页、Windows/Linux 的 C++ 保存、HarmonyOS 的 `importPreferences` 都走这里）和 Tauri 的两个 `save_preferences` 命令上，命令参数从 `Preferences` 改成 `serde_json::Value`。
  - 会话更新（`msime_client_update_preferences`）不是写入，宿主交来的是 Rust 刚读出的快照，仍走宽松读法。

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

### 3. 默认排列不写进文件

`touch_text_edit_button_order` 等于默认排列时，`PreferencesStore` 写盘（`save`、`recover`，经 `document_for_disk`）不写出这个键。

- 已经发出去的 Android 0.6.0、HarmonyOS 0.3.0-beta、macOS 0.55.0 和 Linux 0.14.0 只认识七个动作，读到带 `select_line` 的排列就整份读不了，Linux 和 HarmonyOS 因此打不了字。它们给这个字段配了 `#[serde(default = "default_text_edit_button_order")]`（在各自标签上核对过），缺这个键时用自己的七个动作。
- 旧版本写下的七个动作补齐后正好就是现在的默认排列，所以从旧版本升级上来、没在 Android 上排过顺序的用户，存一次以后文件里就没有这个键，退回旧版本也读得了。
- 只省在盘上：经 FFI 交给宿主的快照（`load`、`default_preferences`）照旧带着完整排列。Android 的 `TextEditButtonOrder.defaults()` 对默认偏好用 `getJSONArray` 取这个键，在 serde 层 `skip_serializing_if` 会让它抛异常，所以不放在 serde 上，Java 也不用改。
- 磁盘格式本来就是「等于默认值的键不写出」：`network`、`multilingual_input` 等几十个字段都带 `skip_serializing_if`，产品读取方一律由共享层补默认值。这一条只是让排列也照此办理。
- 用户在 Android 上排过顺序的排列照常写出，旧版本仍读不了，见「已知上限」。
- Android 的 `TextEditRecentClipboardDeviceSmoke` 验收保存和重置后的有效排列时，通过 `NativeClient.loadPreferences` 比较共享快照；取消则逐字节比较前后文件，确保没有意外写回默认值或增加修订号。直接从磁盘用 `getJSONArray` 读排列，会在重置为默认顺序后遇到合法缺键并抛 `JSONException`；develop 的 API 35 运行 `40639795146` 已实际暴露此问题。备份恢复和修订号仍可读取原始文档，排列含义由共享层解释，既不在 Java 再抄默认顺序，也不撤掉为旧版本保留的稀疏写盘规则。

### 4. 偏好读不了时，输入运行时用内存里救下的设置运行

- **`PreferencesStore::load_for_runtime()`**：
  - `load` 读得了就返回它的结果。
  - 读不了、而且不是存储故障时，用 `Preferences::salvaged_from` 逐键救下本版本认得的设置，其余取本存储所属版本的默认值。逐键取舍与 `recover` 相同。
  - `format_version` 不是 1 的文档不逐键救，整份取默认值：更新的文档格式可能改了同名键的含义，按 v1 的含义应用它会悄悄做错事。不是 JSON 的文档同样取默认值。
  - 修订号沿用文档里的值，读不出时为 0。
  - 结果放在 `RuntimePreferences` 里交出：`load_error` 是严格读取失败的原因，`not_kept` 是没能整段保留的顶层键（整份取默认值时是 `["*"]`）。
  - 只在内存里，不写盘。
- **去向没能保留时关掉联网功能**（`Preferences::close_destinations_not_kept`，只在 `salvaged_from` 里做）：逐键救的时候，一个段落里读不了的字段换成默认值、其余字段照留，可能出现「开关留下了、去向换成了默认值」。例如 `cloud_candidate_provider.source` 是更新版本的取值，开关 `cloud_candidates` 还开着，来源却回到默认的 Google，组字就发给了用户没选的服务；`network.proxy_mode` 读不了，回到跟随系统，用户填的自定义代理被绕开。`load_for_runtime` 还会对救下的偏好调 `network::install`，整个进程都按它发请求。所以文档里写着、救下后却不是同一个值的去向字段，关掉用它的功能：
  - 云候选的 `source`、`endpoint` → `cloud_candidates = false`；
  - AI 的 `provider`、`endpoint`、`endpoints` → `ai_assistant.enabled` 和 `translation_ai` 关掉；
  - 语音识别的 `asr_provider`、`asr_endpoint` → `voice_input.enabled` 关掉；润色的 `polish_provider`、`polish_endpoint` → `polish_enabled` 关掉；
  - 自定义翻译的 `endpoint` → `custom_translation.enabled` 关掉；
  - 代理模式读不了（即使没有地址）、`network` 整段非对象，或自定义代理地址无法保留 → 使用运行时的 `CustomRequired` 状态，并关闭发送用户内容的功能。地址可用时供 Rust HTTP 客户端使用，不可用时构造远程客户端失败，详见「审查补充」。
  - 去向叶子的 JSON pointer 不存在时，继续查最近存在的祖先：祖先是对象，表示整段或字段真正缺省，照旧采用默认去向；祖先不是对象，表示原服务段无法按字段读取，按去向未保留处理。否则 `cloud_candidate_provider` 整段变成字符串或 `null` 后，开关仍能被救下，服务却回到 Google；`ai_assistant` 整段损坏时，顶层的 `translation_ai` 也会错误保留。
  - 这条只约束救援结果，不改变严格反序列化接受的形状：空数组原本可被 serde 读成缺省字段，不一定让文档读不了；只有文档因其它字段损坏而进入救援时，这个非对象服务段才会保守关闭。真正省略段落和对象内字段（包括 `{}`）继续沿用原有缺省行为，AI 空对象的逐字段默认与整段默认不同也不改变。
  - 去向字段的单项变化不单独关闭腾讯、小牛翻译和云剪贴板、词库同步的 WebDAV：它们的去向是固定的服务商或用户填的地址，默认值是空的、不会发到别处。匿名使用报告读的是严格的 `load`，偏好读不了时不上报也不清空（`ffi/reporting.rs` 的 `consent`），不受降级影响。
  - 设置页的「修复」（`recover`）不这样做：它写下的结果用户在设置页看得见、改得了。
- **host-api 有两处改用它**，所有平台都经过这两处：
  - `edition_preferences`（准备宿主时）：`write_first_document` 失败、又不是 I/O 错误时改用它。没有文件时照旧写第一份。
  - `HostOptions::from_document`：副本解析不了或者通不过 `validate`，都改用实时文档。实时文档也读不了时救它；没有实时文档时救副本自己的；副本本身没问题、是文档别处不对时，原样交回副本。
- **报告**：两处都经 `diagnostics::report` 报告（`preferences document unreadable`、`runtime options preferences unusable`），内容只有错误的种类和 `not_kept` 里的键名，不带 serde 的错误原文：类型不对时它会引出文档里的字符串取值（比如新版本把某个密钥字段改了形状），而诊断日志会被打进诊断包发出去。
- **键盘的剪贴板面板**：`msime_client_load_clipboard_history` 和删除条目、取条数上限的读取也改走 `load_for_runtime`，文档里别的设置读不了时面板照常可用。同意与否取文档里写的值，整份读不了时按默认的关闭处理。记录剪贴板（`capture_clipboard_text`）仍然严格读，读不了就不记。
- **文件为什么安全**：
  - 读不了的文件仍然不自动改写，`recover_malformed` 只修不是 JSON 的文件，这条不变。
  - `save` 在锁里会重新严格读一遍，文档读不了时，任何宿主的保存都写不进去。
  - 设置界面经 `load` / `try_load` 读，看到的仍是错误，修不修复由用户在设置页决定（`msime_client_repair_preferences`）。
- **修复不再抹掉更新版本的设置**：`recover` 以前把救下的设置反序列化成结构体再写回，本版本不认识的键和列表元素全部丢掉，只剩 `.corrupt-*` 备份里有。降级运行以后，用户更可能在不知道原因时点修复，所以 `salvage_preferences` 现在同时交出这些键和元素（`deserialize_keeping_unknown` 加 `newer_list_values`），`recover` 照 `save` 那样放回。读不了的那个取值本身仍换成默认值，原值只在备份里。

### 5. 防止再犯的测试

- **已发布版本的样本**：`crates/client-core/tests/fixtures/released-preferences/` 放着六个已发布版本第一次保存时写下的 preferences.json，是在各自的发布标签上运行 `PreferencesStore::save(0, Preferences::default())` 生成的：android-v0.6.0、harmony-v0.3.0-beta、ios-v0.52.0、linux-v0.14.0、macos-v0.55.0、windows7-v0.3.0。`documents_written_by_released_versions_load_and_save` 要求当前版本严格读得了它们、校验通过、改一项后存得下。#9802 那样新版本校验更严的改动会在这里失败。以后发版时补上新的标签。
- **未来取值**：`every_string_setting_with_a_future_value_still_runs` 把默认文档里每一个字符串取值逐个换成 `future_value`，`load_for_runtime` 都必须成功、结果通过校验、文件字节不变。新加一个只认几个取值的字段，会自动被覆盖到。

## Alternatives considered

- **`validate` 改为接受缺项、重复项。** 最省事，一处改动就能让所有旧文档读得进来。但设置页提交残缺排列也会被放过，宿主界面的错误不再暴露。#9975 已经否掉过这个方案，这里沿用它的分工。
- **读不了时自动用救下的设置改写文件。** 用户什么也不用做，下次读取就正常了。但读不了的文档多半出自更新的版本，悄悄改写会丢掉那个版本的设置。`recover_malformed` 正是为此只修不是 JSON 的文件，自动修复和显式修复的区分（`msime_client.h`、`ffi/host.rs`）也是为此。在内存里降级运行，不碰文件，就足以让用户继续打字。
- **给每个枚举加 `#[serde(other)]` 兜底变体。** 粒度最细，每个字段遇到新值都有确定的回退。但约五十个字段都要加一个本版本不可能选中的变体，每个 `match` 都得处理它，新加的枚举还容易漏。第 3 节在整份文档的层面兜底，自动覆盖以后新加的字段，再由未来取值测试把它锁住。
- **多语言列表也放宽并写回。** 它所在的段落等于默认值时不写出（`skip_serializing_if = "MultilingualInputPreferences::is_default"`），丢掉一个新语言后，段落可能正好等于默认值而不写出，写回无处可放，结果就是悄悄丢掉用户的选择。不如让它整份读不了，交给运行时兜底，设置页不能保存，也就不会丢。
- **在 `HostOptions` 里加一个字段，把读不了的原因带给键盘界面。** 键盘能直接显示一行提示。但 `HostOptions` 拒绝未知键，旧版本（macOS 不跟输入法一起更新的设置应用、降级）读到带这个键的 runtime-options.json 会整份拒绝，正好在版本错开、最需要它的时候把问题放大。改走准备结果或单独的 FFI 也行，但六个平台的键盘都要加界面。本次先只走诊断出口，键盘提示另开一项做。
- **在 serde 上给排列加 `skip_serializing_if`。** 一行就能让盘上不写默认排列。但 FFI 交给宿主的快照也会少这个键，Android 的 `TextEditButtonOrder.defaults()` 用 `getJSONArray` 取默认偏好里的排列，键不在就抛 `IllegalStateException`，要连 Java 一起改，还得在 Java 里再抄一份默认排列。只在写盘时省掉，宿主看到的快照不变。
- **排过顺序的排列写盘时也去掉 `select_line`，让旧版本总读得了。** 旧版本能读，但本版本的用户排好的选择行位置每次保存都会丢，下次读回时补在末尾，等于不让用户排这个键。
- **新增一个运行时读偏好的 FFI，让 Android、HarmonyOS、macOS、Linux 的重载也用救下的设置。** 这几个宿主不重新准备运行时，会话建好后靠 `msime_client_load_preferences` 重载实时偏好，它是严格读取，失败时宿主保留现有设置：Android 的 `withLivePreferences` 退回 runtime-options 里首装时的出厂偏好（九键、五笔用户会回到全拼 26 键），HarmonyOS 停在准备时救下的设置。要改的是四个平台的重载路径和 Android 的十几处调用，需要各自上设备验证，另开一项做；本次保证的是「打得了字」，不是「设置完全不变」。

## Consequences

- **收益**：
  - 偏好文档里任何一个键读不了，都不再让用户打不了字：iOS 键盘、HarmonyOS、Windows 的 Server 能准备运行时，macOS、Android、Linux 能建会话。
  - 文本编辑排列、触屏方案、模糊音规则遇到更新的版本写的取值，直接读得进来；方案和规则保存后还在，更新的版本读回时用户的选择不丢。
  - 已发布版本的样本和未来取值测试，会把「新版本校验更严」和「新加只认几个取值的字段」这两类问题拦在 CI 上。
- **代价**：
  - 降级运行时，用户看到的是部分设置回到默认；带去向的联网功能如果去向没能保留，会被关掉。键盘界面目前没有提示，只有诊断日志和设置页的读取错误能说明原因。
  - Android、macOS、Linux 不重新准备运行时，降级时用的是 runtime-options 里的副本（Android 上是首装时的出厂偏好），不是救下的实时设置，见 Alternatives 的最后一条。
  - 设置页在用户修复或升级之前存不了设置。
  - 救一次偏好要对每个顶层键做一次反序列化和校验，只在失败路径上发生，但在这个状态下每建一次会话都会重做一次。
  - 文本编辑排列里不认识的动作在本版本保存后丢掉位置；导入新版本导出的文件时也一样。以前本版本读不了这种文档，也就写不进去；现在能读能写，代价是这个按钮的位置。
  - 启用列表里只有更新版本的方案时，本版本补上的全拼 26 键会随保存写回，更新的版本读回时多出一个用户没启用过的全拼。要避免得在读盘时记下「这个全拼是补的」再在保存时撤掉，为一个可以手动关掉的方案不值得。
  - 用户在本版本里清空模糊音规则并关掉模糊音，更新版本加的规则仍会接回，本版本里也看不到、删不掉；开关关着时它不起作用，更新的版本再打开模糊音时才生效。
  - 两边都读不了（副本通不过校验、实时文档也读不了）时，每次建会话、词库请求和快照请求都会重新救一遍并报告一次，没有缓存。只有副本出自校验更严或更松的另一个版本时才会走到这里，没有实测开销。
- **已知上限**：
  - `network` 整段非对象时无法重建用户的代理地址；运行时保留 `CustomRequired` 并拒绝远程客户端构造，不把未知的约束解释成跟随系统。真正省略 `network` 或空对象仍沿用缺省模式。
  - 已经发出去的版本照旧严格。不写出默认排列以后，降级到 Android 0.6.0、HarmonyOS 0.3.0-beta、macOS 0.55.0、Linux 0.14.0 时读得了用户没排过顺序的文档；在 Android 上排过文本编辑面板顺序的用户，或者文档里有那些版本不认识的其它取值，仍然读不了，只能升级回来或在设置页修复。
  - 除韩文布局、文本编辑排列、触屏方案和模糊音规则之外，其它字段的新取值仍让整份文档严格读不了，只是不再挡住打字。

与 [偏好文档保留未知键](../architecture/2026-10-15-preferences-keep-unknown-fields.md) 部分重叠：它的 Consequences 里写着「新版本给枚举加了新取值，旧版本照样读不了整份文档，本次不处理」，这件事由本篇接手，那篇的事实已经同步更新。[文本编辑面板选择当前行](../feature/2026-10-26-android-text-edit-select-line.md) 里「重复或未知的动作仍然拒绝」已改为本篇的读法。

## Verification

Android 文本编辑设备用例通过正式 `loadPreferences` JNI 接口核对保存、取消和重置后的有效排列，不再把磁盘上省略的默认键当作损坏。默认排列仍由共享层提供，测试不复制默认动作列表。

- 文本编辑排列：
  - `text_edit_button_order_reads_any_list_it_is_given`，从文件读入以下几种排列，结果都是完整排列、文件不被改写：v0.6.0 之前写下的七个动作、现在的八个、带更新版本动作 `select_word` 的、有重复的、空列表、不是字符串的元素。
  - 不是数组时 `load` 报错，`load_for_runtime` 用默认排列运行。
  - `text_edit_button_order_is_strict_on_save_and_tolerant_on_read`（原 `..._rejects_missing_duplicate_and_unknown_actions`，serde 那一半已经不再拒绝）：结构体提交的严格校验不变，`from_submission` 拒绝不认识、重复和缺的动作。
- 提交严格：
  - `submitted_preferences_are_parsed_strictly`：启用列表、选中项、模糊音规则里的错名字，读盘读得进，提交被拒；退役的高情商回复照旧迁移；没带排列时取默认排列。
  - host-api 的 `save_preferences_uses_compare_and_swap_and_rejects_invalid_snapshots` 增加了经 `msime_client_save_preferences` 提交重复动作、未知动作、未知方案、未知规则的用例，都返回错误、文件不动。
- 默认排列不写盘：`a_default_text_edit_order_is_left_out_of_the_file`，默认排列存下后文件里没有这个键、读回不变，FFI 快照里仍有完整排列；排过顺序的照常写出；六个已发布版本的样本改一项再存，文件里都不再有这个键。
- 写回：
  - `newer_list_values_survive_a_save`：更新版本的方案、选中项和模糊音规则在本版本保存后接回，退役方案和不认识的动作不接回；本版本选了自己的方案时以本版本为准；恢复默认后全部清掉；启用列表只剩新方案时回到全拼 26 键并接回新方案。
  - `touch_keyboard_scheme_visibility_matches_apple_order_and_fallback_contract` 原来断言 `future_scheme` 读失败，现在断言读得进来、文件不变。
  - `fuzzy_pinyin_preserves_disabled_rules_and_reads_past_unknown_ids`（原 `..._rejects_unknown_ids`）原来断言不认识的规则名让反序列化失败，现在断言它被丢掉，不是字符串的元素仍然报错。
- 运行时降级：
  - `load_for_runtime_keeps_typing_on_unreadable_documents`：更新版本的标量枚举、通不过校验的值都能运行，读不了的键取默认值并列进 `not_kept`，其余设置照旧，修订号沿用文档里的值；更新的格式版本、不是 JSON 的字节整份取默认值，`not_kept` 是 `["*"]`。文件不变，`save` 仍被拒绝。
  - `runtime_salvage_closes_features_whose_destination_was_not_kept`：别的设置读不了时云候选和代理照常；云候选来源读不了时云候选关掉；代理模式读不了而填了地址时用这个地址；AI 地址表、语音识别和润色地址读不了时对应功能关掉。
  - `runtime_salvage_closes_cloud_when_its_destination_section_is_not_an_object` 和 `runtime_salvage_closes_ai_when_its_destination_section_is_not_an_object`：直接调用真实 `Preferences::salvaged_from`，服务整段分别为字符串、`null`、空数组、数字和布尔值，同时用无关的 `candidate_layout` 未来值确保原文档严格读不了；救援后关闭对应开关，另一功能保留，结果通过校验。修复前两项实际在各自首个 `"future-shape"` 的关闭断言失败，不把后续形状冒称为逐个复现。
  - `runtime_salvage_keeps_features_with_missing_destination_defaults`：整段省略与空对象两种文档都因无关字段进入真实救援，云候选和 AI 翻译继续开启，服务配置与同一原文档严格读取的结果一致。
  - 严格 clippy 首次仍发现此前基线里的 `android_keyboard_skin_slots_round_trip_without_changing_legacy_theme` 在默认初始化后逐字段赋值；该测试现改为等价的结构体初始化，字段取值和校验行为不变，不放宽 lint。
  - 服务父段补修的完整 `bash scripts/verify-local.sh --quick` 退出 0：202 项 Windows 可移植测试、仓库契约、Rust workspace、Android Rust/Java、WASM 及严格 Clippy、Linux 设置壳和原生门禁、macOS 原生编译与共享 Apple 桥通过。Linux 注册 112 项，111 项通过、`linux-ibus-demand-activation` 因环境跳过，full/wubi 构建及安装隔离通过。未配置的 Windows 原生/pipe-only 与鸿蒙 HAP 按脚本规则跳过；没有把跨目标编译计为设备验收。core 全目标严格 Clippy 和救援回归另行通过。
  - `recover_keeps_what_a_newer_version_wrote`：修复后顶层和段落里的未知键、启用列表里的新方案都还在，读不了的取值换成默认值。
  - host-api 的 `clipboard_reader_respects_preferences_and_preserves_history`：偏好里别的设置读不了时剪贴板面板照常列出条目，整份读不了时按关闭处理。
  - `every_string_setting_with_a_future_value_still_runs` 覆盖所有字符串取值。
- 往返：`develop_documents_round_trip_through_the_tolerant_reader` 确认本版本写下的文档经宽松读法往返后取值逐个一致，没有改动的保存不前进修订号、不改写文件。
- 导入：
  - `a_newer_or_damaged_document_is_refused_rather_than_partly_applied` 增加了带更新版本方案、规则的文件，结果是整份拒绝。
  - `a_text_edit_order_from_a_newer_version_is_imported_without_its_unknown_action` 确认排列里的新动作丢掉、其余照常导入。
- 设置同步：`desktop_settings_sync_works_on_a_document_from_a_newer_version`，本机文档带不认识的动作、重复的动作和新方案时照常上传和应用，写回后新方案还在。
- host-api：
  - `preparing_over_an_unreadable_preferences_document_still_types`：分别放入校验更严的值、新的枚举取值、类型不对的列表元素，`msime_client_load_preferences` 都报错，准备宿主都成功，建会话后能打字，文件不变；用户点修复后严格读取恢复正常；事故里那份七个动作的文档直接读得了。
  - `session_reads_live_preferences_when_the_options_copy_is_refused` 改为：没有实时文档时救下副本照常建会话；副本解析得了却通不过校验时同样不再拒绝。
- 命令与结果（在 Mac Studio 上经 `rbuild` 运行，评审修正后重跑）：
  - `cargo test --no-fail-fast -p msime-client-core -p msime-client-services -p msime-host-api`：client-core（库内 667 项）和 client-services（库内 1422 项）连同各自的集成测试全部通过。host-api 1038 项通过、3 项失败，都与本改动无关：
    - `listing_reports_every_pack_and_checks_the_state_root`（列表里多了 `pronunciations`）和 `a_command_gets_the_local_time_to_the_minute`，在未改动的 616bd2888e 上单独运行同样失败；
    - `translation_queries_never_clear_a_mixed_japanese_row_for_the_network` 只在整套并行时失败，单独运行通过。
  - `cargo clippy --keep-going -p msime-client-core -p msime-client-services -p msime-host-api --all-targets -- -D warnings`：本次改动没有新的告警。剩下两处 `field_reassign_with_default`（`preferences/tests.rs` 的 `android_keyboard_skin_slots_round_trip_without_changing_legacy_theme`、`account/settings_sync/tests.rs:1338`）在 616bd2888e 上已经存在。
  - `cargo check -p msime-desktop --tests`（Tauri 保存命令的参数改动）：通过。macOS 上这个检查要求 `target/macos/EngineResources` 和 `target/macos/水杉输入法.app` 存在，检查时临时建了空目录，跑完删掉。
  - `cargo fmt --check`、`bash scripts/run-checks.sh`、`platforms/macos/tests/settings/preference_coverage.py`、`pnpm run verify-notes` 通过。

### 审查补充：运行时要求的代理不可用

- 代理取值无法完整保留时，用运行时状态 `CustomRequired` 保留“必须使用代理”的约束。可用的 HTTP 地址仍供 Rust HTTP 客户端使用；同时关闭降级副本里会发送用户内容的功能，覆盖由原生宿主执行请求的路径。地址无法保留时，两个 HTTP 适配层在构造远程客户端前返回已有的连接失败。本机和局域网目标沿用既有的直连规则。
- 该状态随运行时偏好副本往返，避免序列化后丢掉约束；它不改写原始偏好文件，也不增加设置页选项。普通 `custom` 留空时跟随系统的既有行为保持不变。共享设置的移动 AI 传输同样将构造阶段的拒绝映射为 `Error::Unavailable`，模型列表与润色两个入口都先处理这个结果再配置超时。
- 直接沿用普通 `Custom` 的好处是不用调整客户端构造接口，但该模式对不可用地址的既有回退与这里的约束不一致，因此不采用。为客户端伪造不可达代理或非法请求头虽可减少调用方改动，却把拒绝连接寄托在网络环境或无关字段上；使用显式错误，让调用方映射为各自已有的连接失败。
- 回归覆盖未知模式、未知地址形状和不支持的地址，且验证运行时副本往返；未知模式即使没有地址或明写空地址，也保留约束，不能假定它等同于普通 `custom` 的留空语义。该边界新增用例在原守卫上真实失败后修正。providers 与 services 各自验证远程客户端构造被拒、本机目标仍可构造，以及可用代理仍能构造。实际验证结果：`cargo test --no-fail-fast -p msime-client-core -p msime-client-services -p msime-host-api` 中本改动相关测试通过；`cargo clippy --keep-going -p msime-client-core -p msime-client-services -p msime-host-api --all-targets -- -D warnings` 无新增告警；`cargo check -p msime-desktop --tests`、`cargo fmt --check`、`bash scripts/run-checks.sh`、`platforms/macos/tests/settings/preference_coverage.py` 与 `pnpm run verify-notes` 通过。`target/pr-review-security/network-skin-regressions.log` 记录 Android client-core 与 client-services 皮肤回归各 1 项通过；`target/pr-review-security/network-followup.log` 记录两项 PowerShell 画布和页面就绪回归通过。

- 与已公开的服务父段补修整合后，新增 `runtime_salvage_non_object_network_keeps_proxy_constraint`：旧实现面对 `network: null` 实际失败（`System` 与期望的 `CustomRequired` 不符）；修后与其余六项 `runtime_salvage` 回归一同通过（7 项通过、0 失败）。覆盖 `null`、布尔、数值、数组及字符串五种损坏父段，另以真正缺省和空对象作保持默认联网行为的对照。测试使用固定合成值，未改写原始文档。
