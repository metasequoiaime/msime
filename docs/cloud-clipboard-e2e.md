# 云剪贴板端到端加密协议（v1）

云剪贴板的文字只在设备上加密和解密，服务端只保存密文。用户不需要设置或记住任何口令：每台设备有自己的密钥对，账号下的剪贴板密钥由已有设备在用户点「允许」后转交给新设备。这份文档是服务端（msime-cloud）、Rust 驱动（`crates/client-core/src/cloud/clipboard_e2e.rs`）和各平台客户端共同的契约，自建中继也按这份实现。设计取舍见 [决策笔记](../.agents/notes/implemented/feature/2026-10-11-cloud-clipboard-e2e.md)。

## 威胁模型

保护：数据库泄露、服务端运维人员读取库内容、服务端被攻破后悄悄登记一台设备来读取剪贴板（新设备必须由已有设备的用户点「允许」，弹窗里显示两边都能看到的核对码）。

不保护、并在隐私说明里写明的元数据：条目数量、写入时间、置顶状态、写入设备名、两条内容是否相同（去重标签相同），以及每台设备的名称和登记时间。

已知的残余风险：服务端可以给一台刚登录、还没被批准的设备发一把它自己生成的密钥。这台设备随后上传的内容服务端能读到，但其他设备解不开这些条目（密钥编号不同），所以会表现为「本设备上传的条目别的设备看不到」。v1 不防这一点，代价是让新设备也去核对批准者的身份。

## 编码约定

- `b64`：RFC 4648 §5 的 base64url，不带填充。
- `hex`：小写十六进制。
- `HKDF`：HKDF-SHA256；`AEAD`：XChaCha20-Poly1305（24 字节 nonce，16 字节 tag）；`DH`：X25519。
- 所有 `info`、`aad` 中的字符串都是 ASCII 字节，`‖` 表示拼接，`0x00` 是一个零字节。

## 设备身份

- 每台设备首次使用云剪贴板时随机生成 32 字节的 X25519 私钥，只保存在本机。
- `public_key` = `b64(公钥 32 字节)`，固定 43 个字符。
- `device_id` = `hex(SHA-256(公钥)[0..16])`，固定 32 个字符。服务端据此校验公钥与编号一致。
- 核对码 `code`：取 `SHA-256("msime-clipboard-v1-code" ‖ 公钥)` 的前 8 字节按大端读成无符号整数，对 10¹⁶ 取余，左补零到 16 位，每 4 位一组用空格分开，例如 `0421 9873 5530 1176`。批准时两台设备上显示的必须相同。

## 剪贴板密钥

- 账号同一时刻只有一把剪贴板密钥 `K`（32 字节随机数），由第一台开启云剪贴板的设备生成。
- `key_id` = `hex(HKDF(ikm=K, salt=空, info="msime-clipboard-v1-key-id", 16 字节))`，32 个字符。
- 条目加密密钥 `item_key` = `HKDF(K, info="msime-clipboard-v1-item", 32)`；去重密钥 `dedup_key` = `HKDF(K, info="msime-clipboard-v1-dedup", 32)`。
- 换钥（移除设备、重置）时生成新的 `K`，服务端同时删掉全部条目和旧的封装。云剪贴板最多 50 条、本身是临时内容，所以不做旧条目的重新加密。

## 条目

明文规则与之前相同：UTF-8，不能是空白，不超过 4000 个 UTF-16 单元，除制表符、换行和回车外不含控制字符。

- `nonce`：24 字节随机数。
- `aad` = `"msime-clipboard-v1-item" ‖ 0x00 ‖ key_id`
- `ciphertext` = `"v1." ‖ b64(nonce ‖ AEAD(item_key, nonce, 明文, aad))`，最长 16384 个字符（4000 个 UTF-16 单元的明文最多 12000 字节，编码后约 16057 个字符）。
- `dedup` = `hex(HMAC-SHA256(dedup_key, 明文))`，64 个字符。服务端用它代替原来的明文哈希去重。

## 密钥封装

把 `K` 交给设备 D（公钥 `P_D`，编号 `ID_D`）：

1. 生成临时私钥 `e`，公钥 `E`；`shared = DH(e, P_D)`，全零时视为失败。
2. `wrap_key = HKDF(ikm=shared, salt=E ‖ P_D, info="msime-clipboard-v1-wrap", 32)`
3. `aad = "msime-clipboard-v1-wrap" ‖ 0x00 ‖ key_id ‖ 0x00 ‖ ID_D`
4. `wrap` = `"v1." ‖ b64(E ‖ nonce ‖ AEAD(wrap_key, nonce, K, aad))`，固定 142 个字符。

收到封装的设备解开后必须重新计算 `key_id`，与服务端给的不同就当作没有拿到密钥。

## 服务端接口

都在 `https://api.msime.app` 下，需要登录。用户自建的中继实现同一组接口，挂在中继的基地址下、用中继令牌鉴权，差别见 [自建中继](cloud-clipboard-relay.md)。错误一律是 `{"error":{"code":"...","message":"..."}}`。

### `GET /v1/users/me/clipboard/keys?device_id=<id>`

```json
{
  "enabled": true,
  "key_id": "…" ,
  "wrap": "v1.…",
  "devices": [
    {"device_id": "…", "public_key": "…", "name": "MacBook", "created_at": "2026-10-11T08:00:00Z", "approved": true}
  ]
}
```

`key_id` 在还没有密钥时为 `null`；`wrap` 是当前密钥给请求里那台设备的封装，没有时为 `null`；`approved` 表示该设备持有当前密钥的封装。`devices` 按登记时间排序。`device_id` 缺失或格式不对返回 400 `invalid_device_id`。

### `PUT /v1/users/me/clipboard/devices/{device_id}`

请求体 `{"public_key": "…", "name": "…"}`。`name` 不超过 64 个字符、不含控制字符。已登记时只更新名称。公钥与编号不符、格式不对返回 400 `invalid_clipboard_device`。每个账号最多 20 台设备，满了时先淘汰最早登记、尚未被批准的设备，都已批准时返回 409 `clipboard_device_limit`。成功返回 204。

### `DELETE /v1/users/me/clipboard/devices/{device_id}`

删除设备和它的封装（用于「拒绝」一台待批准的设备）。不存在返回 404 `not_found`，成功返回 204。移除一台已批准的设备要换钥，见下一个接口的 `revoke`。

### `POST /v1/users/me/clipboard/keys`

```json
{"previous_key_id": null, "key_id": "…", "wraps": [{"device_id": "…", "wrap": "v1.…"}], "revoke": []}
```

在一个事务里：确认当前密钥编号等于 `previous_key_id`（没有密钥时为 `null`），不等返回 409 `clipboard_key_conflict`；删除 `revoke` 里的设备；删除全部条目和全部旧封装；写入新的 `key_id` 和 `wraps`。`wraps` 有 1 到 20 项，每个设备都必须已登记且不在 `revoke` 里，否则返回 400 `invalid_clipboard_key`。云剪贴板关闭时返回 403 `clipboard_sync_disabled`。成功返回 204。

### `PUT /v1/users/me/clipboard/keys/{key_id}/wraps/{device_id}`

请求体 `{"wrap": "v1.…"}`，批准一台设备。`key_id` 不是当前密钥返回 409 `clipboard_key_conflict`，设备没登记返回 404 `not_found`，封装格式不对返回 400 `invalid_clipboard_key`。成功返回 204。

### `GET /v1/users/me/clipboard`

```json
{
  "enabled": true,
  "retention_days": 0,
  "key_id": "…",
  "items": [
    {"id": "…", "ciphertext": "v1.…", "key_id": "…", "updated_at": "…", "pinned": false, "device": "MacBook"}
  ]
}
```

服务端不再支持搜索，查询参数 `q` 被忽略，搜索由客户端在解密后进行。最多 50 条，置顶的在前。

### `POST /v1/users/me/clipboard`

请求体 `{"ciphertext": "v1.…", "key_id": "…", "dedup": "…"}`，返回与列表中格式相同的条目。带非空 `text` 字段返回 400 `clipboard_e2e_required`；`key_id` 不是当前密钥（包括还没有密钥）返回 409 `clipboard_key_conflict`；格式不对返回 400 `invalid_clipboard_ciphertext`；云剪贴板关闭时返回 403 `clipboard_sync_disabled`。`dedup` 相同的条目只保留一条并移到最前。

### 不变的接口

`DELETE /v1/users/me/clipboard`（清空）、`DELETE /v1/users/me/clipboard/{id}`、`PUT /v1/users/me/clipboard/{id}/pin`、`PUT /v1/users/me/clipboard/retention` 的格式不变。`PUT /v1/users/me/clipboard/settings` 关闭云剪贴板时，除了条目，还会删除密钥、封装和全部设备登记；`DELETE /v1/users/me/data` 选了剪贴板时同样删除这四样。

### 迁移

这一版上线时服务端删除全部明文条目，此后只接受密文。旧版本客户端的云剪贴板会显示加载失败或上传失败，直到升级。

## 客户端驱动

流程逻辑只在 Rust 里实现一份，采用 sans-IO 的形式：Rust 决定下一步发什么请求，宿主负责把请求发出去并把结果交回。C ABI 入口是 `msime_client_cloud_clipboard_e2e(request, length)`，Android 经 JNI、HarmonyOS 经 NAPI、Apple 平台经宿主注入的闭包调用同一个函数；桌面端在 Rust 里直接驱动。

### 输入

开始一个操作：

```json
{"start": {"op": "list", "search": ""}, "keystore": null, "device_name": "Pixel 8"}
```

`op` 是 `list`（带可选的 `search`）、`add`（带 `text`）、`status`、`approve` / `reject` / `remove`（带 `device_id`）或 `reset`。`keystore` 是本机保存的密钥串，第一次使用时为 `null`。

继续一个操作：

```json
{"resume": <上一步的 state>, "response": {"status": 200, "body": "<响应正文>"}}
```

传输失败（断网、超时）不交回驱动，由宿主按原来的方式报错。

### 输出

`C ABI` 外层仍是 `{"ok": true, "value": …}`；`value` 是下面三种之一，任何一种都可能带 `keystore`，宿主看到就立即保存（替换旧值）：

- 要发请求：`{"request": {"method": "GET", "path": "/v1/…", "body": "<JSON 文本>"|null}, "state": {…}, "keystore": "…"}`。请求一律带登录凭据。
- 完成：`{"done": <结果>, "keystore": "…"}`。
- 失败：`{"error": {"status": 409, "code": "clipboard_key_conflict"}, "keystore": "…"}`。`status` 为 0 表示不是服务端返回的错误（例如本机还在等待批准时上传，`code` 为 `clipboard_device_pending`）。

### 结果

- `list`：与旧接口相同的明文形状，宿主原有的解析可以不改：`{"enabled", "retention_days", "items": [{"id", "text", "updated_at", "pinned", "device"}], "e2e": {"status", "code", "pending_devices", "undecryptable"}}`。`e2e.status` 是 `ready`、`pending`（本机等待批准，`items` 为空）或 `disabled`；`pending_devices` 是等待本机批准的设备数，设置页据此提示；`undecryptable` 是解不开而被略去的条目数。
- `add`：一条明文条目 `{"id", "text", "updated_at", "pinned", "device"}`。
- `status`、`approve`、`reject`、`remove`、`reset`：`{"enabled", "status", "device_id", "code", "devices": [{"device_id", "name", "created_at", "approved", "current", "code"}]}`，`current` 标出本机。

### 宿主的职责

- 保存 `keystore`：每台设备（每个应用安装）只有一份，输入法进程、设置进程和 Tauri 设置页读写同一份，否则同一台设备会在服务端登记成几台、要批准几次。存放位置见下一节，写入一律原子替换，退出登录时与会话一起删除。配置了自建中继时，已经接上中继的宿主（目前是 Tauri 桌面端的设置页和云剪贴板面板）只走中继，密钥串另存一份、退出登录不删除，见 [自建中继](cloud-clipboard-relay.md) 的「本机的密钥串」。
- 设置页显示待批准的设备和核对码，提供「允许」「拒绝」「移除」和「重置云剪贴板」；本机待批准时显示自己的核对码，并说明去哪里批准。移除和重置之后其他设备都要重新批准，确认框里要写明。
- 键盘只用 `list` 和 `add`；本机待批准时显示一句提示，不上传。
- 置顶、保留期、删除、清空和开关仍直接调用原来的接口，不经过驱动。

### 密钥串的存放位置

| 平台 | 位置 | 共用者 |
| --- | --- | --- |
| macOS | 与 `account-session.json` 同一目录下的 `cloud-clipboard-keystore`，仅本人可读写 | 输入法、设置应用（Swift）与 Tauri 面板（Rust） |
| Windows、Linux | 与 `account-session.json` 同一目录下的 `cloud-clipboard-keystore`，仅本人可读写 | 设置窗口与面板（Rust） |
| iOS | 钥匙串通用密码项，service `app.msime.backend.account`，account `cloud-clipboard-keystore`，与账号会话同一个 App Group 访问组 | App、键盘扩展与 Tauri 设置页 |
| Android | 应用私有目录下的 `cloud-clipboard-keystore`，与账号会话一样先用 AndroidKeyStore 加密 | 主进程、`:ime` 进程与 Tauri 设置页 |
| HarmonyOS | 与账号会话同一个应用私有目录下的 `cloud-clipboard-keystore` | 设置应用与键盘 |
| 自建中继（目前是 Tauri 桌面端） | 与 `account-session.json` 同一目录下的 `cloud-clipboard-relay-keystore`，记着它属于哪个中继和令牌空间，仅本人可读写 | 设置页与云剪贴板面板（Rust） |

文件里只有密钥串本身；读取时忽略末尾的换行，否则手工编辑或换行风格不同的写入会让密钥串解析失败，驱动随之生成新的设备身份，这台设备就要重新批准。

Tauri 移动端（iOS、Android）经平台插件读写：`loadCloudClipboardKeystore` 返回 `{"value": string|null}`，`saveCloudClipboardKeystore` 的参数是 `{"value": string}`。没有单独的清除命令，插件的 `clearSession` 同时删除密钥串。

### 共享面板的动作

共享 React 面板（Windows、Linux、macOS 的 Tauri 面板、HarmonyOS 设置页、移动端 Tauri 设置页）经各宿主的 `cloud_clipboard_request` 发出 `CloudClipboardAction`。原有的 `list`、`add`、`delete`、`set_enabled` 不变，`list` 的结果多出 `retention_days`、`e2e` 和条目的 `pinned`、`device`；新增的动作是：

- `{"operation": "devices"}`：驱动的 `status`。
- `{"operation": "approve" | "reject" | "remove", "device_id": "…"}`、`{"operation": "reset"}`：对应驱动的同名操作。

这五个动作的结果都是驱动 `status` 的结果形状。驱动返回的错误按宿主原有的错误通道交给面板，错误码保持驱动给出的 `code`（例如 `clipboard_device_pending`、`clipboard_sync_disabled`、`clipboard_key_conflict`）。macOS 的 Tauri 面板经 IPC 交给原生 provider，失败时的回复是 `{"ok": false, "error": "<code>", "status": <n>}`。

Linux 上由用户自己运行的 socket provider（Fcitx5 与设置窗口经 `msime_client_cloud_clipboard_provider_request` 转发给它）同样会收到这五个动作，它自己持有凭据和网络，要按本文档实现加密，并以驱动的结果形状应答，失败时回 `{"ok": false, "error": "<code>"}`。
