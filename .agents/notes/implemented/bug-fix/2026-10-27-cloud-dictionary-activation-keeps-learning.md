# Agent Note: 云词库整份激活时保留本机学习

Status: implemented

## Problem

#10043 里用户反映各设备的候选顺序越用越不一样。读代码发现，Android 的云同步本身就会把本机学到的东西清空，而且每下载一次清一次：

- 云端变了、本机不脏时，云同步选「下载」（`crates/client-services/src/account/policy.rs` 的 Download 分支）。下载下来的快照由 `crates/host-api/src/cloud_sync.rs` 的 `replace_dictionary` 排进词库快照队列。
- 键盘空闲时，`snapshot_queue_process` 按快照另建一代词库，再由 `activate_with_hook` 把 `user`、`cache`、`dictionaries` 三个根目录的内容整份换掉。
- 新的一代只按快照里的记录重建（`msime_engine::user_dictionary::state::build_generation`）。云端快照只有用户自己的词：云同步上传时 `export_local_snapshot` 的 `include_learning` 为假。

于是旧日志里的东西都随旧的一代被删掉：内置词的调权（`user_inserted=0` 的 upsert）、所有删除记录、固定位置、选词计数，以及输入习惯（整句联想 bigram/trigram、选词对、拼写纠错、自动纠错抑制、置顶）。

修之前先写了 host-api 测试复现。在 origin/develop（eb434d2464）上，按 `snapshot_queue_process` 的路准备并激活一份不带输入记录的快照。测试断言的 8 类全部丢失：内置词调权、内置词删除记录、用户词删除记录、固定位置、选词计数、置顶、bigram、trigram。激活后只剩快照里的用户词。

桌面三端和鸿蒙的「应用到本机」（`cloud_dictionary/full_backup.rs`），以及 Android 词库页云词库面板的「应用到本机」（Tauri 的 `android_account.rs` 经 `AccountPlugin.enqueueSnapshot` 和 `DictionarySnapshotQueue.java` 排进同一个 Rust 队列），走的是同一个队列、同一段激活，同样会清空。

## Decision

快照队列的请求带一个 `keep_local_learning` 标记。激活时，标记为真就把即将换下来的这一代学到的东西并进新的一代，再换入。

- **标记的来源**：`DictionarySnapshotQueue::enqueue` 多一个 `keep_local_learning` 参数，落在 `SnapshotRequest` 上。JSON 的 `enqueue` 动作和 `msime_client_snapshot_prepare` 的 `PrepareRequest` 也加了这个字段，缺省为假。
  - 置真的调用方有三处：`cloud_sync.rs` 的 `replace_dictionary`（云同步的「下载」）；`full_backup.rs` 的 `enqueue_action`（Windows、macOS、Linux 和鸿蒙的「应用到本机」）；Android 的 `android_account.rs` 的 `dictionary_snapshot_enqueue`（云词库面板的「应用到本机」，经 `AccountPlugin.enqueueSnapshot` 的 `keepLocalLearning` 和 `DictionarySnapshotQueue.enqueue` 的 `keepLocalLearning` 参数传下去）。
  - 本地备份恢复（Android `LocalBackup` 经 `CloudSync.enqueueDictionarySnapshot` 排的 `local-backup` 请求）显式传假：备份快照自己带着输入记录，要整份换成备份里的那份。
- **`state.json` 里怎么存**：`keepLocalLearning` 只在与所有者推出来的值不同时才写（`StoredSnapshotRequest`）。推出来的值是：属于账号的请求为真，`local-backup` 的为假。
  - 加这个字段之前，属于账号的请求都来自云端快照，只有 `local-backup` 来自本地备份。所以升级时还排着的请求、旧版 Java 队列 `state.bin` 转换来的请求（`android_v1.rs`），按所有者补上标记，不会在升级后的第一次激活把学习清空一次。
  - 现有入队处排的请求都等于推出来的值，写出来与加字段之前逐字节相同，降级到不认识这个字段的旧版本也读得懂。
- **在哪一步做**：在 `activate_with_hook` 里，拿到独占维护权、版本核对通过、新一代的试开会话关掉、调过 `close_cached_databases` 之后，调用 `carry_local_learning(active, staged, scratch)`。
  - 这时排着的整句联想已经写进旧日志。不在入队时导出：习惯表不在本机版本的校验范围内，入队到激活之间学到的整句联想会漏掉。
  - 合并完不论成败都再调一次 `close_cached_databases`：成功时接着改名换入，失败时调用方要删掉暂存的新一代，Windows 上两者都不能有打开着的 SQLite 文件。
- **合并进哪一代**：并进还没换入的新一代，不是换入以后再并回当前这一代。合并失败（磁盘满、I/O 失败）时报 `snapshot learning merge failed`，激活不做，旧的一代原样留着，请求停在 `preparing`，下次空闲再认领重试。
- **导出和合并的内容**：都复用本地备份的代码，语义也与本地备份恢复相同。
  - 输入记录：用 `learning_records` 写成一份只有输入记录的快照（与 `queue_learning_merge` 写的待合并文件同一个形状），放在这次准备的私有暂存目录里，再用 `merge_dictionary_state` 并进新一代。它同时改新一代的日志和工作词库。
  - 输入习惯：用 `habits::export_learning_habits` 导出，`habits::merge_claimed_habits` 合并。
  - 规则是新一代已有的保留新一代，计数取大。所以快照里的用户词以云端为准：本机删过、云端又有的用户词会回来；本机学过调权、云端是用户词的，按用户词算。
- **读不出来的那一类不并**：旧的一代某一类读不出来（日志或习惯表里有不是 UTF-8 的行等）时，这一类不并，整份换掉，与改动前相同。不让一份坏日志挡住以后每一次下载。输入记录读不出来时，`dictionary_state_revision` 也读不出来，激活在版本核对时就已经失败，所以这条实际只会落在输入习惯上。
  - 只有 `export_learning_habits` 报 `habits read rejected` 才算读不出来。中间文件写不进去（磁盘满、I/O 失败）等其余错误都按合并失败处理，激活不做，不能当成「没有习惯」把它们丢掉。

## Alternatives considered

- **换入以后，在同一把锁里把导出的东西并回当前这一代**：这是 #10043 定案的写法，也最直观，换代逻辑一行不动。但换入以后合并失败，学习已经随旧的一代删掉了，只能再写一段回滚把旧的一代换回来。并进还没换入的新一代，用的是同一把锁、同样的数据，换来的是失败时什么都不动。
- **不带标记，只按请求的所有者判断（账号请求都保留，`local-backup` 不保留）**：不用改入队接口。但这条规则藏在所有者字符串里，以后再加一种整份激活的来源时很容易判错。所以入队接口和认领出来的请求都带显式的标记，入队处一眼看得出意图；所有者只用来给没记下标记的旧请求补值，也用来省掉 `state.json` 里与它相同的字段。
- **`state.json` 里为假时不写、为真时写（`skip_serializing_if` 假值）**：实现最简单。但没有这个字段的旧请求只能读成假，升级时排着的云同步下载会把学习清空一次；为真的请求写进去以后，降级到旧版本还会因 `deny_unknown_fields` 读不出队列。
- **云同步上传时就带输入记录（`include_learning:true`）**：下载回来的快照就有学习，激活不用改。但这等于把输入记录明文传上服务器，违背 [Android 本地备份带上输入记录](../feature/2026-10-09-android-backup-learning-state.md) 里用户定的「输入记录不进云同步」，也和 #10043 定案的端到端加密方向相反。服务器的快照格式也装不下输入习惯。
- **不在激活时并，改为入队时导出、激活后用待合并文件（`merge_pending_learning`）并回来**：可以复用现成的认领和重试机制。但导出发生在入队时，入队到激活之间学到的整句联想会丢。待合并文件同一时间只能有一份，还会和本地备份恢复排的那份互相覆盖。

## Consequences

- **收益**：
  - Android 的云同步下载以后，本机的候选调权、删除、固定、置顶和整句联想都还在，不再每轮下载就清空一次。
  - 桌面三端和鸿蒙的「应用到本机」也一样。
  - 云端快照仍然只有用户词，上传内容一个字节都没变。
- **代价与已知上限**：
  - 激活多一次全量导出和合并，耗时与本机学习记录的多少成正比。这一步在键盘空闲（Android、鸿蒙）或输入会话让出词库（桌面 `QuiescedHosts`）时进行，与本地备份恢复时的合并是同一量级。
  - 合并按本地备份的规则来，有几处损失：
    - 输入记录的 `updated_at` 变成激活时刻。#10043 第 1 期的 Lamport 时间戳要用真实的 `updated_at`，届时要改成原样保留。
    - 选词计数截到 10。
    - 编码、词或上下文里含换行、制表符的行跳过。
  - iOS 的「应用到本机」走自己的 Swift 队列，直接调 `msime_client_snapshot_prepare`/`activate`，没有带这个标记，激活时仍会清空学习。要修，就在 `DictionarySnapshotWorker` 的 `PrepareRequest` 里加 `keep_local_learning: true`。iOS 的云同步不同步词库（capabilities 里 `dictionary` 为假），不会走到这条路。
  - 只有与所有者推出来的值不同的请求（属于账号却不保留学习、`local-backup` 却保留）才会把 `keepLocalLearning` 写进 `state.json`，被不认识这个字段的旧版本读到会因 `deny_unknown_fields` 报 `snapshot_invalid`。现在没有入队处排这样的请求。以后加这样的入队处，就要接受「排着这种请求时降级」读不出队列。

## Verification

- `cargo test -p msime-host-api --lib dictionary_snapshot`：
  - `a_cloud_download_activation_keeps_local_learning`：用云同步的 `HostLocal::replace_dictionary` 入队（标记由它决定），入队后再学一条 trigram，然后像键盘空闲时那样处理队列。断言 7 类学习都在，新一代工作词库里拟好的权重是学到的 200、妮好已删，用户词以云端为准。
  - `an_activation_without_keep_local_learning_replaces_the_learning`：不带标记时 7 类全部换掉，与改动前相同。
- `cargo test -p msime-host-api --lib full_backup`：`applying_the_cloud_dictionary_keeps_local_learning` 把「应用到本机」实际发出的 `enqueue` 动作交给队列的 JSON 入口，认领出来的请求带标记。
- `cargo test -p msime-client-core --lib snapshot_queue`：
  - `keep_local_learning_is_persisted_only_when_it_differs_from_the_owner_default`：账号为真、`local-backup` 为假时 `state.json` 里没有这个字段，反过来时写出来，四种都在认领时原样带出。
  - `a_request_queued_before_the_flag_keeps_learning_unless_it_is_a_local_backup`：没有这个字段的旧请求，账号的读成真，`local-backup` 的读成假。
  - `android_v1` 的 `decodes_every_java_fixture`：旧版 Java 队列转换来的账号请求带标记，本地备份的不带。
- Android 的 Java、Kotlin 和 Tauri 侧（`DictionarySnapshotQueue.enqueue`、`AccountPlugin.enqueueSnapshot`、`android_account.rs`）没有编译验证，也没有跑 Android 构建。
- 真机复现和复验（Android 上先学一个调权，在另一台设备改词库触发下载，收起键盘后看调权是否还在）还没有做。
