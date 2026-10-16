# 云剪贴板的自建中继

云剪贴板可以不经过水杉账号和 `api.msime.app`，改走用户自己部署的中继。中继实现的是 [端到端加密协议](cloud-clipboard-e2e.md) 里同一组接口，只保存密文：文字照旧在设备上加密，新设备照旧要在已有设备上核对后点「允许」，中继拿不到剪贴板密钥。这份文档只写中继和官方服务不同的地方，接口的请求和响应以端到端加密协议为准。设计取舍见 [决策笔记](../.agents/notes/implemented/feature/2026-10-12-cloud-clipboard-relay.md)。

## 客户端怎么配置

偏好文档里的 `cloud_clipboard_relay`：

```json
{"cloud_clipboard_relay": {"url": "https://relay.example.com/msime", "token": "<中继令牌>"}}
```

- 两项要么都空（不用中继，走水杉账号），要么都合规。没配置时这个键不写进文档。
- `url` 是基地址：`https://`，可以带路径前缀，不能带查询串、片段和用户信息，不超过 2048 字节。`http://` 只接受 `localhost`、`127.0.0.1` 和 `[::1]`，给本机测试用。客户端把基地址规范化（主机名小写、去掉默认端口和末尾的 `/`）后拼上接口路径，例如 `https://relay.example.com/msime/v1/users/me/clipboard/keys?device_id=…`。
- 客户端的 TLS 用 rustls 和内置的 webpki-roots 根证书，不读系统信任库。中继必须使用公网可信的证书（例如 Let's Encrypt；只在局域网里用的主机名可以走 DNS-01 签发），自签证书、企业 CA 和私有 CA 都连不上。
- `token` 是中继管理员发的令牌，16 到 256 个可打印 ASCII 字符，不含空格。它和语音、翻译服务的凭据一样明文存在本机的偏好文档里，列在凭据清单（诊断快照里换成 `<redacted>`），不随设置文件导出，不进账号设置同步。
- 配置了中继，Tauri 设置窗口和云剪贴板面板、Windows 原生设置窗口、Android 原生宿主的设置页和键盘的请求就只走中继，不会在中继失败时退回水杉账号。还没接上中继的原生入口不看这个键，在 macOS 上手工写进它会让两条路并存，见下文「支持的宿主」里的注意事项。

Windows 原生设置页、Linux 共享设置页「剪贴板」的「云剪贴板同步方式」和 Android「我的 → 云剪贴板」页顶部的「云剪贴板同步方式」可以填写这两项，其他宿主还不显示（见下文「支持的宿主」）。

## 鉴权

每个请求带 `Authorization: Bearer <令牌>`。每个令牌对应中继上一个独立的剪贴板空间，空间之间互相看不到设备和条目。中继没有登录、刷新令牌这些概念：令牌不认识或缺失时返回 401，客户端把它报成 `relay_unauthorized`，提示用户检查令牌，不会当成水杉账号未登录。

## 接口

挂在 `<基地址>/v1/users/me/clipboard` 之下，与端到端加密协议完全同名同形：

- 驱动用的 7 个：`GET /keys?device_id=`、`PUT` 和 `DELETE /devices/{device_id}`、`POST /keys`、`PUT /keys/{key_id}/wraps/{device_id}`、`GET` 和 `POST /clipboard`。
- 直接调用的 5 个：`DELETE /clipboard`（清空）、`DELETE /clipboard/{id}`、`PUT /clipboard/{id}/pin`（`{"pinned": bool}`）、`PUT /clipboard/retention`（`{"days": 0|1|7|30}`，0 表示一直保留）、`PUT /clipboard/settings`（`{"enabled": bool}`，回答 `{"enabled": bool}`）。

与官方服务不同或需要中继自己保证的地方：

- 一个新空间默认就是开启的，第一台设备直接建钥。官方服务的新账号要先打开开关。
- 条目的 `id` 必须是 64 位小写十六进制，客户端会校验。
- 条目的 `device` 为空字符串：中继没有登录会话，认不出写入的是哪台设备。
- 上限与官方相同：每个空间 50 条（置顶的优先保留）、20 台设备、密文 16384 个字符。50 条都已置顶时，去重标签不落在置顶条目上的新上传答 409 `clipboard_full`，不挤掉置顶的条目。
- 过了保留期的未置顶条目要删掉。参考实现在每次读写条目时顺手清理。
- 错误一律是 `{"error":{"code":"…","message":"…"}}`，错误码与端到端加密协议相同。

## 本机的密钥串

中继的密钥串和水杉账号的那份分开存放，文件名是 `cloud-clipboard-relay-keystore`。Rust 存的那份（桌面端）内容是 `{"relay": 规范化的基地址, "space": 令牌摘要的前 16 字节, "keystore": 驱动的密钥串}`，仅本人可读写，原子替换写入；Android 的那份由宿主自己存，见下表。地址或令牌换了，旧的密钥串就不再使用，驱动生成新的设备身份，这台设备要在新空间里重新登记、重新被允许。退出水杉账号不删除它。同一台设备上几个进程第一次使用时，经 `cloud-clipboard-relay.lock` 串行，不会各自登记出一台设备。

| 宿主 | 位置 |
| --- | --- |
| Windows、Linux、macOS 的 Tauri 设置页与云剪贴板面板 | 与 `account-session.json` 同一目录 |
| Android 原生宿主（设置页和 `:ime` 进程里的键盘共用） | 应用私有的 `noBackupFilesDir/cloud-clipboard-relay/`，由宿主用 AndroidKeyStore 里不导出的密钥加密（与账号那份同样做法），不进云备份和设备间迁移 |

桌面端这份比账号路径的密钥串弱：它是应用私有目录里的明文文件，只靠文件权限保护。Android 不用它：宿主自己跑驱动，中继的密钥串与账号那份一样用 AndroidKeyStore 加密（内容是中继的绑定加密钥串，绑定不同就当作没有），只借 `msime_client_relay_cloud_clipboard_exchange` 发请求。

## 支持的宿主

- Tauri 桌面端（Windows、Linux、macOS）的 `cloud_clipboard_request` 按这个顺序选路：用户显式配置的 provider socket，然后是中继，然后是 macOS 输入法的原生会话、自动发现的 socket，最后是水杉账号。
- Linux Fcitx5 键盘栏读取列表时也遵循显式 socket、中继、自动发现的 socket 的顺序，选路和中继网络请求均在后台运行；中继出错不转发给其他服务。中继密钥串与 Linux 设置面板共用共享偏好目录。地址、令牌、偏好目录或 socket 变化会清空键盘栏中的旧条目，并丢弃旧请求的结果；设备批准和其他管理操作继续在云剪贴板面板完成。
- 原生宿主经 C ABI 接入：`msime_client_cloud_clipboard_source` 告诉宿主该走中继还是账号，`msime_client_relay_cloud_clipboard` 在 Rust 里跑完一个动作，比账号路径多出置顶（`pin`）、保留期（`set_retention`）和清空（`clear`）三个动作；自己保存密钥串的宿主改用 `msime_client_relay_cloud_clipboard_exchange`，自己跑驱动，每一步的请求由它发给中继（宿主传入打开时的地址和令牌，它只发这一个请求、原样交回状态码和正文）。格式见 `crates/host-api/include/msime_client.h`。
- Android 原生宿主经 JNI 接上：「我的 → 云剪贴板」页、键盘剪贴板面板的「云端」（列表、插入、「发到云剪贴板」）都先问 `msime_client_cloud_clipboard_source`，选了中继就只走中继。端到端加密的操作由宿主自己跑驱动（`msime_client_cloud_clipboard_e2e`，与它的水杉账号路径同一个循环），密钥串用 AndroidKeyStore 加密保存，每个请求经 `msime_client_relay_cloud_clipboard_exchange` 由 Rust 发给中继（只发一个请求，不读写密钥串）；开关、置顶、保留、删除和清空经 `msime_client_relay_cloud_clipboard` 直接请求。键盘按偏好快照里的地址和令牌认出中继换过，换过之后旧的条目不再插入、上传不发出。配置了中继时账号页「云剪贴板」那一行不显示条数，也不去问水杉账号。Android 和 iOS 的 Tauri 外壳（Android 的是 `build-client-apk.sh` 的开发合包，不是发布包）只能经水杉账号答云剪贴板，配置了中继时共享设置页的「发到云剪贴板」和共享云剪贴板面板一律拒绝（`relay_unsupported`），不改走账号。
- Windows 原生 WinUI 设置窗口的「剪贴板」页提供同步方式、地址和遮罩令牌，点击「保存同步方式」后由共享 Rust 一起校验和保存；切回账号会删除中继配置。「发到云剪贴板」在后台查询所选来源，中继请求失败或来源读取失败不走账号；`relay_unauthorized` 显示令牌错误。中继密钥串与 Tauri 云剪贴板面板共用账号目录。配置变化后旧请求结果失效。
- 设置页的中继字段目前在 Windows、Linux 和 Android 显示。macOS 输入法菜单的「云剪贴板…」在未登录时仍退回原生窗口，iOS 和 HarmonyOS 的原生宿主还没有接上这两个 C ABI，这些都在后续的改动里做。
- **注意**：在 macOS 上，下面这些入口还不看 `cloud_clipboard_relay`，登录了水杉账号时仍经账号上传到 `api.msime.app`（内容照旧端到端加密，但元数据留在官方服务上）：macOS 输入法表情面板里剪贴板历史的「发到云剪贴板」与原生账号窗口里的云剪贴板（`BackendCloudClipboardProvider`），以及未登录时「云剪贴板…」菜单退回的原生窗口。所以 macOS 设置页暂不显示中继字段；在 macOS 上手工写偏好启用中继的，先退出水杉账号，这些入口就只会提示未登录、不会上传。

## 参考实现

`crates/clipboard-relay`（包名 `msime-clipboard-relay`）是一个单文件 SQLite 存储、只说 HTTP/1 的参考中继。协议以文档为准，官方服务（msime-cloud）是另一份独立实现，两边各自用测试锁住同一份文档；这个 crate 的 `tests/flow.rs` 用真正的客户端驱动连它跑完整流程。

```sh
cargo build --release -p msime-clipboard-relay
target/release/msime-clipboard-relay gen-token >> tokens      # 每行一个令牌，一个令牌一个空间
target/release/msime-clipboard-relay --listen 127.0.0.1:8787 --data relay.db --tokens tokens
```

- 令牌文件里空行和 `#` 开头的行忽略；改了令牌文件要重启。数据库只存 SHA-256(令牌)，不存令牌本身。
- 请求体上限 64 KiB，上传一条的请求体上限 32 KiB。
- 中继只说 HTTP，必须放在终止 TLS 的反向代理后面，监听地址不要直接暴露到公网。例如 Caddy 把 `/msime` 前缀去掉再转给中继：

```
relay.example.com {
	handle_path /msime/* {
		reverse_proxy 127.0.0.1:8787
	}
}
```

客户端里填 `https://relay.example.com/msime`。
