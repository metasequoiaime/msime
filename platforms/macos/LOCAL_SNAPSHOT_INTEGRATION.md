# 完整词库快照的本地接入

云端备份/恢复与本地词库替换是两条不同的链路。这份文件说明后者：一份完整的词库快照怎么从 SwiftUI 界面走到 Engine，以及这条路上的边界由谁负责。

## 链路

账号窗口的「云词库同步与备份」（`BackendSnapshotView.swift`）经 Rust 的快照队列（#8201）：`downloadForLocal` 带本机的 HostOptions（`MSIMEClientSession.activeHostOptions`）调 `msime_client_cloud_dictionary` 的 `snapshot_preview`，`applyLocal` 调 `snapshot_enqueue`。Rust 入列后马上写词库维护租约、等输入会话放开词库，在本进程里准备并激活新的一代。Rust 只写租约、不发通知，而没有获得焦点的输入框时 `InputController.mm` 每秒检查租约的计时器不跑，所以请求进行期间宿主每 250 毫秒在本进程里发一次 `MSIMEDictionaryMaintenanceWillBeginNotification`，叫醒握着词库的输入会话去检查租约；放不开时请求留在 `<状态目录>/dictionary-snapshots` 的队列里，窗口开着时每两秒问一次 `snapshot_status`，关掉窗口也不丢，下一次有面板问状态时接着处理。

Tauri 面板的「应用到本机」（`BackendDesktopSnapshots.swift`）走同一条路：`snapshot_status`、`snapshot_preview`、`snapshot_enqueue` 带同一份 HostOptions，面板每两秒问一次状态时 Rust 接着处理排队中的请求（最多每 60 秒一次），这期间同样叫醒本进程的输入会话。HostOptions 和叫醒都在 `BackendLocalDictionary.swift` 的 `MacLocalDictionary` 里，它按名字动态查找共享的 `MSIMEClientSession`（`activeHostOptions`）。

`shared/apple/MSIMEClientSession.{h,mm}` 的 `snapshotVersion:`、`prepareSnapshot:`、`applySnapshot:`、`discardSnapshot:` 和 `snapshotActivationReady` 是宿主自己准备、激活一份快照的选择器（macOS 的 Swift 后端已不再调用，`snapshot-activation-test` 仍覆盖），它们转成 `crates/host-api/include/msime_client.h` 里的 C ABI：`msime_client_snapshot_version`、`msime_client_snapshot_inspect`、`msime_client_snapshot_prepare`、`msime_client_snapshot_discard`、`msime_client_snapshot_activate`、`msime_client_snapshot_restore` 和 `msime_client_snapshot_queue`。记录流由宿主以 `msime_client_snapshot_next` 回调逐条同步喂进去：正数表示一条 UTF-8 JSON 的长度，0 只在校验过的 EOF 处返回，负数表示取消或损坏，回调本身不得抛出或栈展开。

Rust 侧的实现在 `crates/host-api/src/dictionary_snapshot.rs`（记录解析在 `dictionary_snapshot/record.rs`）。`inspect` 先校验完整的 NDJSON 信封——头尾顺序、整段 body 的校验和、分类顺序和记录边界都属于云端格式的一部分；Engine 记录更深一层的、按方案区分的校验放在 prepare 阶段。准备句柄登记在进程内的注册表里，`activate` 只接受未过期的句柄并比对期望版本，发布前做原子替换，失败时把原目录搬回来；回滚之后留下的空备份目录才会被删掉，`remove_dir` 对非空目录的拒绝正是这里要的保护——那时备份可能是用户词库仅剩的一份。

记录流与状态版本的适配全部在 host-api 的 C ABI 之后，宿主不再有自己的 C++ 桥接层；`shared/snapshot`（Swift Package）负责快照文件格式本身的校验与上传。

下载快照的凭据和网络请求在 Rust 的账号会话里（`msime_client_cloud_dictionary`），不进入 `client-core`。引擎会话的 `snapshot()`（`crates/engine/src/host/session.rs`）返回的是编辑串/候选这类输入状态，与词库快照无关。

## 边界

下面几条对 Rust 快照队列的「准备并激活」和宿主自己调用这些选择器同样成立。

- 准备阶段不修改活动词库，只在隔离的客户端用户目录里落暂存数据，用的是已校验资源。
- 预览之后发生本地学习、编辑或代次切换时，版本比对会让本次覆盖失败，而不是静默覆盖。
- 激活要与现有会话和跨进程词库访问锁协调：探测到新代次成功后才原子发布，并重建当前运行时会话、使旧候选回调失效。失败保持旧安装可用。
- 取消和应用失败只清理暂存目录（Rust 队列的 `dictionary-snapshot-staging`，或 `discardSnapshot:` 丢掉的 staging），不碰活动代次；窗口关闭时排队中的请求留在队列里。

## 本地验证

`snapshot-activation-test` 链接 `apple-client`，需要显式运行并传入一份与 `resources/desktop-dictionary.lock.json` 匹配的已校验资源目录：

```sh
cmake --build <build-dir> --target snapshot-activation-test
<build-dir>/snapshot-activation-test /absolute/path/to/verified-desktop-resources
```

它在新建的临时目录里准备用户数据和暂存快照，验证真实激活、英文模式开关的保留、导入词条的候选与提交，成功后删除测试状态，不修改传入的资源目录或实际用户词库。Rust 侧的单元测试在 `crates/host-api/src/dictionary_snapshot/tests.rs`，覆盖信封校验、记录边界、句柄生命周期与激活失败路径。

上游对照：MSIME-Windows 远端默认分支 `develop`，本文写作时查询到的头为 `04a8df56f86312474a069f4335a1b58da7afaa9e`。
