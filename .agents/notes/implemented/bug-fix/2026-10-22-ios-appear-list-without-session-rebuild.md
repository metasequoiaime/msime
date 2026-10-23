# Agent Note: iOS 键盘出现时刷新个人词库列表不再重建会话

Status: implemented

## Problem

在 iPhone XR 上用 os_signpost 和 Instruments 测到：键盘出现时主线程在 `MSIMEInputThread.sync` 上等引擎约 45 ms，Rust 侧 `msime_client_create` 约 89 ms CPU、`msime_client_focus` 约 59 ms CPU，其中大半花在资源包路径检查上。开着完全访问时，这一对调用每次出现要做两遍：

- `viewWillAppear` → `resumeDictionarySession` → `createFocusedSession`，收起时销毁的会话在这里重建，这一遍是必要的（收起时必须交还词库租约，见 `suspendDictionarySession` 上关于 0xdead10cc 的注释）。
- `viewDidAppear` → `synchronizePersonalDictionary(force: true)` → `PersonalDictionaryStore.synchronize` 的 `page` 回调 → `personalEntries` → `withDictionaryMaintenance`：销毁刚建好的会话、列一页词条、再建一次并聚焦。

第二遍是这次要去掉的。

## 强制同步为什么存在

App 自己从不打开 Engine 词库，个人词库页（`PersonalDictionaryView`）、词库页的「N 条自造词」（`DictionarySettingsView.wordCountLabel`）和统计页的自造词数（`TypingStatisticsView`）都只读 App Group 里 `PersonalDictionary/sync.json` 的快照：`entries`、`hasMore`、`snapshotDate`。这份快照只有键盘写。

App 只在用户动作时请求刷新：搜索、翻页、点刷新按钮、编辑、导入和导出会换一个新的 `refreshID`（`requestPage`、`enqueue`、`enqueueImport`、`requestExport`），键盘的两秒定时器看到 `refreshID != completedRefreshID` 才去同步。打开个人词库页本身不发请求，页面只是每秒重读一次 `sync.json`。词库页和统计页从不发请求。

所以键盘每次出现时的 `force: true` 是这几页数据新鲜度的唯一来源：用户在别的应用里打字学到的词（比如回车上屏学到的英文词，`learn_entered_english_word` 写成 `user_inserted=1` 的行），要等下一次键盘出现时被列进快照，App 才看得到。去掉强制同步，这些页会一直停在上一次有人请求刷新时的样子，而 App 前台时键盘通常不在，用户只能自己去试打框里把键盘叫出来。这是用户看得见的变化，需要产品拍板，所以本篇不动强制同步。

## Decision

强制同步保留，列表不再销毁会话。`MetasequoiaInputSessionBridge.personalEntries` 直接调 `msime_client_dictionary` 的 `list`，不再包在 `withDictionaryMaintenance` 里。键盘出现时只剩 `viewWillAppear` 那一次创建和聚焦；`viewDidAppear` 的同步变成在主线程上读写一次 `sync.json`、跑一条列表查询。

可以这样做的依据：

- 租约：host-api 的 `Operation::List` 取的是 `DictionaryAccess::try_session`（共享），从 2026-09-11 引入时就是这样；输入会话持有的也是共享租约，两者并存（`client-services` 的 `shared_access_excludes_maintenance_and_releases_cleanly` 守着）。只有维护者持有独占租约时列表才报 `dictionary maintenance busy`，`synchronize` 已把这种失败记进 `snapshotError` 并保留上一份列表和所有编辑回执。桌面设置页和 MCP 一直在输入法会话活着时列个人词库，iOS 是唯一为列表重建会话的宿主。
- 数据：列表读日志里的 `user_dictionary_operations`（`user_inserted=1` 的行），这些行由 `record_user_insert` 在上屏时同步写入。会话延迟写的只有个人 n-gram 转移（`PersonalNgramStore`，约 2 秒后台写），销毁会话时 `Drop` 会把它们刷进日志，但列表不读这两张表。所以销毁会话不会让列表多出任何一行，留着会话列出来的结果和原来一样。
- 线程：`msime_client_dictionary` 在调用线程（主线程）上执行，列表结束时 `release_thread_journal` 关的是主线程缓存的日志连接，输入会话在 `app.msime.ios.input` 线程上，它的连接不受影响。

`withDictionaryMaintenance` 当初（2026-09-14，`398ec04465`）把编辑和列表一起包进去，没有记录列表需要它的理由；编辑（`applyPersonalPrevious`）取的是 `try_maintenance`，必须先销毁会话交出共享租约，这一点不变。导出（`personalExport`）在 host-api 里同样只取共享租约，理论上也不必重建会话，但它只在用户请求导出时运行，不在出现路径上，本篇不改。

## Consequences

收益：开着完全访问的键盘每次出现少一次 `msime_client_destroy`、`msime_client_create` 和 `msime_client_focus`。按 XR 上的测量，主线程少等一次会话创建和聚焦，Rust 侧少约 150 ms CPU。App 各页看到的数据和刷新时机不变。

行为上有一处差别：原来第二次创建用的是 `options`，而 `viewWillAppear` 里异步的 `reloadSharedPreferences` 常常在 `viewDidAppear` 之前就把新的偏好文档写进了 `options["preferences"]`，于是第二次创建会顺带让不在 `appEditedKeys`、也不经 `applyLearningPreferences` 等调用实时生效的引擎字段在这次出现就生效。现在这类字段在下一次出现、`resumeDictionarySession` 用已更新的 `options` 建会话时生效。这是没开完全访问的用户一直以来的行为（他们根本不走强制同步），也符合 `reloadSharedPreferences` 里「键盘可见时保持会话的引擎配置不变」的设计；原来那种生效靠的是异步读取抢在 `viewDidAppear` 之前完成，本来就不稳定。

`withDictionaryMaintenance` 原来顺带保证列表时没有组字（组字时直接报「请先结束当前输入再同步个人词库」）。键盘的 `synchronizePersonalDictionary` 自己已经在组字、本地模式时跳过，列表又不碰会话，这层保证不再需要。

队列里有待应用的编辑时，同步仍会为每条编辑销毁、重建会话，和原来一样。

PR #9303（尚未合并）把两秒定时器的文件读取移到后台，改的是 `KeyboardViewController` 的定时器和 `PersonalDictionaryStore`；本篇只改桥接层的 `personalEntries` 和测试，两者没有文本冲突，叠在一起时定时器触发的同步同样不再为列表重建会话。

## Alternatives considered

- **键盘出现时不再强制同步，改由 App 在页面出现时请求刷新。** 最强的理由是彻底去掉出现路径上的词库工作，主线程连 `sync.json` 和列表查询都不用做。不用它：App 前台时键盘一般不在，请求要等用户下次叫出键盘才被处理，个人词库页、词库页的自造词数和统计页都会停在上一份快照上，在别处刚学到的词看不到；要么让用户每次进页面都去试打框，要么接受这种滞后，两者都是产品取舍，不该夹在一次性能修复里。
- **保留重建，只把第二次创建挪出主线程。** 能消掉主线程上的等待，但会话的 C ABI 绑在输入线程上，重建期间按键仍要排队，而且 Rust 侧的 CPU 一点没省；列表本来就不需要重建，挪线程是在掩盖一笔多余的开销。

## Verification

- 新增 `PersonalDictionaryStoreTests.testListingTheUsersWordsLeavesTheLiveSessionAlone`：经引擎写入一条合成快捷短语，在会话组着「nihao」时列表，列表含这条词，组字之后再收一个键仍接在原来的组字后面。原实现在列表这一步报「请先结束当前输入再同步个人词库」，即使不组字也会丢掉组字状态。
- `NineKeyKeyboardTests.testNineKeySurvivesTheSessionRebuiltForDictionaryMaintenance` 原来靠列表触发重建来守九键网格的重放，列表不再重建后它就守不住了，改用仍然重建会话的导出。
- iOS 27.0 模拟器（iPhone 17 Pro）上跑 `MSIMEKeyboardTests` 的 `PersonalDictionaryStoreTests`、`NineKeyKeyboardTests`、`DictionarySnapshotWorkerTests`、`CloudSnapshotStagingTests` 和 `CloudDictionaryTransferTests`：140 项执行，0 失败，1 项因测试包不带笔画词库按原样跳过。把桥接层换回原实现单跑新用例，它以「请先结束当前输入再同步个人词库」失败。
- 没有在真机上用 Instruments 复测出现时的耗时，上面的收益是按原测量里第二次创建和聚焦的开销推算的。也没有在真机上验证 App 个人词库页在键盘出现后照常刷新，这一点靠的是强制同步和 `synchronize` 的流程都没有改。
