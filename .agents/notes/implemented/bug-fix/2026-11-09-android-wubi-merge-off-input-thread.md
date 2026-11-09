# Agent Note: 五笔码表并表移出安卓输入线程

Status: implemented

## Problem

Android 0.7.1 升到 0.7.2 后，选了五笔的用户点任何一个字母键盘就卡住，随后输入法进程重启，再点又卡，循环不止。

API 35 模拟器上复现，ANR 跟踪按 0.7.2 发布符号解出的主线程栈是：`MSIMEInputService.startEngineSession` → `NativeClient.create` → `msime_client_create` → `Session::new` → `session_options` → `merge_downloaded_wubi` → `merge_split_wubi_marked`。工作主词库缺五笔表时，建会话要把下载的 `msime-wubi.db`（`wubi86`/`wubi98`/`wubixsj`，约 142 MB）在一个事务里逐行复制进代次的工作主词库，再建索引、`ANALYZE`、回放日志、补反查索引。模拟器上这一段在主线程上用了 18–25 秒 CPU，超过输入事件分发的时限。系统杀掉进程，事务没提交就回滚，下一次按键建会话又从头并起，于是没完没了。

引擎在 0.7.1 和 0.7.2 之间没改。升级换了词库代次，新代次里没有五笔表，第一次建会话就要并表；第一次下载五笔资源包之后也一样。慢一点的手机上必定超时。

会话中途也有同一条路：聚焦时 `refresh_resource_packs` 发现五笔码表的路径变了（资源包刚装好），置 `resources_pending`，随后在输入线程上的 `apply_pending` 里重建 Engine，同样在主线程上并表。0.7.1 上资源包生效那一刻见过这条路的 ANR。

## Decision

建会话之前把并表做完，会话中途不在输入线程上并表。

- **引擎**：`msime_engine::host::merge_downloaded_wubi_dictionary(options)` 做的就是 `session_options` 对下载码表做的那一步（`merge_downloaded_wubi(&runtime_paths(options), wubi)`），宿主可以在输入线程之外先调。`session_options` 照旧调它，作为没人事先调过时的兜底；表都在时只有几次 schema 读取。`downloaded_wubi_merge_pending(options, wubi)` 只读两个文件的 schema，回答拿这个路径建会话时还有没有并表或回放要做。
- **host-api**：`msime_client_prepare_session(options, length)` 与 `msime_client_create` 用同一份 HostOptions、同样的解析和版本检查、同样的只读状态处理，持着同样的词库会话租约和代次租约调上面的引擎入口，回答 `{wubi_merged, busy}`。词库正在维护时不等不并，回答 `busy: true`，之后的建会话照旧处理（它自己也会因为维护忙而失败、由宿主重试）。两把租约的取法从 `create_session` 里抽成 `session_dictionary_access` 和 `session_generation_lease`，两个入口共用。
- **Android**：`MSIMEInputService.scheduleEngineStartup` 在工作线程上做完个人词库同步之后、`main.post` 建会话之前调 `NativeClient.prepareSession`，前后照常核对 `engineStartGeneration`，失败吞掉。这条路只在本进程没有会话（`session == 0`）时走，所以并表时没有本进程的读者。符号进了 `verify-native.sh` 的两份导出清单，`scripts/test-android-jni-names.py` 核对三处名字一致。
- **会话中途**（所有平台）：`refresh_resource_packs` 发现五笔码表换了路径、而 `downloaded_wubi_merge_pending` 说还要并表或回放时，这个会话不接新路径，五笔照旧按没有这份码表处理（`wubi_missing` 的回退），也不写主词库。别处并好之后（安卓是下一次建会话前的 `prepare_session`，或别的进程），下一次聚焦照常接上，这时 `apply_pending` 里的重建只剩几次 schema 读取。码表被移除（路径变空）不需要并表，照旧当场跟上。判断读不了主词库时按还要并表算。

## Alternatives considered

- **会话中途在后台线程里并表，并完再换路径、置 `resources_pending`**（任务最初的设计）：最强的理由是会话不用等下一次建会话，五笔装好后很快就能用，而且照搬已有的落定重排模型、逐键重排器的后台槽位模式。没有采用：并表写的是会话正在读的工作主词库。这个库是回滚日志模式（发布的文件是 `journal_mode=DELETE`），写者的页缓存溢出后要拿到排他锁并一直占到提交，142 MB 的写入很快就会溢出；在此期间会话的每条查询都要等满 `BUSY_TIMEOUT`（250 ms）再失败，全拼一个键几百条查询，输入线程照样被卡死，候选也会是空的。本机用两个连接复现过：一个连接在大事务里写入、另一个连接带 250 ms 忙等每 20 ms 读一次，读有一半等满时限后失败。要让后台并表不挡读者，就得把并表拆成小事务、先写到暂存表名再改名、索引先建好逐行维护，是对代次准备共用代码的大改，单独评估。
- **在合并副本里并好再改名换掉工作主词库**：读者完全不受影响。没有采用：会话和别的进程（设置应用的候选预览会话）开着旧文件，改名后它们读写的是已经脱离路径的旧 inode，回滚日志却按路径找，崩溃恢复时可能把旧文件的日志套到新文件上；Windows 也不能改名覆盖打开着的文件。快照之后学到的东西还要再回放一次。
- **建会话时（不只是 Android）一律不并表、交给宿主事先准备**：Android 之外的宿主没有准备这一步，五笔会一直用不上。保留 `session_options` 里的兜底，只给 Android 加准备。
- **Android 准备失败后让建会话也跳过并表**（例如 HostOptions 加一个开关）：能保证主线程上永远不并表。没有采用：准备只在设置进程恰好占着写锁之类的少见情况下失败，加开关要扩 HostOptions 契约；先记为剩余风险。

## Consequences

- Android 第一次建会话（升级换代次后、或五笔资源包刚下载好）在工作线程上并表，这段时间键盘按已有的「会话在建」处理，不再触发 ANR，也不会因为进程被杀而无限重来：事务在工作线程上跑完提交，下一次建会话看到表都在。
- 会话开着时下载好的五笔资源包，在这个会话里不生效，五笔继续回退；Android 换输入框或切应用（`onFinishInput` 销毁会话）后的下一次建会话在工作线程上并好就能用。以前是下一次聚焦就接上，但代价是主线程上的几十秒。
- macOS 也按需下载五笔，但它的建会话没有准备这一步：会话中途装好的码表同样不在当前会话里生效，等下一次建会话时在 macOS 自己建会话的线程上照旧同步并表。HarmonyOS、iOS、Linux、Windows 的建会话路径也照旧同步并表（它们的发布包大多随包带码表，平时不触发）。这些平台要跟进时，照 Android 在建会话之前的工作线程上调 `msime_client_prepare_session` 即可。
- 剩余风险：Android 的准备失败（例如别的进程恰好占着工作主词库的读锁，并表事务 `busy_timeout` 为 0 立即失败）时，随后的建会话仍会在主线程上兜底并表。
- 聚焦时多了一次判断：只在五笔码表的路径与会话里的不同时，读一次主词库和码表的 schema。

## Testing

- `cargo test -p msime-engine --lib`：新增 `merging_ahead_of_the_session_leaves_nothing_for_session_options`，先调引擎入口并表、补写日志里的五笔行，之后 `session_options` 不再写主词库（比较文件头的修改计数）。
- `cargo test -p msime-host-api --lib`：新增 `prepare_session_merges_the_downloaded_wubi_table_before_the_session_is_created`（维护租约被占时回答 busy、不写；并表；再调一次什么也不做；坏文档和版本照样拒绝；之后建的会话直接用上五笔）和 `a_wubi_pack_installed_mid_session_is_taken_only_once_merged_elsewhere`（会话中途装好的码表聚焦时不接、主词库不被写；别处并好后下一次聚焦接上；移除时当场跟上）。
- `cargo clippy -p msime-engine -p msime-host-api --all-targets -- -D warnings`、`platforms/android/check-host.sh`、`scripts/test-android-jni-names.py`。

## Related notes audit

- [Android 五笔码表按需下载](../feature/2026-10-28-android-on-demand-wubi.md)：「码表到了、下一次建会话时并入」的事实就地改为建会话之前在工作线程上并入，并指向本篇。部分重叠，互链。
- [macOS 语音运行库与五笔码表按需下载](../feature/2026-10-20-macos-on-demand-voice-runtime-and-wubi.md)：`session_options` 建会话时并表的规则不变，本篇只是让宿主能提前做；会话中途不再接未并的码表，在那篇的「码表到了再并入」一条后补一句并指向本篇。部分重叠，互链。
