# Agent Note: 个人词库操作结束时显式释放锁

Status: implemented

## Problem

#8044 的资源验收曾在 `collections_unowned_words_are_queued_past_the_personal_queue_limit` 返回 `Busy`，完整重跑通过。个人词库的 `PersonalDictionaryLock` 只关闭文件，没有显式解锁；Unix 的 `flock` 属于打开文件描述，`fork` 继承的副本在 `exec` 前可以让已经结束的操作继续占锁。之前的 `851a9883e1` 已确认快照队列有同类机制，但当时未在个人词库测到症状，保留了即时尝试。

## Decision

Unix 的 `PersonalDictionaryLock::drop` 调用现有 `file_lock::unlock`，使互斥范围随操作守卫结束。获取锁仍即时返回，真实持锁者仍使另一操作得到 `Busy`。只在成功获取锁之后建立守卫，失败者不能解锁持有者。

以 `File::try_clone` 保留同一打开文件描述的副本，确定性验证操作结束后可立即再次写入，并验证旧副本关闭不会解开后来操作的锁。该回归证明释放语义缺陷，不把一次偶发日志归因为已经抓到现场的 `fork`。

## Historical notes

- [个人词典同步锁绑定目录句柄](../../implemented/bug-fix/2026-10-09-personal-dictionary-lock-directory.md)：部分重叠，继续保持锁和状态绑定同一目录；本次补充守卫结束时的锁生命周期。
- [本地模型删除锁竞争](../../implemented/bug-fix/2026-10-12-local-model-remove-lock-race.md)：无关，决定删除遇到真实安装锁时报告忙碌。
- 其余个人词库、恢复和快照检索命中涉及分页、数据归属或任务代次，不决定个人词库文件锁的释放方式。

## Alternatives considered

- 复用 `try_exclusive_with_grace`：已有快照队列和模型安装先例，能吸收短暂继承窗口；但会增加真实竞争的等待，而且继承者超过宽限时仍误报。守卫知道操作何时结束，显式解锁直接结束这段互斥。
- 将整个套件串行或只重试失败测试：能减少子进程继承窗口并恢复验收，但宿主也会启动子进程，不能修复运行时的锁残留。

## Verification

macOS 的确定性回归在旧实现因 `Busy` 失败；显式释放后，`cargo test -p msime-client-core --lib dictionary::personal --locked` 的 12 项通过。回归验证旧描述符存活期间可再次写入、活动持锁者仍排斥写入、旧副本关闭及失败的竞争者不能释放新锁。

完整 client-core 套件在 macOS 为 1430 项通过、2 项忽略，另有 6 项集成测试通过；固定 Linux 容器为 1429 项通过、2 项忽略。Android API 28 和 35 各为 1376 项通过、13 项忽略，7 项硬链接测试按设备脚本过滤，新增回归在两台设备均执行并通过。严格 Clippy、格式和笔记校验通过。API 28 原生 APK 的 `smoke.sh emulator-5566 --core` 通过。

`MSIME_BUILD_JOBS=4 CARGO_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 在基线 `8f6b216856` 上通过，含 Linux 原生 87 项和 macOS 原生、Apple bridge 编译。首轮因 OrbStack 停止而失去 Docker socket；恢复后 Linux 桌面独立编译与完整 quick 均通过。APK 使用文档要求的 JDK 21 构建。API 35 首次装包因测试包缺少可选注音词库而在布局切换失败；按现有设备 workflow 下载并校验全部语言词库、重新打包后，该用例通过。

语义依据是 [Rust `File::lock` 文档](https://doc.rust-lang.org/std/fs/struct.File.html#method.lock) 与 [Linux `flock(2)`](https://man7.org/linux/man-pages/man2/flock.2.html)：复制和继承的描述符共享锁，显式解锁使其释放。仓库 `dictionary::quiesce::Lease` 已复用同一解锁函数。

## Consequences

其他共享存储的作用域释放由[共享文件锁守卫](2026-10-15-scoped-file-lock-release.md)统一处理；与本篇部分重叠，各自保留目录身份或租约边界的决定。

个人词库操作完成后，继承描述符不会再延长互斥。获取锁仍即时返回，真正的竞争者仍收到 `Busy`，不引入固定等待。目录绑定、落盘格式和 Windows 行为保持现有契约。

析构不能返回解锁错误；失败时仍关闭原文件描述符，副本全部关闭后由内核释放。其他存储锁的释放方式另行审计，不在本次批量替换。当前回归覆盖同一打开文件描述的复制语义，没有强行在多线程测试进程中执行 `fork`。

相关的引擎会话与维护锁由[词库访问租约释放](../../implemented/bug-fix/2026-10-15-dictionary-access-lock-release.md)单独处理，包括逐目录获取时的部分失败退出。
