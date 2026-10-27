# Agent Note: 云剪贴板同一台设备显示成多条待批准

Status: implemented

## Problem

维护者报告：同一部手机在批准方的「等待允许」里出现 3 条，批准一条后其余的还在（#10028）。

服务端、参考中继和 WebDAV 日志都按 `device_id` 登记（msime-cloud 的 `user_clipboard_devices` 主键是 `(user_id, device_id)`，`crates/clipboard-relay/src/store.rs` 的 `register` 与 `crates/client-core/src/cloud/clipboard_log.rs` 的 `Register` 都是「已登记就只改名」），同一个设备编号不会出现两次；各平台的设置页也只是把驱动交回的 `devices` 原样画出来。所以 3 条是这台设备先后用过的 3 个身份，只有最后一个的私钥还在本机，另外两条永远等不到批准，也没有谁会删。

身份换掉有两类原因：

- 第一次使用时的并发写入。协议把密钥串的保存定义成「原子整体替换」。同一台设备上的两个进程，或者同一进程里同时开始的两个操作，都会读到「没有」、各自生成身份并在第一个请求之前保存、登记；后写的覆盖先写的，输掉的身份已经登记在服务端。Rust 的 `clipboard_e2e::run`（账号路径：Tauri 桌面端、Windows 原生设置、macOS 输入法经 Rust、HarmonyOS）和 Apple 共用的 `BackendAccountClient.clipboardE2E`（iOS 键盘扩展用它，App 经 Rust）都是这样。Android 原生的 `CloudClipboardKeystore` 早就拒绝覆盖另一个身份（`IdentityConflict`，见 [Android 自建中继](../feature/2026-10-12-android-cloud-clipboard-relay.md)），自建中继的 Rust 路径持整次操作的文件锁，这两处不受影响。匿名身份 `ANONYMOUS_IDENTITY` 曾因同样的原因改成「不存在才写」，云剪贴板密钥串没有跟上。
- 身份按设计丢失：退出登录、刷新令牌被服务端拒绝时随会话删掉密钥串，重装、清除数据，自建中继换地址或令牌（绑定变了就生成新身份）。这些本身是有意的，但旧登记从此只能由批准方处理，批准方却把它和新身份并排列出、名字一模一样。

维护者自己的 Mac 用水杉账号（本机偏好里没有 `cloud_clipboard_relay` / `cloud_clipboard_webdav`），手机是哪一条原因造成的 3 个身份没法从这边确认；两条都会造成同样的现象，所以两条都修。

## Decision

- 一个安装只有一个设备身份，保存不能替换另一个身份。`clipboard_e2e::run` 的 `persist` 改为返回 `Persisted`：存储里已有另一个解析得了的身份（`Keystore::other_identity`，比较密钥串开头的设备私钥）时答 `Conflict(存着的那份)`，`run` 在还没发出请求时改用它从头开始，发过请求之后才冲突就以 `clipboard_keystore_conflict` 失败。账号路径（`account/clipboard.rs`）在 `with_refresh_lock` 里先 `with_generation` 再读、比、写，与登录、刷新同一把跨进程锁、同一个加锁顺序。Apple 的 `BackendClipboardE2E.saveKeystore` 在 `withSessionStorage`（会话的跨进程锁）里做同样的检查，驱动循环同样重来或失败。做法与 Android 的 `CloudClipboardE2e.run` 一致，三处现在是同一条规则。
- 共享驱动认得「被同名设备盖住的旧登记」（`superseded_devices`）：还没被批准、不是本机，并且之后（设备列表按登记时间排序）又登记过一台同名的设备，两边的公钥都与编号相符。`status` 结果的 `devices` 和 `list` 的 `pending_devices` 不含它们。本机能读写时，`status`、`approve`、`reject`、`remove`、`reset` 在回报之前顺手以 `DELETE /devices/{id}` 删掉它们（`Phase::Prune`，每次操作一轮、最多 16 条，单条删不掉不影响操作），再取一次状态；`list` 不删，键盘只读。这条规则在驱动里，Android（经 JNI）、Windows 原生、共享 React 设置页、Apple、HarmonyOS 的界面都不用改。
- 已经积下的重复登记：升级后批准方看到的只有每个名字最新的那一条；打开设备页或点一次「允许」就把旧的从服务端删掉，不再占 20 台的名额。

## 契约

- `docs/cloud-clipboard-e2e.md` 的「宿主的职责」写明保存不能替换另一个身份、要在跨进程锁里读比写、冲突时重来或报 `clipboard_keystore_conflict`；「结果」写明 `devices` 与 `pending_devices` 不含被盖住的旧登记、哪些操作会删它们。`msime_client.h` 的 session store 读写语义补了一句：`CLIPBOARD_KEYSTORE` 的 save 本身仍是整体替换，Rust 只在 ACCOUNT 槽的锁里确认没有另一个身份之后才调用它，自己也写这一项的宿主要守同一条规则。
- 正在给 iOS、HarmonyOS 接中继和 WebDAV、以及复用这套批准流程做 [自建词库同步](../feature/2026-10-27-dictionary-self-hosted-sync.md) 的改动：自己跑驱动、自己存密钥串的宿主，保存要按上面的规则做（参考 Android 的 `CloudClipboardKeystore.save` 或 Apple 的 `saveKeystore`）；经 `clipboard_e2e::run` 的，`persist` 要如实答 `Conflict`，不能一律答 `Saved`，除非像 `RelayClipboard::drive` 那样整次操作持锁、开始时读到的就是保存前存着的那份。

## Alternatives considered

- **服务端去重**（按名字合并、或按 `last_seen` 过期没批准的登记）。服务端没有错：它看到的是几个真正不同的公钥，按名字合并要把猜测放进三份独立实现（msime-cloud、参考中继、WebDAV 日志）里，而且 WebDAV 根本没有服务端。`last_seen` 过期只能让旧登记过一段时间消失，不解决「批准方现在就看到三条」。所以不改服务端，msime-cloud 不开 PR。
- **设备离开时自己注销旧登记**（退出登录前 `DELETE /devices/{own_id}`）。最直接，但只覆盖退出登录这一条路径：重装、清除数据、令牌被吊销时本机没有机会发请求；而且已批准的设备服务端不让自己删（要换钥），换钥又会让其他设备全部重新批准。
- **退出登录时保留设备私钥、只删剪贴板密钥**，让身份跨登录不变。能少掉一大类旧登记，但改变了「退出后本机不再保留能解开云端条目的密钥」之外的另一项隐私性质：同一台设备在两个账号下会用同一个设备编号，服务端可以把两个账号关联起来。这是产品决定，这次不动。
- **只在界面上按名字折叠，不删登记**。改动更小，但旧登记一直占着 20 台的名额，而且一台真正还在用、只是和别的设备同名的设备会被永远藏起来；删掉之后它下次同步会重新登记、排到最后，就又能被看到。
- **按名字合并时连已批准的旧身份一起处理**。已批准的旧身份持有当前密钥的封装，删它要换钥（所有设备重新批准），藏起来又会让用户看不到一台能读剪贴板的设备，所以已批准的设备照常列出，由用户自己「移除」。

## Consequences

- 收益：同一台设备的并发首次使用只登记一个身份；批准方每个名字只看到一条待批准，批准一次即可，旧登记被删掉、不再占名额。所有平台的界面和三种后端（水杉账号、自建中继、WebDAV）一起生效。
- 代价：只按名字认是猜测。两台同名设备（iOS 16 起设备名都是「iPhone」，同型号的 Android 也同名）同时等待批准时，先登记的那台会被当成旧登记删掉，它下次同步时重新登记才再出现，要多等一轮。删除会多发几个请求，WebDAV 后端上每删一条是一个 `reject` 事件。
- `clipboard_keystore_conflict` 进了共享的错误码表，Apple 有对应的提示语；Android 原来就有这个码。
- 验证：`crates/client-services/src/account/clipboard_tests.rs` 的 `two_processes_starting_at_once_register_one_device` 在修之前失败（服务端 2 台设备），修之后通过；`crates/client-core/src/cloud/clipboard_e2e_tests.rs` 的 `a_phone_registered_three_times_waits_for_one_approval` 在去掉合并时失败（`pending_devices` 为 3），另有同名设备重新出现、待批准设备不删、`run` 冲突重来与中途冲突的用例；`shared/backend/Tests/BackendClipboardE2ETests.swift` 的两个用例覆盖 Apple 循环。没有在真机上复现维护者那部手机：不知道它的 3 个身份是哪条路径造成的，也没有在 iOS App 与键盘扩展同时首次使用的真机场景里跑过：键盘扩展的 Swift 检查在 `withSessionStorage` 的锁里，App 经 Rust 的检查在宿主注入存储的 ACCOUNT 槽锁里，没有核对这两把在 iOS 上是不是同一把锁；不是同一把时，两边各自的读比写之间仍有极短的窗口。
