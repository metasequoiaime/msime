# Agent Note: 云词库整份激活时保留本机学习

Status: implemented

## Problem

#10043 里用户反映各设备的候选顺序越用越不一样。读代码发现，Android 的云同步本身就会把本机学到的东西清空，而且每下载一次清一次：

- 云端变了、本机不脏时，云同步选「下载」（`crates/client-services/src/account/policy.rs` 的 Download 分支）。下载下来的快照由 `crates/host-api/src/cloud_sync.rs` 的 `replace_dictionary` 排进词库快照队列。
- 键盘空闲时，`snapshot_queue_process` 按快照另建一代词库，再由 `activate_with_hook` 把 `user`、`cache`、`dictionaries` 三个根目录的内容整份换掉。
- 新的一代只按快照里的记录重建（`msime_engine::user_dictionary::state::build_generation`）。云端快照只有用户自己的词：云同步上传时 `export_local_snapshot` 的 `include_learning` 为假。

于是旧日志里的东西都随旧的一代被删掉：内置词的调权（`user_inserted=0` 的 upsert）、所有删除记录、固定位置、选词计数，以及输入习惯（整句联想 bigram/trigram、选词对、拼写纠错、自动纠错抑制、置顶）。

修之前先写了 host-api 测试复现。在 origin/develop（eb434d2464）上，按 `snapshot_queue_process` 的路准备并激活一份不带输入记录的快照。测试断言的 8 类全部丢失：内置词调权、内置词删除记录、用户词删除记录、固定位置、选词计数、置顶、bigram、trigram。激活后只剩快照里的用户词。

桌面三端和鸿蒙的「应用到本机」（`cloud_dictionary/full_backup.rs`）走的是同一个队列、同一段激活，同样会清空。

## Decision

快照队列的请求带一个 `keep_local_learning` 标记。激活时，标记为真就把即将换下来的这一代学到的东西并进新的一代，再换入。

- **标记的来源**：`DictionarySnapshotQueue::enqueue` 多一个 `keep_local_learning` 参数，落在 `SnapshotRequest` 上（`state.json` 里是 `keepLocalLearning`）。为假时不写这个字段，状态文件与改动前逐字节相同。JSON 的 `enqueue` 动作和 `msime_client_snapshot_prepare` 的 `PrepareRequest` 也加了这个字段，缺省为假。
  - 置真的调用方有两处：`cloud_sync.rs` 的 `replace_dictionary`（云同步的「下载」），以及 `full_backup.rs` 的 `enqueue`（Windows、macOS、Linux 和鸿蒙的「应用到本机」）。
  - 本地备份恢复（Android `LocalBackup` 排的 `local-backup` 请求）不置：备份快照自己带着输入记录，要整份换成备份里的那份。
- **在哪一步做**：在 `activate_with_hook` 里，拿到独占维护权、版本核对通过、新一代的试开会话关掉、调过 `close_cached_databases` 之后，调用 `carry_local_learning(active, staged, scratch)`。
  - 这时排着的整句联想已经写进旧日志。不在入队时导出：习惯表不在本机版本的校验范围内，入队到激活之间学到的整句联想会漏掉。
  - 合并完再调一次 `close_cached_databases`，然后才改名换入。
- **合并进哪一代**：并进还没换入的新一代，不是换入以后再并回当前这一代。合并失败（磁盘满、I/O 失败）时报 `snapshot learning merge failed`，激活不做，旧的一代原样留着，请求停在 `preparing`，下次空闲再认领重试。
- **导出和合并的内容**：都复用本地备份的代码，语义也与本地备份恢复相同。
  - 输入记录：用 `learning_records` 写成一份只有输入记录的快照（与 `queue_learning_merge` 写的待合并文件同一个形状），放在这次准备的私有暂存目录里，再用 `merge_dictionary_state` 并进新一代。它同时改新一代的日志和工作词库。
  - 输入习惯：用 `habits::export_learning_habits` 导出，`habits::merge_claimed_habits` 合并。
  - 规则是新一代已有的保留新一代，计数取大。所以快照里的用户词以云端为准：本机删过、云端又有的用户词会回来；本机学过调权、云端是用户词的，按用户词算。
- **读不出来的那一类不并**：旧的一代某一类读不出来（日志或习惯表里有不是 UTF-8 的行等）时，这一类不并，整份换掉，与改动前相同。不让一份坏日志挡住以后每一次下载。输入记录读不出来时，`dictionary_state_revision` 也读不出来，激活在版本核对时就已经失败，所以这条实际只会落在输入习惯上。

## Alternatives considered

- **换入以后，在同一把锁里把导出的东西并回当前这一代**：这是 #10043 定案的写法，也最直观，换代逻辑一行不动。但换入以后合并失败，学习已经随旧的一代删掉了，只能再写一段回滚把旧的一代换回来。并进还没换入的新一代，用的是同一把锁、同样的数据，换来的是失败时什么都不动。
- **不带标记，按请求的所有者判断（账号请求都保留，`local-backup` 不保留）**：不用改 `state.json`，旧版本读新状态也没有风险。但这条规则藏在所有者字符串里，以后再加一种整份激活的来源时很容易判错。显式的字段在入队处一眼看得出意图。
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
  - 带标记的请求写进 `state.json` 后，如果被不认识这个字段的旧版本读到，会因 `deny_unknown_fields` 报 `snapshot_invalid`。宿主和库同包发版，请求一般在入队后很快就处理完，所以只有「排着请求时降级」才会撞上。

## Verification

- `cargo test -p msime-host-api --lib dictionary_snapshot`：
  - `a_cloud_download_activation_keeps_local_learning`：像 `replace_dictionary` 那样按键盘发布的版本入队，入队后再学一条 trigram，然后像键盘空闲时那样处理队列。断言 7 类学习都在，新一代工作词库里拟好的权重是学到的 200、妮好已删，用户词以云端为准。
  - `an_activation_without_keep_local_learning_replaces_the_learning`：不带标记时 7 类全部换掉，与改动前相同。
- `cargo test -p msime-client-core --lib snapshot_queue`：`keep_local_learning_is_persisted_only_when_set` 断言标记为假时 `state.json` 里没有这个字段，为真时落盘并在认领时带出。
- 真机复现和复验（Android 上先学一个调权，在另一台设备改词库触发下载，收起键盘后看调权是否还在）还没有做。
