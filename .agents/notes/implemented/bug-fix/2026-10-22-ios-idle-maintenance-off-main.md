# Agent Note: iOS 词库维护定时器的文件读取移出主线程

Status: implemented

## Problem

iPhone XR 真机的 os_signpost 采样里，键盘扩展的两秒维护定时器在主线程上每轮耗时 7–22 ms（8 个样本），下一个词的首键经常排在它后面。这一轮在没有组字、最后一次按下已过去 300 ms 时运行，做两件事：`synchronizePersonalDictionary(force: false)` 每轮新建 `PersonalDictionaryStore` 并 `read()`，即使没有任何待同步内容也要做符号链接检查、读文件和 JSON 解码；`DictionarySnapshotWorker.tick` 每轮 `queue.read()` 读词库快照队列的状态文件。绝大多数轮次两边都无事可做，主线程的这段时间全花在读文件上。

## Decision

定时器每一轮在主线程只做空闲判断和读一次完全访问权限，然后由 `IdleMaintenancePoll` 在一条串行的后台队列上读两份共享文件，结果回到主线程后重新核对空闲条件，只在仍然空闲时才做真正要做的事：

- 个人词库：后台调用 `PersonalDictionaryStore.pollKeyboardSynchronization(idle:)`，只判断有没有要同步的内容（`needsKeyboardSynchronization`：还有待应用的编辑，或刷新代次没被确认）。需要同步时主线程照旧调用 `synchronizePersonalDictionary(force: false)`，它在 `sync.lock` 下重读并同步；读不出来时主线程显示同一条诊断。
- 词库快照：后台调用 `DictionarySnapshotWorker.stateReader()` 返回的函数读队列状态，交给 `tick(prefetched:)`。工作者有一个只在主线程推进的状态代次：每次 tick 读状态之前、每个准备任务结束时加一。预读带着读取前的代次，交回时代次变了就作废、当场重读，所以本进程自己写过的状态不会被更早的预读盖住。别的进程随时会改这个文件，预读和当场读一样只是某一时刻的快照，凡是据此改动队列的操作（`cancel`、`claim`、`complete`、`fail`）都会在 `state.lock` 下重新核对。
- 上一轮没回来时定时器不开新的一轮；串行队列保证读取不会重叠。键盘收起时 `IdleMaintenancePoll.cancel()`，之后才回来的结果不交付，不会在收起后启动新的准备任务或同步。

后台只读文件、不取任何文件锁，所以键盘被挂起时即使正读到一半，也不会因为持有 App Group 里的锁被系统以 `0xdead10cc` 终止（见[收起时释放租约](2026-10-14-ios-snapshot-worker-lock-release.md)）。

### 未变化文件不再读取

`pollKeyboardSynchronization(idle:)` 记住上一次判定无事可做时 `sync.json` 的文件身份（`lstat` 得到的设备号、inode、大小和纳秒修改时间），文件还是那一个就不再读取和解码。这个判断成立的前提是写 `sync.json` 的只有两处，都是写临时文件再改名替换：Swift 的 `PersonalDictionaryStore.update` 用 `.atomic` 写入，Rust 的 `PersonalDictionaryStore::write_locked` 经 `write_private_file_at` 用 `renameat`。内容每变一次路径上就是一个新的 inode，APFS 在同一个卷上不重用 inode 号。只有判定无事可做、并且读取前后两次 `lstat` 一致时才记下身份；文件不存在、不是普通文件或读取出错时不记，下一轮照常读取，符号链接检查和报错仍由 `read()` 负责。以后若有第三处写入者原地改写这个文件，必须同时更新文件身份或改用替换写入。

### 引擎调用仍在主线程

真正的同步不搬到后台。`PersonalDictionaryStore.synchronize` 的每个回调都经 `MetasequoiaInputSessionBridge.withDictionaryMaintenance`：先销毁会话、执行词库操作、再新建并聚焦会话，期间改写 `handle`。这个属性和 `options` 只在主线程读写，主线程的每个按键在投递到输入线程之前就读好了 `handle`；从后台调用会和按键并发读写它，还可能让按键带着已经销毁的句柄排到重建之后。同步期间还一直持有 `sync.lock`，放到后台就可能在键盘被挂起时仍持着它。快照激活（`activateDictionarySnapshot`）同理。所以需要同步的那一轮主线程仍要等它完成，和改动前一样；这种轮次只在宿主入队编辑、翻页、导出或云端快照待激活时才出现。

## Alternatives considered

- **整段维护放到后台，引擎调用经桥接排进输入线程**：主线程不再等任何维护，按键只会排在输入线程上的单次引擎调用后面。但 `withDictionaryMaintenance` 的销毁、操作、重建是几次独立的输入线程调用，中间可以插入按键；`handle` 也不是线程安全的。要做到必须把会话句柄的读写整体移进输入线程，改动面是桥接里的全部会话调用，而真正需要引擎的轮次很少，不值得为它承担这个风险。
- **只把读取挪到后台，不缓存文件身份**：更简单，主线程同样不再读文件。但后台每两秒仍完整读一次 JSON，键盘开着时一直耗电；身份比较只多一次 `lstat`，写入方式也能证明它安全，所以一起做了。
- **把快照工作者的 10 秒版本发布（`publishLocalVersion`）也挪到后台**：它在准备任务回到主线程后执行，要取 `state.lock` 并写文件。挪到后台会在键盘收起后仍可能持锁写入，违反「取消后不发布迟到结果」，所以保持不变；它不在定时器回调里，也不是这次测到的那段停顿。

## Consequences

- **收益**：没有待同步内容的轮次里，主线程只剩空闲判断和一次完全访问权限读取，不再读写任何共享文件；个人词库文件没变时后台也只做一次 `lstat`。
- **代价与已知上限**：多一条后台串行队列和一次主线程跳转；一轮的结果最多晚几毫秒才被处理，期间用户开始输入就整轮让出，下一次定时器再来。需要真正同步或激活快照的轮次仍在主线程执行，每次都要销毁并重建引擎会话，那一轮的停顿和改动前相同。缓存文件身份依赖上面写明的写入方式。

## Related decisions

[词库版本轮询离开主线程](2026-10-19-ios-snapshot-version-background-poll.md)部分重叠：它把版本读取移到后台并规定定时器的空闲条件和每轮只读一次完全访问权限，本篇沿用这些条件，把同一轮里剩下的两次文件读取也移出主线程。[会话拥有线程](2026-10-19-ios-nine-key-owned-thread.md)部分重叠：本篇不让后台直接调用有会话句柄的接口，理由见上。[收起时释放租约](2026-10-14-ios-snapshot-worker-lock-release.md)的立即释放契约不变。没有完全吸收或需要否决的旧笔记。

## Verification

iPhone 17 模拟器（iOS 27.0）上 `MSIMEKeyboardTests` 全套 876 项通过、8 项跳过；`MSIMESharedTests` 里的个人词库与快照工作者两组 12 项通过。新增的回归：

- `IdleMaintenancePollTests`：后台读取被阻塞时主线程照常运行、读取在主线程之外、上一轮没回来不开新一轮、取消后迟到的结果不交付、之后的新一轮照常交付；个人词库文件没被替换时，拿走读权限也照样判定无事可做，宿主入队编辑后下一轮读到它。
- `PersonalDictionaryStoreTests.testIdlePollSkipsAnUnchangedFileAndRereadsAReplacedOne`：文件不存在时要求同步且不记身份；同一文件不再读取；宿主翻页请求替换文件后身份失效。
- `DictionarySnapshotWorkerTests` 的两项预读回归：预读之后把状态文件写坏，tick 用预读结果、不报错；同一份预读第二次交回时当场重读；预读早于准备任务发布版本时，tick 重读而不是再开一次版本轮询。把代次比较改成恒真后这两项都失败。

真机没有重新采样：本篇没有 XR 上改动前后的主线程耗时对照，不能据此说明那段 7–22 ms 的停顿减少了多少，只能说明那一轮在主线程上不再做文件读取。
