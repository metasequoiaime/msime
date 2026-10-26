# Agent Note: Android 设置的插件页只列手机上生效的六种，规则移植成纯 Java 策略

Status: implemented

## Problem

维护者要用户自己做、自己分享插件，桌面的共享设置页早就有完整的「插件」页（我的插件、详情、编辑器、社区插件），Android 只有两处零散的入口：「输入 › 辅助码方案」里导入单个 `.txt` 码表，键盘页「按键反馈」里在内置音效包里挑一个。Android 键盘本来就经 runtime option `preferences_directory` 读插件目录，短语表、指令表、辅助码表在获得焦点时就会载入，可是手机上没有地方装、启用、编辑这些包，社区里别人分享的包也装不上。Android 的设置界面是原生 Java（见[设置界面按平台归属](../architecture/2026-10-15-settings-ui-ownership-by-platform.md)），共享页拿不过来。

## Decision

设置首页「AI 润色与回复」下面加一行「插件」（`home/PluginsPage`，与共享页把插件和 AI 放在「工具」组的做法对应），两个分类：

- 「我的插件」按类型分组，只列手机上生效的六种：按键音效包（`mode = "keys"`；按键旋律手机上不播，保持 Android 原来的规则）、指令表、短语表、辅助码表、单词本、符号集。特效包、音乐包、用户装的按键旋律和脚本插件不列，末尾一行说有几个（内置的旋律包和音乐包随安装包带着，不算）。偏好里指着已不在的包的选择列在对应分组里，点开可以去掉。下面是「新建插件」和「导入 .zip」。
- 「社区插件」经 `msime_client_community` 的 plugin_* 操作浏览、搜索、按类型筛选（只给这六种）、看详情（`CommunityPluginPage`）、安装（要登录，同 id 的包先问再整包替换）、评分、举报（登录账号优先，否则本机匿名账号）、下架自己的作品，以及「发布我的插件」（`PluginPublishPage`，`plugin_pack_preview` 预检、`plugin_publish` 上传，发布 id 由页面生成、失败重试沿用）。不按类型筛选时服务端也会给特效包、音乐包，这里去掉并计入下一页的偏移，整页都被去掉时说「前面的插件这台设备都不能安装」，与桌面对「这台设备不能安装的新类型」的说法相同。社区条目不带音效包的发声方式，按键旋律要装上才知道，装上后详情说明手机上不播。

读写全部经共享 Rust，宿主只是垫片（`home/PluginStore`）：

- 插件目录是键盘读的 `HostStore.directory` 下的 `plugins/`，内置音效包在 `<filesDir>/sound-packs`，与辅助码表导入、按键音包选择同一份；请求不带 `script_page`，脚本插件永远不出现。
- 导入只有 `.zip`：SAF 交回的 content URI 原生库打不开，`importArchive` 把内容流式复制进 `cache/plugin-import/<随机名>/pack.zip`，超过 client-services 的 80 MiB 压缩包上限就停，再由 `msime_client_plugins` 的 `import` 安装，最后删掉整个暂存目录；上一次中途被杀留下的暂存目录在下一次导入前清掉。与鸿蒙 `Settings.ets` 的 `importPlugin` 同一套做法。手机上不提供导入文件夹。
- 编辑器（`PluginEditorPage`）经 `authoring_load` / `authoring_save` 读写，宿主按导入同一套规则再查一遍。JNI 的 `msime_client_plugins` 请求上限从 2 MiB 提到头文件写的 8 MiB：一本两万词的单词本整份在请求里。
- 详情（`PluginDetailPage`）照共享页 `plugin-detail-view.tsx` 的内容和文字：短语表、指令表的「启用」写 `plugins.phrase_tables` / `command_tables`（启用的排到最后，最多 16 个）；辅助码表「用于全拼 / 双拼」写 `plugins.helpcode_pack_*`；单词本「去背单词」经 `VocabularyReviewPage.openBook` 选中 `pack-<id>`，背单词的请求从此带上 `plugins` 目录，插件词书标「· 插件」、不能在那里删除；不是内置的包可以删除，删后偏好里指着它的选择回到默认（`withoutRemovedPack`）。

## 与共享页不同的地方

- 按键音的开关在 Android 是键盘页「按键反馈」里的本机开关（`KeyboardFeedbackStore`），不是共享偏好的 `plugins.key_sound.enabled`。「设为当前音效包」写 `plugins.key_sound.pack` 并打开本机开关，不写共享偏好的开关：那个键随设置同步到电脑，写了会把电脑上的按键音也打开。列表的「使用中」看本机开关；说明里的去处是「键盘 → 按键反馈」而不是「声音与效果」。
- 手机的「输入」页没有「快捷模式」。K 模式（`local_modes.quick_phrase`）或 `/` 指令（`local_modes.command`）关着时，短语表、指令表详情里的提醒带一个就地打开的按钮；共享页对 K 模式只给一个去输入页的链接。保存后的提醒（`authoredSaveNote`）也改成指向这些按钮和键盘的「本地输入」，不写「Shift+K」和「输入 → 快捷模式」。
- 共享页按当前方案判断打不打得开 K、/ 模式（`schemeTableModeNote`）；这里没有做这条提醒。
- 符号集详情说「颜文字组排在「颜文字」之后」：Android 符号面板的「颜文字」就是共享页说的 All。

## 规则放在哪里

校验句子、草稿整理、批量粘贴和保存后的提醒逐条移植自 `plugin-editor-model.ts`，放在不依赖 Android 和 org.json 的 `policy/PluginEditorPolicy`；列表的分组、标记、缺失的选择、启用顺序和失败句子（`pluginErrorMessage`、`communityPluginMessage`）放在 `policy/PluginCatalogPolicy`。长度和空白按 TypeScript 的写法算：「UTF-16」是 `String.length()`，「字节」是 UTF-8 字节数，首尾空白按 JavaScript 的字符集（含 U+3000、U+FEFF）去，不用 Java 的 `trim()`。Windows 的 `PluginEditorModel.h` 是同一份规则的另一份移植，两边与共享页不一致时都以宿主（client-services）的拒绝为准。

## Alternatives considered

- **把校验规则做成 JNI 调 Rust**：只有一份规则，最不会漂移，这是它最强的理由。不用它：client-services 的 `authoring` 本来就只在保存时整包校验、报一处 `detail`，没有「第几行哪里不对」的逐字段接口，要先在 Rust 加一套页面专用的预检；共享页和 Windows 原生设置都选了「页面先查一遍、宿主再查一遍」，这里跟着做，保存时宿主拒绝照样就地显示。
- **像桌面一样列出全部八种，用不上的在详情里说「这台设备不……」**：与桌面列表完全一致，用户在两边看到同样的东西。不用它：维护者要求只列手机上生效的类型，手机屏幕上一长串点开才知道没用的包只会让人以为装坏了；用不上的包仍在磁盘上，报一个数并说明能在电脑上用。
- **用 SAF 的目录选择器导入文件夹**：与桌面的两个入口对齐。不用它：手机上的包几乎都是下载来的 `.zip`，目录选择器交回的是 tree URI，要在宿主里逐个文件复制并重做 client-services 的文件数、大小和子文件夹检查（鸿蒙为此写了 `PluginFolderScan`），换来的是一个手机用户几乎不会用的入口。
- **「设为当前音效包」同时写共享偏好的 `key_sound.enabled`**：与共享页的 `withPackSelected` 一模一样。不用它：Android 键盘不读这个键，写了只会经设置同步把电脑上的按键音打开。

## Consequences

- **收益**：手机用户能装、启用、编辑、分享六种插件，键盘读的就是这里写的插件目录和偏好；做插件的门槛降到「在手机上填几行」。规则仍以 client-services 为准，页面上的预检只是让人早一步知道。
- **代价与已知上限**：编辑器规则多了一份 Java 移植，共享页改了规则要同步改 `PluginEditorPolicy` 和它的冒烟（与 Windows 的 `PluginEditorModel.h` 一样）。社区列表不按类型筛选时，一整页可能都是手机用不上的类型，要多点几次「加载更多」。符号集要靠键盘的符号面板读插件目录才有效果，那一半与键盘「本地输入」里的「指令」入口一起在另一个 PR 里。
- 与[设置页插件编辑器](2026-10-26-in-app-plugin-editor.md)部分重叠：编辑器的规则和宿主接口沿用那里的决定；那里「移动端没有插件页」的说法已改成指向这里。与[移动端导入单个辅助码表](2026-10-12-mobile-helpcode-table-import.md)、[Android 选按键音效包](2026-10-25-android-key-sound-pack-picker.md)部分重叠：两条入口保留，插件页的辅助码表、按键音效包读写的是同一组偏好键。

## Verification

- `rbuild bash platforms/android/check-host.sh`（Mac Studio）：通过，新增 `tests/settings/PluginEditorPolicySmoke.java`（用例取自 Windows 移植的 `plugin_page_model.cpp`，同一组输入同一句话，另测 JavaScript 空白和保存后的提醒不写 Shift）、`PluginCatalogPolicySmoke.java`（手机类型、用不上的计数、标记、缺失、启用顺序、失败句子、发布字段），`VocabularyReviewTextSmoke` 覆盖插件词书的标注。
- Gradle `lintFullRelease`（NewApi 门禁）通过；第一版评分星号用了 API 33 的 `String.repeat`，已改成循环。
- API 35 专用模拟器（自建 AVD，emulator-5610，arm64，开发密钥签的完整 APK）：经 SAF 从「下载」导入合成的 `.zip`（按键音效包、符号集、短语表、特效包），列表按类型分组、特效包只报「另有 1 个」，`cache/plugin-import/` 用完为空（`adb root` 后核对）；编辑器新建指令表、单词本（保存前问题提示、返回时「放弃修改」确认、保存后打开详情并提醒），详情里启用指令表、就地打开 `/` 指令后提醒变成「已保存，修改已经生效」；按键音效包「设为当前音效包」变成「使用中」，删除后回到「清脆键盘 · 使用中」；单词本「去背单词」选中插件词书并显示「这本书来自插件」；社区插件列表、详情读到线上数据，未登录点「安装」报「请先登录后执行此操作」并给出「登录」；发布页预检出 `v1.0.0 · 授权 CC0-1.0 · 1 个文件`。
- API 28 自建 AVD（emulator-5612）：插件页、导入 `.zip`、编辑器打开正常。
- 没有在真机上跑过；登录后的安装、评分、举报、发布没有在设备上走过（模拟器上没有测试账号）。
