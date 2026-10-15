# Agent Note: 本地模型操作结束时显式释放根目录锁

Status: implemented

## Problem

本地模型的安装、文件包安装、归档成员安装、收编和删除共用模型根目录锁，持锁对象只是 `File`。Unix 的文件锁跟随打开文件描述，关闭当前描述符时，复制或 `fork` 继承的副本可以让已结束的操作继续持锁；后续安装继续等待，删除则可能返回 `InstallRunning`。

此前 Android 验证里的模型删除测试曾出现 100 毫秒超时，但日志没有记录锁的继承或调度现场，本次不据此归因。回归用描述符副本确定性核对锁释放契约。

## Decision

本地模型成功取得的根目录锁由私有 `ModelLock` 守卫持有，Unix 析构时复用 `file_lock::unlock`。阻塞安装和非阻塞删除都由同一个守卫负责释放；只有取得锁之后才构造守卫，失败的竞争者不得解锁其他操作。Windows 继续沿用关闭描述符的行为。现有根目录检查、清理顺序、文件格式和删除忙碌策略不变。

## Historical notes

- [模型删除拒绝安装竞争](../../implemented/bug-fix/2026-10-12-local-model-remove-lock-race.md)：部分重叠，保留真实竞争返回忙碌的决定，本篇约束持锁操作结束时的释放。
- [个人词库锁释放](../../implemented/bug-fix/2026-10-15-personal-dictionary-lock-release.md)和[词库访问锁释放](../../implemented/bug-fix/2026-10-15-dictionary-access-lock-release.md)：部分重叠，复用相同的 Unix 释放语义；各自维护不同的操作边界。
- 其余本地模型目录句柄笔记处理目录替换时的读写与清理对象，不规定根目录锁的析构方式，保持原文。

## Alternatives considered

- 继续关闭描述符并增加取锁重试：已有 `try_exclusive_with_grace` 可吸收短窗口，改动少；但后续操作仍依赖副本何时关闭，阻塞安装可能持续等待，不能保证操作边界。
- 在每条安装和删除返回路径手动解锁：能够上报正常返回时的解锁错误；但多个 `?`、校验失败和清理路径容易遗漏，守卫可以统一覆盖并保持清理先于解锁。

## Verification

新增回归在旧实现因守卫结束后仍不能取得锁而失败，修复后本地模型模块 77 项通过，1 项既有忽略。回归同时验证活动操作仍排斥竞争、失败删除不会解开持有者的锁、旧副本关闭不解开后来锁，最后可正常删除。

最终 macOS 完整 client-core 为 1444 项单测与 6 项集成测试通过，2 项忽略；固定 Linux 容器为 1443 项单测与 6 项集成测试通过，2 项忽略。host-api 为 631 项单测与 15 项集成测试通过，2 项忽略。client-core 与 host-api 严格 Clippy 通过。Android API 28/35 最终完整核心各 1390 项通过、13 项忽略，7 项硬链接用例由既有设备脚本过滤；新增锁释放回归和两项并发删除测试均执行通过。API 35 首轮的旧等待协议失败及修正、阻塞变异验证由[模型删除锁竞争](2026-10-12-local-model-remove-lock-race.md)记录，首轮日志保留。

本地 quick 门禁通过，包含 macOS 原生构建、共享 Apple bridge、Linux 原生构建与 87 项测试；HarmonyOS 真编译缺前置环境、Windows 原生及 pipe-only 编译缺 MinGW，按脚本跳过。原生 Android APK 已构建，API 28 的 `--core` 装包验收和 API 35 默认完整套件验收均通过，未跳过默认套件；后者覆盖系统输入、实时偏好及剪贴板保存生命周期，不包含需额外开关启用的设置、统计和手写套件。

语义依据为 [Rust File::unlock](https://doc.rust-lang.org/std/fs/struct.File.html#method.unlock) 与 [Linux flock(2)](https://man7.org/linux/man-pages/man2/flock.2.html)，沿用仓库个人词库和词库访问租约的解锁实现。

## Consequences

操作结束后继承描述符不会再延长互斥，后续安装和删除可以按实际持锁状态继续。析构不能返回解锁错误，失败由描述符关闭兜底。回归验证复制描述符的锁语义，不声称捕获实际产品 `fork` 现场；模型删除测试首次超时仍是独立调查线索。
