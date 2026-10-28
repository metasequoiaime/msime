# Agent Note: iOS 键盘在低端机上的耗电、发热和卡顿

Status: implemented

## Problem

低端手机用户报告键盘耗电、发热、打字卡。一次逐平台的代码审查在 iOS 键盘和它依赖的共享层里找到一批每次按键、每次出现或每隔几秒就重复做、而结果几乎从不变化的工作，大多在主线程上：

- 出现路径：`viewDidAppear` 每次都 `synchronizePersonalDictionary(force: true)`（主线程读写 `sync.json`、查 100 条个人词条）并强制重算词库版本；`viewWillAppear` 无条件整棵换装（重画背景、给每个按键包括藏着的布局重设配置、设计皮肤每键新建表面），还清掉候选皮肤请求键，使已解码的候选皮肤整包重新读、重新解码；第一次出现把大千注音、14 键、18 键的七十多个按键全建好再藏起来；每次出现重建引擎会话时，`PersonalNgramStore` 把个人二元、三元转移整表（上限 20 万条）重新读进内存。
- 每次按键：英文模式每个字母在主线程上同步跑一次整段纠错搜索；每次空闲渲染把整份偏好文档经 C ABI 推两遍，值没变也照推；照片皮肤的照片（base64，最大约 680 KB）留在交给引擎的偏好里，每次建会话、列词库、查版本都要编一遍 JSON、Rust 再解一遍；`InputSchemePreference.appGroupDefaults` 每读一次就新建一个 `UserDefaults`，满页候选时一次按键五十到八十个；开着释义时每次停顿都 `allCandidates()` 取回几百个候选再整份查离线词典；释义行截断每删一个字量一次宽度。
- 后台节拍：快照工作者每 10 秒把学习日志里的操作、固定位置和选词状态整份读一遍做 SHA-256，然后在主线程上取锁、读、写一遍 `state.json`，版本几乎总是没变；手写时每个触摸事件整块重画面板和所有已写笔画；按住说话之后主 App 的音频引擎在后台一直开 5 分钟。

这些都没有在真机上量过耗电，只有审查时的代码路径分析和早先 iPhone XR 上的 os_signpost 采样（见 [出现时列表不再重建会话](2026-10-22-ios-appear-list-without-session-rebuild.md)、[维护定时器的读取移出主线程](2026-10-22-ios-idle-maintenance-off-main.md)）作依据。

## Decision

每一项都只去掉「结果不变还在重复做」的那部分，可见行为保持原样；确有可见差别的三项（个人词库列表刷新时机、词库版本新鲜度、语音会话时限）单独列在 Consequences。

出现路径：

- 键盘出现时不再强制同步个人词库，改为立即跑一轮现有的空闲维护（`startIdleMaintenance`）：两份队列文件在后台读，个人词库只在有待应用的编辑、导出、设置页的刷新请求，或 `sync.json` 超过 `PersonalDictionaryStore.listingLifetime`（15 分钟）没被写过时才回主线程同步。个人词库页、词库设置页和统计页打开时调 `PersonalDictionaryStore.requestRefresh()` 要一次刷新。陈旧判断用文件修改时间而不用 `snapshotDate`：列出失败时 `snapshotDate` 不变，按它判断会每轮空闲都重试，而键盘每次同步无论成败都会重写这个文件。
- `applyKeyboardSkin(onlyIfChanged:)` 把背景重画、按键树重走和回车键配置收进 `recolorKeys`，记下这一遍用的皮肤和 trait。`viewWillAppear` 只按皮肤是否变了决定要不要重走；trait 变了、有按键在窗口外经过 `decorateKey`（阴影色按默认外观解析）这两项交给 `viewIsAppearing` 补走，那时视图已在窗口里、trait 已定、第一帧还没画。按状态更新的部分（回车、方案键、工具栏、空格、候选条）每次照做；换季仍强制整棵重走。`decorateKey` 复用已有按键表面。`viewWillAppear` 不再清候选皮肤请求键，键里已有开关、明暗、主题、皮肤目录和清单的修改时间与大小，内存警告照旧清键。
- 大千注音、14 键、18 键的各排由 `ensureAlternateLetterRows` 在第一次要画时建，插在按键列原来的位置，建好后一直留着。布局重载测试在切换前检查行尚未创建，切换后再条件等待真实按键出现，不要求启动时存在隐藏的备用行。
- 引擎会话仍在每次收起时销毁（不能带着 App Group 文件锁被挂起，否则 0xdead10cc）。重建时不再整表重读个人上下文模型由本 PR 叠着的共享 Rust PR 负责：会话放手时记下日志的文件身份和 `PRAGMA data_version`，下一个会话打开同一日志、期间没有别的连接改过时沿用模型（细节见 [共享 Rust 层空闲 I/O](2026-10-27-shared-rust-idle-io-hot-path.md)）。iOS 的日志不进 WAL，只读过日志的 store 留下的那条只读探测连接在回滚日志模式下空闲时不持锁。本 PR 在诊断日志里补 `dictionary_resumed ms=`、`dictionary_suspended ms=`、`keys_reskinned in_window=`，供真机确认重建花多久、沿用是否生效。

每次按键：

- 英文逐字母纠错走 `englishCorrections(forPrefix:completion:)`，在 `MSIMEInputThread` 上算、结果回主线程，被新请求取代的排队查询直接跳过；回来时候选条已被别的路径改过、换了输入框或光标前的词已变就丢弃。同步版本也在输入线程上算并复用最后一个字母的结果，空格自动纠错（`commitEnglishBoundary`）仍同步取，行为不变。新词查询排队前先展示当前原文，旧纠错和已不匹配的邮箱后缀立即撤下，候选条不闪回工具栏；同一输入框同一词已有的答案保留。可点的英文建议记住原文和 `documentIdentifier`，点选时重新核对，外部编辑或换输入框后的旧答案只收起，不能替换当前文档。任何非英文路径重写候选条都会清掉这份身份。读取当前英文词时拒绝非空选区，点击旧建议和边界自动纠错都不会把选区当作词尾退格删除。
- 桥记下当前会话最后一次已生效（`deferred` 为 false）的偏好和隐私会话标记，完全相同的请求直接报告成功、不跨 FFI。组字中被推迟的那次不记，组字结束后照旧再发一次。会话重建、`applyAppEditedPreferences` 都重新记。
- 交给 Rust 会话和词库的偏好都经 `MetasequoiaInputSessionBridge.engineBound` 去掉 `custom_theme.keyboard.photo`（`photoShade`、`photoPosition` 保留）；`options["preferences"]` 和 `sharedPreferences` 仍是完整文档，皮肤照旧读得到照片，写回共享文档的路径都先从磁盘重读。`ResolvedTheme.resolve` 的缓存键里照片换成长度加哈希。
- `appGroupDefaults` 改为 `static let`。CFPreferences 的值缓存按进程和 suite 共享，不跟着实例走，读到的值和看到的设置 App 写入与每次新建时相同。
- 候选网格没开时，释义只为候选条这一页请求，代次取渲染出来的视图的 `visibleInputGeneration`；网格打开时用刚读出的完整列表补查整份，`candidateGlossRequestedFullList` 记下那一代查的是不是整份。
- `KeyboardKeyButton.fittedGloss` 按前缀长度二分查找放得下的最长前缀，只记量过确实放得下的结果。

后台节拍：

- `DictionarySnapshotQueue.publishLocalVersion` 只在版本变了或有请求因此标为已应用时才写 `state.json`；`DictionarySnapshotWorker.tick` 读出的版本和刚读到的状态一样、又没有待处理请求时，直接结束，不回主线程取锁。
- `msime_client_snapshot_version` 的预览版本在共享 Rust PR 里按日志、`-wal`、`-journal` 的文件身份缓存，最后一次改动不足两秒的日志不进缓存；准备和激活快照核对版本仍走不缓存的精确计算。这一项本来在 iOS 这条线里另做过一份 host-api 侧的缓存，与 Rust 那条线的引擎侧缓存重复，合并时只留了覆盖 `state`、`process` 和 `snapshot_version` 三处的那一份。
- 没有活动请求、状态文件里已有版本时，两次读版本至少隔 `DictionarySnapshotWorker.idleVersionPollInterval`（60 秒）；有活动请求或还没有版本时立即读，读失败后的节流也用这个间隔。打字时个人上下文模型约每两秒写一次学习日志，按文件身份的缓存在打字期间总对不上，这个间隔才是打字时真正起作用的那道闸。
- 手写画布在 `touchesMoved` 里经 `extendStroke` 只让「原末点到新点」外框外扩半个笔宽加 2 pt 的区域重画，`drawInk` 跳过碰不到这块区域的笔画；清空、撤销、换皮肤、改笔迹或卡片位置仍整块重画。
- `VoiceSessionChannel.idleLifetime` 从 300 秒降到 60 秒。听写之间不能停音频引擎（App 在后台不能重新开始录音），所以保留会话设计，只缩短空闲时限。

打字统计：共享 Rust PR 把 `record` 改成在进程里攒批（距上次写满 30 秒的下一次上屏才写，不再每次上屏整份重写加 `F_FULLFSYNC`），新增 `{"operation":"flush"}`，并在会话失焦和销毁时交给后台线程写出一次。本 PR 的 `TypingStatisticsStore.flush()` 在 `viewWillDisappear`、换输入框、`NSExtensionHostWillResignActive` 和原有的 30 秒按键定时器上调 flush，排在同一个统计串行队列上已交出的 `record` 之后，只有交过 `record` 才调；flush 失败只记诊断日志。本 PR 叠在那条 Rust PR 上，在它之后合入。

合入共享英文补全后，逐字母异步查询使用完整候选展示接口并保留 `corrected` 标记，不退回仅纠错的旧接口；按边界同步纠错复用完整展示中的唯一纠错结果，不把补全当作自动纠错。集成回归覆盖前缀补全、纠错标记、最后请求生效与边界查询。

## Alternatives considered

- **出现时保留强制同步个人词库**：[出现时列表不再重建会话](2026-10-22-ios-appear-list-without-session-rebuild.md) 当时为保住设置页数据的新鲜度留着它，理由是 App 前台时键盘一般不在，去掉之后各页会停在上一份快照上。不再保留：每次点输入框、切 App 都在用户开始打字的那一刻读写共享文件、查询引擎，而这几页数据的新鲜度可以由页面打开时的刷新请求加 15 分钟陈旧上限兜住，陈旧程度有上限，不再取决于用户多久没开过这些页面。
- **异步英文查询期间保留上一词的候选**：候选条内容不变，能避免每一键先闪回工具栏；但旧纠错仍可点击，若编辑器已换词便可能删掉新词，邮箱后缀也会拖到查询返回才撤下。新词先展示自己的原文也能保持候选条，不保留旧词答案；点击时的原文和输入框身份核对进一步兜住编辑器尚未回报的外部改动。
- **异步建会话，把 `msime_client_create`/`focus` 移出主线程**：能让 `viewWillAppear` 立即返回。没有做：主线程上读 `handle` 的地方（视图、组字判断、偏好重载回调）都要处理「正在创建」，快速收起时挂起和在途创建会竞争，多项测试要求 `resume` 之后立即有句柄；沿用磁盘上没变的个人模型已经去掉重建里随用户数据增长的那一块，先看真机 `dictionary_resumed ms=` 再决定值不值得承担这些时序风险。
- **收起时只交还词库租约和 SQLite 连接、保留整个 Runtime**：各词库连接分散在 `QuanpinDictionary`、`WubiProvider` 和各个 store 里，改成可关可重开是跨 crate 的大改，出错就是 0xdead10cc 被杀，比卡一下更糟。
- **把 `prepare_host` 移出第一帧路径**：`viewDidLoad` 同步读 `sharedPreferences` 和会话状态，按键在会话就绪前还要排队或丢弃，改动面大，这次只做了按需建按键行和跳过重复换装两项低风险的。
- **语音会话：调大空闲时的 IO 缓冲、心跳只在状态变了时写**：能在会话开着时少醒、少写文件，但大缓冲会让开头最多晚约 100 ms、可能吞掉第一个音节，心跳间隔又和键盘的失效判定 `staleAfter` 绑在一起，两边要一起改。缩短时限直接缩短引擎开着的总时长，改动只有一个常量。
- **手写：把静态卡片拆成独立的底层视图**：能让每帧不再画卡片和投影，但要多维护一个视图和它的约束、换皮肤时的同步；脏区重画不改视图结构，UIKit 裁剪后卡片只在那一小块里重画。
- **快照版本缓存在 Swift 侧做**：安卓、鸿蒙也反复调同一个接口，放在共享 Rust 层里三端都受益。

## Consequences

- 收益：没有任何变化时，键盘出现不再读写共享文件、不再整棵换装、不再重新解码候选皮肤，老用户也不再每次出现都整表重读个人转移；打字时主线程不再跑英文纠错、不再重复推偏好、不再为每个偏好读新建 `UserDefaults`；键盘停在屏幕上时不再每 10 秒重读日志、重写 `state.json`。
- 个人词库列表的刷新时机变了：设置页打开时请求刷新，要等键盘下次出现或空闲时才列出；在此之前看到的是上一次列出的结果，最多落后 15 分钟（键盘在用时）。键盘一直不出现时，请求一直挂着，这一点和以前相同。
- 已有版本、没有活动请求时，状态文件里的本地词库版本最多晚一分钟。它只给 App 下载云词库时记下预期版本用，晚了的代价是那次应用报「本地词库已变化，本次快照未应用。请重新确认。」，完成时会写出当时的版本，重试即可；激活前的版本核对不走缓存，不会把快照应用到变过的词库上。
- 按住说话的会话空闲一分钟就关，隔一分钟以上再说要多跳一次 App；语音页底部的「N 分钟不用会自动关闭」随常量变成 1 分钟。[按住空格说话](../feature/2026-10-21-ios-hold-to-talk-voice-session.md) 里的时限事实已同步。
- 开着释义时，停顿不再顺带放出引擎暂扣的尾部，候选条第一页不会在停顿后因尾部重排跳一下；网格里尾部候选的释义改为打开网格时才查。
- 英文联想从同步显示变成下一轮主线程显示，内容逐字不变；在途结果在 `updateCandidateStrip` 时作废，不会盖住别的路径写上的候选。
- 收起期间 `PersonalNgramStore` 的模型和（没有写入连接时）一条只读探测连接留在进程里，内存和收起前键盘可见时相同；iOS 的日志是回滚日志模式，空闲连接不持锁，所以不增加挂起时持锁的风险，这一点没有在真机挂起场景里验证。
- 大千注音、14 键、18 键第一次切过去时要现建按键行，代价从第一次出现挪到第一次切换。

## Verification

- 单元测试：KeyboardTests 新增或扩展了 `VoiceSessionTests`（时限不超过一分钟及与心跳、失效判定的大小关系）、`EighteenKeyKeyboardTests`（26 键不带备用行、切到 18 键才建、切回不重建）、`DictionarySnapshotQueueTests`/`DictionarySnapshotWorkerTests`（同版本不写文件、版本没变不回主线程、失败节流、已有版本后 30 秒不读满一分钟才读）、`PersonalDictionaryStoreTests`（超过 15 分钟没列过即使记住身份也要同步、列出失败不反复重试）、`KeyboardSkinTests`/`CandidateCanvasTests`（皮肤没变时再出现不重走按键树、窗口外的出现由 `viewIsAppearing` 补走、收起再出现沿用同一解码快照）、`EnglishSuggestionPolicyTests`、`InputBridgeResponseTests`、`KeyboardSkinDocumentTests`、`CandidateTranslationTests`（二分截断与逐字删在多种宽度下结果相同）、`HandwritingTests`（脏区重画与整块重画逐像素比较）和 `testCommitsAreHeldInMemoryUntilFlush`（record 之后文件里的总数不变，flush 之后才写进去）。个人上下文模型沿用和预览版本缓存的 Rust 用例随实现挪进了共享 Rust PR。
- 原实现阶段的编译：在 Mac Studio（Xcode 27.0）上 `platforms/ios/build-native.sh simulator` 产出 arm64 模拟器 `libmsime_host_api.a`，`xcodegen generate` 后 `xcodebuild build-for-testing -scheme MSIMEClientTests -destination 'generic/platform=iOS Simulator'`（键盘扩展、测试宿主和三个测试包）与 `xcodebuild build -scheme MSIMEApp` 都通过，告警全是改动之前就有的。iOS 测试套件要启动模拟器，这一轮没有运行，上面列的 Swift 用例只保证编译通过。
- 审查回归：CI 在异步化后检出邮箱后缀残留和三项备用布局预建假设。`EmailSuffixKeyboardTests` 补充旧纠错不替换另一词、相同词不跨输入框、下一次查询返回前只显示当前原文、非空选区时旧建议不改文本、空格只替换选区的真实控制器用例；当时尚未执行，不能以新增测试代替通过证据。
- 没有做的：没有在任何低端真机上测量耗电、温度或帧率，也没有用 Instruments 复测出现路径的耗时；收益都是按代码路径和早先 XR 上的采样推算的。真机上可以先看诊断日志里的 `dictionary_resumed ms=`、`keys_reskinned in_window=`，再用 Energy Log 对比打字和停留时的 CPU。
- 安卓同一批问题的处理见 [Android 键盘只在窗口显示时每秒重读偏好](2026-10-27-android-preference-poll-only-while-visible.md)。

- **英文候选集成用例的查询前置条件**：Xcode 26.3 的 CI 在五条 `EmailSuffixKeyboardTests` 用例里等不到 `language` 候选。夹具把完整的 `languge` 文档挂到控制器上后只切换模式，没有走实际输入字符的查询入口。补一次 `textDidChange` 仍不足：它只经 `refreshEmailSuffixes` 刷新，邮箱后缀从空到空时提前返回。夹具现在从 `langug` 开始，切到英文并同步上下文后，通过可见的 E 键输入末尾字母，先断言文档确实成为 `languge`，再等待真实纠错出现。五条保护旧文档和选区的断言与原有超时保持不变，不修改生产查询触发策略。
- **本轮本机验证范围**：真实按键夹具补修后的完整 `bash scripts/verify-local.sh --quick` 退出 0；202 项 Windows 可移植用例、仓库契约与笔记、Rust workspace、Android 目标与 Java 宿主、WASM、Linux 设置应用和原生门禁（112 项，其中 `linux-ibus-demand-activation` 环境跳过，0 失败）、macOS 原生编译及 Apple 桥均通过。缺 MinGW 的 Windows 原生/pipe-only、缺暂存输入的鸿蒙 HAP 按规则跳过。Swift 语法解析通过。本机空闲低于仓库规定的 50 GB，因此没有启动新的 Xcode 构建；前次仅补回调的 `ac2c9a2b22` 新 CI 仍有相同五项失败，本次夹具的 iOS 运行时结果仍待新提交 CI 实跑确认。专用模拟器和生成工程已清理，以上本地检查不计设备验收。

英文建议的控制器回归另覆盖两条交互：`eng` 展示 `English` 前缀补全时点原文，只收起候选，不记录拒绝纠错；按空格仍输入 `eng `，不能把展示用补全当成唯一纠错自动上屏。用例通过真实字母键触发查询，等待真实候选再点按钮；既有测试辅助方法允许指定末尾字母和期望原文。沿用 #10067 已合入的异步展示与边界查询实现，本补修不改生产缓存或候选策略。对固定上游提交提取的实际桥与控制器方法，合成 C ABI/UI 适配器已执行 34 条断言；它只验证合成环境，新增 UIKit XCTest 的实际执行以本 PR CI 为准。冻结上述两个文件后运行完整本地 `bash scripts/verify-local.sh --quick`，正式退出 0，未在运行期间改写源码。新增两条 UIKit XCTest 仍待 iOS CI 执行，不把本地 quick 视为原生测试验收。
