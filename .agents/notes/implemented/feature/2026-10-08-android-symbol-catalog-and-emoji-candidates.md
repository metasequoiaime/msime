# Agent Note: Android 符号面板接入 Engine 符号目录，设置里补上候选带 emoji / 颜文字

Status: implemented

## Problem

#5667 有两个请求。一是「词汇和图案联动」：输入 meiguo，第一个候选是「美国」，第二个是 🇺🇸。二是「文本符号过少」：Android 符号面板只有写死的常用、中文、英文、数字、网络五类，用户拿另一款输入法的符号面板和 yuyansdk 的 `EmojiconData.kt`（大量符号和颜文字）作参照。

查下来两件事 Engine 都已经有了。候选混入 emoji / 颜文字是 `crates/engine/src/ime/queries.rs` 的 `mixed`，按拼音查随包 `msime-others.db` 的 `emoji_pinyin`、`kaomoji` 表，开关是共享偏好 `mixed_input.emoji` / `mixed_input.kaomoji`；Windows、macOS 默认开，其他平台默认关，而 Android 的设置页没有这两个开关，用户无从打开。符号目录也在同一个库里：`kaomoji_catalog` 约一千四百个颜文字，`symbol_catalog` 两千多个符号分在十二个上级分类下，iOS 键盘和桌面面板都经 `msime_client_emoji_catalog_request` 读它，Android 只把表情那一部分接进了表情面板。

## Decision

- 设置「表达 › 智能」加「候选带 emoji」「候选带颜文字」两个开关，写共享偏好 `mixed_input.emoji` / `mixed_input.kaomoji`，默认值不变（Android 上仍是关）。`MixedInputPreferences` 四个字段都必填且拒绝未知字段，快照里缺这个对象或缺字段时按共享默认值（english=true、minimum_prefix=5、emoji/kaomoji=false）补齐再写回，否则整份偏好会被拒绝。同一组里排在这两个开关前面的「中英混输」开关和 1–8 的「触发字母数」滑块（#7081）写同一个对象的 `english` / `minimum_prefix`，经同一个补齐函数写回；开关关闭时滑块置灰，与桌面、iOS 一致。
- 符号面板在五个写死的分类之后加「颜文字」和十二个符号上级分类（形状、箭头、标点、数学、货币、爱心、字母、游戏、文化、自然、人物、更多；中文名与 iOS `KeyboardEmojiCatalog.symbolParentTitles` 相同）。这些分类没有写在宿主里：点到时在表情目录的工作线程上经 `NativeClient.emojiCatalog` 每次读 64 条（颜文字 `category=kaomoji, group=All`，符号 `category=symbols, parent=<上级分类>`），滚到底再读下一页，最多 2048 条。同一个符号在目录里属于几个小类时只显示一次。每一页都校验：条数、游标前进（与表情目录相同的规则）、文字非空无 NUL、颜文字不超过 96 个码位、符号不超过 32 个；任何一条不合格整页作废，面板显示「目录暂时不可用；点分类重试」。
- 颜文字每行两个、字号 14；不记进「常用」，因为「常用」是每行五格。目录里的成对符号同样遵守「自动补全成对标点」和长按只输入半个（见 [2026-10-08-android-paired-punctuation.md](2026-10-08-android-paired-punctuation.md)）；「常用」的记录规则见 [2026-10-08-android-symbol-panel-recents.md](2026-10-08-android-symbol-panel-recents.md)。

## Alternatives considered

- **照 issue 的建议，把 yuyansdk 的 `EmojiconData.kt` 抄进宿主**：它的符号和颜文字分类正是用户想要的样子，许可证是 BSD-3-Clause，与本仓的 GPL-3.0 兼容，只要保留版权声明。但那样宿主里会多一份与 Engine 目录重复、只在 Android 上有的数据，iOS、桌面看到的又是另一套；按仓库约定数据归共享层，Engine 目录已经覆盖了它的大部分分类，用现成的那份。
- **自己在宿主里写一批颜文字**：没有许可证问题，也不依赖资源文件。但数量只能是几十个，而随包目录有一千多个、带拼音关键字，还会和 iOS、桌面不一致。
- **改默认值，让 Android 默认就在候选里带 emoji**：用户看不到开关也能用上。但 `source_mixed_emoji_default` 的默认值是按来源产品定的，Android 一直是关；改默认会让所有现有用户的候选栏突然多出 emoji，这次只补开关。
- **把颜文字和符号放进表情面板（iOS 的做法）**：表情面板的分类栏是一行九个图标、网格固定每行八格，放颜文字要另做宽格子和分类栏；用户的诉求和截图都是符号面板，符号面板左列本来就能滚动、能放任意多类。

## Consequences

- **收益**：用户能在 Android 上打开已有的 emoji / 颜文字混输；符号面板多了一千多个颜文字和两千多个符号，数据与其他平台一致、随词库更新，宿主没有多一份表。
- **代价与缺口**：九键当时不混入 emoji（Engine 的 `NineKeySession` 不走 `mixed`），开关说明写的是「26 键全拼、双拼」；#5848 在 Engine 里给九键补上了这条路径，说明随之改为「全拼（26 键或 9 键）、双拼」，见 [2026-10-08-nine-key-expressive-candidates.md](2026-10-08-nine-key-expressive-candidates.md)。目录分类里有些符号在旧系统字体里没有字形，起初没有像表情面板那样按字形过滤，#6070 在 Android 11（vivo）上报出「箭头」两行空白格（⭠⭡⭢⭣⭤⭥⮂⮃⮐⮑）和「爱心」里三个空白格（Unicode 15 的 🩷🩵🩶），点了上屏的是看不见的字；之后符号目录的每一页也在 `emojiWorker` 上经同一个 `Paint.hasGlyph` 过滤（`SymbolPanelModel.renderableCatalogItems`），游标仍按扫过的行数前进。过滤上线前点过的空白格已经记进「常用」，面板一打开就是「常用」，所以读「常用」时也在主线程上用另一个 `Paint` 过滤（`SymbolPanelModel.renderableRecents`），只判断一个码位（可带变体选择符）的条目，「http://」这类多字形条目原样保留。候选栏里的 emoji（`mixed_input.emoji`）仍没有按字形过滤：候选按 Engine 的序号选择，宿主隐藏某一项会打乱翻页和数字键选词，需要另做。颜文字不过滤：`hasGlyph` 只在整串排成一个字形时回答「能画」，对多字形的颜文字一律是「不能」，按它过滤会把颜文字全部去掉；逐个码位判断又会误杀组合附加符号拼出的颜文字，目前没有颜文字显示不出的报告，暂不处理。上级分类是写死的十二个，目录以后新增的上级分类不会自动出现。只在 `check-host.sh`（JVM 冒烟、守卫）上验证，没有在真机上打开过这些分类。「中英混输」只补设置项，运行时一直在按这两项偏好出英文候选。`GroupCard.slider` 没有副标题，触发长度的说明只能写进开关的副标题，写成「输入的字母或按键数」，好把九键、14 键按按键数计的情况包进去，不逐个列举键盘，也不说关闭后完全没有英文：九键和 14 键在拼音一行候选都没有且没有锁定拼音时、或处在英文模式时，Engine 不看 `english` 和 `minimum_prefix` 照样给英文词（`nine_key.rs` 的 `english_candidates`）。表达页没有设备冒烟，「写回时四个字段齐全」靠 `check-host.sh` 的守卫和手工验证，搜索关键词靠 `SettingsSearchIndexSmoke`。#7081 的报告者可能要的是中日韩混输，这次只补 Android 设置与其他平台的差距。HarmonyOS 的设置用的是共享设置页，这两项已在「输入 › 更多 › 候选与联想」里（`input-shared-settings-section.tsx` 的 `MixedInputSection`），不缺设置项；那边的缺口是设置应用与键盘进程不共用偏好文件（`platforms/harmony/README.md`），与本次无关。

[粗等号候选绘制兜底](../bug-fix/2026-10-15-android-heavy-equals-candidate-glyph.md)补上 #8038 中旧系统缺失 U+1F7F0 的可见性；它保留候选身份与原始文字，不改变本篇的候选不按字形过滤策略。

[Android 组合表情](2026-10-16-android-emoji-combinations.md)补充 #8193 附件中不在普通 Emoji 目录内的 165 个多字形聊天短语，单独保留共享文本资源与上游 BSD 通知。本篇继续持有普通符号、颜文字目录和候选混排规则。
