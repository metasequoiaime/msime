# Agent Note: iOS 设置 App 的插件页，以及键盘里的 / 指令入口和符号集插件

Status: implemented

## Problem

维护者要让用户自己做插件、互相分享。桌面和鸿蒙电脑有完整的「插件」页（我的插件、导入、新建和编辑、社区插件），iOS 却只能在辅助码页导入单个 `.txt` 码表、在键盘页选内置的按键音效包。键盘那一侧引擎早就按插件目录读短语表、指令表和辅助码表（`MetasequoiaInputSessionBridge` 准备宿主时的 `preferences_directory` 就是 App Group 里的共享状态目录，插件目录是它下面的 `plugins/`），但用户没有入口把包装进去、启用它们，`/` 模式在 iOS 上也没有开关和入口；符号面板不读符号集，背单词不传插件目录，单词本插件不出现。三个手机端要同时补上，并且界面和行为一致。

## Decision

**插件页。** 设置根页「输入 / 表达 / 词库」那一组的末尾加「插件」（`SettingsPage.plugins`，深链 `settings/plugins`），照共享设置页插件页的结构画成原生 SwiftUI，标题下固定「我的插件 / 社区插件」两段，代码在 `App/Sources/plugins/`：

- 「我的插件」按桌面 `kindOrder` 里在手机上起作用的六种分组：音效包（只列 `mode = "keys"`）、指令表、短语表、辅助码表、单词本、符号集。特效包、音乐包和旋律音效包不列，用一行说本机另外装着几个（内置的不算）。行尾标记、找不到的选择（`missingSelections`）和「无法载入的插件」与桌面相同。
- 详情页的动作与 `plugin-detail-view.tsx` 相同：短语表、指令表的「启用」写 `plugins.phrase_tables` / `command_tables`（排在已启用的之后，最多 16 个）；按键音效包「设为当前音效包」写 `plugins.key_sound.pack` 并打开 App Group 里的按键音开关（选用即使用）；辅助码表「用于全拼」「用于双拼」写 `plugins.helpcode_pack_*`；单词本「去背单词」先在背单词里选中 `pack-<id>` 再推入背单词页；不是内置的包可以删除（先确认），删掉后清掉选中、启用它的设置。K 模式或 / 指令关着时提醒，/ 指令可以就地打开（写 `local_modes.command`）。
- 导入只有「导入 .zip」：系统文件选择器交回的文件先复制进 App 自己的临时目录（固定名 `pack.zip`，超过 80 MiB 不复制），再由 `msime_client_plugins` 的 `import` 安装，暂存目录用完就删，与鸿蒙插件页的做法相同。手机上不导入文件夹。
- 新建和编辑四种文本类插件走 `authoring_load` / `authoring_save`。检查规则和每一句提示逐条移植自 `plugin-editor-model.ts`，放在不碰 SwiftUI 的 `PluginEditorModel.swift`，单元测试直接测它；编辑器以弹出页打开，保存成功后关掉并推入这个包的详情，有没保存的修改时关掉前确认。
- 「社区插件」经 `msime_client_community` 的 `plugin_list`、`plugin_detail`、`plugin_install`、`plugin_rate`、`plugin_delete`、`plugin_pack_preview`、`plugin_publish`，请求由 `CommunityPluginAPI` 拼，比 `BackendRustAccount.community` 多带一个 `state_root`（App Group 的共享状态目录），iOS 已有的账号会话（`session_store`）照常认证，不需要新的 Rust，也不改 `shared/backend`。类型筛选只列六种，「全部」里音乐包、特效包在客户端去掉但仍计入下一页偏移（`CommunityPluginPaging`）。发布只列本机用得上的、不是内置的包；举报沿用皮肤和词库的举报表单。
- 「输入 › 快捷模式」加「指令」一行（`local_modes.command`，与 client-core 一样默认关）。

**键盘。**

- 本地输入菜单（候选栏的「水杉输入法」和「更多 → 本地输入」）加「指令」，触发 `/`，跟着 `local_modes.command`，文档里没写时不出现。它不是 Shift 加字母，点下去时先问会话 `/` 此刻能不能打开模式（`slashOpensCommandMode`：没有组字、不在本地模式、引擎把 `/` 列在 `spelling_symbols` 里，即中文标点下），能就把 `/` 当标点交给会话，不能就提示切到中文标点，不写出一个斜杠。九键进了本地模式后键面本来就换成 26 个字母（K 模式一直如此），所以九键也打得出触发词，入口不只给 26 键。
- 符号面板在内置分类之后，每个符号集插件一个分类，以插件名为标题，内容是它全部 `symbols` 组首尾相接；表情面板的颜文字页在「颜文字」（All）之后逐组列出 `kaomoji` 组。两处的搜索都在内置结果后面接上关键词或条目匹配的插件符号。组经 `msime_client_emoji_catalog_request` 的 `list_plugin_symbol_groups` 读，打开面板时读一次，坏组跳过；放置规则与鸿蒙电脑的 `PluginSymbolGroupPolicy`、桌面的 `append_plugin_symbol_groups` 相同。
- 背单词的请求带上插件目录，单词本插件列在内置书之后、导入的词表之前，标「插件」，不能在背单词里删除。

## 与桌面文案不同的地方

- 打开 K 模式的说法换成 iOS 的入口（「在候选栏点「水杉输入法」，或在「更多 → 本地输入」里选…」，同 `iosLocalModeEntry`）；短语表详情的「前往输入设置」换成直接推入「快捷模式」页，辅助码表的「前往辅助码设置」推入「更多辅助码设置」。
- 按键音效包的说明不提「声音与效果」（iOS 没有这一页，也没有上屏音和成就音效），改说按键音开关在「键盘 → 按键反馈」、要允许完全访问。
- 方案打不开模式的说明分两种：K 模式在 iOS 上只能从本地输入菜单进，而五笔没有这个菜单，所以说「切换到全拼或双拼」；/ 模式在五笔下仍能从符号键盘的 `/` 进，沿用桌面的「全拼、双拼或五笔」。
- 「快捷模式」页里「指令」的说明不列 `fy` 翻译：iOS 键盘不转交 / 模式的翻译请求，`/fy` 在 iOS 上不出译文。
- 编辑器删一行用左滑，不画每行的「删除」按钮；社区插件的音效包详情多一句「这台设备只播放按键音效包」，因为社区条目不带 `mode`，装之前分不出旋律包。

## 旧笔记审计

检索活跃笔记里的插件页、插件编辑器、按键音效包、辅助码表导入和符号集。[设置页插件编辑器](2026-10-26-in-app-plugin-editor.md)定义 `authoring_load` / `authoring_save` 和编辑器规则，本篇在 iOS 上用它们，部分重叠，那篇「移动端没有编辑器」一句已改成只说 Android；[iOS 按键音效包](2026-10-25-ios-key-sound-packs.md)定义键盘怎样播包、键盘页怎样选包，本篇只是多一个从插件页选包的入口，写的是同一个 `plugins.key_sound.pack`，继续有效；[手机导入单个辅助码表](2026-10-12-mobile-helpcode-table-import.md)的入口在 iOS 上保留，与本篇的 `.zip` 导入并存。鸿蒙和安卓手机的插件页由各自的笔记记录。本篇不取代已有决定。

## Alternatives considered

- 社区插件放进「社区」标签页，做成「皮肤 / 词库 / 插件」第三段：社区内容集中在一处，和词库社区同一个入口。但插件装好以后要回到插件页启用、选用，桌面也是在插件页里分「我的插件 / 社区插件」两段；三个手机端都照桌面结构，跨端的说法（「插件 → 社区插件」）才一致。
- 编辑器像桌面一样在列表栈里推入一页，保存后原地换成详情：与桌面完全一样。但 SwiftUI 的导航栈里，从编辑页换到详情要么改写整条路径，要么叠在编辑页上面，返回时又回到已经保存过的表单；弹出页关掉再推入详情没有这个问题，放弃修改的确认也能挂在弹出页的下拉关闭上。
- 「指令」入口只给 26 键：九键的数字键打不出字母。但进了本地模式九键已经换成 26 个字母，测试在九键网格下打出了触发词并选中了插件指令，限制入口只会让九键用户找不到它。
- 插件页按 `Form` 之外的设计卡片（`DesignGroup`）画：与词库页的外观一致。但插件页的行大多是开关、说明和多行表单，`Form` 的分区、脚注和左滑删除正好合用，辅助码页、快捷模式页也是 `Form`。
- 一项拆成两个 PR（设置页一个，键盘入口、符号面板和背单词一个）：每个 PR 更小。但设置页的符号集详情说「装上即在符号面板里显示」、单词本详情的「去背单词」都要键盘那一半先在，分开合并时总有一段时间页面说的和键盘做的不一致；键盘那一半只有两百来行，合成一个 PR。

## Consequences

收益是 iOS 用户能导入、自己做、从社区装和发布插件，装上的短语表、指令表、符号集、单词本、按键音效包和辅助码表在键盘里起作用；规则仍只有 client-services 一份，编辑器和页面上的检查只是先提示一步，与宿主不一致时以 `msime_client_plugins` 的拒绝为准。

代价是 iOS 设置多了一份插件页的界面代码，桌面插件页的文案或规则改了要跟着改这里（`PluginCatalog.swift`、`PluginEditorModel.swift`）。特效包、音乐包、旋律包能经 `.zip` 或社区装进来却不起作用，列表只给个数。`/` 模式默认关，打开以后中文标点下点符号键盘的 `/` 不再直接写出斜杠，而是进 / 模式（与桌面相同，紧接着按回车或再点一次 `/` 仍能写出斜杠）。插件符号组每次打开面板读一次插件目录，包多时打开面板略慢。

## Verification

- `KeyboardTests/settings/PluginCatalogTests.swift`、`PluginEditorModelTests.swift` 覆盖目录解析和分组、行尾标记、找不到的选择、共享文档的写法、失败提示、方案判断、社区翻页，以及经真的 `msime_client_plugins` 在临时状态目录里新建、列出、读回和删除一个短语表；编辑器的每条检查、批量粘贴、整理成 AuthoredPack 和保存后的提示。
- `KeyboardTests/settings/PluginKeyboardTests.swift` 用 `authoring_save` 装包、写共享设置，再建真的输入会话：启用的指令表在 26 键和九键网格下经 `/` 模式出候选并上屏，英文标点下 `/` 打不开模式；短语表启用后已打开的键盘重读设置就进 K 模式；符号集的组按规则进符号面板的分类和颜文字组，坏组跳过；`/` 指令默认不在菜单里。
- 以上和改动碰到的既有测试类（`VocabularyReviewTests` 新增一项单词本插件、`LocalModeSettingsTests`、`KeyboardEmojiCatalogTests`、`HelpcodeChoicePolicyTests`、`KeySoundPackPolicyTests`）在 Xcode 27 / iOS 27.0 模拟器上 78 项全部通过；整套 `MSIMEClientTests` 交给 PR 的 CI。
- `UITests/PluginsUITests.swift` 经深链打开插件页，新建短语表、在详情里启用、看列表标记和社区插件；`KeyboardExtensionEditorUITests.testPluginCommandAndSymbolSetReachTheSystemKeyboard` 在插件页做指令表和符号集，再在系统键盘里从「更多 → 本地输入 → 指令」打出插件指令上屏，长按 123 在符号面板里找到插件分类。两项在 iOS 27.0 模拟器上通过，截图在 PR 里。编译机经 SSH 没有图形会话，界面用例是在编译机上 `build-for-testing`、拉回本机模拟器 `test-without-building` 跑的。
- 没有在真机上验证；颜文字页的插件组只有单元测试，没有截图。
