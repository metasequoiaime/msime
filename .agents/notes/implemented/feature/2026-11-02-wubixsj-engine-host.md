# Agent Note: 引擎与宿主接入新世纪五笔

Status: implemented

## Problem

新世纪五笔要作为第三个五笔版本接进来。它和 86 差得很多：GB2312 里约 23% 的字全码不同，二码简码有三分之一不同（力 86 是 `lt`、新世纪是 `ent`；为 `ylyi` / `ytny`）。所以它必须有自己的码表，不能在 86 表上打补丁。

码表本身由 dict-builder 写进 `msime-wubi.db` 的第三张表 `wubixsj`，那是另外的 PR。这一步要做的是：引擎、C 接口、宿主接口和共享客户端层都认识这个版本，能读这张表、能存这个版本的个人词条。同时不能伤到两类人：

- 还在用 86/98 的用户；
- 已经发布、不认识新值的旧客户端。

计划全文在维护者的 `~/msime-reports/wubi-xinshiji-plan.md` 第 4 节，维护者的决定在第 8 节。宽松读取（不写任何新值、让之后的版本遇到新值只跳过）是前一个 PR，见 [五笔版本与词库种类的宽松读取](../architecture/2026-11-02-wubi-kind-forward-compatible-reads.md)。

## Decision

内部 id 统一是 `wubixsj`。所有枚举只追加、不重新编号：

| 位置 | 新值 |
|---|---|
| `WubiProfileKind` | `WubiXsj = 2` |
| `PersonalDictionaryKind` | `WubiXsj = 5` |
| client-core | `WubiProfile::WubiXsj`、`cloud::dictionary::DictionaryKind::Wubixsj`、`dictionary_sync::WordKind::WubiXsj` |
| client-services | `PersonalWordKind::WubiXsj`、`ImportKind::Wubixsj` |
| host-api | `Kind::Wubixsj`、`WordKind::WubiXsj` |
| engine-capi | `WubiProfile::Wubixsj`、`KindJson::Wubixsj` |
| mcp-server | `WubiVersion::WubiXsj`、`Dictionary::Wubixsj` |

所有序列化名都是 `wubixsj`（`rename_all = "snake_case"` 的枚举用 `Wubixsj` 命名，或者显式 `rename`）。网页引擎（engine-wasm）不变，仍然只有 86。

### 读码表

- **provider 和查码**：`wubi::provider` 加第三组 SQL（前缀、通配、14 键、反查），`REVERSE_INDEXES` 加 `idx_wubixsj_value`。`wubi::lookup::WubiTextCodes` 加 `wubixsj` 列，`wubi_codes` 的 JSON 多一个 `wubixsj` 键，词库里没有这张表时是空数组。
- **逐表并入代次**：`SPLIT_WUBI_TABLES` 是三张表。`merge_downloaded_wubi` 不再是「主词库里有任何一张五笔表就不动」，改用 `lacks_split_wubi_tables`：主词库还缺、而下载的码表里有的表才补进来。
  - 这一条是给已有代次用的：代次是用没有新世纪表的旧码表准备的，之后下载的码表带着 `wubixsj`，只补这一张，86/98 和学到的权重不动。
  - `merge_split_wubi` 本来就逐表处理，不用改。
  - 查码选「读代次还是读下载的那份」也用同一个判断。
- **回放逐行看表**：`replay` 先查出代次里有哪几张五笔表，一行五笔记录只看它自己的表在不在，不在就计入 `skipped`。以前是「有任何一张五笔表就逐行写、缺的那张算失败」。两种情况因此都只是跳过，不再整次回滚：
  - 旧词库没有 `wubixsj` 表；
  - 有 86、没有 98 的代次。

### 写日志：只写主动选了新世纪的用户

`wubixsj` 是已经发布的引擎不认识的日志种类。前一个 PR 以前的版本遇到它会整次回放回滚，降级后输入法就用不了。所以这个种类的行只出自选着新世纪的用户：

- 新世纪方案下的调频、删词、手动加词，本来就按 `WubiProfileKind::dictionary_kind` 记成 `wubixsj`；
- 拼音造词派生五笔词（`wubi_learning::learn`）原来总是同时写 86 和 98。现在接收会话当前的版本：86 和 98 照旧一起学，新世纪只在当前就是新世纪时学（`learned_kinds`）。`session::commit` 把 `wubi_input_options().profile` 传进去。

### 同步暂不打开（第 8 节）

| 通道 | 处理 |
|---|---|
| 账号设置 | `settings_sync::account_wubi_schema` 返回 `Option`，新世纪为 `None`，不导出 `input.wubi_schema`。iOS 的 `local_account_preferences` 同样处理。应用账号设置时，本机选着新世纪就不拿账号里留着的旧版本覆盖它，与 `shuangpin_schema` 对智能 ABC 的处理相同。别的设备写来的 `wubixsj` 照常读 |
| 云词库 | `policy::personal_kind("wubixsj")` 是 `None`。`validate_cloud_request` 的种类表不加它，按种类分的编码规则已经写上它，打开时只改那一处 |
| WebDAV | `dictionary_webdav::word_kind(WubiXsj)` 是 `None`。反方向的 `engine_kind` 写全 |
| 词库快照导出 | `export_local_snapshot` 只在 `include_learning`（本地备份）时写新世纪的词。云同步上传用的是不带这个参数的那份，服务端还不收这个种类 |
| 快照读回 | `record::decode` 认识 `wubixsj` |

### 其余

- 原来写死了「两种五笔」的地方都改成显式的值：
  - host-api `stored_word` 里的 `wubi_profile == 1` 改为 `WordKind::of_wubi_profile`；
  - `personal_kind` 的 `unreachable!` 前补上新种类；
  - backend 的 `wubi_profile()` 不再用 `_ => Wubi86` 兜底，改成显式列出 `wubi`/`pinyin`/`shuangpin`；
  - backend 的 `STORED_KINDS` 是 6 个。
- `third_party_identity` 和 98 一样把新世纪并入 `Wubi`。

## Alternatives considered

- **拼音造词对三版都学。** 这样最简单，换到新世纪时以前造的词也在。不用它，是因为这样每个用拼音造过词的用户，日志里都会有 `wubixsj` 行。前一个 PR 以前的版本读到它会整次回滚，那就成了所有用户都不能降级，而不只是用过新世纪的人。维护者选的是只在当前方案是新世纪时写（第 7 节第 7 条）。代价是：从 86 换到新世纪之后，以前造的词要重新打一遍才有新世纪码。
- **账号设置照常导出 `wubixsj`。** 选择能跨设备同步，用户省事。不用它，是因为已经发布的 iOS 读到不认识的 `input.wubi_schema` 会拒绝整份账号设置，那台设备从此收不到任何设置。等宽松读取的版本覆盖了足够多的用户再打开（第 8 节）。
- **云同步上传的快照照常带新世纪的词，交给服务端拒绝。** 不用改导出。不用它，是因为服务端的 `user_dictionary_entries_kind_check` 会让整份上传失败，所有新世纪用户的云词库同步就都断了。
- **给快照导出加一个显式的「是否本地备份」参数，不借 `include_learning`。** 语义更直。不用它，是因为 `msime_client.h` 已经把 `include_learning` 写成「为真是本地备份、为假是云同步上传」。Android 的 `CloudSync.java` 和 `LocalBackup` 都按这个约定调用。另加参数的话，Java 那边不跟着改，云同步上传就会带上新世纪的词。
- **`merge_downloaded_wubi` 保持「有任何五笔表就不动」，靠换代次补表。** 代码不动。不用它，是因为按需下载的码表更新后，宿主的代次内容标识怎么推导、会不会换代次，没有核实过。逐表判断不依赖这一点，已有代次也能拿到新表。

## Consequences

- **收益**：
  - 引擎选着新世纪时读 `wubixsj` 表，查码、反查、14 键、通配和个人词条都按这一版来。
  - 宿主接口、C 接口、MCP 都能选这个版本、存这个种类的词。
  - 词库还没有新世纪表时，日志里的新世纪行只是跳过，不影响 86/98 用户。
- **代价**：
  - 新世纪的选择、个人词和学习暂时只在本机。快照本地备份带着它们，云同步和 WebDAV 不带。
  - 从 86/98 换到新世纪，以前用拼音造的词没有新世纪码。
  - 用过新世纪的用户降级到宽松读取之前的版本不受支持，发布说明要写明。
- **还没做的**：
  - 平台界面（设置页、菜单、空格键、查码第三列）在各平台的 PR 里。
  - `msime-wubi.db` 的 `wubixsj` 表在 dict-builder 的 PR 和 msime-dictionary 的发布里。
  - 打开同步是之后单独的 PR，要先部署 msime-cloud。
  - `docs/learning-sync.md` 设计里的学习同步还没实现；实现时也要按这一条排除 `wubixsj`。

## Verification

在 Mac Studio 上经 `rbuild` 运行。Studio 的 SSH 会话 `ulimit -n` 只有 256，测试命令前加 `ulimit -n 65536`。

- `cargo test -p msime-engine`：全部通过，含以下用例。
  - `the_new_century_profile_answers_from_its_own_table`：用 fixture 词库建会话，选着新世纪时 `ent` 出「力」、`ler` 出「男」、`ytny` 出「为」、`hgd` 出「上」，86 的码不出；选着 86 时反过来。拼音造「男力」只在选着新世纪时写进 `wubixsj`，码是 `leen`。
  - `the_profile_picks_the_table`、`reverse_indexes_are_created_once_for_the_tables_present`。
  - `a_downloaded_wubi_dictionary_is_merged_into_a_generation_prepared_without_one`：已有 86/98 的代次收到带新世纪表的码表后只补第三张表，日志里的新世纪行这时才写进去，再来一次什么也不做。
  - `split_wubi_tables_are_merged_into_the_generation_before_replay`：码表没有新世纪表时，日志里的新世纪行跳过，代次照常就绪。
  - `wubi_rows_are_skipped_when_the_generation_has_no_wubi_table`：缺 `wubixsj` 表时只跳过新世纪行，86/98 照常写。
  - `the_new_century_table_is_learned_only_while_it_is_the_active_profile`。
  - `wubi98_and_wubixsj_write_to_their_own_table_under_their_own_kind`。
  - `invalid_options_return_errors_instead_of_unwinding_into_rust`：不合法的版本哨兵从 2 改成 3。
- `cargo test -p msime-engine-capi`：全部通过。选项接受三个版本、拒绝 `wubi06`；`wubixsj` 词条的校验和规范化。
- `cargo test -p msime-host-api`：全部通过，含以下用例。
  - `a_new_century_lookup_reads_its_own_table_and_user_words`；
  - `every_engine_dictionary_kind_maps_to_a_host_kind_and_back`（`personal_kind` 不会走到 `unreachable!`）；
  - `wubi_codes` 的三列；
  - `new_century_words_go_into_local_backups_but_not_the_cloud_snapshot`；
  - `new_century_wubi_words_stay_out_of_webdav_sync`；
  - `wubi_profile_reaches_engine_options`（新世纪是 2）。
- `cargo test -p msime-client-core`：全部通过。新世纪偏好的存取与严格提交；`WordKind` 的序列化名和排序。
- `cargo test -p msime-client-services`：`typing_statistics::batch_tests` 的几项在并行时随机失败，`--test-threads=1` 单独运行全部通过。这几项与本改动无关，没有动 `typing_statistics`。其余全部通过，含以下用例：
  - 鸿蒙和 iOS 的 `the_new_century_wubi_profile_stays_on_the_device`；
  - `new_century_wubi_words_are_not_synced_through_the_cloud_yet`。
- `cargo test -p msime-mcp-server`、`-p msime-input-runtime`（五笔用例扩到三个版本）、`-p msime-engine-wasm`（`Scheme::parse("wubixsj")` 仍是 `None`）：全部通过。
- `cargo clippy --workspace --exclude msime-desktop --all-targets -- -D warnings`：无告警。
- `bash scripts/run-checks.sh`：通过。
- 没有用真实的新世纪码表跑过。表要等 dict-builder 的 PR 生成，这里的 `力 ent` 等都是 fixture 里的行。
