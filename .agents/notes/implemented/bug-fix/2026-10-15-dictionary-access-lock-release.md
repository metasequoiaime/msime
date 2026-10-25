# Agent Note: 词库访问租约结束时释放全部文件锁

Status: implemented

## Problem

`DictionaryAccess` 为用户目录和词库目录持有共享会话锁或独占维护锁，原实现只关闭描述符。Unix 的 `flock` 随打开文件描述存活，描述符被复制或在 `fork` 后继承时，租约结束不能立即恢复访问。现有 client-core 和 host-api 测试用最长两秒重试吸收这个窗口，产品入口仍即时报告维护忙碌。

## Decision

Unix 租约析构时使用现有 `file_lock::unlock` 释放所有成功取得的锁。在获取第一把锁前建立空租约，逐个把成功持锁的描述符加入其中；后续目录失败或竞争时也经同一个析构路径释放已有锁。未取得的锁不能进入负责解锁的集合。

会话间共享、维护独占、排序去重和目录句柄绑定契约保持不变。用描述符副本确定性复现共享与独占租约结束后的阻塞，并验证旧副本关闭和失败的竞争者不会解开新租约。client-core 与 host-api 原有两秒测试宽限改为即时断言。

## Historical notes

- [个人词库操作结束时显式释放锁](../../implemented/bug-fix/2026-10-15-personal-dictionary-lock-release.md)：部分重叠，复用释放机制；它只涉及个人词库队列，本篇处理引擎会话和维护的访问租约，互链保留。
- [iOS 键盘收起时立即释放词库快照租约](../../implemented/bug-fix/2026-10-14-ios-snapshot-worker-lock-release.md)：无关，处理 Swift 异步任务捕获对象导致租约不析构。
- 其他 `DictionaryAccess` 命中说明只读查码为何不持租约及备份恢复的空闲维护时机，不规定文件锁的析构方式，保持原文。

## Alternatives considered

- 继续关闭文件并在取锁端重试：改动局部，已有测试先例；但延迟依赖子进程何时关闭描述符，还会拖慢真正的锁竞争，不能保证租约边界。
- 仅给完整构造的租约增加析构：正常结束的修复最小；但两个目录逐个获取，第二个目录失败时第一个锁尚未进入租约，仍依赖关闭描述符，因此提前建立空租约。

## Verification

macOS 新增的共享和独占描述符回归在旧实现均因无法立即取得维护租约失败，修复后访问模块 7 项通过。回归还验证副本关闭不释放后来锁、活动持锁者继续排斥访问，以及部分获取竞争或文件错误后第一把锁已归还。macOS 完整 client-core 为 1434 项单测与 6 项集成测试通过；固定 Linux 容器为 1433 项通过、2 项忽略。host-api 为 631 项单测与 15 项集成测试通过、2 项忽略。最终源码的访问模块 7 项通过，client-core 与 host-api 严格 Clippy、格式和笔记校验通过。

Android API 28 和 35 的最终完整核心套件各 1380 项通过、13 项忽略，7 项硬链接测试由设备脚本过滤；新增共享与独占描述符回归实际执行通过。API 28 首轮曾在未改动的本地模型删除测试上触发 100 毫秒超时；保留首轮记录，没有修改超时或跳过测试，重新构建最终测试程序后完整重跑通过。本地 `MSIME_BUILD_JOBS=4 CARGO_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，包含 Linux 原生 87 项测试、macOS 原生和 Apple bridge 编译；Windows 交叉编译和 HarmonyOS 真编译因前置不齐跳过。API 28 原生 APK 的 `smoke.sh emulator-5566 --core` 通过。

语义依据为 [Rust File::unlock](https://doc.rust-lang.org/std/fs/struct.File.html#method.unlock) 与 [Linux flock(2)](https://man7.org/linux/man-pages/man2/flock.2.html)。

## Consequences

Windows 维护期间新客户端不能重新创建持锁会话，见[维护期间延迟新连接的会话创建](2026-10-24-windows-maintenance-new-connection.md)；该缺陷属于宿主创建时机，与本篇的租约析构部分重叠，分别保留。

其他共享存储的作用域释放由[共享文件锁守卫](2026-10-15-scoped-file-lock-release.md)统一处理；与本篇部分重叠，各自保留目录身份或租约边界的决定。

析构不能上报解锁错误，失败仍由文件关闭兜底。新增显式解锁只在 Unix 编译，Windows 保持描述符关闭行为。测试验证同一打开文件描述的复制语义，不声称捕获真实产品 `fork` 现场。
