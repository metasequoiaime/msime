# Agent Note: 多分类自动云同步的编排收进 Rust

Status: proposed

## Problem

设置、皮肤库、无编码常用语和个人词库的自动云同步，现在只有 iOS（`App/Sources/account/CloudSync.swift`、`SharedUI/core/CloudSyncState.swift`、`shared/backend/storage/BackendSyncPolicy.swift`）和 Android（`home/CloudSync.java`、`account/SyncSwitch.java`、`account/SyncMergePolicy.java`）两份实现，规则大体相同，细节各不相同：绑定键（iOS 把同账号重新登录也当成换绑）、退出时机、同步进行中用户的选择（Android 丢掉）、请求体预检（iOS 1 MiB、Android 不查，服务端常用语只收 256 KiB）、只有空白的正文（两端都丢）。两端还有共同的数据安全问题：设置合并是本机整份覆盖，另一台设备改的键会被本机旧值盖掉；常用语合并是并集，一侧删掉的条目在另一侧合并时复活；四端都不看 `Retry-After`。#8201 要把这套编排收进 Rust，平台只留界面和平台专属的存储。

## Proposal

### 分层

- client-services 的 `account::cloud_sync`，不碰网络：
  - `policy`：纯函数，节流、模式表、隐私键、按字段表过滤、三方合并、合并后修复常用语、皮肤预算、失败文案和状态行。文案与两端逐字相同。
  - `state`：状态文件 `<state_directory>/cloud-sync/state.json`（版本 2，256 KiB，同目录 `state.lock` 用 flock 保护）和待提交进度 `pending.json`（4 MiB，iOS 的设置文档可能带 800 KB 的皮肤），以及 iOS 第 1 版状态和 Android 两份偏好的导入。
  - `machine`：sans-IO 的单轮状态机。`gate` 判断这一轮跑不跑；`SectionMachine` 给出 `Fetch`、`ReadLocal`、`NeedsChoice`、`Put`、`WriteLocal`、`Commit`、`Done`，驱动把 `Fetched`、`LocalRead`、`Written`、`HttpFailed`、`LocalFailed` 喂回去。
- host-api 的 `cloud_sync` 是驱动：联网用宿主那一份会话（`BackendAccountSession::authenticated_as` 把每个请求绑在开始那一轮的账号和代次上），本机写入用 `PreferencesStore`、`CommonPhrasesStore`、词库导出、快照激活队列和个人词库导入队列，都在 `with_generation` 的闭包里做、闭包里不联网。词库要用 Engine，所以驱动在 host-api。C 入口是 `msime_client_cloud_sync`。
- `AccountApi` 加了几个带状态码、服务端错误码和 `Retry-After` 的方法（偏好、字段表、词库探测、快照上传下载），默认实现转调原来的方法，已有的实现者不受影响。

### 规则

- 绑定按账号 id：每次调用读会话里的用户 id，对不上就清空进度、关开关；同一账号重新登录保留进度。会话读不出来时不对账。
- 一轮开始时在状态锁里写下租约 `{id, user_id, state_version, started_ms}`，同一时刻只跑一轮（租约 180 秒过期）。提交时比较并交换：租约 id、账号、`state_version` 都对得上才推进；退出登录、开关、另一轮都会推进 `state_version`，旧的一轮报 `stale_run`。打标记和记下用户的选择不推进它。
- 游标、清标记和基线只在提交时推进；清标记只清本轮开始时读到的代数。要宿主写的值（iOS 的键盘设置、Android 的按键反馈和本地设置、皮肤库、要认领的示例常用语）写完以后宿主调 `commit`；没有这类值时自动提交。
- 设置按键三方合并；常用语按正文三方合并，两边都有的沿用云端 id，合并后 id 冲突的换新 id，超过 500 条或 256 KiB 时这一类失败、两边都不写；基线和本机收不下的正文只存 SHA-256。
- 云端被清空（本机有基线、修订号变了、内容为空；词库是第一条新改动为 `reset` 且汇总里词库为空）时不合并也不下载，先问用户；「使用云端」遇到从没上传过或为空的云端、而本机有数据时按合并处理。
- 尊重 `Retry-After`，记成 `not_before_ms`，手动同步也挡。
- 上传前按本轮读到的字段表过滤，字段表外的键既不上传也不写回本机。

### 与规格的偏差

- 状态机多一步 `ReadLocal`、多一个事件 `LocalFailed`：本机读要在云端读之后，读失败要能结束这一类。
- 按内容判断本机改动（规格的 D7）只用于常用语。设置的映射不是逐键可逆（本机收不下的值、皮肤库裁剪与合并），词库的本机版本会随输入学习变化，用内容判断会让两台设备每一轮都整份上传。设置和词库仍只看显式标记。
- `cloud_cleared` 对修订号后退同样成立：后退说明云端状态异常，下载会把本机清空。
- `mark_dirty` 和 `import_legacy` 可以不带会话来源，`preferences_directory` 只有 `run` 和 `commit` 要带；词库另要 `snapshot_queue_directory`，规格没有列它。

## Alternatives considered

- **合并规则照搬两端（设置本机全覆盖、常用语并集）** — 最强的理由：迁移后用户看不到任何行为变化，规格里的差异清单短得多。否掉：两条规则都会丢数据，另一台设备改的设置会被本机旧值盖掉，删掉的常用语会复活；仓库负责人已定以 Rust 为准，迁移是改掉它们的唯一机会。
- **设置和词库也按内容判断本机改动** — 最强的理由：漏打一处标记就会丢上传或被覆盖，按内容判断兜得住。否掉：设置的导出和应用不能逐键往返，词库版本含学习调权，按内容判断会在两台设备间来回整份上传；常用语的正文原样存取，所以只在那里用。
- **完全 sans-IO：Rust 只给出请求，所有 HTTP 由宿主发** — 最强的理由：没有联网线程和会话锁的顾虑，与 `msime_client_account_gloss` 一致。否掉：一轮里有字段表、偏好、常用语、词库探测和快照上传下载，每一步都要带 401 重试和账号核对，推回平台就是要删掉的那几份重复实现；状态机本身仍是 sans-IO，驱动只是它的一个执行者。
- **绑定沿用 iOS 的（账号 id，`sessionID`）** — 最强的理由：同账号重新登录也是一次新的「授权」，清空进度更保守。否掉：游标反映的是服务端账号的状态，同一账号的云端数据没变，重新弹首次选择只是打扰；开关打开期间的标记一直在记，不会漏上传。
- **`unheld` 继续存原文** — 最强的理由：状态行能直接给出是哪几条。否掉：500 条乘 2000 个单元会写不下 256 KiB 的状态文件；比对只需要摘要。
- **字段表外的键原样带回（两端现状）** — 最强的理由：不会删掉别的平台写下、服务端暂时不认识的键。否掉：服务端只要有一个键不在表里就拒绝整份文档，带回去的结果是所有设备的设置同步一直失败；被过滤的键在 PR 正文列出，交给服务端补字段表。

## Acceptance criteria

- `cargo test -p msime-client-services cloud_sync`：模式表、节流、提示、三方合并、合并后修复、皮肤预算与合并、失败文案、状态行；状态文件的读写往返、损坏和超长、两个描述符互斥、租约与提交、旧状态导入；状态机按分类、游标、标记、基线、清空标记、选择、云端和本机情况穷举组合的不变式，以及冲突重试、云端被清空、`use_cloud` 遇 revision 0 和词库的几条规则；loopback 假服务器回放带 `Retry-After` 和 `revision_conflict` 的响应。
- `cargo test -p msime-host-api cloud_sync`：用假后端和内存里的本机覆盖首次同步、冲突、删除、退出登录、退避、并发与过期提交、字段表外的键、合并后超限与 id 冲突、云端被清空、跑到一半换账号、皮肤库裁剪和词库的上传、推迟、下载、合并；默认的本机实现在临时目录里覆盖 Android 与 iOS 的设置映射、常用语对齐和快照解析；C 层覆盖请求校验和不联网的操作。
- 第二阶段：iOS 与 Android 改调 `msime_client_cloud_sync` 后，`CloudSync`、`CloudSyncState`、`BackendSyncPolicy`、`SyncSwitch`、`SyncMergePolicy` 和 `home/CloudSync` 删除，首次启动用 `import_legacy` 带上旧状态。

## Risks

- 租约 180 秒，词库快照上传最长 10 分钟：上传慢时租约会过期，另一个进程可以开始新的一轮。服务端的修订号比较能挡住重复写入，状态的比较并交换能挡住旧的提交，但会多一次上传。
- 「云端被清空」后两个选择都会把本机重新上传，删除云端数据的设备如果还开着同步，删掉的数据会回来；用户要在那台设备上先关掉同步。这是规格定下的保护，PR 正文写明。
- 宿主不提交 `pending_commit` 时，同一份云端内容下一轮会再应用一次；应用是幂等的，但 iOS 的键盘设置会被再写一次。
- 各平台还没有改调这个接口，真机上的行为（键盘扩展打标记、`:ime` 进程、示例常用语认领）要到迁移时验证。

## Related notes

- [iOS 原生应用的自动云同步](../../implemented/feature/2026-10-12-ios-automatic-cloud-sync.md)：部分重叠。那篇定下了 iOS 的状态文件、节流、首次选择和不同步词库；这里沿用状态文件放在共享容器、节流和首次选择，把合并规则、绑定键和 `unheld` 的存法改掉，迁移以后那篇描述的 Swift 实现删除。
- [账号会话由宿主注入存储](2026-10-16-host-injected-session-storage.md)：部分重叠。这里的会话来源和 `session_store` 字段沿用那篇，同属 #8201 第一阶段。
- [iOS 云同步状态复用有界读取器](../../implemented/simplification/2026-10-13-ios-cloud-sync-state-reader.md)：无关；那是 Swift 读状态文件的实现细节，Rust 的状态文件有自己的有界读取。
