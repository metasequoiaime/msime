# Agent Note: Android 符号面板接入 Engine 符号目录，设置里补上候选带 emoji / 颜文字

Status: implemented

## Problem

#5667 有两个请求。一是「词汇和图案联动」：输入 meiguo，第一个候选是「美国」，第二个是 🇺🇸。二是「文本符号过少」：Android 符号面板只有写死的常用、中文、英文、数字、网络五类，用户拿另一款输入法的符号面板和 yuyansdk 的 `EmojiconData.kt`（大量符号和颜文字）作参照。

查下来两件事 Engine 都已经有了。候选混入 emoji / 颜文字是 `crates/engine/src/ime/queries.rs` 的 `mixed`，按拼音查随包 `msime-others.db` 的 `emoji_pinyin`、`kaomoji` 表，开关是共享偏好 `mixed_input.emoji` / `mixed_input.kaomoji`；Windows、macOS 默认开，其他平台默认关，而 Android 的设置页没有这两个开关，用户无从打开。符号目录也在同一个库里：`kaomoji_catalog` 约一千四百个颜文字，`symbol_catalog` 两千多个符号分在十二个上级分类下，iOS 键盘和桌面面板都经 `msime_client_emoji_catalog_request` 读它，Android 只把表情那一部分接进了表情面板。

## Decision

- 设置「表达 › 智能」加「候选带 emoji」「候选带颜文字」两个开关，写共享偏好 `mixed_input.emoji` / `mixed_input.kaomoji`，默认值不变（Android 上仍是关）。`MixedInputPreferences` 四个字段都必填且拒绝未知字段，快照里缺这个对象或缺字段时按共享默认值（english=true、minimum_prefix=5、emoji/kaomoji=false）补齐再写回，否则整份偏好会被拒绝。
- 符号面板在五个写死的分类之后加「颜文字」和十二个符号上级分类（形状、箭头、标点、数学、货币、爱心、字母、游戏、文化、自然、人物、更多；中文名与 iOS `KeyboardEmojiCatalog.symbolParentTitles` 相同）。这些分类没有写在宿主里：点到时在表情目录的工作线程上经 `NativeClient.emojiCatalog` 每次读 64 条（颜文字 `category=kaomoji, group=All`，符号 `category=symbols, parent=<上级分类>`），滚到底再读下一页，最多 2048 条。同一个符号在目录里属于几个小类时只显示一次。每一页都校验：条数、游标前进（与表情目录相同的规则）、文字非空无 NUL、颜文字不超过 96 个码位、符号不超过 32 个；任何一条不合格整页作废，面板显示「目录暂时不可用；点分类重试」。
- 颜文字每行两个、字号 14；不记进「常用」，因为「常用」是每行五格。目录里的成对符号同样遵守「自动补全成对标点」和长按只输入半个（见 [2026-10-08-android-paired-punctuation.md](2026-10-08-android-paired-punctuation.md)）；「常用」的记录规则见 [2026-10-08-android-symbol-panel-recents.md](2026-10-08-android-symbol-panel-recents.md)。

## Alternatives considered

- **照 issue 的建议，把 yuyansdk 的 `EmojiconData.kt` 抄进宿主**：它的符号和颜文字分类正是用户想要的样子，许可证是 BSD-3-Clause，与本仓的 GPL-3.0 兼容，只要保留版权声明。但那样宿主里会多一份与 Engine 目录重复、只在 Android 上有的数据，iOS、桌面看到的又是另一套；按仓库约定数据归共享层，Engine 目录已经覆盖了它的大部分分类，用现成的那份。
- **自己在宿主里写一批颜文字**：没有许可证问题，也不依赖资源文件。但数量只能是几十个，而随包目录有一千多个、带拼音关键字，还会和 iOS、桌面不一致。
- **改默认值，让 Android 默认就在候选里带 emoji**：用户看不到开关也能用上。但 `source_mixed_emoji_default` 的默认值是按来源产品定的，Android 一直是关；改默认会让所有现有用户的候选栏突然多出 emoji，这次只补开关。
- **把颜文字和符号放进表情面板（iOS 的做法）**：表情面板的分类栏是一行九个图标、网格固定每行八格，放颜文字要另做宽格子和分类栏；用户的诉求和截图都是符号面板，符号面板左列本来就能滚动、能放任意多类。

## Consequences

- **收益**：用户能在 Android 上打开已有的 emoji / 颜文字混输；符号面板多了一千多个颜文字和两千多个符号，数据与其他平台一致、随词库更新，宿主没有多一份表。
- **代价与缺口**：九键当时不混入 emoji（Engine 的 `NineKeySession` 不走 `mixed`），开关说明写的是「26 键全拼、双拼」；#5848 在 Engine 里给九键补上了这条路径，说明随之改为「全拼（26 键或 9 键）、双拼」，见 [2026-10-08-nine-key-expressive-candidates.md](2026-10-08-nine-key-expressive-candidates.md)。目录分类里有些符号（盲文、国际象棋等）在旧系统字体里可能显示成方框，这次没有像表情面板那样按字形过滤。上级分类是写死的十二个，目录以后新增的上级分类不会自动出现。只在 `check-host.sh`（JVM 冒烟、守卫）上验证，没有在真机上打开过这些分类。
