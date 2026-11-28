# Agent Note: 词库在线更新不在 App Group 里持锁等网络

Status: implemented

## Problem

#11973：iOS App（0.54.0，iOS 17.4）在后台被 RunningBoard 以 `0xdead10cc` 结束。崩溃时唯一在干活的线程是 `dictionary-update`，停在 `reqwest` 的读取上，调用栈是 `dictionary_updates::check` → `install` → `ResourceStore::install` → `download_file` → `local_models::download_partial`。

一次检查从头到尾持着两把状态目录里的独占 `flock`，而 iOS 的状态目录是 App Group 共享容器（`MetasequoiaInputSessionBridge.sharedStateDirectory`）：

- `dictionary-updates/update.lock`：`check_with` 一开始就取，罩住版本 API 请求、manifest 下载和全部词库文件下载，到检查返回才放。
- `dictionary-updates/generations/resources.lock`：`ResourceStore::install` 取锁之后才调 `fetch(artifact)`，而 `dictionary_updates::install` 的 `fetch` 就在里面下载文件。

App 每次回到前台都发一次 `automatic`（`platforms/ios/App/Sources/MetasequoiaImeApp.swift`），任务跑在脱离的 Rust 线程上。词库发布包几百 MB，用户切走时下载往往还没完，应用带着这两把锁被挂起，系统随即把它结束。

## Decision

等网络的那一段不持任何 App Group 里的锁；共享容器里的锁只罩住本机的复制和发布。

- **`resources.lock` 只罩复制**：`dictionary_updates::install` 先不取锁地调 `gather`，按 `set` 的顺序为每个文件备好已校验的字节（本机有相同长度和 SHA-256 的文件就直接打开，其余用 `download_file` 下载到 `dictionary-updates/.partial-downloads/`），全部备齐之后才调 `ResourceStore::install`，它的 `fetch` 只从备好的句柄里取。ResourceStore 照旧在锁内重新校验每个文件，备好的字节损坏或被替换也进不了新代次。这一代已经发布过（代次目录已在）时不备字节，ResourceStore 只校验它，与原来一样不下载。这一条对所有平台生效。
- **`update.lock` 在 iOS 上换成进程内互斥**：`dictionary_updates/check_lock.rs` 的 `CheckLock` 统一承担「一次检查、回退互斥」和 `status()` 判断「还有没有检查在进行」（没有就把停在 `checking` / `downloading` 的状态报成 `interrupted`）。iOS 上它按状态目录登记在进程内的表里（`Mutex` + `Condvar`），`rollback` 等进行中的检查结束；其他平台仍是 `update.lock` 的跨进程 `flock`，行为不变。

### 为什么 iOS 上进程内互斥就够

`update.lock` 的使用者只有 `check`、`rollback` 和 `status`，都只经 host-api 的 `dictionary_updates::request` 调到；iOS 上调它的只有主应用（`MetasequoiaInputSessionBridge.dictionaryUpdates`，调用方是 `MetasequoiaImeApp` 和 `DictionarySettingsView`），键盘扩展不发这些请求。键盘和 App 之间真正要跨进程互斥的是代次租约 `generations/<代次>.lock` 和 `resources.lock`，它们不变。host-api 本来就在进程内按状态目录只留一条检查任务（`RUNNING`），`CheckLock` 再保证 `rollback` 不和检查交错。

## Alternatives considered

- **只用 `UIApplication.beginBackgroundTask` 包住检查**——改动全在 Swift，不碰共享 Rust。不够：后台任务大约 30 秒就到期，而下载要几分钟；阻塞的 `reqwest` 读取只在下一块数据到来或 60 秒读取超时后才看得到取消，到期回调里也没法及时放锁。而且 `automatic` 请求是起线程后立刻返回的，Swift 侧不知道任务什么时候结束。
- **所有平台都在网络阶段放掉 `update.lock`，只在开头和发布时短暂持有**——平台之间一套逻辑。不用：桌面上这把锁是跨进程的现有契约，没有排除过同时有两个进程发检查的情况；网络阶段没有跨进程互斥时，两个进程会同时往同一个 `.partial-downloads` 文件里续传（`download_partial` 的文档要求调用方保证同一时间只有一个下载）；`status()` 也就没有可靠的「检查还在进行」的信号，得另造带心跳的租约文件。桌面上持着这把锁等网络没有任何代价，没有理由换掉。
- **把 `update.lock` 挪到 App 自己的容器（由 Swift 传一个私有目录进来）**——仍是文件锁，跨进程语义不变。不用：要给 host-api 的选项和 Swift 桥加一个只有 iOS 用的字段，而 iOS 上这把锁本来就只在一个进程里用，文件锁换不来任何东西。

## Consequences

- **收益**：iOS 上检查等网络期间，App Group 里没有这次检查持有的锁，应用被挂起不再触发 `0xdead10cc`；挂起后连接断开，下次检查从 `.partial-downloads` 续传。
- **收益**：所有平台上 `resources.lock` 只在复制、`fsync` 和发布期间持有，资源包安装、旧版本回收等别的 `resources.lock` 使用者不再被几分钟的下载挡住。
- **代价**：文件全部下载完才开始复制，下载好的句柄（含复用的本机文件）在复制前一直开着；下载失败时已经下完的文件随句柄删除，与原来「复制进暂存目录后整个暂存目录被丢掉」的结果相同。
- **残留风险**：复制和发布阶段（几百 MB 的读、校验、写和 `fsync`）仍持着 App Group 里的 `resources.lock`，恰好在这几秒里被挂起仍可能被结束。要完全堵上，需要 App 在进入后台时用后台任务盖住这一段，这次没有做。
- **残留**：资源包下载（日文、五笔、多语言词典）在 `local_models::install_files_with` / `install_archive_members_with` 里持着 App Group 里的模型目录锁等网络，是另一条路径，#11973 写明单独跟踪。
- **平台差异**：iOS 上 `CheckLock` 不写 `update.lock` 文件，其他平台的锁文件和语义不变。

## Verification

- `dictionary_updates::tests::downloads_never_wait_on_the_network_while_holding_the_resource_store_lock`：假传输在每次请求词库文件时去取 `generations/resources.lock` 的独占锁，两次检查（第二次时代次目录和锁文件都已存在）的每次请求都取得到。把 `dictionary_updates.rs` 换回改动前的版本，这条测试失败。
- `dictionary_updates::check_lock::process::tests::one_holder_per_state_directory_and_waiters_resume_after_release`：iOS 用的进程内互斥在宿主上编译测试，同一状态目录只有一个持有者、不同目录互不影响、等待者在释放后继续。
- `cargo test -p msime-client-providers --lib` 全部通过；`cargo clippy -p msime-client-providers --all-targets -- -D warnings` 和 `--target aarch64-apple-ios` 都没有警告。
- 未在真机上复现和复验 `0xdead10cc`。
