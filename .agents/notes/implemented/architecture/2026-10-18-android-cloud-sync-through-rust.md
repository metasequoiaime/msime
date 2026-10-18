# Agent Note: Android 的词库快照队列与自动云同步改调 Rust

Status: implemented

## Problem

#8201 迁移计划 5d。5c 以后 Android 主进程的会话和设置页面都已经经 Rust（[Android 设置页面的账号与云端操作改调 Rust](2026-10-18-android-settings-pages-through-rust.md)），但词库快照激活队列和自动云同步还是 Java 自己的一份：`DictionarySnapshotQueue.java` 把状态写在 `state.bin`、用 `FileChannel.lock`（fcntl）加锁，`home/CloudSync.java` 自己联网、合并、记进度（`SyncSwitch` 的 `msime_sync_v1` 和 `msime_cloud_sync_status` 两份 SharedPreferences）。Rust 的 `msime_client_snapshot_queue` 和 `msime_client_cloud_sync` 已经有同样的能力（[多分类自动云同步的编排收进 Rust](../../proposed/architecture/2026-10-16-cloud-sync-orchestration-in-rust.md)），而 Java 的 fcntl 与 Rust 的 flock 互相看不见：只要有一个使用方还用 Java 那份队列，另一边就会在它持锁时照样改状态。

## Decision

- **快照队列整组一起切。** `DictionarySnapshotQueue` 只剩 Rust 的薄包装（`read` 经 `import_android_v1`、`enqueue`、`cancel`、`process`），使用方在同一个提交里全部改调它：`:ime` 空闲时的 `DictionarySnapshotWorker`（现在是一次 `process`，只有排着的请求属于某个账号时才经 provider 问当前账号）、本地备份恢复的整份激活、退出登录和换账号时的取消、Tauri 合包 `AccountPlugin` 的三个命令。Java 的实现、`DictionarySnapshotPolicy`、两个 JVM 冒烟（`DictionarySnapshotQueueSmoke`、`DictionarySnapshotQueueWriteSmoke`）和 `test-android-snapshot-json-policy.py` 删掉；旧冒烟里还测着两道留下来的路径检查（Tauri 合包 `enqueueSnapshot` 来源文件的 `DictionarySnapshotPathPolicy.privateSource`、`:ime` 暂存目录的 `DictionarySnapshotWorker.ensureSafeDirectory`），这两项挪进新的 `DictionarySnapshotPathPolicySmoke`；`check-host.sh` 守着 Java 里不再有队列的锁、`state.bin` 和队列路径（路径只在 `DictionarySnapshotQueue` 里）。
- **云同步的编排、状态和措辞都在 Rust。** `account/RustCloudSync` 带着 `session_store` 调 `status`、`set_enabled`、`sign_out`、`run`、`commit`；`run` 四类都开，交出 Rust 读不到的本机值（按键反馈、Android 本地设置、皮肤库、还没认领的示例常用语）。`pending_commit` 时在账号绑定锁里先核对登录的仍是这一轮的账号，再把 `host_writes` 写进各自的存储、以 `run_id` 提交。`home/CloudSync` 只剩排队和弹选择：Rust 回答 `needs_choice` 时弹「合并 / 使用云端」，原因是 `cloud_cleared` 时多一个「关闭同步，保留本机」（`choice:"disable"`，表在 `CloudSyncChoicePolicy`）。「我的」页同步行用 Rust 的 `status_line`。
- **状态目录是 `files/bootstrap/state`**（状态在它下面的 `cloud-sync/`），两个进程用同一个绝对路径。`SyncSignals` 的记改动是 Rust 的 `mark_dirty`、看开关是 `peek`，两个进程都直接调 Rust；provider 去掉 `sync_dirty`，`sync_state` 只回账号绑定。
- **`SyncSwitch` 只剩账号绑定**（账号编号、登录方式、绑定代次和那把锁），它仍是 `:ime` 判断云剪贴板和候选翻译归属的依据；开关、游标、改动代数、收不下的常用语、上次同步时间都删掉，`check-host.sh` 守着 Java 不再读写这些键。
- **迁移顺序**：每个进程第一次用到云同步时做一次 `RustCloudSync.migrate`：主进程先 `import_android_v1` 转换旧队列，再 `import_legacy{android}` 导入两份旧偏好并带上队列目录（`check-host.sh` 核对这两步的先后）；`:ime` 只做导入这一步。登录后调一次 `status` 对齐账号；退出登录先调 `sign_out`（带队列目录，取消旧账号排着的快照），再清会话。
- **Rust 补一处缺口**：`HostLocal::dictionary` 把具名词库待发送队列（`DictionaryCollections/outbox.json`）里还没送进个人词库导入队列的增删也算作排着的改动，有就推迟上传（`dictionary_pending`），与原来 Java 的 `pendingQueuesBlocked` 相同。

## Wire 与时序

- `:ime` 必须也导入：升级后用户多半先用键盘，键盘记的改动落在还没导入的全新状态上（开关关着，Rust 不记），导入以后这次改动不算待上传，云端一变还会被下载盖回去。`:ime` 不转换旧队列：转换要在 `state.lock` 上等正在进行的激活，而键盘可能在主线程上记改动；还没转换的旧队列 Rust 的只读读取直接解码 `state.bin`，`:ime` 自己的 `process` 持锁时也会先转换。
- 主进程以外 never 调 `status`、`set_enabled`、`run`、`commit`、`sign_out`：它们按会话对齐，`:ime` 看不到登录，调了会把共享的进度清空。
- 本地备份恢复排快照要用键盘发布的本机词库版本（`read` 不发布），键盘没打开过时退回合并，与原来相同。

## Alternatives considered

- **只把队列切到 Rust，云同步留到 5f 以后再切。** 最强的理由是这一组会小很多，云同步的行为变化（三方合并、退避、词库合并分多轮）可以晚一点再让用户碰上。没有采用：云同步下载词库时要往队列里排快照，Java 的 `CloudSync` 留着就得继续用 Java 的队列，而队列不能有两种锁，计划第 6 节把两者列成一组原子提交。
- **`SyncSwitch` 整个删掉，账号绑定也改看 Rust 云同步状态里的 `account_id`。** 理由是又少一份 Java 状态。没有采用：Rust 状态里的账号只在调过 `status`、`run` 等以后才对齐，退出登录后是空的，而 `:ime` 的云剪贴板和候选翻译要的是「主进程这一刻绑定的是谁、绑定换过几次」，代次 Rust 没有；换成它要改 `:ime` 两处归属判断，不在这一组的范围里。
- **`:ime` 的 `process` 不给 `account_id`（头文件说不知道或未登录时不给）。** 理由是更贴头文件的字面。没有采用：原来 Java 在账号绑定已经清空（空串）时取消属于账号的请求，`sign_out` 之外这是唯一能取消旧账号快照的地方（比如登录时没读到用户 id），照旧传空串；provider 不可达时才不给。
- **具名词库待发送队列的问题留着，写进已知差异。** 理由是 Rust 是权威，这一组不改 Rust 更省事。没有采用：Rust 不看待发送队列时会先上传缺词的快照、清掉待上传标记，之后云端一有改动，下载的快照还会整份替换掉随后才送进去的词，本地备份恢复的词就丢了。

## Consequences

- **收益**：快照队列和同步状态只有 Rust 一份、一种锁；Android 拿到 Rust 的三方合并、删掉的常用语不复活、Retry-After 退避、词库合并分多轮做完才上传、空快照不替换有词的本机、同步进行中做的选择不丢、云端被清空时先问（迁移差异第 L 节，第 37–65、112–149 条）。
- **用户可见的变化**：云端被清空时多一个「关闭同步，保留本机」；同步行的措辞由 Rust 给（失败原因按 Rust 的文案，新加「云端数据已被清空…」两句）；回到前台时同步关着就直接跳过。
- **代价**：`home/` 下的接线只能靠 Gradle 编译和设备冒烟，JVM 冒烟跑不了 org.json 和 JNI；`SyncApi`、`SyncMergePolicy` 里只给旧 `CloudSync` 用的部分已经没人调用，5f 删掉，本地备份还要的读快照词挪进 `DictionarySnapshotWords`（[删掉 Android 已经交给 Rust 的账号与云端重复实现](../simplification/2026-10-19-android-duplicate-account-code-removed.md)）。Tauri 合包的 `cancelSnapshot` 回来的是取消时的状态，Rust 随后把终态请求从状态里拿掉，之后 `snapshotState` 看不到它。
- 相关：[Android 本地备份](../feature/2026-10-08-android-local-backup.md)（恢复时排整份激活的那一步改经 Rust 的队列，规则不变）。

## Verification

- JVM 冒烟（`check-host.sh`）：新加 `CloudSyncChoicePolicySmoke`（云端被清空时多「关闭同步」）；`AccountSessionRoutingPolicySmoke` 改成 provider 不再收 `sync_dirty`；`SyncSignalsSmoke` 只剩分类名。两项都临时改回源码确认会失败。`DictionarySnapshotPathPolicySmoke` 接住旧队列冒烟里那两道路径检查：来源路径经过指向 files 以外的链接时拒绝（链接那头真有文件，只靠最后一级不跟随链接拦不下），暂存目录本身或上一级是链接时抛 IOException、不在链接那头建目录；去掉逐级链接检查、或把 `SafePaths.ensureDirectory` 换成 `Files.createDirectories`，冒烟都会失败。
- `check-host.sh` 的两段新守卫、改写的 `test-android-sync-signals-routing-policy.py` 和 `test-android-file-reader-links.py` 都逐项改回源码确认会失败。
- Rust：`cloud_sync::tests::words_still_in_the_collections_outbox_block_the_dictionary_upload`，去掉待发送队列那一项时失败。
- 设备冒烟 `SessionStoreDeviceSmoke` 新加两项：一份 DataOutputStream 写的真实 `state.bin` 经 `import_android_v1` 转换以后，`DictionarySnapshotWorker` 照常激活它（本地备份的请求不问 provider）；两份旧偏好经 `importLegacy` 导入，队列里有这个账号的快照时词库游标保留、队列读不懂时只丢词库游标，导入以后 `peek` 和 `mark_dirty` 读写的是 Rust 的状态，属于账号的请求在不知道当前账号时原样留着、`cancel` 取消它。
- 写回本机（审查后补的）：设备冒烟 `SessionStoreDeviceSmoke.cloudSyncHostWrites` 直接调 `RustCloudSync.commit`，按 Rust 的 `HostWrites` 拼一份合成的 `host_writes`，四个存储放在 ContextWrapper 换出来的临时目录里：账号对不上时四项都不写、也不提交；对得上时按键反馈、Android 本地设置、皮肤库、示例常用语的认领都写对，随后的提交带着 `run_id` 到了 Rust（回答 stale_run）。设备冒烟 CI 不跑，`scripts/test-android-cloud-sync-host-writes.py` 另对着 Rust 源码核对 `write` 读的键名：`HostWrites` 除 `ios_native` 以外的每个字段和 `HostKeyboardFeedback` 的三个 camelCase 字段；逐个把 Java 里的键名改错，它都失败。
