# 云剪贴板的 WebDAV 后端

云剪贴板除了水杉账号和 [自建中继](cloud-clipboard-relay.md)，还可以存在用户自己的 WebDAV 网盘上（坚果云、Nextcloud 等）。网盘上没有能运行的服务端逻辑，所以客户端在本机扮演一个「虚拟中继」：[端到端加密](cloud-clipboard-e2e.md) 的驱动照旧发中继的那 12 个接口，虚拟中继不把它们发给任何服务端，而是读写网盘上的一份只追加的事件日志来回答，语义与参考中继（`crates/clipboard-relay`）相同。协议、加密、密钥串、设备批准和核对码都不变，网盘只见到密文。设计取舍见 [决策笔记](../.agents/notes/implemented/feature/2026-10-26-cloud-clipboard-webdav.md)。

这份文档的「存储布局」一节是契约：任何实现（各平台的客户端、以后的工具）都按它读写网盘上的文件。

## 客户端怎么配置

偏好文档里的 `cloud_clipboard_webdav`：

```json
{"cloud_clipboard_webdav": {"url": "https://dav.jianguoyun.com/dav/msime", "username": "<网盘账号>", "password": "<应用密码>"}}
```

- 三项要么都空（不用网盘），要么都合规；没配置时这个键不写进文档，旧版本照样能读。与 `cloud_clipboard_relay` 只能二选一，两者同时配置的偏好存不进去。配置了网盘就只走网盘，失败时不退回水杉账号。
- `url` 是用户选的文件夹，规则与 [中继地址](cloud-clipboard-relay.md) 相同：`https://`（`http://` 只接受本机回环地址），不带查询串、片段和用户信息，不跟随重定向；TLS 用 rustls 和内置的 webpki-roots，不认自签证书和私有 CA。
- `username` 是网盘账号（坚果云是登录邮箱），1 到 256 字节，不含控制字符和冒号；`password` 是应用密码（坚果云、Nextcloud 都要求第三方应用用应用密码而不是登录密码），1 到 256 字节，不含控制字符。应用密码列在凭据清单里，账号在诊断快照里也换成 `<redacted>`；整组不随设置文件导出、不进账号设置同步，MCP 的偏好白名单也不包括它。
- 保存之前先「测试连接」（见下文），不通过不保存。
- 本机密钥串仍是中继那份 `cloud-clipboard-relay-keystore`，只是它绑定的空间换成 `sha256("webdav\0" ‖ 基地址 ‖ "\0" ‖ 账号)` 的前 16 字节：换了地址或账号就生成新的设备身份，要重新批准；换应用密码不换，不用重新批准。

## 宿主怎么接入

- `msime_client_cloud_clipboard_source` 对网盘也答 `{"source":"relay"}`，另带 `"backend":"webdav"`（中继是 `"relay"`）。已有的宿主把不认识的来源当作错误，所以不加新的来源值；`msime_client_relay_cloud_clipboard` 按偏好里的配置自己分派，Tauri 桌面端的设置页和云剪贴板面板、Windows 原生设置窗口、macOS 输入法进程、Linux Fcitx5 键盘栏这些已经接上中继的入口不用改就能走网盘。宿主只在给用户说明「现在经哪里同步」时看 `backend`。
- 自己跑驱动、自己保存密钥串的宿主（Android）经 `msime_client_relay_cloud_clipboard_exchange` 发请求，网盘时传 `{"backend":"webdav", url, username, password, directory, method, path, body}`；`directory` 放日志的本机缓存和锁文件，要与 `msime_client_relay_cloud_clipboard` 用的那个目录相同（Android 是 `noBackupFilesDir/cloud-clipboard-relay`，里面只有密文和公开数据）。Android 的绑定（键盘用来认出「换过了」）是 `webdav:` 加地址和账号的摘要，同样不含应用密码。
- 「测试连接」是 `msime_client_cloud_clipboard_webdav_probe`（Tauri 桌面端是命令 `cloud_clipboard_webdav_probe`，Android 是 `NativeClient.cloudClipboardWebDavProbe`），带设置页里还没保存的草稿；给了本机密钥串的目录时，通过后删掉那里的日志缓存。格式见 `crates/host-api/include/msime_client.h`。
- Android 和 iOS 的 Tauri 外壳只经水杉账号答，配置了网盘时与配置了中继一样拒绝（`relay_unsupported`）。iOS 和 HarmonyOS 的原生宿主还没有接中继，也就没有网盘。

## 实现在哪里

- `crates/client-core/src/cloud/clipboard_log.rs`：不碰网络的日志核心。事件格式、按序号重放得出状态（与参考中继的 `RelayStore::handle` 一一对应，单元测试照着中继的测试逐条移植）、请求到「要写的事件和写成后的响应」的映射、封口和快照。请求的解析和不依赖状态的校验也在这里，参考中继直接用它，两边的错误码不会漂移。
- `crates/client-services/src/clipboard_webdav.rs`：WebDAV 传输（reqwest 阻塞客户端上的 PROPFIND、GET、PUT、MKCOL、DELETE、MOVE，quick-xml 读多状态响应）、同步与写入、压缩、限流退避、本机缓存和「测试连接」。
- `crates/client-services/src/clipboard_relay.rs`：`RelayConfig::webdav` 和 `RelayTransport` 两种后端都能装，`RelayClipboard` 的其余部分不分后端。

## 存储布局

用户选一个文件夹作基地址（例如坚果云的 `https://dav.jianguoyun.com/dav/msime`），客户端只在它下面的 `msime-clipboard/` 里读写：

```text
<基地址>/msime-clipboard/v1/format.json
<基地址>/msime-clipboard/v1/tmp/<Unix 秒>-<32 位十六进制>.json
<基地址>/msime-clipboard/v1/epoch-NNNNNN/snapshot.json
<基地址>/msime-clipboard/v1/epoch-NNNNNN/e-NNNNNNNNNNNN.json
<基地址>/msime-clipboard/probe-<16 位十六进制>/      （只在「测试连接」时短暂存在）
```

- `format.json` 是 `{"format":"msime-clipboard","version":1}`。不认识的格式或版本，客户端报 `webdav_unsupported`，不读也不写。
- 文件名里只有序号和 epoch 编号，不出现设备编号、条目编号或任何内容。
- 客户端从不删除 `msime-clipboard/`、`v1/` 和 `tmp/` 本身。坚果云不许删除直接位于网盘根目录下的文件夹（答 403，那是它的「同步文件夹」），用户把基地址选在根目录时 `msime-clipboard/` 就是这样一个文件夹；会被删除的只有它下面的 epoch 目录、临时文件和探测目录。

### 事件

每个改变状态的请求写成一个不可变的事件文件 `e-<序号>.json`，序号是 12 位、左补零的十进制，跨 epoch 连续递增。内容是一个 JSON 对象：

| 字段 | 含义 |
| --- | --- |
| `seq` | 序号，必须与文件名相同 |
| `prev` | 上一个事件文件字节的 SHA-256（小写十六进制）；第一个 epoch 里第一个事件的 `prev` 是 64 个 `0` |
| `nonce` | 16 字节随机数的十六进制，同样的请求写出的文件也各不相同 |
| `at` | 写入设备的 Unix 秒数，登记时间和条目的 `updated_at` 取自它 |
| `op` | 操作，其余字段随操作而定 |

`op` 与中继的写接口一一对应，字段就是那个请求的字段：

| `op` | 字段 | 对应的接口 |
| --- | --- | --- |
| `register` | `device_id`、`public_key`、`name` | `PUT /devices/{device_id}` |
| `reject` | `device_id` | `DELETE /devices/{device_id}` |
| `rotate` | `previous_key_id`、`key_id`、`wraps`、`revoke` | `POST /keys` |
| `approve` | `key_id`、`device_id`、`wrap` | `PUT /keys/{key_id}/wraps/{device_id}` |
| `add` | `id`（新条目的编号，64 位十六进制，写入的设备随机生成）、`ciphertext`、`key_id`、`dedup` | `POST /clipboard` |
| `clear` | 无 | `DELETE /clipboard` |
| `delete` | `id` | `DELETE /clipboard/{id}` |
| `pin` | `id`、`pinned` | `PUT /clipboard/{id}/pin` |
| `retention` | `days` | `PUT /clipboard/retention` |
| `settings` | `enabled` | `PUT /clipboard/settings` |
| `seal` | `next_epoch` | 封口，见下文「压缩」 |

状态由事件按序号依次重放得出，规则与参考中继完全相同：新空间默认开启；设备最多 20 台，满了淘汰最早登记、尚未批准的一台；「已批准」是持有当前密钥的封装；换钥比较 `previous_key_id`，不等的不生效；`dedup` 相同的上传保留原来的条目编号并移到最前；最多 50 条，全部置顶时新的上传不生效；关闭开关删掉条目、密钥、封装和全部设备登记。

- 解析不了、序号与文件名不符、请求本身不合规、或者在当时的状态上会被拒绝的事件，照样占用这个序号、接上哈希链，但不改变状态。写入的设备在写之前已经在同一个状态上试过，被拒绝的请求不会写成事件；这条规则只是让一个坏文件不会卡住所有设备。
- 过了保留期的未置顶条目：读取时滤掉；写 `add` 和 `seal` 时按事件的 `at` 删掉。

### 写入：抢下一个序号

写入的设备在它读到的最后一个序号 `n` 上处理请求，把要写的事件独占创建为 `e-<n+1>.json`。谁先创建出这个文件，谁的写入就算数；没抢到的设备重读日志，在新的状态上再处理一次（比如另一台设备刚换过钥，这时比较 `previous_key_id` 就会失败，驱动收到 409 `clipboard_key_conflict` 后重取状态）。

独占创建的做法：先 `PUT` 到 `tmp/` 下一个唯一的临时名，再 `MOVE` 到目标并带 `Overwrite: F`。`MOVE` 答 201 或 204 是抢到了；答 412（Nextcloud）或 409（坚果云对已存在的目标答 409；目标所在的 epoch 目录不存在时各家也答 409）是没抢到，临时文件随即删掉。不用 `PUT If-None-Match: *`：坚果云不理会这个头，并发的 PUT 全都答 204 并互相覆盖。

### 读取

客户端在本机记着读到的 epoch、序号、链头和由它算出的状态。一次读取是对当前 epoch 目录的一个 `PROPFIND`（Depth 1），然后按序号 `GET` 新出现的事件文件、依次接上：

- 下一个事件的 `prev` 必须等于本机记得的链头，否则报 `webdav_rollback`，不接受（见下文「防回退」）。
- 列出来的序号中间有缺口（网盘的列表还没跟上）时，读到缺口为止，下次再读。
- 读到 `seal` 就进入下一个 epoch；当前 epoch 目录已经不存在（别的设备压缩后删掉了）时，从网盘上最新的快照重新开始。

### 压缩

一个 epoch 里攒了 64 个以上的事件、有条目过了保留期、或者最近一次写入是清空、换钥或关闭开关时，写入的设备顺手压缩：

1. 写一个 `seal` 事件（`next_epoch` 是当前 epoch 加一），和普通事件一样抢下一个序号。抢不到就算了，下次再说。
2. `MKCOL` 下一个 epoch 的目录。
3. 独占创建它的 `snapshot.json`：`{"format":"msime-clipboard","version":1,"epoch":…,"seq":…,"head":…,"state":{…}}`，`seq` 和 `head` 是封口事件的序号和哈希，`state` 是封口那一刻的完整状态。内容由日志唯一决定，几台设备同时写出的是同一份字节，谁写成都一样。封口的设备中断时，下一台读到封口的设备补上第 2、3 步。
4. 新快照写好 120 秒之后，下一次操作删掉旧的 epoch 目录，顺带删掉放了 10 分钟以上的临时文件（写入的设备在 `PUT` 和 `MOVE` 之间中断留下的）。

新设备（或者本机缓存没了）从版本最大、带快照的 epoch 开始读，只下载快照和它之后的事件。快照来自网盘、谁都能改，每个字段都按请求的规则核对一遍才用。

## 测试连接

`clipboard_webdav::probe` 在用户选的文件夹里试一遍日志要用的网盘行为，不合格是 `webdav_unsupported`。它在 `msime-clipboard/` 下建一个随机名字的探测目录，然后：

1. 独占创建一个文件，必须成功；同名再创建一次，必须失败，文件内容还是第一次的。
2. 刚写的文件马上要能在 `PROPFIND` 里列出来。
3. 四个写入同时独占创建同一个名字，只能有一个成功，文件内容是它写的。
4. 删掉探测目录。

账号或应用密码不对是 `relay_unauthorized`（与中继令牌错误同一个码，宿主不会当成水杉账号未登录），连不上是 `relay_unavailable`。测试连接大约发 20 个请求。它证明网盘有日志需要的这些语义，证明不了在任何负载下都是原子的；独占创建不可靠的网盘会在后续读写里以 `webdav_rollback` 暴露出来（两台设备都以为自己写成了同一个序号，后来的事件接不上其中一台的链头）。

## 本机缓存

`cloud-clipboard-webdav-state` 放在中继密钥串的同一个目录里（桌面端与 `account-session.json` 同一目录），仅本人可读写、原子替换写入。里面是读到的位置、链头、算出的状态（只有密文和公开数据：设备公钥和名称、封装、条目密文）和待删的旧 epoch，绑定基地址和账号；换了地址或账号就当作没有。同一台设备上的几个进程经 `cloud-clipboard-relay.lock` 串行。

驱动的一个操作要连着发两三个读请求，两秒内读过的缓存直接用，不再去网盘。写入不受这个影响：独占创建保证了写在最新的位置上，被拒绝或不改变状态的写入只在这一次刚读过网盘时才直接回答。

## 防回退

能写网盘的人（包括已经被移除、但还知道应用密码的设备）可以删掉事件，比如删掉一次换钥，想让旧密钥复活。客户端记着读到的位置：

- 新事件必须接在本机记得的链头后面；
- 从快照重新开始时，epoch、序号和密钥代数（换钥和关闭开关各算一次）都不许比本机记得的小，序号相同时链头也要相同；
- 本机读到过事件，而网盘上的 `msime-clipboard/v1/` 不见了时，不会从头再建一份。

不满足时报 `webdav_rollback`。如果是用户自己清理了网盘上的文件夹，在设置里重新「测试连接」并保存（它在通过后删掉本机的 `cloud-clipboard-webdav-state`），就会从网盘现在的样子重新开始。这只防「改写本机已经读过的历史」：知道应用密码的人仍然可以往后追加事件，比如用一把它自己生成的密钥换钥、封装给所有设备的公钥，这与自建中继的令牌泄露、官方服务端被攻破时的风险相同（见 [端到端加密](cloud-clipboard-e2e.md) 的威胁模型）。所以移除一台设备之后要在网盘上换一个应用密码，并且最好给水杉输入法单独开一个应用密码。没有本机缓存的新安装没有这层保护。

## 请求量和限制

- 打开面板或刷新：一个 `PROPFIND` 加上新事件的 `GET`。上传一条：一个 `PROPFIND`（缓存过期时）加一个 `PUT` 和一个 `MOVE`。压缩另外约 6 个请求。
- 坚果云免费账号每 30 分钟 600 个请求，与这个账号授权的其他第三方应用共用。网盘答 429 或 503 时按 `Retry-After` 退避（没有时 1、2、4 秒），最多重试三次；`Retry-After` 超过 8 秒或仍被限流时报 `webdav_rate_limited`，不让输入法和设置页挂着等。
- 坚果云一次 `PROPFIND` 最多列 750 项；压缩让一个 epoch 目录里通常不超过 66 项。
- 没有实时推送，也没有后台轮询：打开面板、刷新、复制时才同步。

## 网盘能看到什么

条目内容、去重标签的明文、剪贴板密钥都在本机加密，网盘看不到。网盘上能看到的元数据与 [端到端加密](cloud-clipboard-e2e.md) 的威胁模型里列的相同：条目数量和写入时间、置顶状态、两条内容是否相同、每台设备的名称、公钥和登记时间、保留期，另外还有每个写入的时间（事件的 `at`）和次数。

- 删除、清空、换钥之后，旧的密文在压缩后从网盘上删掉，但网盘的回收站和历史版本可能还留着它们（坚果云、Nextcloud 都有）。密文没有密钥解不开，介意的话可以去网盘清空回收站。
