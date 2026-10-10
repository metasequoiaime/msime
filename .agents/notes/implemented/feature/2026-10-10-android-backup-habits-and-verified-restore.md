# Agent Note: Android 本地备份带上全部输入习惯，恢复前整份校验、失败时撤销

Status: implemented

## Problem

#5659 要的是「导出全部配置、词库和输入记录，换手机或降级后再导入」，用户最看重输入记录。[本地备份](./2026-10-08-android-local-backup.md)已经有 SAF 导出/导入、带版本号的文件名和合并式恢复，[输入记录那一篇](./2026-10-09-android-backup-learning-state.md)补上了学习调权、删除记录、固定位置和选词计数。剩下两处让这个 issue 关不掉：

- 日志里还有六张学习表不在备份里：整句联想的二元/三元计数（`personal_bigram`/`personal_trigram`）、连续选词对（`pick_transitions`）、拼写纠错计数（`pinyin_typo_counts`）、自动纠错抑制（`pinyin_autocorrect_suppressions`）和置顶的候选（`pinned_candidates`）。换手机后整句联想、纠错习惯和置顶都要从头学，正是用户说的「输入记录」里最显眼的那部分。
- 恢复没有完整性校验，也没有回滚。包被截断或改过时，manifest 和各条目各自能读出多少就恢复多少；设置、皮肤、常用语按顺序写，一部分失败时前面的已经改了、后面的没改，本机落在一个备份和原状都不是的中间状态，结果里只是点名失败的部分。

## Decision

### 输入习惯另存一个条目

- **Engine**（`msime_engine::user_dictionary::habits`，host 门面 `stream_learning_habits`/`count_learning_habits`/`merge_learning_habits`）：六张表各自一种 `LearningHabit` 记录，只读流在一个 `query_only` 事务里按主键顺序读出，旧日志缺的表当作空表，文字不是 UTF-8 时整体失败（与 `stream_dictionary_state` 相同）。合并在一个 IMMEDIATE 事务里：各种计数取两边较大的那个（不相加，同一份备份恢复两次不翻倍），`updated_at` 取较新的，置顶的候选本机这个上下文已有就保留本机；任何一条不合规（`habit_is_storable`：文字空、超长、含 NUL，计数越界，纠错音节不是小写字母）、读流失败或写库出错都整体回滚。写完按各表平时的规则压回上限：整句联想超过 `max_transitions`（20 万）时像 `decay_locked` 那样反复减半到四分之三以下（共用 `ngram_store::HALVE_SQL`），选词对按 `record_pick_transition` 的语句删最旧的，纠错计数和抑制按 `trim_typo_table` 的顺序删到 2048 行。不压的话，一份大备份合并进来后整句联想的 store 下次加载会因「超出内存上限」整个读不进来。合并前先 `flush_journal` 把 store 排着的写入落盘，合并后 `invalidate_journal` 把它的内存模型标成过期，下次用到时重读。
- **文件格式**（host-api `dictionary_snapshot::habits`）：NDJSON，header `{"type":"header","format":"msime-learning-habits","version":1}`，每条一行（`bigram`/`trigram`/`pick`/`typo`/`suppression`/`pin`），footer 带条数和前面全部字节的 SHA-256，写法与词库快照一致。读的时候每行字段必须恰好是那几个、类型正确且能写进日志；条数、校验和对不上，footer 后还有内容，或 `version` 不是 1，都算坏文件。
- **host-api 操作**（`msime_client_dictionary`）：`export_habits {destination}` 写文件并自检，返回 `{path,habits,skipped}`；`inspect_habits {source}` 只读校验；`queue_habits_merge {source}` 校验后存成 `<preferences_directory>/pending-habits-merge.ndjson`。`merge_pending_learning` 在合并完待合并的输入记录后，用同一套认领、重试三次后放弃的规则（提成 `merge_pending` 与 `PendingFiles`）合并它，结果多一个 `habits` 字段；没有待合并的输入习惯时返回值与以前逐字节相同。`learning_count` 多返回 `habits`：整份快照激活另建的那一代没有这几张表，Android 判断能不能整份激活时要把它算进「本机学过的东西」，读不出来时为 null，按合并处理。
- **Android**：导出时 `export_habits` 写 `habits.ndjson` 进包，manifest 的 `contents.habits` 记条数；读不出来时与输入记录一样不让整份备份失败，只是包里没有这一项、导出结果如实说。恢复时不管词库走整份激活还是合并都排输入习惯（激活后的新一代里没有这几张表），由键盘收起后的空闲处理先激活、再合并输入记录和输入习惯。界面上把这几项从「不在备份里」挪到「输入记录」的说明里。

### 恢复前整份校验

- manifest 新增 `checksums`：其余每个条目的 SHA-256（`LocalBackupPolicy.ENTRIES`）。导出时先算好再把 manifest 写在最前面。
- `LocalBackup.prepare` 在让用户确认之前做完全部校验，任何一项不过就是 `Compatibility.DAMAGED`，提示「文件已损坏或不完整，本机没有任何改动」：每个认识的条目按自己的上限读完算 SHA-256，与 `checksums` 核对（`LocalBackupPolicy.checksumsMatch`：记了的都在且相同，在的都记了；manifest 里这个版本不认识的条目名不核对）；设置、本地设置、皮肤和常用语按恢复时的读法解析一遍；词库快照和输入习惯解到工作目录交给原生侧 `inspect_snapshot`、`inspect_habits` 完整校验。旧版本导出的包没有 `checksums`，跳过核对，其余照做。
- 格式版本 `LocalBackupPolicy.VERSION` 仍是 1：`checksums` 和 `habits.ndjson` 都是旧版本会忽略的键和条目（它只按固定条目名读包、只读 manifest 里认识的键），降级后恢复新版本导出的包照样能恢复其余部分。

### 设置、皮肤和常用语失败时撤销

- `LocalBackup.restore` 先恢复会改写本机现有内容的三部分（设置含 Android 本地设置和按键反馈、皮肤、常用语），动手前 `Undo.capture` 记下原状：本机自己导出的设置文档、按键反馈、本地设置的显式值、皮肤库，恢复过程中再记下新加的常用语 id。任何一部分写入失败就 `Undo.apply` 全部换回去，不再碰词库和输入记录，提示「这次恢复已经全部撤销」；撤销本身失败时如实说可能只恢复了一部分（`LocalBackupPolicy.rolledBack`）。
  - 设置不按文件换回，而是把记下的设置文档按与恢复相同的方式再应用一次：偏好由 client-core 按修订号比较并交换写入，键盘进程同时在读写，直接拷回旧文件会让修订号倒退。
  - 本地设置只把变了的键改回原值，原来没写过的键删掉；皮肤库整份写回（新增的 `CustomSkinLibrary.replaceAll`，照片引用原样，照片文件不受合并影响）；常用语删掉这次新加的那几条。认领预置示例挪到三部分全部成功之后，失败时不用撤销认领。
  - 常用语里正文不合规、超过条数或大小上限的条目仍然只是跳过并在结果里说明，不触发撤销；client-core 报其他错误（文件损坏、存储不可用）才算失败。
- 词库、输入记录和输入习惯在三部分都成功后才排：它们只增不删、本机优先，排到一半失败时已排进去的照样写入、在结果里点名，再恢复一次不会重复，所以不做撤销。

### 本地恢复的整份激活不按账号比对

- 新手机或重装（本机没有用户词、也没有任何输入记录）时，恢复把整份词库快照交给激活队列 `DictionarySnapshotQueue`，所有者是固定的 `DictionarySnapshotQueue.LOCAL_RESTORE_OWNER`（`"local-backup"`，磁盘上已经排着的请求就是这个值，不能改）。队列的账号围栏（认领和激活时请求的所有者必须等于 `SyncSignals.accountId`）是给云同步的快照准备的：退出登录或换账号后，旧账号下载的快照不能再激活。当前账号只会是空字符串（未登录）或真实账号 id，永远不等于这个标记，所以本地恢复的请求以前一到认领就被当成别的账号的请求取消、快照被删，词和输入记录全部丢掉，而界面已经说了「会在键盘空闲时写入」。
- 现在认领（`claim`）和激活（`complete`）先看请求的所有者：是 `LOCAL_RESTORE_OWNER` 就直接算本机的，不比对、也不去问当前账号（provider 暂时不可达时照样激活）；其余请求的比对原样不变，云同步请求在账号不符时照旧取消，账号读不到时照旧原样保留。退出登录的 `cancel(accountId)` 只取消该账号的请求，碰不到本地恢复的请求。
- 绕过围栏之后在模拟器上发现激活本身也从来没成功过：队列里记的本机版本是 `local-v1:<代次>:<摘要>`，`DictionarySnapshotWorker` 把这整串原样交给原生的 `snapshotPrepare`（`expected_version`）和 `snapshotActivate`，而原生侧只收 64 位的摘要（`prepare` 长度不是 64 就报「invalid snapshot bounds」，JNI 的激活入口长度不是 64 直接拒绝），请求于是一律 FAILED、快照被删。自 2026-09-13 队列上线起 Android 的整份激活（云同步第一次同步和本地恢复）都是这样，JVM 冒烟只用假的激活回调，碰不到原生侧。现在由 `DictionarySnapshotQueue.nativeVersion` 从本机版本里拆出摘要再交给原生侧，与 iOS 的 `activateDictionarySnapshot` 相同；队列自己的比较（`complete` 里的版本核对、回执）仍用带前缀的整串。

## Alternatives considered

- **把六张表塞进词库快照**：快照格式本来就有版本号和校验，旧版本也读这个文件。但快照记录同时是 `DictionaryStateRecord`，其修订号按记录逐字节计算、与 Apple 共用，加一种记录就会改掉每一台设备的词库版本、让云同步把它们都当成新版本；旧版本读到不认识的记录类型会整份拒绝快照，降级恢复时连词都恢复不了。另起一个条目，旧版本只是读不到它。
- **合并时把计数相加**：两台设备各学各的，相加最接近「两边的习惯都算」。但同一份备份恢复两次就翻倍，一次中断后重试也会多算；取较大值可重复执行，与选词计数的规则一致。
- **整个恢复做成全有全无，词库也撤销**：语义最干净。但词进了命名词库的待发送队列、输入记录排给了键盘的空闲处理，撤销要跨进程把队列里的请求一条条收回来，而这几部分本来就是只增不删、本机优先的合并，半途失败的后果只是少恢复一些，用户再恢复一次即可。为它们引入跨进程撤销，复杂度远超收益。
- **撤销时直接拷回设置文件和皮肤库文件**：最省事，字节级还原。但偏好文件带修订号，键盘进程在同时读写，拷回旧文件会让修订号倒退、让之后的比较并交换判断错误；所以设置走与恢复相同的应用路径，只有皮肤库这种整份原子替换、没有修订号的文件才整份写回。
- **只靠 zip 自带的 CRC 判断损坏**：不用改格式。但 CRC 只在解压到条目末尾时才校验，管不了被换掉或被删掉的整个条目，也不是给篡改准备的；manifest 里的 SHA-256 一次核对全部条目，再加上逐项解析和原生侧的完整校验，恢复前就能拒绝坏包。
- **本地恢复不再整份激活，一律走合并**：词进待发送队列、输入记录和输入习惯排给空闲合并，合并进空的本机得到同样的结果，这条路已经有测试，账号围栏也就碰不到本地恢复。但几千个词要经个人词库队列一批批送、分很多次键盘空闲才写完，整份激活一次就到位；整份激活还原样保留旧设备的词库版本，合并则是在本机另记一遍。新手机正是 #5659 最主要的场景，为了绕开围栏让它变慢不划算，围栏本身只需要认出一种不属于账号的请求。
- **用当前账号 id 作所有者入队**：不用动队列。但未登录时账号是空字符串，队列拒绝空的所有者；登录状态下恢复后再退出登录，`cancel(accountId)` 会把这份本地恢复一起取消，用户没做错什么却丢了恢复。

## Consequences

- **收益**：换手机、重装或降级后，整句联想学到的上下文、连续选词、拼写纠错习惯和置顶都随备份带走，计数取大、本机优先；坏掉或不完整的备份在确认前就被拒绝，本机一点不改；设置、皮肤或常用语写到一半失败时本机回到恢复前，不会落在中间状态。#5659 的需求（全部在本机、导出成一个带应用名和版本号的文件、经系统文件选择器存到任意位置、从文件导入、设置/词库/输入记录）至此都覆盖了。
- **新手机恢复真正生效**：本地恢复的整份激活不再被账号围栏取消，激活也不再因版本格式被原生侧拒绝；同一处原因此前也让云同步第一次同步（本机没有词时整份激活云端快照）在 Android 上从未成功，这次一并修好。Android 9 上导出个人词库快照（本地备份导出和云同步上传都经 `export_snapshot`）原来因为在系统临时目录建自检文件而每次失败，现在自检文件建在目标旁边。
- **代价与已知上限**：输入统计（用户决定不进备份）、剪贴板历史、诊断日志、账号与 AI/翻译服务的凭据、语音授权和开发者选项仍不在备份里；自定义皮肤的照片和命名词库的分组也还不在，需要维护者决定是否补（照片会让包变大，分组恢复要和社区词库的重新安装协调）。备份格式只有 Android 实现，其他平台读不了它。输入习惯与输入记录一样要等键盘收起一次才合并，键盘侧的失败不提示用户。撤销依赖本机能导出自己的设置文档，读不出来时整次恢复不开始。旧版本导出的包没有校验和，只能靠解析和原生侧校验发现损坏。

## Verification

- `cargo test -p msime-engine --lib user_dictionary::habits`：流与合并的往返（新日志里合并后流出相同的记录，再合并一次什么也不改）、本机优先和计数取大、不合规记录/读流失败/超出条数整体回滚、触发器模拟的写库错误整体回滚、选词对和抑制记录压回上限、整句联想超过 20 万行时减半到四分之三以下。
- `cargo test -p msime-host-api --lib dictionary`：`learning_habits_export_queue_and_merge_when_idle`（导出、校验、排队，会话开着时留着文件，空闲时合并并删掉，计数取大、置顶保留本机，`learning_count` 带上 `habits`）、`a_damaged_habits_file_is_refused_and_a_damaged_queue_is_dropped`、`a_snapshot_is_inspected_without_side_effects`，以及原有输入记录合并的用例。
- `bash platforms/android/check-host.sh`：`LocalBackupPolicySmoke` 覆盖校验和核对（旧包、对上、大小写、改过、缺少、多出）、条目上限、撤销提示和带输入习惯的恢复结果。
- API 35 模拟器（`msime-client-test`，arm64，`build-apk.sh` 出的 full 包）：在「试用键盘」里打几句话后经「备份与恢复」导出，SAF 存到 Downloads，拉回的包里 `checksums` 与各条目的 SHA-256 全部一致，`habits.ndjson` 有 26 条（二元、三元、选词对）。把其中一条计数改掉重新打包，恢复时提示文件已损坏；去掉 `checksums` 模拟旧格式再改同一条，原生校验同样拒绝；截断的 zip 提示读不出文件。`pm clear` 后重新准备、恢复原来那份，键盘弹出再收起一次后再导出，输入习惯与原来的逐行相同，设置文档也相同。撤销路径（设置、皮肤或常用语写到一半失败）没有在设备上制造出来，只有代码路径和 JVM 冒烟。
- `platforms/android/tests/dictionary/DictionarySnapshotQueueSmoke.java`（`check-host.sh` 自动发现）：`LOCAL_RESTORE_OWNER` 的请求在当前账号为空字符串、真实账号 id、null（provider 不可达）三种情况下都被认领并激活，`cancel(account)` 碰不到它；云同步请求原有的账号比对用例不变。修复前这段在第一处 `claim` 的断言上失败。`nativeVersion` 从 `legacy` 和 UUID 代次的本机版本里拆出摘要，裸摘要、null、大写摘要都报 INVALID。
- API 35（emulator-5580）与 API 28（emulator-5566）专用 AVD，Studio 上 `build-apk.sh` 出的 full 包（本机重签发布密钥）：`run-core-test.sh` 两台都是 1167 passed；`smoke.sh emulator-5580` 全套与 `smoke.sh emulator-5566 --core` 通过。新手机路径：在 API 35 上加两个词、打几个字后导出（3 个词、`learning` 1、`habits` 11），`pm clear` 后启用键盘、弹出收起一次（本机已发布词库版本，未登录），恢复走整份激活；键盘再收起一次后队列状态为 APPLIED、本机版本换成带激活 id 的新一代，词库页 3 个词都在，再导出的 `dictionary.ndjson`、`habits.ndjson`、`settings.json` 去掉时间戳后与原备份逐行相同。只修围栏、未修版本格式的包在同一步骤里请求变成 FAILED、词库为空，这就是发现第二处问题的经过。API 28 上同一份备份走同样的新手机路径，结果同样逐行相同；另在 API 28 上先打字让本机有输入记录再恢复，走合并路径：`pending-learning-merge.ndjson` 和 `pending-habits-merge.ndjson` 写在 `files/bootstrap/state/`，键盘收起一次后两份都被合并删掉，再导出时备份里的每条输入记录和输入习惯都在、计数不小于备份。API 28 上导出在修临时文件之前报 `snapshot file unavailable`，修后成功。
