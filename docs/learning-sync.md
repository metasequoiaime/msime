# 输入习惯同步协议（v1）

输入习惯同步把一台设备上学到的候选顺序（内置词的调频和阶梯降权、删除记录、置顶、固定位置）带到同一账号的其他设备上。内容在设备上压缩、补齐、加密后才上传，每台设备在服务端有自己的一份密文，服务端只保存不透明的字节，读不到内容。这份文档是服务端（msime-cloud）、Rust 驱动（client-core、engine、client-services、host-api）和各平台客户端共同的契约。设计取舍、分期和被否掉的方案见 [决策笔记](../.agents/notes/proposed/feature/2026-10-27-learning-sync-e2e.md)。

本协议还没有实现。各部分按决策笔记里的分期落地，落地时改动这份文档，不另起一份。

## 与其他同步的关系

- 账号的「词库」同步（Android 的「云同步」）上传的是记为用户词的词条，明文，服务端可读。输入习惯同步不碰这些行：用户词归词库同步管，下文合并规则里写明远端行从不覆盖本机的用户词。
- 输入习惯同步是 [云剪贴板端到端加密协议](cloud-clipboard-e2e.md) 的又一个实例，叫 learning 域。设备身份、账号密钥、本机密钥串、服务端表都和剪贴板分开，同一台设备在两个功能里是两个设备身份，要分别批准。
- 各域的账号密钥和设备身份 never 合并或互相派生：关掉云剪贴板或删除剪贴板数据时，服务端会删掉剪贴板的设备登记和密钥，learning 域不能因此失效；反过来也一样。
- learning 域在封装和核对上比剪贴板 v1 多两步（带认证的封装、反向核对码），见下文。剪贴板仍按它自己的协议，不受影响。

## 威胁模型

**防的：**

- 数据库泄露、服务端运维人员读库。
- 服务端或拿到会话令牌的人登记一台假设备，借此取得账号密钥。新设备必须由用户在已有设备上核对后点「允许」。
- 服务端给一台还没被批准的新设备塞一把它自己生成的密钥（剪贴板 v1 不防的那一条）。新设备只有在用户核对反向核对码一致后才启用密钥，见「反向核对」。
- 服务端把 A 设备的密文冒充成 B 设备的。设备编号在密文头里，并且是 AEAD 的附加数据。
- 服务端把某台设备的数据回滚到旧版本。代次在密文头里、受 AEAD 保护，读取方记住每台设备见过的最大代次，见「防回滚」。
- 泄露的会话令牌覆盖或删除别的设备的密文。写和删只允许当前会话绑定的那台设备。

**不防的：**

- 本机被攻破。桌面端和 HarmonyOS 的密钥串只是当前用户私有的文件，没有进系统密钥库；能读这个文件的程序就能解开全部输入习惯。所以能还原句子的整句联想数据（n-gram）要等这些平台的密钥串进了系统密钥库才同步。
- 服务端看得到的元数据，见「服务端看得到的元数据」，PRIVACY.md 会写明。
- 设备时钟偏差很大时，并发修改的胜负可能和真实先后相反（见「时间戳」）。

## 编码约定

与云剪贴板协议相同：

- `b64`：RFC 4648 §5 的 base64url，不带填充。
- `hex`：小写十六进制。
- `HKDF`：HKDF-SHA256；`AEAD`：XChaCha20-Poly1305（24 字节 nonce，16 字节 tag）；`DH`：X25519。全零的 DH 结果视为失败。
- `info`、`aad` 中的字符串都是 ASCII 字节，`‖` 表示拼接，`0x00` 是一个零字节。整数一律大端。

## HKDF 标签

learning 域只用下面这些标签，不沿用 `msime-clipboard-v1-*`：

| 标签 | 用途 |
| --- | --- |
| `msime-learning-v1-key-id` | 由账号密钥算 `key_id` |
| `msime-learning-v1-blob` | 由账号密钥算密文加密密钥 |
| `msime-learning-v1-wrap` | 封装密钥的派生，以及封装的附加数据前缀 |
| `msime-learning-v1-code` | 设备核对码 |
| `msime-learning-v1-sas` | 反向核对码 |

## 设备身份

- 每台设备第一次打开输入习惯同步时随机生成 32 字节的 X25519 私钥，只保存在本机的 learning 密钥串里。
- `public_key` = `b64(公钥 32 字节)`；`device_id` = `hex(SHA-256(公钥)[0..16])`，32 个字符；`device_id_raw` 是同样的 16 字节原值，用在密文头里。
- 设备核对码 `code`：`SHA-256("msime-learning-v1-code" ‖ 公钥)` 的前 8 字节读成无符号整数，对 10¹⁶ 取余，左补零到 16 位，每 4 位一组。批准时已有设备显示它，新设备也显示自己的，用户核对一致才点「允许」。

## 账号密钥

- 账号同一时刻只有一把 `K_learn`（32 字节随机数），由第一台打开输入习惯同步的设备在本机生成，只封装给它自己。服务端谎称「你是第一台」没有收益：密钥是本机生成的，服务端读不到。
- `key_id` = `hex(HKDF(ikm=K_learn, salt=空, info="msime-learning-v1-key-id", 16))`；`key_id_raw` 是这 16 字节原值。
- `blob_key` = `HKDF(ikm=K_learn, salt=空, info="msime-learning-v1-blob", 32)`。
- 换钥只在移除设备和「重置输入习惯同步」时发生：生成新的 `K_learn`，只封装给本机；服务端在同一事务里删掉全部密文和全部旧封装。其余设备都要重新批准，之后用新密钥重新上传。被移除的设备再也拿不到能解的密文，服务端不保留旧密钥下的任何密文。
- 没有恢复码，也没有口令派生。每台设备本地都有完整的学习数据，云端只是副本；所有设备都丢了时，在新设备上重置，旧密文直接删除。

## 带认证的封装

批准者 A（静态私钥 `s_A`，公钥 `P_A`，编号 `ID_A`）把 `K_learn` 交给设备 D（公钥 `P_D`，编号 `ID_D`）：

1. 生成临时私钥 `e`，公钥 `E`。`dh1 = DH(e, P_D)`，`dh2 = DH(s_A, P_D)`，任一全零即失败。
2. `wrap_key = HKDF(ikm=dh1 ‖ dh2, salt=E ‖ P_A ‖ P_D, info="msime-learning-v1-wrap", 32)`
3. `aad = "msime-learning-v1-wrap" ‖ 0x00 ‖ key_id ‖ 0x00 ‖ ID_A ‖ 0x00 ‖ ID_D`（`key_id`、`ID_A`、`ID_D` 用 hex 字符串）
4. `wrap = "v1." ‖ b64(E ‖ nonce ‖ AEAD(wrap_key, nonce, K_learn, aad))`，固定 142 个字符。

服务端随封装保存 `approver_device_id`。D 解封时用 `GET` 里给出的批准者公钥，以 `DH(s_D, E)` 和 `DH(s_D, P_A)` 得到同样的 `dh1`、`dh2`；批准者公钥不对时 AEAD 校验失败。解开后重新计算 `key_id`，与服务端给的不同就当作没有拿到密钥。第一台设备封装给自己时 A 就是 D。

带认证的封装保证「封装出自持有 `s_A` 的设备」，但服务端可以自己登记一台假的批准者，用它的私钥封装一把假密钥。挡住这一步的是下面的反向核对。

## 反向核对

- A 点「允许」并上传封装后，显示反向核对码 `sas`：`HKDF(ikm=K_learn, salt=P_A ‖ P_D, info="msime-learning-v1-sas", 8)` 读成无符号整数，对 10¹⁶ 取余，左补零到 16 位，每 4 位一组。
- D 解封后用自己解出的密钥和封装记录的批准者公钥算同一个值并显示，用户在 D 上点「一致」之后，`K_learn` 才生效（写进密钥串的「已确认」状态）。确认之前 D 不上传、不下载、不解密任何密文。
- 用户点「不一致」时，D 丢掉这把密钥和封装，保留自己的设备身份，报 `learning_sas_mismatch`，提示用户检查设备列表、移除不认识的设备。
- 核对码里带上 `K_learn`，是为了让服务端无法提前算出真设备会显示的号码：两台设备的公钥服务端都知道，只用公钥的话，服务端可以离线搜索一个让号码相同的假批准者公钥（约 2⁵³ 次运算）。服务端造的假批准者给 D 的是另一把密钥，两边显示的号码相互独立，碰上的概率是 10⁻¹⁶。

## 密文信封 MSL1

每台设备在服务端只有一份密文，内容是这台设备导出的全部输入习惯。

明文头，68 字节，不加密：

| 偏移 | 长度 | 内容 |
| --- | --- | --- |
| 0 | 4 | ASCII `MSL1` |
| 4 | 16 | `key_id_raw` |
| 20 | 16 | 上传设备的 `device_id_raw` |
| 36 | 8 | `generation`，u64 大端 |
| 44 | 24 | `nonce`，随机 |

之后是 `AEAD(blob_key, nonce, padded, aad=整个 68 字节明文头)`，长度是 `padded` 加 16 字节 tag。

- `gz`：明文流（见下一节）按 RFC 1952 压缩成单个 gzip 成员，头部 `MTIME` 为 0、不带文件名和注释，免得压缩头泄露时间。
- `padded` = `u32 大端(len(gz)) ‖ gz ‖ 零字节`，总长 `padme(4 + len(gz))`。解密方校验长度字段不越界、补齐的字节全为零，否则按解不开处理。
- `padme(L)`：`L < 2` 时为 `L`；否则 `E = ⌊log₂ L⌋`，`S = ⌊log₂ E⌋ + 1`，`m = 2^(E−S) − 1`，结果是 `(L + m) & ~m`。额外开销不超过 12%。不按 2 的幂分桶：那样 4.1 MiB 会补到 8 MiB，正好顶到上限。
- `generation` 就是这次上传的 `?revision=N`，见服务端接口。
- 整个信封（明文头加密文）不超过 8 MiB（8,388,608 字节）。补齐后超过时不上传，报 `learning_too_large`；其他设备的照常下载合并。

## 明文流 `msime-learning-sync` v1

UTF-8 的 NDJSON，一行一条记录，顺序固定，同样的学习数据总是写出同样的字节（上传前用它的 SHA-256 判断有没有变化）：

```json
{"type":"header","format":"msime-learning-sync","version":1,"content_id":"<本机词库内容标识>"}
{"type":"weight","dictionary":"pinyin","key":"ni'hao","value":"你好","weight":12345,"display":"","updated_at":1793000000}
{"type":"delete","dictionary":"pinyin","key":"ni'hao","value":"拟好","updated_at":1793000001}
{"type":"pin","context":"…","value":"…","updated_at":1793000002}
{"type":"position","context":"…","key":"…","value":"…","position":2}
{"type":"footer","records":4}
```

- `weight`：日志里 `operation='upsert'` 且 `user_inserted=0` 的行，即内置词学到的权重，包括阶梯降权写下的那些行。`user_inserted=1` 的行不导出，它们归词库同步。
- `delete`：日志里全部 `operation='delete'` 的行（删除记录）。
- `pin`：`pinned_candidates` 的行。
- `position`：`fixed_candidate_positions` 的行。这张表没有时间戳。
- 排序：`weight` 与 `delete` 按 `(dictionary, key, value)` 混排，然后 `pin` 按 `context`，然后 `position` 按 `(context, key, value)`；字符串按 UTF-8 字节比较。
- `updated_at` 是这一行在日志里真实的修改时间（Unix 秒），不是导出时间。
- `content_id` 是导出设备当前词库代次的内容标识。与本机不同时照样合并，设置页提示「这台设备的词库版本与其他设备不同，部分候选顺序可能不一致」：学到的权重是相对导出设备词库邻居的绝对值，词库版本不同时重放后名次可能不同。
- `dictionary` 只取 Engine 认识的种类（`pinyin`、`wubi`、`wubi98`、`quick`、`english`），其余的行不导出；读到不认识的种类时跳过这一条。
- 不认识的 `type` 跳过并计数，不让整份失败：第二期在同一版本里加整句联想的记录，旧客户端照样合并它认识的部分。只有不兼容的改动才升 `version`，读到更高的 `version` 时整份跳过，提示升级。
- 记录数和 `footer.records` 不符、`footer` 之后还有内容、字段缺失或类型不对，按解不开处理。

## 时间戳

Engine 里每一条在冲突分支里改写 `user_dictionary_operations` 或 `pinned_candidates` 的语句，都把 `updated_at` 写成 `max(unixepoch(), 旧值 + 1)`；第一次插入时是 `unixepoch()`。同一行的时间戳因此单调递增：收到一条带「未来时间」的远端行之后，本机下一次修改这一行仍然比它大，仍然胜出。合并写入远端行时原样保留它的 `updated_at`。

这保证之后的修改能胜出，不保证「真实时间更晚的先胜」：两台设备在各自时钟下几乎同时改同一行时，时钟快的一方胜。

## 合并规则

合并分两步：先在纯函数 `combine` 里把所有远端设备的记录按下表折叠成一份（可交换、可结合、幂等，与设备顺序和重复次数无关），再在一个事务里和本机日志比较、写入胜出的远端行。

| 数据 | 第一期 | 规则 |
| --- | --- | --- |
| `weight` 与 `delete` | 同步 | 按 `(dictionary, key, value)` 比较：`updated_at` 大的胜；相同时 `delete` 胜；都是 `weight` 时 `weight` 大的胜；再相同时 `display` 按字节大的胜。本机这一行 `user_inserted=1` 时从不被远端行覆盖。远端行写进本机时记为 `user_inserted=0`，写法与 `merge_dictionary_state` 相同（先改工作词库，再记日志）。阶梯降权是同一事务写下的一组行，时间戳相同，逐行比较时整组一起胜出 |
| `delete` 胜出时的副作用 | 同步 | 与本机删词相同：删掉这个词的置顶，删掉包含它的整句联想记录（`removal.rs` 的删词事务）。删除记录必须同步：连续选词造词前会查删除记录，不同步的话对端会把删掉的词又造回来 |
| `pin` | 同步 | 按 `context` 比较 `updated_at`，大的胜，相同时 `value` 按字节大的胜。取消置顶只发生在删词时，由 `delete` 的副作用带过去 |
| `position` | 同步，只增不删 | `combine` 里同一 `(context, key, value)` 取较小的位置，同一 `(context, position)` 取 `(key, value)` 按字节较小的那条；写入本机用 `INSERT OR IGNORE`，本机已有的保留本机。取消固定不留记录，第一期传不过去 |
| 快捷短语槽位（`quick_phrase_slots`） | 第二期 | 不在任何导出流里，第二期给它加时间戳和删除记录 |
| 整句联想（`personal_bigram`、`personal_trigram`） | 第二期 | 按设备分槽：每台设备上传自己计数最高的 5 万条；接收方按来源设备另存，读取时本机计数加上远端计数之和；远端合计截到 20 万行；移除设备时删掉它的贡献 |
| 拼写纠错计数、自动纠错抑制 | 不同步 | 纠错类别取决于键位相邻，触屏和实体键盘之间不能混用 |
| 连续选词对、选词计数、回执 | 永不同步 | 都是用完就丢的进度计数，产出的词已经随词库同步 |

其他约定：

- 首次打开时不问「合并还是用云端」：先上传本机这份，再合并其他设备的。本机原有的行保留原来的 `updated_at`，久未使用的设备加入时不会盖掉别人更新的调整。
- 「重置学习」只清本机，界面上写明。它把本机日志整份换成空的，不写删除记录，所以其他设备上的数据不受影响。本机同时记下重置时刻（与防回滚进度放在一起），之后合并时远端 `updated_at` 不晚于这个时刻的行一律跳过：远端密文是整份的，不这样做的话，下一次合并会把重置之前各设备学到的东西又带回本机。重置之后其他设备新学到的照常合并进来。
- 合并在 Engine 的独占维护锁下执行。待合并的结果先写成本机的待合并文件，由持有词库的进程在没有输入会话时认领、合并，失败重试三次后放弃，做法与本地备份恢复的待合并文件相同。

## 防回滚

- 读取方按（账号，`key_id`，来源设备）分别记两项：已验证的最大代次 `verified`、已应用的代次 `applied`，都只增不减，永不清零。存在本机的 `cloud-sync/learning.json`，与 `state.json` 分开（旧版本改写 `state.json` 时会丢掉它不认识的字段），退出登录不删。
- 只下载、只接受 `key_id` 等于本机当前密钥的密文。`generation < verified` 的一律拒收；`generation == applied` 的跳过；其余的解密成功后先推进 `verified`，合并成功后再推进 `applied`。合并失败只是 `applied` 不动，下一轮重新下载同一个代次再合并。
- 本机拿到新的 `key_id`（换钥后被重新批准）时，旧 `key_id` 下的记录可以删掉：旧密钥下的密文不会再被接受。
- 上传方记自己上次上传的代次 `uploaded` 和明文流的 SHA-256。下一次上传用 `N = max(服务端当前代次, uploaded) + 1`：服务端丢了或回滚了这台设备的密文时，新的代次仍然比其他设备记着的大。

## 服务端接口

都在 `https://api.msime.app/v1/users/me/learning` 下，需要登录。错误一律是 `{"error":{"code":"...","message":"..."}}`。只有匿名身份的账号（有 anonymous 身份、没有任何非匿名身份）一律返回 403 `learning_requires_account`。

| 方法与路径 | 作用 |
| --- | --- |
| `GET /?device_id=<id>` | 返回 `{"key_id", "wrap": {"wrap", "approver_device_id"}\|null, "devices": [{"device_id", "public_key", "name", "created_at", "last_seen", "approved"}], "blobs": [{"device_id", "key_id", "revision", "bytes", "updated_at"}]}`。`wrap` 是给请求里那台设备的封装 |
| `PUT /devices/{device_id}` | 请求体 `{"public_key", "name"}`，登记设备并绑定当前会话；已登记时只更新名称。一个会话只绑定一台设备，会话被吊销时设备行级联删除。每个账号最多 20 台设备 |
| `DELETE /devices/{device_id}` | 删除设备、它的封装和密文。用于拒绝待批准的设备和本机关闭功能；移除一台已批准的设备要走 `rekey`。删掉最后一台设备时一并删除 `key_id` |
| `POST /key` | 请求体 `{"key_id", "wrap"}`，只在账号还没有密钥时可建，封装给的必须是调用会话绑定的设备 |
| `PUT /wraps/{device_id}` | 请求体 `{"key_id", "wrap", "approver_device_id"}`，批准一台设备。`approver_device_id` 必须是调用会话绑定、并且持有当前密钥封装的设备 |
| `POST /rekey` | 请求体 `{"previous_key_id", "key_id", "wrap", "revoke": []}`，同一事务里确认当前密钥、删除 `revoke` 里的设备、删除全部密文和全部旧封装、写入新密钥和给调用设备的封装 |
| `GET /blobs/{device_id}` | 返回这台设备的信封原样字节（`application/octet-stream`） |
| `PUT /blobs/{device_id}?revision=N` | 请求体是信封原样字节。只允许调用会话绑定的那台设备写自己的那份 |
| `DELETE /blobs/{device_id}` | 只允许调用会话绑定的那台设备删自己的那份 |

服务端对密文只做格式检查，不尝试理解内容：

- 以 `MSL1` 开头，长度在 68 + 16 字节到 8 MiB 之间，`padded` 的长度（总长减 84）是 `padme` 的不动点。
- 头里的 `key_id_raw` 是账号当前的 `key_id`，`device_id_raw` 与路径一致，`generation` 等于 `N`，并且 `N` 大于这台设备已存的代次（没有时为 0），否则 409 `revision_conflict`。这一检查在写入事务里做（比较并交换）。
- 不以 `MSL1` 开头、而以 `{` 或 gzip 魔数 `1f 8b` 开头的请求体，返回 400 `learning_e2e_required`，防止旧客户端或写错的客户端上传明文。
- 请求体用 `MaxBytesReader` 限在 8 MiB 加明文头，路由超时 60 秒。

## 上限与额度

| 项 | 上限 |
| --- | --- |
| 一台设备的信封 | 8 MiB，超出返回 413 `learning_too_large` |
| 一个账号全部密文合计 | 64 MiB，超出返回 413 `learning_quota` |
| 一个账号的设备 | 20 台，满了返回 409 `learning_device_limit` |
| 明文流记录数 | 500,000 条（不含 header 与 footer） |
| `key` / `value` / `display` / `context` | 512 / 2048 / 2048 / 1024 字节 |
| `weight` | 1 到 100,000,000 |
| `position` | 1 到 5 |
| 上传频率 | 客户端自动上传每小时最多一次，明文流的 SHA-256 没变时不上传；手动「立即同步」不限 |
| 服务端限流 | 每个用户 `PUT` 每小时 30 次，`GET` 每小时 240 次，超出返回 429 `rate_limit_exceeded` |
| 不在线的设备 | 180 天没有请求的设备，服务端删掉它的设备行、封装和密文 |
| 第二期整句联想 | 每台设备上传 5 万条，接收方远端合计 20 万行 |

客户端在明文流超过记录上限或信封超过 8 MiB 时截断最旧的行并提示（`learning_too_large`），不让整份同步失败。

## 错误码

服务端返回：

| 状态 | `code` | 含义 |
| --- | --- | --- |
| 400 | `invalid_device_id` | `device_id` 缺失或格式不对 |
| 400 | `invalid_learning_device` | 公钥与编号不符、名称超长或含控制字符 |
| 400 | `invalid_learning_key` | `key_id` 或封装格式不对、封装对象没登记 |
| 400 | `invalid_learning_blob` | 信封格式不对（长度、补齐、头部与路径或 `revision` 不一致） |
| 400 | `learning_e2e_required` | 请求体看起来是明文 |
| 403 | `learning_requires_account` | 只有匿名身份的账号 |
| 403 | `learning_device_mismatch` | 写、删、批准所用的设备不是当前会话绑定的那台 |
| 404 | `not_found` | 设备或密文不存在 |
| 409 | `learning_key_conflict` | 已有密钥时建钥、`key_id` 不是当前密钥 |
| 409 | `revision_conflict` | `revision` 不大于已存的代次 |
| 409 | `learning_device_limit` | 设备已满 |
| 413 | `learning_too_large` | 单个信封超过 8 MiB |
| 413 | `learning_quota` | 账号合计超过 64 MiB |
| 429 | `rate_limit_exceeded` | 超出限流，带 `Retry-After` |

客户端驱动给宿主的（`status` 为 0，宿主只负责翻译成界面文案）：

| `code` | 含义 |
| --- | --- |
| `learning_disabled_locally` | 本机关掉了学习（`input.learning`、`platform.ios.dictionary_learning`、`platform.macos.candidate_learning`），没有可同步的东西 |
| `learning_waiting_approval` | 本机在等另一台设备允许，同时给出本机核对码 |
| `learning_sas_mismatch` | 用户在反向核对时点了「不一致」，密钥已丢弃 |
| `learning_key_rotated` | 账号换了密钥，本机要在其他设备上重新被允许 |
| `learning_undecryptable` | 某台设备的密文解不开或格式不对，已跳过，带来源设备名 |
| `learning_rollback` | 某台设备的密文代次比见过的小，已拒收 |
| `learning_dictionary_mismatch` | 有设备的 `content_id` 与本机不同，照常合并，只做提示 |
| `learning_merge_pending` | 已下载，等持有词库的进程空闲时合并 |

## 服务端看得到的元数据

- 每台设备的名称、公钥、登记时间、最后在线时间、绑定的是哪个会话。
- 谁批准了谁（封装上的 `approver_device_id`）。
- 账号当前的 `key_id`，设备数量。
- 每台设备密文补齐后的大小、代次和上传时间。因为只在内容变了时上传，上传时间能看出这台设备大约在哪个小时里学到了新东西。
- 请求本身带的 IP 地址和时间，与其他账号接口相同。

## 本机的密钥串

learning 域的密钥串与云剪贴板的分开存放，名字是 `learning-keystore`，放在与 `cloud-clipboard-keystore` 相同的位置（各平台的位置见 [云剪贴板协议的密钥串存放位置](cloud-clipboard-e2e.md#密钥串的存放位置)），里面另外记着 `K_learn` 是否已经过反向核对。同一台设备的输入法、设置应用和面板共用一份；读、比、写在同一把跨进程锁里做完，不替换另一个设备身份，规则与剪贴板相同。退出登录时与会话一起删除，重新登录要重新批准。HarmonyOS 的键盘不生成身份，用设置应用经共享沙箱镜像过来的那份。
