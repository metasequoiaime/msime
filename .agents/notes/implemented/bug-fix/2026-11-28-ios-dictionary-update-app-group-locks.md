# Agent Note: 词库在线更新不在 App Group 里持锁等网络

Status: implemented

## Problem

TestFlight 上 iOS 0.54.0 的 App 以 `RUNNINGBOARD 0xdead10cc` 被系统结束（#11973）：主线程空闲，`dictionary-update` 线程停在 `local_models::download_resumable` 的 reqwest 阻塞读里。iOS 在挂起进程时发现它还持着共享容器（App Group）里的文件锁，就直接杀掉它。

词库在线更新在等网络的整个过程里持着两把独占 `flock`，都在 `state_root/dictionary-updates/` 下，而 iOS 的 `state_root` 是 App Group：

- `update.lock`：`check_with` 一开始取，版本接口、manifest 和每个词库文件的下载都在它里面，检查返回才放。
- `generations/resources.lock`：`ResourceStore::install` 取锁后逐个调 `fetch`，`dictionary_updates::install` 的 `fetch` 回调就在里面下载。

App 每次回到前台都发起一次自动检查（`MetasequoiaImeApp.swift`），任务是 Rust 自己起的后台线程，外面没有后台任务。一个词库版本几百 MB，用户切走时下载多半还没完，进程带着两把锁被挂起。

## Decision

- **下载在 `resources.lock` 之外完成（所有平台）。** `dictionary_updates::install` 分两步：`prepare` 先把这一代的每个文件准备成本机文件（复用本机相同字节的，其余经 `download_file` 下到续传暂存里），这一步不持 `resources.lock`；再调 `ResourceStore::install`，它的 `fetch` 只从准备好的本机文件里取，在锁下复制、校验、发布。这一代的目录已经在时什么也不准备，由 `ResourceStore` 校验已有目录。`ResourceStore::install` 的文档写明 `fetch` 在锁下调用，应用里的调用方不能在里面联网。
- **iOS 上检查的锁和下载暂存放在 App 自己的容器里。** `UpdateContext` 新增 `private_root`：给了时 `update.lock`、`.partial-downloads/` 和 manifest 的匿名暂存文件都在它下面，`status`（判断检查是否中断）和 `rollback` 也用那里的 `update.lock`。词库请求 `updates` 的 action 增加可选字段 `private_directory`（绝对路径，相对路径整个请求被拒），iOS App 传 `Caches/dictionary-updates`，别的宿主不传，行为与以前相同。给了 `private_root` 时，检查开始先清掉共享状态目录里以前留下的 `.partial-downloads/`，那份再也续不上。
- **前提：只有一个进程会检查、回退词库更新。** 锁搬到私有目录后不再跨进程互斥。iOS 上只有 App 调 `updates`，键盘扩展从不调；键盘只经 `GenerationLease` 持代次的共享锁，这部分不变。

改完以后，iOS 上一次检查只在两处短暂持 App Group 里的锁，都是本机读写：`ResourceStore::install` 从私有暂存复制进代次目录并校验，以及检查成功后 `remove_old_versions` 逐个取旧代次的独占锁。

## Alternatives considered

- **给检查套 `beginBackgroundTask`，到期时取消下载。** 这是本仓库已有的做法（账号刷新、词库同步），改动最小，App 切到后台后还能多下 30 秒左右。但下载要几分钟，后台时间总会到期；到期回调要在返回前放掉锁，而停在 reqwest 阻塞读里的线程要等下一块数据或 60 秒读超时才看得到取消，到期回调等不了这么久。只靠它，挂起时仍可能持锁，所以没有采用。
- **在所有平台都把 `update.lock` 收窄到本机读写的几段，网络阶段不持锁。** 不用新增字段，也不分平台。但下载阶段仍需要跨进程互斥：桌面上设置应用、MCP 服务这类进程可能同时发起检查，两个进程往同一份 `.partial-downloads/` 续传会互相写坏、互相删文件。要保住互斥又不持锁，只能自己做带存活判断的租约文件，复杂度和出错面都比把锁放进私有目录大。
- **按 `cfg(target_os = "ios")` 把锁放进 `std::env::temp_dir()`。** 不用改 Swift 和请求格式。但锁的位置变成了隐式的平台约定，「只有一个进程检查」这个前提也没有地方写；由宿主显式交目录，约束写在 `UpdateContext::private_root` 上，别的平台要用时也清楚该满足什么。

## Verification

- `crates/client-providers/src/dictionary_updates/tests.rs` 新增四项。`network_waits_hold_no_lock_in_the_shared_state_directory` 在每次网络请求时扫一遍临时目录里被锁着的 `*.lock`，给了 `private_root` 时只能是 `private/update.lock`；`downloads_finish_before_the_resource_lock_is_taken` 在不给 `private_root` 时只能是 `state/dictionary-updates/update.lock`。把 `install` 换回旧写法（下载在 `ResourceStore` 的回调里）后，这两项都失败，报出 `generations/resources.lock` 在下载时被锁着。另两项检查私有目录里的续传和旧暂存的清理，以及 `status`、`rollback` 用的是私有的 `update.lock`。
- `crates/host-api/src/dictionary_updates.rs` 新增一项：`updates` 请求带绝对路径的 `private_directory` 时锁建在那里，相对路径被拒。
- `cargo test -p msime-client-providers --lib dictionary_updates`（51 项）、`-p msime-host-api --lib dictionary_updates`（9 项）、`-p msime-client-core --lib resources`（34 项）通过；`cargo clippy -p msime-client-providers -p msime-host-api --all-targets -- -D warnings` 通过。
- 没有在 iOS 真机上复现或验收挂起场景，也没有在 Android 模拟器上跑过设备单测。

## Consequences

- iOS 上下载期间 App 被挂起不再因为这两把锁被系统结束；被挂起的下载在下次检查时从私有暂存续传。
- 资源包（日文、五笔、语言词库）在 iOS 上同样装在 App Group 里，`local_models::install_files_with` / `install_archive_members_with` 在下载期间持着模型目录锁，问题相同，不在这次改动里。本机语音模型在 App 自己的容器里，不受影响。
- 代价：准备阶段要等这一代所有要下载的文件都下好才复制，下好的暂存在复制完之前都留着，最多多占一份要下载的那几个文件的空间（以前每个文件复制完就删）。iOS 的暂存在 `Caches` 下，系统空间紧张时可能清掉没下完的部分，下次检查从头下。
- 复制和校验仍在 App Group 的 `resources.lock` 下做，几百 MB 要一两秒；App 恰好在这一两秒里被挂起仍会被结束。这一段没有套后台任务，因为检查是 Rust 起的线程，Swift 那边不知道它什么时候到这一步。
- 相关笔记：[词库在线更新](../feature/2026-10-18-dictionary-auto-update.md)定下检查的流程和节流，本篇只改锁和暂存的位置，不替代它；[资源安装结束时显式释放安装锁](2026-10-15-resource-install-lock-release.md)规定 `resources.lock` 的释放边界，本篇收窄的是它被取着时 `fetch` 能做什么。
