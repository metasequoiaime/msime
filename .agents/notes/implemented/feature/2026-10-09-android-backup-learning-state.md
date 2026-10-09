# Agent Note: Android 本地备份带上输入记录并按本机优先合并

Status: implemented

## Problem

#5659 的用户最看重的是「输入记录」，而[本地备份](./2026-10-08-android-local-backup.md)只带了设置、皮肤、常用语和个人词库。Engine 的学习状态没进备份，原因有两层：导出端 `export_local_snapshot` 用 `dictionary_entries` 只读 `user_inserted=1` 的词，学习调权（`user_inserted=0` 的 upsert）、删除记录、固定位置和选词计数都没读；它的注释说「没有只读接口」，其实 `state::stream_dictionary_state` 早就有，只是没经 host 门面露出来。恢复端只有本机用户词数为 0 时才整份激活快照（会连同学习状态一起换掉），本机已有词时只合并词，没有任何把学习记录合并进现有日志的接口。

用户拍板的取舍：恢复时与本机已有记录合并，选词计数取大、位置冲突保留本机；备份仍是明文 zip，导出页明确提示文件含打字习惯、请妥善保管；输入记录不进云同步；输入统计不算在内。

## Decision

- **导出**：`export_snapshot` 加 `include_learning`（serde 缺省 false）。为假时代码路径不变，输出与以前逐字节相同，云同步上传的快照因此不受影响（`CloudSync` 的云同步调用不带这个字段）。为真时在用户的词之后，按快照格式第 1 版已有的记录追加输入记录：学习调权写成 `user_inserted:false` 的 `overlay`，删除记录写成 `deleted:true` 的 `overlay`，固定位置写成 `position`，选词计数写成 `selection`（截到 0..=10）。编码、词或上下文含换行/制表符、位置不在 1..=5、或总记录数超过 500,000 的跳过并计入 `learning_skipped`。写完照常用 `inspect_snapshot` 自检。数据来自新露出的 `msime_engine::host::stream_dictionary_state`。备份包格式 `LocalBackupPolicy.VERSION` 仍是 1，manifest 的 `contents` 多一个 `learning` 计数；旧版本读到这些记录也是合法的 v1 快照。
- **合并规则**（`msime_engine::user_dictionary::state::merge_dictionary_state`，host 门面 `merge_dictionary_state`）：一个 IMMEDIATE 事务里写工作词库和日志，任何一条记录不合规或读流失败都整体回滚。学习调权和删除记录只在本机日志里还没有同一 `(dictionary,key,value)` 时写入，写法与个人词库编辑相同（`apply_pinyin`/`apply_simple`/`apply_english` 改工作词库，再记日志，`user_inserted` 照记录保留）；本机已有就保留本机。固定位置用 `INSERT OR IGNORE`：本机已固定这个词，或这个位置已被占用，都保留本机。选词计数 `ON CONFLICT ... WHERE excluded.selection_count>selection_count`，取两边较大的那个。用户自己的词不在这里写，只计数，仍走原来的命名词库待发送队列。编码拼不出拼音表名、这一代没有那张表、没有中文词库的版本里的非英文行、权重低于 1 的拼音调权都跳过计数，不让整次合并失败。
- **合并时机**：改工作词库要独占维护权（`DictionaryAccess::try_maintenance`），键盘开着会话时设置页拿不到。所以设置页恢复时只调 `queue_learning_merge`：把备份快照里的输入记录挑出来，另存成一份只有这些记录的合法快照 `<preferences_directory>/pending-learning-merge.ndjson`（替换已有的一份），写完再校验一遍。键盘在建会话前调 `msime_client_personal_dictionary_sync` 时，`merge_pending_learning` 先合并这份文件：拿不到独占访问（还有会话）就保留文件下次再试；文件校验不过就删掉，不每次重试；合并失败保留；成功后删掉。结果在同步返回的 `learning_merged`/`learning_error` 里，不影响个人词库队列。HarmonyOS 也调这个同步，但没有待合并的文件，等于空操作。
- **整份激活的条件收紧**：原来本机用户词数为 0 就整份激活。现在快照带了学习状态，整份激活会替换本机的全部学习，所以条件改为「备份里有词、本机没有用户词、并且本机没有任何输入记录」（新增只读的 `learning_count`）。其余情况词走待发送队列、输入记录走待合并文件。备份里只有输入记录没有词时也走合并。
- **界面**：备份页的说明和导出结果都写明「备份文件里有你的输入记录……文件没有加密，请妥善保管」（`LocalBackup.PRIVACY_NOTICE`）；恢复确认框在备份带输入记录时列出条数；恢复结果多一句「N 条输入记录会在键盘空闲时合并，本机已有的保留本机」。页脚列出仍不在备份里的：输入统计、整句联想学到的上下文、拼写纠错习惯。

## Alternatives considered

- **设置页直接合并，不经待合并文件**：最直接，恢复完马上生效。但合并要独占维护权，Android 上键盘一显示就持有会话的共享锁，用户从设置页恢复时键盘常常还在后台持有它，直接合并大多会以 busy 失败；排进文件交给键盘在建会话前做，正好落在仓库已有的「个人词库写入只在没有会话时做」的维护边界上（`personal_dictionary_sync` 的约定），不用另起协调机制。
- **复用词库快照激活队列（`DictionarySnapshotQueue`）加一个合并模式**：激活队列已经是崩溃安全的，有租约和状态机。但它的落盘状态和 Java/Kotlin 两侧的读写都按「整份替换」设计，加模式要改 client-core 的队列格式和 Android 两处调用方，改动面远大于一份待合并文件；合并本身可重入（本机已有的都保留），文件丢了或重复合并都不会出错，用不着那套状态机。
- **恢复时一律整份激活，不做合并**：实现最省，学习状态原样恢复。但会悄悄替换掉新设备上已经学到的东西，和用户拍板的「合并、本机优先」相反。
- **把输入记录放进备份包的独立条目，另定格式**：能顺带装下快照格式没有的几张表（`pick_transitions`、`personal_bigram`/`personal_trigram`、`pinyin_typo_counts`、`pinyin_autocorrect_suppressions`、`pinned_candidates`）。但这些表的合并规则各不相同，要单独设计和校验；快照格式第 1 版本来就能装调权、删除、位置和计数，复用它旧版本也能读、整份激活路径零改动。那几张表这次不做，留在页脚里说明。
- **SyncApi.snapshotWords 加过滤**：分诊担心学习调权的 overlay 会被当成用户词塞进待发送队列。读代码确认 `snapshotWords` 只从 `entry` 记录取词，`overlay` 只用来标记删除，而学习记录从不写成 `entry`，所以不用改它。

## Consequences

- **收益**：换手机、重装或降级后，候选顺序的学习调整、固定位置和选词计数能随备份带走；新设备上已经学到的不会被覆盖；云同步上传的快照一个字节都没变。
- **代价与已知上限**：输入记录要等键盘下次建会话前才合并，键盘一直开着时不会生效；一份待合并文件只留最新的那份，两次恢复之间没打开过键盘时前一份会被替换（合并是本机优先的，再恢复一次即可补上）。整份激活和以前一样只在键盘没有会话时进行，激活前本机学到了东西会因版本冲突被拒，这时词和输入记录都没恢复，需要再恢复一次（这条走合并路径）。学习调权是相对旧设备的词库版本学到的，合并后按回放同样的方式写进本机词库。英文学习调权的显示形式在快照格式里没有单独字段，恢复后以词本身为显示形式。日志里有不是 UTF-8 的行时 `stream_dictionary_state` 会失败，带输入记录的导出随之失败（同样的日志也会让云同步的版本计算失败）。上面列的另外几张学习表仍不在备份里，#5659 仍只是部分完成。备份文件是明文，能看出用户常打的字词，只靠页面提示提醒用户保管。

## Verification

- `cargo test -p msime-engine --lib user_dictionary::state`：`a_merge_keeps_local_rows_takes_the_larger_count_and_writes_the_rest`（本机优先、计数取大、位置冲突、拼不出表名的跳过，再合并一次不改任何东西）、`a_failed_merge_changes_nothing`（不合规记录、读流失败、超出记录上限都整体回滚，工作词库字节不变）、`a_generation_without_the_main_dictionary_merges_english_rows_only`。
- `cargo test -p msime-host-api --lib dictionary_snapshot`：`an_export_without_learning_is_byte_for_byte_the_cloud_snapshot`（不带参数和 `include_learning:false` 的输出与抄下来的旧算法逐字节相同）、`a_learning_export_carries_the_journal_and_restages_to_the_same_revision`（带输入记录的导出经整份激活的记录流另建一代，`dictionary_state_revision` 与原日志相同）、`a_learning_export_leaves_out_what_the_format_cannot_carry`、`queued_learning_merges_at_the_next_sync_keeping_local_rows_and_the_larger_count`（会话开着时保留文件，之后按本机优先合并并删掉文件）、`a_backup_without_learning_queues_nothing`（旧格式备份不排任何东西）。
- `bash platforms/android/check-host.sh`：`LocalBackupPolicySmoke` 覆盖恢复结果里的输入记录说明，Gradle 编译覆盖页面与打包代码。
- 真机上「A 机导出、B 机恢复后调过序的候选还在」没有自动化，依赖 PR 上的 `android-device.yml` 和人工验收。
