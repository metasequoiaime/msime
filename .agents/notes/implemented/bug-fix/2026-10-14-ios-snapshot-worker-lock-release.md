# Agent Note: iOS 键盘收起时立即释放词库快照租约

Status: implemented

## Problem

iOS 键盘扩展的 `DictionarySnapshotWorker` 在后台准备云端词库快照时持有 App Group 的 `worker.lock`。`stop()` 会取消 `Task` 并清掉主线程上的租约引用，但准备任务的闭包仍捕获同一个 `WorkerLease`；如果词库准备或原生桥调用没有立即响应取消，租约析构会推迟到任务结束。扩展在此期间被挂起会继续持有共享容器的文件锁，触发系统的 `RUNNINGBOARD 0xdead10cc` 终止（#7695）。

## Decision

`WorkerLease` 提供幂等的 `release()`：在锁描述符上显式解锁并关闭，之后由析构再次调用也不会重复关闭。`DictionarySnapshotWorker.stop()` 在取消任务后立即调用它，再清除租约引用。这样异步准备仍可在取消点收尾，但不会把 App Group 文件锁的释放绑定到后台任务的析构时机。

## Alternatives considered

- **等待后台任务完成后再返回 `stop()`**：能保持租约状态完整，但 `stop()` 在主线程调用，词库准备可能需要较长时间，阻塞键盘收起又会增加系统挂起前超时的风险。
- **只把 `WorkerLease` 从属性置空**：析构任务闭包仍持有同一对象，无法释放锁，正是本问题的触发条件。
- **键盘收起时取消所有快照状态而不保留任务**：会丢掉已认领的快照，且仍不能保证正在执行的原生准备立即释放锁。

## Consequences

键盘进入后台时，快照租约会在主线程同步归还，另一个进程可以立即取得 `worker.lock`；后台任务若稍后完成，使用已释放租约的取消路径不会再次持锁。租约的显式释放成为队列租约的生命周期契约，测试覆盖了所有者尚未析构时重新取得租约的情况。
