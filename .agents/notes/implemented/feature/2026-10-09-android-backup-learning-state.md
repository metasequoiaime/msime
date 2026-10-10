# Agent Note: Android 本地备份带上输入记录并按本机优先合并

Status: implemented

## Problem

#5659 的用户最看重的是「输入记录」，而[本地备份](./2026-10-08-android-local-backup.md)只带了设置、皮肤、常用语和个人词库。Engine 的学习状态没进备份，原因有两层：导出端 `export_local_snapshot` 用 `dictionary_entries` 只读 `user_inserted=1` 的词，学习调权（`user_inserted=0` 的 upsert）、删除记录、固定位置和选词计数都没读；它的注释说「没有只读接口」，其实 `state::stream_dictionary_state` 早就有，只是没经 host 门面露出来。恢复端只有本机用户词数为 0 时才整份激活快照（会连同学习状态一起换掉），本机已有词时只合并词，没有任何把学习记录合并进现有日志的接口。

用户拍板的取舍：恢复时与本机已有记录合并，选词计数取大、位置冲突保留本机；备份仍是明文 zip，导出页明确提示文件含打字习惯、请妥善保管；输入记录不进云同步；输入统计不算在内。

## Decision

- **导出**：`export_snapshot` 加 `include_learning`（serde 缺省 false）。为假时代码路径不变，输出与以前逐字节相同，云同步上传的快照因此不受影响（`CloudSync` 的云同步调用不带这个字段）。为真时在用户的词之后，按快照格式第 1 版已有的记录追加输入记录：学习调权写成 `user_inserted:false` 的 `overlay`，删除记录写成 `deleted:true` 的 `overlay`，固定位置写成 `position`，选词计数写成 `selection`（截到 0..=10）。编码、词或上下文含换行/制表符、位置不在 1..=5、或总记录数超过 500,000 的跳过并计入 `learning_skipped`。写完照常用 `inspect_snapshot` 自检。数据来自新露出的 `msime_engine::host::stream_dictionary_state`。它对日志是整体读取的，一行不是 UTF-8、未知的词库种类、超长的行或超过 500,000 行都会让整次读取失败；这时导出不失败，照样写出词，`learning` 为 0，原因放在 `learning_error`，导出结果告诉用户这份备份里没有输入记录。否则一台日志有坏行的设备连原本能导出的词、设置和皮肤都备份不了，比加这个功能之前还差。备份包格式 `LocalBackupPolicy.VERSION` 仍是 1，manifest 的 `contents` 多一个 `learning` 计数；旧版本读到这些记录也是合法的 v1 快照。
- **合并规则**（`msime_engine::user_dictionary::state::merge_dictionary_state`，host 门面 `merge_dictionary_state`）：一个 IMMEDIATE 事务里写工作词库和日志，任何一条记录不合规或读流失败都整体回滚。学习调权和删除记录只在本机日志里还没有同一 `(dictionary,key,value)` 时写入，写法与个人词库编辑相同（`apply_pinyin`/`apply_simple`/`apply_english` 改工作词库，再记日志，`user_inserted` 照记录保留）；本机已有就保留本机。固定位置用 `INSERT OR IGNORE`：本机已固定这个词，或这个位置已被占用，都保留本机。选词计数 `ON CONFLICT ... WHERE excluded.selection_count>selection_count`，取两边较大的那个。用户自己的词不在这里写，只计数，仍走原来的命名词库待发送队列。编码拼不出拼音表名、这一代没有那张表（准备语句时报 `no such table`，什么也没执行）、没有中文词库的版本里的非英文行、权重低于 1 的拼音调权都跳过计数，不让整次合并失败。别的 SQL 错误（磁盘满、I/O 失败、库损坏、约束）不算跳过，照常上抛让整个事务回滚：把它们也当成跳过，磁盘满时没写进去的记录会被计成 skipped、事务照样提交、待合并的文件随即删掉，这些记录就永久丢了。
- **合并时机**：改工作词库要独占维护权（`DictionaryAccess::try_maintenance`），键盘开着会话时设置页拿不到。所以设置页恢复时只调 `queue_learning_merge`：把备份快照里的输入记录挑出来，另存成一份只有这些记录的合法快照 `<preferences_directory>/pending-learning-merge.ndjson`（替换已有的一份），写完再校验一遍。键盘收起、会话销毁后，`DictionarySnapshotWorker.process` 在处理完排着的整份快照激活之后调 `msime_client_dictionary` 的 `merge_pending_learning`（排在激活之后，是因为合并会改本机词库版本，先合并会让排着的激活因版本不符被拒）。它先拿独占维护权，拿不到就什么也不动；拿到后把文件改名认领成 `pending-learning-merge.claimed.ndjson`，只合并、只删认领的那份，合并期间设置页又排的新一份写在原来的名字下，不会被误删，下次空闲再合并；上次没合并完的认领文件还在时先合并它。文件不是合法快照就删掉；其余失败（读文件出错、写库出错）保留文件，次数记在 `pending-learning-merge.attempts`，连续三次失败后放弃并删掉（`learning merge abandoned`），不会每次空闲都把整份合并跑一遍再回滚。成功后删掉文件。不放进建会话前的 `msime_client_personal_dictionary_sync`：那条同步跑完才建会话，一份几万条的备份会让恢复后第一次弹出键盘迟迟没有引擎，而整份快照激活本来就只在空闲时做。HarmonyOS 的个人词库同步因此完全不受影响。
- **整份激活的条件收紧**：原来本机用户词数为 0 就整份激活。现在快照带了学习状态，整份激活会替换本机的全部学习，所以条件改为「备份里有词、本机没有用户词、并且本机没有任何输入记录」（新增只读的 `learning_count`）。其余情况词走待发送队列、输入记录走待合并文件。备份里只有输入记录没有词时也走合并。
- **界面**：备份页的说明和导出结果都写明「备份文件里有你的输入记录……文件没有加密，请妥善保管」（`LocalBackup.PRIVACY_NOTICE`）；恢复确认框在备份带输入记录时列出条数；恢复结果多一句「N 条输入记录会在键盘空闲时合并，本机已有的保留本机」。页脚列出仍不在备份里的：输入统计、整句联想学到的上下文、拼写纠错习惯、置顶的候选。
- **整份激活还是合并**的判断是纯函数 `LocalBackupPolicy.dictionaryRestore`，有 JVM 冒烟覆盖；本机的词数或输入记录条数读不出来时按合并处理，宁可不整份恢复，也不冒覆盖本机学习状态的险。

## Alternatives considered

- **设置页直接合并，不经待合并文件**：最直接，恢复完马上生效。但合并要独占维护权，Android 上键盘一显示就持有会话的共享锁，用户从设置页恢复时键盘常常还在后台持有它，直接合并大多会以 busy 失败；排进文件交给键盘在会话销毁后的空闲处理里做，正好落在仓库已有的「词库维护只在没有会话时做」的边界上（快照激活用的就是这个时机），不用另起协调机制。
- **复用词库快照激活队列（`DictionarySnapshotQueue`）加一个合并模式**：激活队列已经是崩溃安全的，有租约和状态机。但它的落盘状态和 Java/Kotlin 两侧的读写都按「整份替换」设计，加模式要改 client-core 的队列格式和 Android 两处调用方，改动面远大于一份待合并文件；合并本身可重入（本机已有的都保留），文件丢了或重复合并都不会出错，用不着那套状态机。
- **恢复时一律整份激活，不做合并**：实现最省，学习状态原样恢复。但会悄悄替换掉新设备上已经学到的东西，和用户拍板的「合并、本机优先」相反。
- **把输入记录放进备份包的独立条目，另定格式**：能顺带装下快照格式没有的几张表（`pick_transitions`、`personal_bigram`/`personal_trigram`、`pinyin_typo_counts`、`pinyin_autocorrect_suppressions`、`pinned_candidates`）。但这些表的合并规则各不相同，要单独设计和校验；快照格式第 1 版本来就能装调权、删除、位置和计数，复用它旧版本也能读、整份激活路径零改动。那几张表这次不做；后来按这条路另存了一个条目，见[输入习惯与恢复校验](./2026-10-10-android-backup-habits-and-verified-restore.md)。
- **在建会话前的个人词库同步里合并**：键盘每次弹出都会先跑这条同步，恢复后最快生效，也是这个分支最初的做法。但同步跑完才建会话，合并一份大备份要逐条查日志、改词库，恢复后第一次弹出键盘会有几秒没有引擎；合并一直失败时更是每次弹出都付一遍代价。HarmonyOS 也调这条同步，放在这里还让它的契约平白多出一块。所以改到空闲处理里。
- **SyncApi.snapshotWords 加过滤**：分诊担心学习调权的 overlay 会被当成用户词塞进待发送队列。读代码确认 `snapshotWords` 只从 `entry` 记录取词，`overlay` 只用来标记删除，而学习记录从不写成 `entry`，所以不用改它。

## Consequences

- **收益**：换手机、重装或降级后，候选顺序的学习调整、固定位置和选词计数能随备份带走；新设备上已经学到的不会被覆盖；云同步上传的快照一个字节都没变。
- **代价与已知上限**：输入记录要等键盘收起一次（会话销毁后的空闲处理）才合并，恢复后键盘一直没收起时不会生效；还没被认领的待合并文件只留最新的那份，两次恢复之间键盘没收起过时前一份会被替换（合并是本机优先的，再恢复一次即可补上）。连续三次合并失败后放弃那份文件，用户要再恢复一次；键盘侧的失败不会提示用户。合并在键盘进程的 `preferencesWorker` 上跑，与建会话前的同步共用这个串行线程，正在合并时立刻弹出键盘，建会话要等它跑完（与整份快照激活相同）。整份激活和以前一样只在键盘没有会话时进行，激活前本机学到了东西会因版本冲突被拒，这时词和输入记录都没恢复，需要再恢复一次（这条走合并路径）。学习调权是相对旧设备的词库版本学到的，合并后按回放同样的方式写进本机词库。英文学习调权的显示形式在快照格式里没有单独字段，恢复后以词本身为显示形式。日志里有不是 UTF-8 的行时 `stream_dictionary_state` 会失败，这时备份只带词、不带输入记录（同样的日志也会让云同步的版本计算失败）。上面列的另外几张学习表（`personal_bigram`/`personal_trigram`、`pick_transitions`、`pinyin_typo_counts`、`pinyin_autocorrect_suppressions`、`pinned_candidates`）不在快照里，由[输入习惯与恢复校验](./2026-10-10-android-backup-habits-and-verified-restore.md)另存成 `habits.ndjson` 条目，与这里的输入记录在同一次空闲处理里合并。备份文件是明文，能看出用户常打的字词，只靠页面提示提醒用户保管。

## Verification

- `cargo test -p msime-engine --lib user_dictionary::state`：`a_merge_keeps_local_rows_takes_the_larger_count_and_writes_the_rest`（本机优先、计数取大、位置冲突、拼不出表名的跳过，再合并一次不改任何东西）、`a_failed_merge_changes_nothing`（不合规记录、读流失败、超出记录上限都整体回滚，工作词库字节不变）、`a_missing_table_is_skipped_but_a_write_failure_rolls_back`（没有那张表的行跳过；触发器模拟的写库错误让整个合并回滚，已调大的选词计数也撤销）、`a_generation_without_the_main_dictionary_merges_english_rows_only`。
- `cargo test -p msime-host-api --lib dictionary_snapshot`：`an_export_without_learning_is_byte_for_byte_the_cloud_snapshot`（不带参数和 `include_learning:false` 的输出与抄下来的旧算法逐字节相同）、`a_learning_export_carries_the_journal_and_restages_to_the_same_revision`（带输入记录的导出经整份激活的记录流另建一代，`dictionary_state_revision` 与原日志相同）、`a_learning_export_leaves_out_what_the_format_cannot_carry`、`queued_learning_merges_when_idle_keeping_local_rows_and_the_larger_count`（会话开着时保留文件，建会话前的个人词库同步不碰它，之后按本机优先合并并删掉文件）、`a_queue_made_while_a_claimed_merge_runs_survives_it`（认领后新排的一份不被删掉，下一次合并它）、`a_failing_merge_is_retried_then_abandoned`（写库一直出错时保留两次、第三次放弃，格式不对的文件直接删）、`a_learning_export_falls_back_to_words_when_the_journal_cannot_be_read`（日志有非 UTF-8 行时只导出词并报 `learning_error`）、`a_backup_without_learning_queues_nothing`（旧格式备份不排任何东西）。
- `bash platforms/android/check-host.sh`：`LocalBackupPolicySmoke` 覆盖恢复结果里的输入记录说明，以及整份激活还是合并的判断（新设备激活；本机学过但没有词、本机有词、备份只有输入记录、计数读不出来都合并），Gradle 编译覆盖页面与打包代码。
- 真机上「A 机导出、B 机恢复后调过序的候选还在」没有自动化，依赖 PR 上的 `android-device.yml` 和人工验收。
