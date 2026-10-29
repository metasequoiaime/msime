# Agent Note: 共享 Rust 层在没有变化时不再整份读写和 fsync

Status: implemented

## Problem

低端手机用户反馈键盘耗电、发热、卡顿。Android 那一侧的每秒偏好轮询已改成只在窗口可见时运行（见 [Android 键盘只在窗口显示时每秒重读偏好](2026-10-27-android-preference-poll-only-while-visible.md)），但三端共用的 Rust 层（`msime-engine`、`msime-client-core`、`msime-client-services`、`msime-host-api`）在按键、上屏、聚焦和空闲轮询路径上还有六处「什么都没变也照样整份读写」的开销，都在宿主的输入线程、主线程或频繁唤醒的工作线程上：

- 打字统计 `record` 每次上屏都在文件锁下整份读入、解析、重写 `typing-statistics.json` 并 fsync（Apple 上是 `F_FULLFSYNC`），英文模式下每个字母一次；文件按天增长，统计关着时也要整份读一遍才知道不记。
- 选词计数、调频、选词转移、整句联想都同步提交到用户日志 `msime_user.db`，SQLite 默认的回滚日志加 `synchronous=FULL` 让每次提交建 `-journal`、fsync 两三次再删掉。低端机 eMMC 上一次 fsync 是几到几十毫秒，选非首位候选时键盘随之卡顿。
- 整句联想的个人 n-gram 模型（最多 20 万条）只要没有会话引用就被注册表丢掉。Android 每进一个输入框都销毁再新建会话，离上次上屏超过约 2 秒时，下一个会话就在主线程上把 `personal_bigram`、`personal_trigram` 整表重读。
- `msime_client_create` 拿空的 `PluginTables` 作对照，每个新会话都重读指令表、短语表、辅助码表包和联系人名单；没填拼音的联系人姓名还要现场扫主词库查读音。
- `personal_dictionary_sync` 在 Android 每次聚焦、HarmonyOS 每次重建会话前都要扫用户日志凑一页，再把 `sync.json` 原样重写并 fsync，即使队列是空的。
- 偏好：Android 键盘可见时每秒 `msime_client_load_preferences` 整读并两遍解析、校验 `preferences.json`，主线程再把同一份快照交给 `msime_client_update_preferences`，后者照样校验、四次整棵比较、跑十几个 setter、重读效果包清单。快照队列的本机词库版本（Android 每次失焦、iOS 每 10 秒、HarmonyOS 每 2 秒）每次都把日志全表排序做 SHA-256，并重写一次 `state.json`。
- Android 触屏默认的横向候选条每键调 `all_candidates`：整代候选（几百到几千行）序列化过 JNI，而条上一次只建几十个按钮；首字母时还逼引擎放出暂扣的尾部，按 `UNLIMITED_ROWS` 把整张 `tbl_1_<字母>`（483 到 4490 行）查出来并换一代。
- 候选释义或候选翻译开着时，`candidate_glosses_with_user`、`candidate_target_glosses` 每次调用都重新打开 `msime-english.db`（算 learned 还要再开一次）、学到的释义库、两份释义库和离线目标语言库，并把 `custom_translations.txt` 读两遍；鸿蒙在 UI 线程上每个改变候选页的按键都调一次。
- 鸿蒙键盘每次构建会话都 `process` 一次快照队列、每次显示后再 `state` 一次，两者都先算本机版本并发布；而鸿蒙上没有任何东西往键盘这份队列里排请求，发布出去的版本没人读。

这一批问题由四条线并行审过：本篇这条共享 Rust 线，以及 Android、iOS、鸿蒙三条宿主线。宿主线各自也改到了共享 Rust（候选条按段取、释义缓存、快照队列空闲时只读、个人模型跨会话沿用、预览版本缓存），其中个人模型沿用、预览版本缓存、`state.json` 不重写三处与本线重复。合并后共享 Rust 的改动只在本篇这条 PR 里，重复的各留一套；三条宿主 PR 叠在本 PR 上，只含各自 `platforms/` 下的接线。

## Decision

总原则：先用便宜的方式（stat、字节比较、内存里的上一份）判断「来源没变」，没变就沿用上一次的结果、不写盘；判断不了（stat 失败、戳读不出、刚写过）就照旧走完整路径。凡是可能决定数据正确性的地方仍然精确计算。

### 打字统计攒批（`client-services/src/typing_statistics.rs`、`host-api/src/ffi/host.rs`）

- `record` 只在内存里分类计数，按统计目录攒在进程级缓冲里，只存分类计数、日期、小时和时刻，不留原文。写出的时机是维护者定下的跨平台约定「失焦或每 30 秒」：距上次读写该目录满 `COMMIT_FLUSH_INTERVAL_MS`（30 秒）后的下一次 `record`、显式 `{"operation":"flush"}`、或同一目录上的任何其他操作（load、summary、reset、上传……），都先在文件锁内重读文件，再按到达顺序逐条合并后写一次。逐条合并复用原 `record_at` 的逻辑（`TypingStatistics::apply_commit`），活跃时长、连续输入、小时分布与逐次写入完全相同。
- 清空统计换一个随机的清空代号（generation）。攒着的批次记下开始攒时的代号，写出时代号变了就整批丢掉，设置页清空后键盘不会把清空前打的字写回来；别的进程关掉统计时同样丢掉这批，改保留期则合并。代号用随机值不用计数，因为清空次数本身也在说用户。
- 成就里程碑按 `record` 答出的「文件累计 + 进程内未写」总数判断，不再先 load。
- 会话失焦、`msime_client_destroy`、`msime_client_flush_all` 写出全部目录的批次。macOS、Linux 的失焦和移动宿主的失焦与销毁在新开的后台线程上写，不占主线程；Windows 失焦在服务进程的 IPC 线程上当场写；桌面销毁当场写（之后进程可能退出）。Tauri 面板和 Windows 7 的剪贴板、云剪贴板、表情、语音、手写面板没有失焦或退出收尾，记完立刻 `flush`，行为与攒批前相同。头文件 `msime_client.h` 写明 flush 和攒批语义。

### 用户日志进 WAL（`engine/src/user_dictionary/journal.rs`、`ngram_store.rs`、`reset.rs`）

- `open_journal` 缓存的连接和整句联想写线程的连接经 `prefer_write_ahead_log` 切到 WAL，只有真的进了 WAL 才把 `synchronous` 降到 `NORMAL` 并设 `journal_size_limit` 为 4 MiB；切换失败（别的进程占着锁、没有共享内存）时保持原模式和 `FULL`，因为回滚日志配 `NORMAL` 断电可能损坏。
- iOS 不切：键盘扩展与 App 在 App Group 里共用日志，WAL 连接整个生命周期对 `-shm` 持锁，挂起时持锁会被按 `0xdead10cc` 杀掉。
- 主词库工作副本（`msime-pinyin.db`、`msime-english.db`）保留回滚日志：重置学习数据把它们挪作备份时不带旁路文件，旧 `-wal` 会套到新词库上。
- 重置学习数据挪走日志之前先 `wal_checkpoint(TRUNCATE)`。损坏日志隔离本来就连同 `-wal`/`-shm` 一起改名（见 [用户词库日志损坏时挪开](2026-10-13-corrupt-user-journal-quarantine.md)），macOS/Linux 迁移数据目录先停输入法再连同旁路文件复制，都不用改。

### 个人上下文模型跨会话沿用（`ngram_store.rs`、`host/session.rs`）

iOS 键盘每次收起都销毁会话，Android 每进一个输入框都重建会话，之前只要没有会话引用，注册表就把 store 丢掉，下一个会话把两张表整表重读。现在：

- 记录点：核心 `Session` 的 drop（宿主门面 `host::Session` 不再自己收尾，见 [核心会话收尾与日志连接的线程归属](../architecture/2026-10-29-core-session-lifecycle-and-journal-threads.md)）在写完队列、放掉本线程的日志连接、本日志最后一个会话还关掉各线程空闲的日志连接之后（host-api 会话这时还拿着词库租约）调 `note_settled`，本会话所有连接（整句联想写连接、选词计数和调频用的日志缓存连接）的写入都在记录之前。模型过期、队列没写完、上次写失败、模型为空时不记。
- 模型来源：完整加载前后在同一连接读取 `PRAGMA data_version`，两次相等且日志文件身份未换才保存来源版本。会话结束、自己刷新队列不能把当前版本重新当成旧模型的来源；只有完整重读才能重建来源。首次从只读切到写连接先失效并真正重载，再叠本次学习，避免两条连接的版本号混用。
- 记录内容：会话结束时保存日志和非空 `-wal` 的大小、修改时间，Unix 上加设备号和 inode，同时验证实际模型来源仍未改变。写连接自身的提交不会改变它看到的 `data_version`，任何其他连接的提交都会使来源失效。只读来源仅在实际 `journal_mode` 为 `delete`、`truncate` 或 `persist` 且不是 Windows 时保留加载用的那条连接；只读 WAL 无法在不长期占用共享内存锁的情况下证明来源，保守不缓存。空 `-wal` 与没有 `-wal` 同等看待，但不能用文件戳替代连接版本：检查点 RESTART 后 WAL 会在原空间复用。
- 接手：`for_journal` 打开同一路径时同时比文件戳和来源连接版本，匹配才沿用模型。会话结束的记录只担保一次接手，但只读来源连接跨后续接手保留，避免第二轮重新采样为旧模型背书。还有会话在用的 store 丢掉结束记录，等那个会话放手时重记。其他路径的空闲 store 仍在下次打开日志时回收。`release_all`（重置、换代次之前）连同记录和来源连接一起丢掉；`release_idle_personal_models` 接进 `Session::reset_cache` 和 `msime_client_release_process_cache`，内存告警时交还模型和连接。
- 会话期间别的输入法进程学到、模型没读到的计数会使来源校验失败，空闲 store 下次接手时丢弃并从磁盘重载。判定与队列检查持有 writer、pending 锁直至记录完成，避免检查后又有本地学习入队。

### 插件表进程缓存（`host-api/src/plugin_tables.rs`）

指令表、短语表、辅助码表包、联系人名单各在进程里留最近一份，键是读它时的来源：插件目录加上会话本来就盖的戳（清单戳、包目录戳、`mentions.json` 戳，仍沿用 [带 inode 的 FileStamp](2026-10-11-plugin-table-stamp-inode.md)），联系人读音另加主词库路径和它的戳（引擎新增 `main_dictionary_path`）。读不出名单、辅助码包载入失败、要查读音的姓名一个也没查到时不记下，下一个会话重试。`release_cached_tables` 在两处清缓存入口清空四个槽位，开着的会话各自持有读进 Engine 的那份，不受影响。

### 个人词库同步（`client-services/src/dictionary/personal.rs`、`engine/src/host/dictionary.rs`）

`PersonalDictionaryStore::update` 状态没变时不写文件。状态新增可选的 `snapshotSource`，记下读这一页时用户日志的来源戳（`dictionary_entries_source`：日志和 `-wal` 的 inode、长度、修改时间、状态改变时间）。本批没有应用请求、宿主没有未确认的页请求、页参数和戳都与已存的一页相同、上次读页没出错时直接沿用这一页。日志最后改动离现在不到 `REVISION_SETTLE`（两秒）时不给戳，调用方照常重查，避开同一时间粒度里长度不变的写入。旧的 `synchronize` 入口不给戳，行为不变。与 [忙碌重试](2026-10-17-personal-dictionary-sync-busy-retry.md) 的维护租约语义不冲突：取戳时持有独占租约。

### 候选条按段取候选（`input-runtime/src/runtime.rs`、`host-api/src/ffi/input.rs`）

`Runtime::candidate_range(start, count)` 只复制座位 `start` 起的一段，身份与 `all_candidates` 同一座位那一行相同；暂扣的尾部只在 `start` 走到已放出的末尾时才放，放出来照旧换一代。放尾部的逻辑抽成 `release_held_candidates` 与 `all_candidates` 共用，代次耗尽时的保护不变。host-api 导出 `msime_client_candidate_range`，`all_candidates` 不变。Android 候选条改用它在 Android PR 里。

### 候选释义按文件身份缓存（`engine/src/host/glosses.rs`）

`candidate_glosses_with_user` 和 `candidate_target_glosses` 在查询线程的 `thread_local` 里留着打开过的发布词库、学到的释义库、最多 8 份离线释义库连接和读好的自定义译文，键是路径加文件身份（长度、修改时间，unix 上加设备号和 inode，复用日文模型缓存的 `FileIdentity`）；发布词库和学到的释义库的键还带同目录 `custom_translations.txt` 的身份。文件被替换、改写或删除时下一次查询就重新打开。Windows 上 SQLite 打开文件不带 `FILE_SHARE_DELETE`，翻译线程与服务同寿命，所以那里只缓存自定义译文。`release_gloss_caches` 清掉调用线程的这四份缓存：`msime_client_release_process_cache` 一并调它，新增 `msime_client_release_gloss_caches` 只放释义、不动日文词典，鸿蒙在收起和失焦时调（鸿蒙 PR）。

### 偏好与快照版本（`client-core/src/preferences.rs`、`host-api/src/lib.rs`、`client-core/src/cloud/snapshot_queue.rs`、`engine/src/user_dictionary/state.rs`）

- `PreferencesStore::load_reusing`：照旧加锁读出文件字节，与上一次逐字节相同就沿用上一份快照，跳过两遍解析和校验。文件不存在时每次照常取默认快照。`msime_client_load_preferences` 按目录留最近一份，C ABI 不变。
- `HostSession::update` 认出与上一份相同的快照（且上一次 update 没有中途失败，`requested_settled`）后走 `update_unchanged`：不校验、不比较、不调只有偏好会改的 setter；仍按偏好重设繁简投影、用 `SessionSound::restamp` 给效果包清单盖戳、调 `apply_pending`，回复与完整更新字段相同。
- `cached_dictionary_state_revision` 按日志主文件、`-wal`、`-journal` 的 stat 记下上次修订号，stat 一样就直接返回；最后改动早于两秒前才记缓存（git racy-clean 的做法）。只有空闲预览用它（`snapshot_queue_state`、`process` 认领前、`msime_client_snapshot_version`）；`process` 认领到请求后、prepare 前后、激活后都重新精确计算，缓存过时最多表现为一次可恢复的冲突。
- 快照队列 `update_if_changed`：发布的本机版本没变、也没有请求因此标成已应用时不重写 `state.json`。
- 快照队列的 `state` 和 `process` 可以带 `skip_when_idle`（默认 false）：队列里没有排队或准备中的请求时只读状态返回，不算本机版本、不发布、不建队列目录。鸿蒙键盘这样查（鸿蒙 PR），其他宿主不带，行为不变；真有请求时照旧先发布预览版本，`process` 认领后精确计算再比对。

## Alternatives considered

- **统计去掉 fsync、保留逐次写入**：改动最小，能省下最贵的那一步。但 rename 前不 fsync 在部分文件系统上断电后会留下空文件或半截文件，整份统计没了；而且整读整写和解析的 CPU 仍按每次上屏付。没有采用。
- **统计按条数攒批（如选词位置的 32 次一写）**：实现简单、内存有界。没有采用：维护者定的跨平台约定是「失焦或每 30 秒」，按条数写在慢速打字时会把计数无限期留在内存里，被杀时丢失的量不可控；而时间门槛加失焦写出把上限钉在 30 秒。
- **把一批上屏合成一次大提交写入**：只算一次活跃时长，代码更少。但批内上屏之间的间隔会被抹掉，`daily_active_ms`、最长连续输入都会变，所以按原时刻逐条重放。
- **主词库工作副本也进 WAL、或给它缓存一条可写连接并降 `synchronous`**：调频时主词库的那次提交同样要 fsync，缓存连接还能省掉每次打开。本线没有做：重置学习数据挪走工作副本时不带旁路文件，WAL 会把旧 `-wal` 套到新词库上；缓存可写连接要和换代次、替换词库的关闭时机重新对齐，风险超出这一轮的范围。日志进 WAL 已拿到大部分收益，主词库这一步留给后续单独评估。
- **n-gram store 无条件常驻**：最省事，内存代价与会话期间相同。但别的进程或别的连接改过日志而没有通知本进程时，新会话就读不到改动；用文件戳和 `data_version` 把关，代价只是几次 stat 和一次 PRAGMA。拼写纠错 profile 没有一起改：它的表很小、重读代价低，常驻会吃掉九键过滤刷新那条分配计数用例原有的余量。
- **个人模型按读完日志、自己写完一批时的（修改时间, 大小）判断能否沿用**（本线最初的做法）：不用改会话的 drop。但会话里选词计数、调频、删词经日志缓存连接写同一个文件，整句联想最后一批写完之后只要还有一次这样的写入，下一个会话就对不上、照样整表重读；同一节拍里另一个连接同样大小的原地改写也看不出来。改成会话放手时记录、加 `data_version`。
- **所有 WAL 来源都丢弃**：不用保留只读探测连接就能避开 WAL 共享内存锁。但 Android 的写 store 也会失去跨会话收益，因此已有写连接时继续用它在实际加载时记录的 `data_version`，仅只读 WAL 保守丢弃。
- **会话结束时重新采样版本，WAL 只读时靠文件戳**：能尽量保留缓存命中，且不保留 WAL 只读连接。但外部提交发生在加载后、结束前时，新版本会错误地为旧模型背书；RESTART 后 WAL 可以等长复用，文件戳也不能替代实际来源。仅在完整加载前后版本相等时建立来源。
- **预览版本缓存放在 host-api、只给 `msime_client_snapshot_version` 用**（iOS 线另做过一份）：与本线引擎里的 `cached_dictionary_state_revision` 规则相同（日志、`-wal`、`-journal` 的文件身份，两秒落定），但覆盖不到队列的 `state` 和 `process` 发布，留引擎那一份。
- **偏好按文件戳（mtime、inode）判断、或新增 `msime_client_load_preferences_if_changed` 只 stat 不读**：比逐字节比较更省 I/O。没有采用：mtime 粒度粗、inode 可能被重用、文件可能被原地改写，戳一样内容不一样时会读到旧快照；新增 C ABI 还要四端同时接。偏好文件不大，读字节的成本远小于两遍解析加校验。
- **联系人读音在保存时算好写进 `mentions.json`**：聚焦时完全不用查词库。但改了落盘格式，主词库更新后读音也不会跟着变，没有采用。
- **快照版本的缓存也用于冲突判定**：能把 `process` 认领后的那次全表扫描也省掉。但缓存过时就可能把旧数据当成新数据应用，正确性优先，只给空闲预览用。
- **根治：Android 换输入框时保留会话、只发 focus(false/true)**：n-gram 重读、插件表重读、个人词库同步的大部分触发都来自每次聚焦销毁重建会话。这是 Android 生命周期的改动，影响面大，不在共享 Rust 这一线；Android PR 只在同一输入框重启连接时留会话，本线让「重建」本身变便宜。

## 旧笔记审计

按打字统计、WAL/journal、n-gram、插件表、`sync.json`、快照队列、偏好轮询、低端机耗电检索活跃笔记：

- [Android 键盘只在窗口显示时每秒重读偏好](2026-10-27-android-preference-poll-only-while-visible.md)：同一批反馈的 Android 侧，本篇处理可见期间那一轮在 Rust 里的开销，部分重叠，互链。
- [用户词库日志损坏时挪开](2026-10-13-corrupt-user-journal-quarantine.md)：隔离已连同 `-wal`/`-shm` 一起改名，WAL 下仍成立，无需修改。
- [插件表的文件戳带上 inode](2026-10-11-plugin-table-stamp-inode.md)：本篇的进程缓存沿用它的 `FileStamp`，该篇事实不变。
- [个人词库同步忙碌重试](2026-10-17-personal-dictionary-sync-busy-retry.md)、[iOS 词库维护移出主线程](2026-10-22-ios-idle-maintenance-off-main.md)：前者持有租约语义，后者是 iOS 宿主侧的空闲轮询，本篇让同一轮在 Rust 里变便宜，部分重叠。
- [输入统计锁绑定目录句柄](2026-10-09-typing-statistics-lock-directory.md)、[关闭上传时串行化云端操作](2026-10-17-typing-statistics-upload-race.md)：攒批写出仍走同一把目录句柄锁，上传前先写出批次，两篇不受影响。打字统计 UI 侧的模型与状态笔记只涉及 React 展示，无关。
- 三条宿主线的笔记（`2026-10-27-android-keyboard-low-end-power.md`、`2026-10-27-ios-keyboard-low-end-power.md`、`2026-10-27-harmony-keyboard-idle-and-hot-path-costs.md`）随各自的宿主 PR 叠在本 PR 上合入，本分支里还没有，所以这里不链接；它们引用的共享层改动以本篇为准。

## Consequences

- 空闲聚焦、空闲轮询、偏好原样再交、同一个输入框里的连续打字，不再整份读写或 fsync 统计文件、`sync.json`、`state.json`，也不再全表扫日志、重读 n-gram 表和插件表；候选条每键只过一段候选，释义每键不再重开词库。代价是进程里多留几份数据（n-gram 模型几 MB 和没有写连接时的一条只读探测连接、联系人名单约 100 KB、上一份偏好快照、查询线程上的释义库连接），内存告警时经清缓存入口放掉，释义另可经 `msime_client_release_gloss_caches` 按线程放掉。
- 维护者认可的持久性取舍：进程被系统杀掉时最多丢 30 秒的打字统计计数（移动端失焦和销毁已在后台线程写出，实际窗口更小）；用户日志在 WAL + `NORMAL` 下断电最多丢最后几次学习，但不会损坏；把日志 ATTACH 到词库连接上的事务（删词、个人词库编辑、五笔造词）不再由超级日志跨文件绑定，只在每个文件内原子，按 SQLite 的提交顺序崩溃只会留下「日志已记下、词库工作副本回滚」，下次回放补齐，不会出现词库已删、日志没有墓碑。
- 设置页读统计文件时，正在打字的计数最多晚 30 秒出现；设置页的任何统计操作会先把批次写出。
- 新增 `{"operation":"flush"}` 统计操作、`snapshotSource` 字段（可选，旧文件照读）、快照队列的 `skip_when_idle` 字段（可选），以及 C ABI `msime_client_candidate_range` 和 `msime_client_release_gloss_caches`。宿主一侧的接线在叠在本 PR 上的三条宿主 PR 里，要在本 PR 之后合入；本 PR 单独合入时攒批也会在会话失焦和销毁时写出，不需要宿主先接 flush。
- WAL 让测试进程同时开着的文件从约 100 涨到近 400，超过 macOS launchd 默认的软上限 256；`scripts/verify-local.sh` 在跑 Rust 测试前把软上限抬到 4096。CI 的 Rust 测试在 Linux 容器里跑，不受这个上限影响；在 macOS 上直接 `cargo test` 需要先 `ulimit -n 4096`。
- 主词库那一次调频提交仍是回滚日志加 `FULL`，选非首位候选仍要为它 fsync 一次；Android 换输入框时仍销毁重建会话、打开并预热词库连接（同一输入框重启连接时 Android PR 已留会话）。这两处是下一步的候选。

## Verification

各项改动都带了单元测试：统计的攒批不逐次写文件、flush 写出、30 秒门槛、时钟回拨、攒批与逐条写入得到相同文档、与另一进程的清空和关闭合并、清空代号丢弃旧批次；日志以 WAL/`NORMAL` 提交、检查点后主文件单独拷走仍完整；个人模型在会话放手后沿用且只担保一次接手、别的连接写入或换文件后重读、同样大小的原地改写经探测连接发现、模型可能与磁盘不一致或为空时不记录、清缓存放掉、空 `-wal` 出现又消失不算改动、Windows 上不留探测连接、会话 drop 时记下日志；`candidate_range` 一批之内不放尾部、越过末尾才放并换代，真实全拼引擎与整代复制逐行一致；释义缓存在改写、删除、替换各类文件后与重新打开一致、释放后再查不变；`skip_when_idle` 下空队列的 `state`、`process` 不动、排着的请求照样处理，版本没变但正是激活出来的一代时请求仍标为已应用；预览版本与精确版本一致、日志改过后跟着变；插件表缓存命中与主词库或名单变化后重查；空闲同步既不读页也不重写 `sync.json`、各种失效条件下重查、来源戳落定规则；同一份偏好快照再交时回复与第一次相同、效果包跟上、同修订号换内容仍拒绝；偏好文件字节不变时沿用、保存或改坏后重读。多语言「选词不写日志」的断言改为连同 `-wal` 一起比较，否则 WAL 下写入只进 `-wal` 会让断言形同虚设。

合并四条线之后，在编译机上跑 `ulimit -n 4096; cargo test --no-fail-fast -p msime-engine -p msime-host-api -p msime-client-core -p msime-client-services -p msime-input-runtime`：client-core 582 项、client-services 1406 项、engine 2705 项、input-runtime 406 项全部通过；host-api 1016 项通过、2 项失败（`listing_reports_every_pack_and_checks_the_state_root` 没列 `pronunciations` 包、`a_command_gets_the_local_time_to_the_minute`），两项在基点上同样失败，与本线无关。`cargo clippy --keep-going --all-targets -- -D warnings` 只报 client-core 和 client-services 测试里两处 develop 上已有的 `field_reassign_with_default`；`cargo fmt --all --check` 通过。第一次全量跑时 engine 的分配预算用例（临时英文候选选词，预算 67）报 68，逐个提交单独跑定位到给 `journal_size_limit` 拼 PRAGMA 的 `format!`；改成字面量后通过。日志首次打开常在选词提交路径上，这条路径上的分配预算要留意。

来源校验补修在旧实现上真实复现四类失败：加载后结束前的外部提交、只读加载后外部提交、首次只读转写丢掉中间提交、本地 flush 前后的外部提交。补修后，macOS 复用现有 `target/` 运行 `cargo test --locked -p msime-engine --lib -- --test-threads=1`：2717 项通过、57 项忽略、0 项失败；`cargo clippy --locked -p msime-engine --all-targets -- -D warnings` 通过。新增用例还覆盖只读来源连续接手后再修改、回滚日志在结束后修改、WAL 检查点 RESTART 后等长复用。独立代码复核未发现生产阻断问题；修正复核指出的 iOS WAL 夹具和并行代次收尾后，`user_dictionary::ngram_store::tests` 33 项再次全部通过。完整 quick 的最终结果见下；本机引擎测试不替代 Android 设备验证。

整合验证补修：资源清单重构新增 `snapshot_queue_process_with` 后，`skip_when_idle` 必须从公开处理包装器传入该函数；云端激活的直接测试调用显式使用不跳过空闲的策略。新增学习保留参数后的队列测试和状态测试也显式补齐参数，避免合并后测试仍调用旧签名。修前 Rust 工作区检查报三处编译错误，修后复用现有 `target/` 的 `cargo check --locked --workspace --all-targets --exclude msime-desktop` 通过，`cloud::snapshot_queue` 45 项和 Host API 的 `dictionary_snapshot` 47 项通过，包含空闲队列不创建目录、排队请求仍处理、云端激活保留本机学习和不保留策略的回归。Host API 和 client-core 的严格 `--lib` Clippy 通过，新增代码中的多余借用也已修正。这条 Rust 工作区检查本身排除了尚未暂存原生 bundle 的桌面壳；完整 quick 另行覆盖 Linux 桌面壳，结果见下。

整套仓库契约 `bash scripts/run-checks.sh` 复用主工作区现有 `target/` 后通过；鸿蒙个人词库检查跟随带来源戳的有界批次入口，兼容入口仍保留批次上限，移除有界入口或批次上限的反向检查均被拒绝。该命令只覆盖仓库契约，完整 quick 另列，Android 设备验证仍缺。

Linux 原生整合验证发现 `linux-typing-statistics` 仍假定每次 `record` 都替换统计文件。测试现在分别验证攒批期间文件身份不变、刷新开关不额外查询，显式 `flush` 后文件身份改变且下一次刷新重新查询并保持开启；关闭统计和切换目录的既有断言继续保留。

完整 `bash scripts/verify-local.sh --quick` 在 Linux 输入统计测试修正后退出 0：Windows 便携原生测试 176 项通过、2 项宿主差异排除；Rust 工作区、Android Rust 目标、WASM 与严格 Clippy、Android Java、鸿蒙共享设置页、Linux 设置外壳与原生门禁、macOS 原生构建、共享 Apple 桥均通过。Linux CTest 111 项实际通过、1 项环境条件跳过；full/wubi 生产目标、安装暂存及路径隔离也通过。MinGW 和 Windows 原生构建未配置，鸿蒙完整打包输入未暂存，相关阶段按脚本规则跳过，不计通过。另用本分支真实 Host API 链接的 macOS `typing-statistics`、`key-press-statistics`、`session-smoke`、`dictionary-runtime` 四项 CTest 全部通过。

缺口：没有在任何真机上测过耗电、发热或卡顿。验证办法是在低端 Android 机上对比改动前后 `:ime` 进程的 CPU 时间增量（`adb shell top -p <pid>`、`dumpsys batterystats`）、`/proc/<pid>/io` 的写入量，以及聚焦到可打字的 trace。Windows 7 面板的 flush 改动只在 C++ 源码层面跟随，没有在 Windows 7 上运行。三条宿主 PR 的编译和宿主侧测试记在各自的笔记里，都没有装机复测。

后续偏好宽松读取合入后的整合：保留逐字节快照复用接口，同时让普通读取与缓存未命中共用 `settle_newer_values_on_read` 和未知列表元素收集；新增运行时降级入口与缓存接口并存，存储错误仍拒绝。缓存用例补上新版本文本编辑动作的文件，要求缓存读取与普通读取一致，改坏文件仍报错。整合后的命令结果另行记录，不沿用整合前的通过结论。
