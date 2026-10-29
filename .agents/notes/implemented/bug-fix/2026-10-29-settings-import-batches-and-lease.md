# Agent Note: 设置页词库导入按有界事务写入，维护期间租约不断档

Status: implemented

## Problem

#9918：Deepin 25 上用官方 deb 0.14.0 在设置页导入搜狗细胞词库后，输入法打不出字，设置页的导入按钮、个人词库和文件格式下拉菜单、「清除全部学习数据」都点不动。附件诊断日志在约 5 分钟里几乎每次按键都写一条 `operation_failed operation=fcitx_event`。

沿调用链有三处叠在一起：

一、导入逐行开库、逐行提交。设置页把整份文件交给 `settings_dictionary::request`，它把文本切成 60 KiB、至多 1000 行的请求；每个请求在 Host API 的 `Operation::Import` 里先取独占维护锁，再对每一行调一次 `msime_engine::host::dictionary_edit`。这一调用每次都要准备日志库和英文库、打开主库并 attach、刷新日志、开一个 IMMEDIATE 事务再提交，即每行至少两次落盘。[个人词库有界事务](2026-10-18-personal-dictionary-batches.md)早已给 Engine 加了 128 项一个事务的 `dictionary_edit_batch`，Android 队列同步和 WebDAV 同步都在用，唯独这条桌面导入路径没换过来。

二、租约只在请求之前续。`QuiescedHosts::run` 在每个请求发出前把租约续到 30 秒后，请求执行期间不再续。一个 1000 行的请求在慢盘上超过 30 秒，租约就在请求中途过期，而设置页还持着独占锁。

三、Linux 宿主把「词库正在维护」当成配置错误。租约过期后 Fcitx5 的 `ensure()` 不再提前返回，`msime_client_create` 因 `DictionaryAccess::try_session` 拿不到共享锁而答 `dictionary maintenance busy`；`response()` 把它抛成异常，`keyEvent` 的 `catch (...)` 交给 `unavailable()`，写下 `operation_failed operation=fcitx_event`、关闭会话并显示「MSIME：请检查运行配置」。下一个键再来一遍，于是每个键一条日志。IBus 宿主的 `open()` 同样把它抛给 `guarded()`，记成宿主操作失败。

## Decision

`Operation::Import` 按 128 行一块（`IMPORT_EDIT_BATCH`，与 Engine 的 `MAX_EDIT_BATCH` 相同）调 `dictionary_edit_batch`：每块只准备一次连接、提交一次事务，单行被拒由保存点隔离。回执仍是 `<request_id>-<index>`，重复导入同一请求照旧是空操作；被拒行按原来的行号映射记进 `first_failures`；整份都被拒时仍答 `dictionary import rejected`。整块返回 `Err`（触发器 `RAISE(ROLLBACK)` 撤销了外层事务、提交失败）时，这一块退回逐行 `dictionary_edit`，保证一行异常只拒掉它自己，结果与分批之前相同。`import_dictionary_words`（MCP 的 `import_dictionary_words`，一次至多 200 词、逐词先查是否已有）不在这次范围内。

`QuiescedHosts` 在租约已经挂上时，每次 `attempt` 执行期间另起一个线程，每隔 `RENEW_INTERVAL`（10 秒，租约时长的三分之一）调一次 `publish`，`attempt` 一返回（或 panic）就停。这对所有 `QuiescedHosts` 的用户都生效：设置页导入、编辑和清除学习数据、完整云备份的「应用到本机」、WebDAV 同步、`msime-mcp` 的写操作。Windows 的 `WindowsRelease::publish` 同时续租约并再请 Server 放开，所以 Server 30 秒自动恢复也不会落在请求中途。线程起不来时退回只在请求之前续的那一次。租约时长、锁的语义和租约文件格式都不变。

Linux 两个宿主建会话时先拿整份回复，`msime::linux_host::dictionary_maintenance_busy`（`platforms/linux/src/core/DictionaryQuiesceLease.h`）认出 `{"ok":false,"error":"dictionary maintenance busy"}` 时，与看到租约一样不开会话、把按键交给应用，等下一个键或定时器再试；不调 `unavailable()`、不显示「请检查运行配置」。日志改写成 `dictionary_busy operation=session_create`，一段忙碌只记一条，会话建起来后清掉记号。别的创建失败照旧走原来的报错路径。Windows TIP 在[让 TIP 宿主会话放开](2026-10-25-windows-dictionary-maintenance-tip-sessions.md)里已经这样处理同一个原因，这次是 Linux 宿主补上。

## Alternatives considered

- **只加长租约（例如 5 分钟）。** 改一个常量就能盖住慢请求。但租约时长是写进 `platforms/common/DictionaryQuiesceLeaseFormat.h` 的上限，宿主拒绝比它更远的过期时间；设置进程中途退出时输入法要停这么久才恢复。导入慢的根源是逐行落盘，加长租约只是让输入法停得更久。
- **请求执行期间不续租约，只在请求前后各续一次，并约定每个请求必须短于 30 秒。** 不用起线程。但这要求每个调用方都守住时长，完整云备份的激活整代词库这类请求并不由导入的批量大小决定；续期线程不依赖调用方。
- **用集合导入的 4096 项专用入口 `dictionary_edit_collection_batch`。** 事务更少。但那条入口是为集合追加、`preserve_existing` 语义和它自己的预算设计的；128 项的普通入口已把单个 1000 行请求压到亚秒级，没有必要扩大单个事务持锁的时长。
- **Linux 宿主在 `unavailable()` 里按异常文本分辨。** 改动最少。但 `response()` 为了不让错误内容进日志和面板，异常只带固定文本，拿不到 Host API 的原因；在建会话处读整份回复才分得清。

## Consequences

- **收益**：一个 1000 行的导入请求从逐行几千次落盘变成 8 个事务，设置页导入大词库的总时长和每个请求的持锁时长都大幅下降；即使某个请求仍然很长，宿主看到的租约也一直有效，不会在请求中途重新建会话。Linux 上维护期间按键直通应用，不再弹「请检查运行配置」，诊断日志也不再每个键一条。
- **代价与已知上限**：每个持租约的请求多起一个短命线程。一块整体失败时退回逐行，最坏情况与原来一样慢。导入期间设置页仍然一直是忙碌状态，没有进度和取消，这属于后续的体验改进，不在 #9918 里做。macOS 输入法建会话遇到同一原因时怎样处理没有在这次核对。

## Verification

- `cargo test -p msime-host-api --lib settings_import`：`a_settings_import_writes_across_bounded_batches_and_names_a_refused_line` 导入 300 行，第 201 行用触发器 `RAISE(ROLLBACK)` 撤销外层事务，断言只拒掉这一行（applied 299、`first_failures` 行号 201）、同一请求重放不重复写入、去掉触发器后重放补上这一行；若没有逐行退回，第二块的 128 行会因为这一行整块被拒。`a_settings_import_the_engine_refuses_entirely_is_a_failed_import` 断言全部被拒时整次导入失败。
- `synthetic_settings_import_throughput`（`#[ignore]`，`MSIME_BATCH_BENCH_WORDS=20000`，debug 构建，Mac Studio）：逐行写入共 165641 ms、最长的 1000 行请求 11830 ms；分批之后共 7051 ms、最长请求 595 ms。只用合成词和空主库，不能外推为真机或慢盘上的承诺。
- `cargo test -p msime-client-services --lib dictionary::quiesce`：`a_request_that_runs_past_the_renewal_interval_keeps_the_lease_renewed` 把续期间隔调成 20 ms，断言请求执行 200 ms 期间租约过期时间被推后、请求返回后不再改写、放下后租约撤掉。
- `platforms/linux/tests/core/dictionary_quiesce_lease.cpp` 覆盖 `dictionary_maintenance_busy` 只认 `ok` 为假且原因正好是 `dictionary maintenance busy` 的回复；`tests/core/fcitx5_contract.py` 断言两个宿主都在建会话之后、会话 opt-in 之前做这项判断。Fcitx5 和 IBus 宿主本体只在 PR CI 的 Linux 构建里编译；在 Deepin 或其他 Linux 实机上导入大词库并同时打字还没有验收。
