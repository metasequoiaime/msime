# Agent Note: 核心会话收尾、候选修订号与日志连接的线程归属

Status: implemented

## Problem

引擎要做成给其他输入法嵌入的库（PR #10167 的提案，P3「ABI 稳定之前引擎侧要修的硬点」）。其中四处与会话生命周期和线程有关，不修就定不下 C ABI 的会话与线程契约：

- 收尾在产品门面里。冲刷本日志的整句联想队列、放掉本线程的日志连接、`note_settled`、本地模式的词库租约都由 `host::Session` 持有和执行，直接用核心 `msime_engine::Session` 的采用方结束会话时什么也不做：最近约 2 秒学到的上下文要等后台线程，本线程的日志连接一直开着。
- 引擎没有候选修订号。host-api 的 `generation` 来自 input-runtime，按下标选词的 ABI（`select(revision, index)`）无从核对下标是否还指同一行。
- 日志连接按线程缓存在 `thread_local` 里，`close_cached_journals` 和 `release_thread_journal` 只能关调用线程的那一份，别的线程要等它们下次访问日志才关。Windows 上开着的句柄挡住重置学习数据的改名和代次删除；会话在一个线程上建、在另一个线程上用或销毁时，建它的线程会一直留着连接。
- 整句联想的冲刷线程 `msime-personal-context` 创建失败就 panic，从不 join，进程没有办法在卸载前冲刷并停掉它。

## Decision

### 收尾归核心会话

核心 `Session` 实现 `Drop`，依次：冲刷本日志的整句联想队列（`flush_journal`）、放掉本线程的日志连接（`release_thread_journal`）、交还本会话对日志的占用（`JournalUse::end`）、`note_settled`。本地模式的词库租约 `LocalDatabaseLease`（代次目录与资源目录）也移进核心会话，作为最后一个字段在其余部分之后放掉。`host::Session` 不再实现 `Drop`，也不再持有租约，只是委托。顺序沿用原来门面里的顺序，并且关连接都在 `note_settled` 之前：关掉 WAL 日志的最后一条连接会做检查点，改变 `note_settled` 记下的文件戳。

不提供 `close(self)`：收尾里唯一可能失败的冲刷失败时队列留在 store 里并打上标记，下一次 `record` 报告并重试，没有需要交还给调用方的东西。

### 候选修订号

核心会话维护 `revision: u64`，经只读的 `candidate_revision()` 暴露，从 0 开始只增不减。凡是可能改变候选列表的 `&mut self` 调用都先递增（按键、命令、选择与逐候选学习、换方案、`set_dedicated_english`、辅助码与指令表等会刷新列表的 setter、`set_caret`、`set_first_page_rows`、`reset_context`、`reset_cache`、开关联想或云候选）；返回布尔值的 `apply_online_candidates`、`apply_online_candidate`、`apply_command_translation`、`apply_plugin_command_rows`、`expand_initial_candidates` 只在确实写入或扩充时递增。只影响之后按键的 setter（时钟、待用触点、键位几何、触点计价参数、标点开关与锁、标点样式、重排上下文）和所有只读调用不递增。多递增只会让宿主多读一次，少递增会让按下标的选词指错行，所以拿不准的调用归为递增。

### 日志连接：停放列表

每个线程的日志状态 `ThreadJournal` 留在 `thread_local` 里，连接却只在本线程有操作用着它时才留在线程里（`active: Rc<OnceCell<Connection>>`，按线程只分配一次）。最后一个 `JournalConnection` 放手时，连接连同路径和代次移进进程级的停放列表 `PARKED_JOURNALS`，按线程编号认领；下一次打开时再取回。`Connection` 是 `Send`，所以：

- `close_cached_journals`（`msime_engine::close_cached_databases`、重置、快照恢复、隔离损坏日志都经它）先递增代次再清空停放列表，任何线程空闲的连接当场关闭。停放时在列表的锁里读代次，所以和关闭并发的停放要么看到新代次自己关掉，要么被这次关闭取走。正被某个操作用着的连接在那次操作结束时发现代次已过期，直接关闭而不停放。
- 每个核心会话建立时以 `JournalUse` 记一次对本日志的占用（按路径计数）。本日志最后一个会话结束时关掉所有线程停放的、指向这个日志的连接，所以会话在哪个线程建、在哪个线程用和销毁，都不会在任何线程留下连接。还有别的会话在用这个日志时不关，免得它们下一次按键重新打开日志并跑一遍建表。
- 同一线程上一个操作还拿着连接时，同一路径的嵌套打开共享这条连接，即使中间来过一次关闭（它在操作结束时关）。另开第二条连接会等在第一条的事务上。只有正用着的是另一个日志时才另开一条不缓存的连接。
- 线程退出时 `ThreadJournal` 的析构把自己停放的连接取出关掉。
- 停放和取回在稳态下不分配：`Rc` 分配一次，路径在线程与列表之间移动。列表的容量在 `JournalUse::new`（建会话，冷路径）里预留 `PARKING_HEADROOM` 个空位，按键路径上的停放不会让列表扩容。

核心 `Session` 是 `Send`（`session/mod.rs` 里有编译期断言），不是 `Sync`：一个会话可以在两次调用之间换线程，同一会话的调用仍由持有者串行。`host::Session` 仍以 `PhantomData<Rc<()>>` 绑定在创建它的线程上，host-api 的线程模型不变。

### 冲刷线程

冲刷线程的状态（到期表、线程句柄、停止标记）放在一个 `Mutex` 里，不再 `Box::leak`。`schedule_flush` 在没有线程或线程已经结束时创建线程，创建失败返回 `Io` 错误，`record` 把它交给调用方（会话报 `PERSONAL_CONTEXT_NOT_PERSISTED`），转移留在 store 的队列里，由下一次冲刷或之后某次 `record` 起的线程写入。新增进程级的 `msime_engine::shutdown()`：置停止标记、唤醒并 join 线程（线程退出前把排着的全部写掉，不论是否到期），清掉停止期间新排的到期项，再 `flush_all`。之后会话照常可用，下一次 `record` 重新起线程。停止标记置着时 `schedule_flush` 只登记到期项、不起新线程，所以 `shutdown` 返回时没有它没 join 过的写线程还在跑，停止期间记下的转移由最后的 `flush_all` 写下。整个 `shutdown` 拿着一把自己的锁：宿主的几个组件可能各自在退出钩子里调它，没有这把锁时第二个调用者找不到线程可 join，会在被停的线程看到停止标记之前把标记清掉，那个线程回去等待，第一个调用者就永远 join 不完。

## Alternatives considered

- **ABI 写明「会话固定在创建它的线程」，不动日志缓存** — 改动最小，host-api 今天就是这么做的。否掉是因为这把线程绑定推给每个采用方（工作线程池、异步运行时都要自己绑），而且重置时别的线程留着的连接这个问题依旧存在：一个线程用过日志、之后没有会话了，它的连接照样开着。
- **每个线程一个 `Arc` 槽位，全局表里存 `Weak`** — 线程和关闭方各自锁自己的槽位，没有全局锁。实测线程第一次打开日志要多两次分配（槽位的 `Arc`、全局表的增长），会话测试里选词路径的分配预算因此超出；停放列表的锁只在停放、取回时拿一下，打开日志的线程又不多，争用可以忽略。
- **同一路径的嵌套打开在代次过期后另开连接** — 与「每次打开都看代次」的旧语义一致。并行测试里别的测试不停地关缓存，嵌套打开就一次次新开连接再关掉；外层连接拿着锁时这些描述符被 SQLite 挂起延迟关闭，进程很快用尽描述符。共享正在用的连接既没有这个问题，也不会自己等自己的事务。
- **会话结束时关掉所有线程停放的连接，不按日志计数** — 实现更简单。否掉是因为同一日志上别的会话（iOS 主线程的词库列表与输入线程、Windows 的多个输入线程）下一次按键要重新打开日志并跑建表。
- **把冲刷线程换成调用方线程上的同步写** — 没有后台线程就没有 join 和创建失败的问题。否掉是因为每次选词都同步写整句联想会把写盘放回输入线程，正是当初引入延迟写要避免的。

## Consequences

- **收益**：直接用核心 `Session` 的采用方结束会话时学习不丢、连接不留；会话可以跨线程移动，重置前的 `close_cached_databases` 真正关掉所有线程的空闲连接；ABI 的 `STALE` 有了引擎内的依据；冲刷线程可以在卸载前停掉，创建失败不再让进程 panic。产品行为不变：host-api 与 input-runtime 经 `host::Session`，收尾的步骤和顺序与原来门面里相同；engine-wasm 直接用核心 `Session`，关着整句联想，结束会话时多出的只是放掉本线程的日志连接和本地模式词库的租约，下一个会话按需重新打开。
- **代价与已知上限**：停放列表是一把进程级锁，每次一个线程开始或结束使用日志各拿一次；线程很多且频繁访问日志时需要重访。正被操作用着的连接仍要等那次操作结束才关，`close_cached_databases` 不等它。修订号偏保守，某些 setter 即使没改列表也会递增。`lattice/decode.rs`、`lattice/ngram.rs`、`ime/personal_rerank.rs` 的线程局部缓存和 `host/glosses.rs` 的线程局部连接不在本次范围内，跨线程使用后它们仍各自留在原线程；单位换算的后台线程 `msime-unit-context` 也没有 `shutdown`。

## Verification

- `crates/engine/tests/session_threads.rs`：会话在一个线程上建并使用、交给另一个线程使用并销毁，建它的线程仍活着时 `reset_learned_data` 成功且学习数据被清空。这一条测的是 `close_cached_journals` 关得到别的线程的连接（重置自己会调它），不是会话收尾；Unix 允许改名开着的文件，只有 Windows 上才会失败，所以 CI 的 Windows 设置作业单独跑这个测试二进制。会话收尾不留连接由下面 `lifecycle.rs` 的停放计数证明。`shutdown` 立即写下排着的转移；之后的一次选词不靠任何冲刷、自己落盘，证明写线程重新起来了；再 `shutdown` 仍写下。几个线程同时 `shutdown`、同时还在记转移时，每次调用都返回，记下的都落盘（这条竞争靠时序，测试只是压力冒烟，不能稳定复现原来的挂起）。选词带回的诊断必须为空，起线程失败会在这里露出来。
- `session/tests/lifecycle.rs`：修订号随改变候选的调用递增、读和只影响之后按键的 setter 不变；核心会话销毁时写下队列并放掉本线程的连接；跨线程使用后没有任何线程停放指向该日志的连接。别的测试随时会关掉所有线程停放的连接，所以「销毁前本线程还拿着连接」这类正向断言在关闭计数（`cache_generation`）变过时放行，销毁后的反向断言照常严格。
- `user_dictionary/journal.rs` 的测试：同一线程复用同一条连接直到关闭；`close_cached_journals` 关掉别的线程空闲的连接；关闭时正被使用的连接在操作结束后关闭而不停放；两个会话共用日志时结束一个不关、结束最后一个才关。
- `cargo test -p msime-engine`（含 golden）、`-p msime-input-runtime`、`-p msime-host-api --lib`、`-p msime-engine-wasm`，`cargo check -p msime-engine-wasm --target wasm32-unknown-unknown`。在 Studio 上经 `rbuild` 跑时 SSH 会话的描述符上限是 256，会话测试并行时 develop 上同样会因 `Too many open files` 失败，要先 `ulimit -n 4096`。
