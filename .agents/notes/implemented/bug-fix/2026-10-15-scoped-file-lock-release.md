# Agent Note: 共享存储锁随操作与租约作用域结束

Status: implemented

## Problem

多个存储只持有裸 `File`，Unix 关闭当前描述符后，复制或 `fork` 继承的副本仍能延长 `flock`。20 项真实取锁入口回归全部复现：偏好、剪贴板、遥测、常用语、通知、统计、社区资源、自定义皮肤、键盘试用、词库集合、背词进度与词书、插件导入与名单、候选皮肤同步、快照状态及工作租约、账号刷新、云剪贴板中继及 MCP 写入。后续阻塞操作可能继续等待，非阻塞操作可能误报忙碌。

## Decision

`file_lock::ScopedFileLock` 在成功取得锁后拥有文件。Unix 析构复用 `unlock`，Windows 继续关闭文件。阻塞、即时尝试和现有宽限尝试均复用原取锁函数，维持每条路径的等待策略和错误映射。原先已显式解锁的个人词库、词库访问租约、本地模型与资源安装守卫继续保留。

账号刷新和云剪贴板中继仍需先用同一个裸 `File` 轮询 `try_exclusive`，以保留各自的超时、轮询间隔和错误映射；成功后立即用 `ScopedFileLock::from_locked` 接管该文件。这样不会重新打开文件或改变有界等待语义，同时让操作返回时由守卫显式解除锁。

`ScopedFileLock`、阻塞构造器 `exclusive` 和只借用同一文件的 `file()` 对工作区调用方公开。MCP 的 `claim_shared_write` 在读时间戳前建立守卫，通过借用读写原文件，最终返回守卫覆盖六个写工具的实际操作；读取失败、节流拒绝、写入失败和展开都由同一守卫释放。该文件同时保存跨进程节流时间戳，不能换一个文件句柄或提前释放锁。非阻塞及接管已持锁文件的构造器仍限于 client-core 内部；借用调用方不得手动解锁。

快照状态锁与工作锁都持有同一守卫类型；`acquire_worker_lease` 移交守卫所有权，取得租约后原容器结束不能解锁，租约结束才释放。插件根目录仍先持有进程互斥，再持有文件锁；释放文件锁后才释放进程互斥。路径打开、目录句柄绑定、清理顺序及落盘格式沿用现有契约。

## Historical notes

- [个人词库锁释放](../../implemented/bug-fix/2026-10-15-personal-dictionary-lock-release.md)、[词库访问租约](../../implemented/bug-fix/2026-10-15-dictionary-access-lock-release.md)、[本地模型锁释放](../../implemented/bug-fix/2026-10-15-local-model-lock-release.md)、[资源安装锁释放](../../implemented/bug-fix/2026-10-15-resource-install-lock-release.md)：部分重叠，复用同一释放语义，保留各自已经落实的守卫。
- [快照队列锁目录](../../implemented/bug-fix/2026-10-09-cloud-snapshot-queue-lock-directory.md)、[通知锁目录](../../implemented/bug-fix/2026-10-09-notices-lock-directory.md)、[插件名单锁目录](../../implemented/bug-fix/2026-10-09-mentions-lock-directory.md)及其他目录句柄笔记：部分重叠，继续决定打开对象，本篇决定成功持锁的生命周期。
- [偏好恢复备份](2026-10-09-preferences-recovery-backup-directory-handle.md)：设备验证时发现并修复既有重名用例的跨秒依赖，具体取时边界与确定性验证由原笔记维护。
- [Android 高度测量窗口稳定](2026-10-14-android-keyboard-height-window-stability.md)：完整设备验收还发现十四键测量漏用既有窗口等待；修复与现场证据记回原笔记，产品几何及断言不变。
- iOS 原生工作租约的取消释放由其独立笔记维护，不是本篇 Rust 守卫的实现；检索未发现本篇统一守卫的已有归属，没有完全吸收或应废弃的提案。

## Alternatives considered

- 为每个容器分别实现 `Drop`：改动局部，能直接沿用已修复模块的方式；但快照锁容器必须移动字段给租约，容器实现 `Drop` 后不能直接完成该移交，且十余处重复析构更容易漏掉新的返回路径。由文件字段持有守卫可以把所有权一起移动。
- MCP 自建守卫并公开底层 `unlock`：跨 crate 改动很少，也能读写锁文件；但会重复既有析构逻辑，并把手工释放约定传给调用方。公开现有守卫的阻塞构造与文件借用能复用同一平台释放实现。
- 只增加锁重试：既有宽限尝试能吸收短暂继承窗口，改动少；但无法规定操作结束时的释放，阻塞路径仍依赖副本关闭，故保留已有等待策略并直接修复释放边界。

## Verification

17 项真实入口回归在旧实现全部行为断言失败；仅把文件包装成无析构守卫后，17 项仍在同一断言失败；加入显式解锁后全部通过。回归验证活动期间互斥、旧副本关闭不释放新锁、工作租约移交不提前释放。另核对三种取锁方式的失败竞争和栈展开释放。完整 Rust 测试：client-core 1471 passed、2 ignored，host-api 633 passed、2 ignored，相关集成测试通过。设备发现既有备份重名用例的跨秒依赖后修正，偏好测试 143 passed，严格 Clippy 通过；最终 Android API 28、35 核心各 1417 passed、13 ignored、7 filtered。quick 通过，包含 Linux 原生 87 项测试及 macOS 原生宿主编译；本机缺少 MinGW 与完整 HarmonyOS 打包环境，对应阶段按脚本跳过。最终完整 APK 的 API 28 核心安装验收通过；API 35 完整安装验收与 CI 的最终结果记录于对应 PR 的验证清单。独立代码及文件权限边界审查均无待修复问题。

后续两个有界等待入口在旧实现下均失败，接管同一个已持锁文件后相关 23 项回归全部通过；本批完整 client-core 单测 1473 passed、2 ignored，host-api 单测 633 passed、2 ignored，15 项集成测试通过，严格 Clippy、格式与 quick 门禁通过。Android API 28/35 的核心单测各 1419 passed、13 ignored、7 filtered；API 28 原生 APK 核心安装验收与 JNI 中继端到端验收通过。独立审查确认超时、轮询、路径对象和权限检查均未改变。

MCP 的两项真实入口回归在旧实现均于释放断言失败，改用共享守卫后全部通过；MCP 单元 60 项、stdio 集成 11 项通过。共享层本机 client-core 单元 1473 passed、2 ignored，host-api 单元 633 passed、2 ignored，相关集成测试通过；Android API 28/35 的核心单测各 1419 passed、13 ignored、7 filtered。独立审查确认六个写工具的持锁范围、文件对象、权限检查及各平台取锁实现未改变，并补全公开借用接口的 Unix/Windows 语义说明。

## Consequences

操作和租约结束后，描述符副本不再延长互斥；每条路径保留现有等待策略，新增守卫集中维护 Unix 析构。析构无法返回解锁错误，描述符关闭仍是兜底。测试复现打开文件描述的复制语义，不声称捕获实际产品 `fork` 现场；统一守卫不移交底层文件所有权；需要读写锁文件自身内容的调用方可以借用同一文件，因此必须遵守不手工解锁的契约。client-core 的测试复制与竞争断言放在仅测试编译的工具模块；MCP 回归通过生产借用接口复制描述符。
