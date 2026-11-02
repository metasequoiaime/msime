# Agent Note: 五笔版本与词库种类的宽松读取

Status: implemented

## Problem

新世纪五笔（内部 id `wubixsj`）要作为第三种五笔版本接入，计划见 `~/msime-reports/wubi-xinshiji-plan.md` 第 4 节。接入以后，五笔版本和个人词库种类都会出现一个新的取值，它会写进偏好文档、账号设置、学习日志、词库快照、WebDAV 检查点和云词库的改动记录。

已经发布的版本遇到这类不认识的取值，不是只跳过这一项，而是整份拒绝：

- **学习日志回放**：不认识的 `dictionary` 值算失败行，整次回放回滚，代次准备被拒，降级以后输入法用不了。
- **偏好文档**：`wubi_profile` 是严格的枚举，整份文档读不了。输入运行时靠逐键救援还能打字，但设置页读不了、存不了。
- **iOS 账号设置**：`input.wubi_schema` 的值不认识时拒绝整份文档，这台设备以后收不到任何设置。Android、桌面和鸿蒙共用的 `apply_input_settings` 本来就只跳过这个键。
- **词库快照**：快照校验（Rust 的 `inspect_snapshot_record`，以及 iOS 的两份 Swift 校验）只认五种种类，整份快照不合规。
- **WebDAV 检查点**：`ReplicaFile::into_replica` 遇到不认识的类别，整份检查点不认（`dictionary_sync_corrupt`）。事件里的操作本来就逐条跳过。
- **云词库改动记录**：`GET /v1/users/me/dictionary/changes` 的一侧带着不认识的 `kind`，`AccountDictionaryChange` 反序列化失败，整页读不了。

这些已经发布的版本救不了。能做的是先发一个不写任何新值、只把读取放宽的版本，让之后的版本遇到新世纪的数据时不出问题。

## Decision

这一步只放宽读取，不写任何新值，可以单独发布。每一处都是「只跳过不认识的那一项，其余照常」。

### 学习日志回放

- `replay_with` 先判断行的种类在不在 `JournalRow::KNOWN_KINDS`（`pinyin`、`wubi`、`wubi98`、`quick`、`english`）里。不在就计入 `skipped`，行留在日志里不动，回到认识它的版本时照常回放。
- 存不进去的键（例如不合规的拼音键）仍然算失败行，整次回放回滚。
- `docs/data-formats.md` §6.2 同步写明：更早发布的引擎仍把未知取值算失败行。

### 偏好文档

- 用已有的 `settle_newer_values_on_read`，与韩文布局、粤拼解释方式相同：读盘时 `wubi_profile` 是字符串、但 `WubiProfile::parse` 不认识的，就换成 `wubi86`。
- 只放宽读盘：`load`、`load_for_runtime` 和 `salvaged_from` 都经过这个函数。经 FFI 或 Tauri 提交的设置（`Preferences::from_submission`）和直接的反序列化仍是严格的，提交 `wubixsj` 会被拒绝。
- 本版本保存时写回 `wubi86`，不保留原值。

### 账号设置

- iOS 的 `IosPreferencePlan::from_cloud` 遇到不认识的 `input.wubi_schema` 时，`wubi_profile` 为 `None`，保留本机的五笔版本，文档其余的键照常应用。
- `apply_input_settings` 原来就是这样处理的，这次只把映射改成复用 `WubiProfile::parse`。

### 词库快照

- 种类合规的条件从「五种之一」放宽为「1 到 32 个小写字母、数字或下划线」（`record::is_foreign_kind`）。不是这个形状的种类，快照照旧不合规。
- **条数核对**：不认识的种类的那条记录照常参与格式核对和 entry/overlay 配对，并且计入 `engine_records`。iOS 交来的条数是按行数算的（`overlays + positions + selections`），所以条数必须按行算。
- **读成 Engine 记录时跳过**：`record::decode` 对这种 overlay 返回 `Ok(None)`。
  - `prepare` 读的是逐行的 `SnapshotLine`：跳过的那行照样算一条，不交给 Engine。文件路径用 `SnapshotFileRecords::into_lines`，宿主回调直接用 `decode` 的结果。
  - 合并输入记录（`merge_dictionary_state`）只给条数上限，`SnapshotFileRecords` 的 `Iterator` 直接跳过这种行。
- iOS 的 `shared/backend/clients/BackendSnapshotClient.swift` 和 `shared/snapshot/Sources/MSIMESnapshot/Snapshot.swift` 改用同样的形状判断（`SnapshotRecordFields.kindName`）。

### WebDAV 检查点

- `into_replica` 先核对一条词条的时间戳、写入者下标和序号，再看类别。类别不认识的只跳过那一条。
- 已知的代价写进了 `docs/dictionary-webdav-sync.md`：这样的客户端压缩时写出的检查点里，不再有这些词条。

### 云词库改动

- `AccountDictionaryChange` 的 `previous` 和 `replacement` 经 `known_kind_entry` 读入。`kind` 是字符串、但 `cloud::dictionary::kind_from_path` 不认识时，那一侧读成 `None`。
- 改动本身和它的修订号保留，翻页游标和「云端有没有改动」的判断（完整快照应用前的冲突检查）不受影响。
- `kind` 不是字符串仍报错。

## Alternatives considered

- **给 `WubiProfile` 写宽松的 `Deserialize`，不认识的值按 86。** 一处改动就让所有读取路径都宽松，`UpdateChannel` 和 `CandidateTranslationScope` 就是这样做的。不用它，是因为它同时放宽了设置提交：宿主提交一个写错的版本名会被悄悄当成 86 保存，接口照样返回成功。[偏好文档读不了时输入运行时照常运行](../bug-fix/2026-10-28-preferences-forward-compat-runtime.md) 定下的分工是读盘宽松、提交严格，`settle_newer_values_on_read` 正是为标量枚举留的口子。
- **快照里不认识种类的记录不计入 `engine_records`。** Rust 这一侧更直观：条数等于真正交给 Engine 的记录数。但 iOS 是自己按行数算条数再交给 `msime_client_snapshot_prepare` 的，两边会对不上，整份准备失败。要改就得同时改 iOS 的 Swift，而且新旧 iOS 和 host-api 的组合会错开。按行计数只需要 Rust 这一侧改。
- **WebDAV 检查点和快照原样保留不认识的词条。** 把它们作为不透明的数据带着走，压缩和恢复时不会丢，降级再升级也不丢。代价是 `Replica`、`DictionaryStateRecord` 和日志写入都要多一条「不认识的种类」的通道，而新世纪的词条在同步打开之前根本不会写进这两处（计划第 8 节：新世纪的云词库和 WebDAV 暂不同步）。等宽松读取的版本覆盖了足够多的设备再打开同步，丢的窗口就很小。
- **云词库改动里，带不认识种类的整条改动丢掉。** 页面和宿主看不到半条改动，形状更干净。但「云端有没有改动」的检查只看列表是否为空，丢掉以后它会把有改动判成没有，完整快照的冲突检查就漏了。所以只清空那一侧。

## Consequences

- **收益**：
  - 这一版之后的客户端遇到新世纪五笔的偏好、账号设置、快照记录、检查点词条和云端改动，都只跳过那一项，读取不中断。日志里的新世纪行在回放时跳过，输入法照常可用。
  - 但日志里只要有新世纪行，凡是要读出整份日志状态的功能在这一版里仍会失败，云同步和 WebDAV 同步因此停下，见「已知上限」。降级到这一版不会让输入法用不了，也不会丢数据，但同步要等升级回去才恢复。
  - 计划里的 PR-4 可以开始写 `wubixsj`：降级到这一版时输入法照常可用，降级到这一版之前的版本才会让输入法用不了。
- **代价**：
  - 跳过的数据在本版本里看不到。偏好保存时写回 86；检查点压缩时去掉那些词条；快照恢复时那些词条不进本机日志。升级回来以后，新世纪的选择和词条要靠别的设备或重新选择补回。
  - 回放不再用「未知取值算失败」来发现日志里的垃圾种类：日志被写坏、种类成了别的字符串时，现在也只是跳过。
  - 快照的种类校验从五个固定值放宽到名字的形状，iOS 两份 Swift 的旧用例 `"other"` 改成了不合形状的 `"Other-Kind"` 和空串。
- **已知上限**：
  - 已经发布的版本照旧整份拒绝，计划里的发布说明要写明：用过新世纪以后，降级到这一版之前的版本不受支持。
  - `state.rs` 读出整份日志状态（`stream_dictionary_state`）时，遇到不认识的种类仍报 `invalid_state`，这次没有放宽。从写 `wubixsj` 的版本降级回这一版，日志里就会有这样的行，依赖它的功能都会失败，直到升级回去：
    - 词库状态修订号（`dictionary_state_revision`）算不出来。host-api 的快照版本（准备时的 `version`、激活时的 `version_without_access`、空闲时发布给队列的本机版本）都依赖它，所以云同步的上传和下载都停在准备或激活这一步，每一轮都失败。本机的一代原样不动，不丢东西。
    - WebDAV 同步读本机词库（`dictionary_webdav::EngineDictionary::read`）每一轮都报 `LocalError::Unavailable`。鸿蒙设置应用和输入法之间的词条互通（`user_words_mirror`）用的是同一个读法，也停下。
    - 本地备份导出照样写词，但不带输入记录（`learning_error`）；`learning_count` 报错。
    - 为什么不放宽：云同步上传是整份替换云端快照。放宽以后，降级的设备会上传一份少了这些行的快照；等新世纪的词进了云同步，这会把别的设备的新世纪词从云端删掉。明确失败只让这台设备的同步停下，不伤数据。WebDAV 是逐条同步的，单看它放宽是安全的，但它和快照用的是同一个读法，这次没有为降级场景单独开口子。
    - 这一条由 `state.rs` 的 `unknown_kinds_text_that_is_not_utf8_and_a_stopped_export_are_invalid` 固定。
  - Tauri Android 合包（`apps/desktop/src-tauri/src/platform/android/android_account.rs`）有一份快照校验的副本没有改。这个包只用于开发验证、不发布，本机也编译不到它。

## Verification

在 Mac Studio 上经 `rbuild` 运行。Studio 上 SSH 会话默认 `ulimit -n` 是 256，引擎的会话测试并行时会打不开 fixture 数据库，所以测试命令前加了 `ulimit -n 65536`。

- `cargo test -p msime-engine`：全部通过，含 `rows_of_an_unknown_dictionary_kind_are_skipped_not_failed` 和改过的 `a_failed_row_rolls_the_whole_replay_back`。
- `cargo test -p msime-client-core`：全部通过，含以下用例：
  - `wubi_profile_defaults_to_86_and_reads_unknown_values_as_86`（原 `..._rejects_unknown_values`）：读盘按 86、保存写回 86、不是字符串仍报错，`from_submission` 拒绝 `wubixsj`。
  - `released_documents_with_an_unknown_wubi_profile_still_load`：六个已发布版本的样本换上 `wubixsj` 后都读得了、存得下。
  - `checkpoint_entries_of_an_unknown_kind_are_skipped`。
  - `kind_paths_round_trip_and_unknown_kinds_are_none`。
- `cargo test -p msime-client-services`：`typing_statistics::batch_tests::batched_commits_produce_the_same_document_as_writing_each_one` 在整套并行时失败一次，单独运行通过，与本改动无关；其余全部通过。相关用例：
  - iOS 的 `an_unknown_wubi_profile_keeps_the_local_one_and_the_rest_still_applies` 和 `the_wubi_profile_travels_as_the_account_wubi_schema`。
  - 桌面和鸿蒙的未知取值用例加了 `wubixsj`。
  - `dictionary_changes` 改动页带 `wubixsj` 的用例。
- `cargo test -p msime-host-api`：全部通过，含 `snapshot_records_of_an_unknown_kind_are_skipped_not_rejected`。
- `cargo clippy -p msime-engine -p msime-client-core -p msime-client-services -p msime-host-api --all-targets -- -D warnings`：无告警。
- `swift test --package-path shared/snapshot` 和 `swift test --package-path shared/backend --filter BackendSnapshotTests`：全部通过，含 `testRecordsOfAnUnknownKindStillValidate`。
