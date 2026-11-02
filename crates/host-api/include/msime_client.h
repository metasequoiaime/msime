#ifndef MSIME_CLIENT_H
#define MSIME_CLIENT_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

/* 离线单字手写识别，仅工作线程调用；请求 {model:绝对路径,request:{language:"zh-CN",strokes:[{points:[{x,y}]}]}}，坐标在 420×420 内，最多 262144 字节。返回标准封装 {candidates:[文字]}；不联网，Android 使用自己的 ML Kit 入口。 */
char *msime_client_handwriting(const uint8_t *request, size_t length);

/* Host-neutral focus lease and key event contract. Values are opaque to the ABI. */
typedef struct msime_client_focus_lease {
  uint64_t client;
  uint64_t epoch;
  uint64_t token;
} msime_client_focus_lease;

typedef struct msime_client_key_event {
  msime_client_focus_lease lease;
  uint32_t virtual_key;
  uint32_t scan_code;
  uint32_t modifiers;
  uint32_t character;
  bool ui_less;
} msime_client_key_event;

/* Outcome for platform key-router writes. Only definitely-not-sent may use a
 * local fallback; ambiguous delivery must wait for lease recovery. */
typedef enum msime_client_key_dispatch_result {
  MSIME_CLIENT_KEY_SENT = 0,
  MSIME_CLIENT_KEY_DEFINITELY_NOT_SENT = 1,
  MSIME_CLIENT_KEY_DELIVERY_AMBIGUOUS = 2,
} msime_client_key_dispatch_result;

static inline bool msime_client_key_dispatch_allows_fallback(
    msime_client_key_dispatch_result result) {
  return result == MSIME_CLIENT_KEY_DEFINITELY_NOT_SENT;
}

static inline bool msime_client_key_event_valid(const msime_client_key_event *event) {
  return event != NULL && event->lease.client != 0 && event->lease.epoch != 0 &&
         event->lease.token != 0 && event->virtual_key <= 0xff &&
         (event->modifiers & ~UINT32_C(0x0f)) == 0;
}

/* ABI 3 (3 replaced msime_client_builtin_skins with the global theme functions). All functions return owned, NUL-terminated UTF-8 JSON. Free exactly once using msime_client_string_free, including error responses. Never use free(). Responses: {"ok":true,"value":...} or {"ok":false,"error":"..."}. Creation returns a View; its session field is the handle. Handles are confined to their creating thread. Dispatch, focus, view and destroy on that thread. Text and candidate values are copied; no Engine pointers escape. */
uint32_t msime_client_abi_version(void);
/* Worker-thread bootstrap: {resources: absolute path, state_root: absolute path}.
 * Verifies pinned resources, delegates working data preparation to Engine and
 * returns HostOptions. Maximum 16384 bytes; no session may use state_root during
 * preparation. Caller publishes the returned config atomically after success.
 * 可选的 edition 是版本 id（shared/contracts/editions.json），缺省为 full；不是 full 时返回的 HostOptions 带 edition 键，未知 id 返回错误。
 * 可选的 runtime_options 是已发布的 runtime-options.json 的绝对路径：准备成功后，文件里的 dictionaries 不是这次准备的代次时改写成它，其余键不动；改写失败只记诊断，不影响返回值。Windows Server 每次启动都重新准备、会话用返回的文档，靠它让照这份文件工作的设置窗口和 TIP 跟上同一个代次。
 */
char *msime_client_prepare_host(const uint8_t *options, size_t length);
/* path is an absolute UTF-8 runtime options file path of length bytes; maximum 4096. When its dictionaries directory is not the installed resource generation (after a package upgrade), prepares that generation, replays the user dictionary into it and atomically rewrites resources/dictionaries, keeping every other key. It also keeps language_dictionaries in step with the Cantonese, Zhuyin and Stroke dictionaries installed beside the resources, whatever the generation; only an input method host may call it, because a host older than this library rejects that key. Value is true when the file was rewritten. Call before creating any session from the file. When the recorded resource directory does not match the compiled dictionary lock (downloaded dictionaries an upgrade did not replace) the error text begins with "dictionary_outdated:" and the file is left unchanged; the rest of that text may name private paths. */
char *msime_client_refresh_host(const uint8_t *path, size_t length);
/* 宿主注入的会话存储（ABI 不变，仍是 3，只新增）。钥匙串、App Group、AndroidKeyStore 这类只有平台能读写的地方由宿主经下面的回调提供字节和跨进程锁，编解码、布局、合法性校验和刷新时的加锁顺序都在 Rust 里。注册以后，msime_client_account、msime_client_community、msime_client_account_cloud_clipboard、msime_client_account_access_token、msime_client_cloud_dictionary、msime_client_ai_skin_generate、msime_client_ai_skin_job 和 msime_client_cloud_sync 的请求用 {session_store: id} 代替 {directory}；两者恰好给一个，legacy_directory 只能与 directory 一起给，都给、都不给或 id 没有注册时是该入口的「请求不合法」码（account_invalid、community_invalid、invalid_cloud_clipboard、invalid_cloud_dictionary、ai_skin_invalid、cloud_sync_invalid）。带 directory 的请求与以前完全相同，Windows 和 Linux 不需要改；msime_client_account 和 msime_client_account_chat_stream 带 directory 时还可以给 client_agent（见 msime_client_account），其余入口的目录模式仍发 MSIME/Android。 */
#define MSIME_CLIENT_SESSION_STORE_VERSION 1
/* flags：钥匙串、App Group 这类多个进程读写同一份会话的存储必须置位。置位时每次访问都重新读，刷新、登录、退出、改资料和补登录 ID 时持 lock(item)；不置位时会话读一次后缓存在进程里，lock/unlock 可以为 NULL。 */
#define MSIME_CLIENT_SESSION_STORE_SHARED 1u
typedef enum msime_client_session_item {
  MSIME_CLIENT_SESSION_ITEM_ACCOUNT = 1,            /* 登录会话；也是 lock/unlock 的一个 item */
  MSIME_CLIENT_SESSION_ITEM_CLIPBOARD_KEYSTORE = 2, /* 云剪贴板密钥串，UTF-8，不带换行；属于 ACCOUNT 槽，清掉登录会话时一起删除 */
  MSIME_CLIENT_SESSION_ITEM_ANONYMOUS = 3,          /* 本机匿名账号的会话；也是 lock/unlock 的一个 item */
  MSIME_CLIENT_SESSION_ITEM_ANONYMOUS_IDENTITY = 4, /* 匿名账号的身份 {subject,secret}；save 是「不存在才写」 */
} msime_client_session_item;
/* 64 位目标上 80 字节：version@0、layout@4、flags@8、（填充 4 字节）、client_agent@16、context@24、load@32、save@40、remove@48、lock@56、unlock@64、release@72。 */
typedef struct msime_client_session_store {
  uint32_t version;         /* MSIME_CLIENT_SESSION_STORE_VERSION */
  uint32_t layout;          /* 0 = native SavedAccountSession；1 = apple BackendSavedSession（expiresAt 为 2001 年起的秒数，含 sessionID）。读取时两种都接受，写入按这里选的布局 */
  uint32_t flags;           /* MSIME_CLIENT_SESSION_STORE_SHARED 或 0；其他位必须为 0 */
  const char *client_agent; /* 可为 NULL，此时 User-Agent 与不带 client_agent 的目录模式相同（MSIME/Android）；否则是以 NUL 结尾、不超过 256 字节的可打印 ASCII，形状为 msime-<android|ios|macos|windows|linux|harmony>/<版本，1 到 32 个 [0-9A-Za-z.+_-]> (<设备>; <系统>; edition=<id>)，括号部分可省略、里面不能有括号；注册时复制，之后不再读 */
  void *context;            /* 原样交回每个回调；注册成功后要一直有效到 release 被调用 */
  intptr_t (*load)(void *context, uint32_t item, uint8_t *buffer, size_t capacity); /* 把该项写进 buffer：>0 是写入的字节数，0 是不存在，<0 是失败；大于 capacity 按失败处理 */
  int32_t (*save)(void *context, uint32_t item, const uint8_t *bytes, size_t length); /* 0 是已写入；ANONYMOUS_IDENTITY 已存在时不覆盖并返回 1；其他值是失败 */
  int32_t (*remove)(void *context, uint32_t item); /* 0 是成功，不存在也算成功；其他值是失败 */
  int32_t (*lock)(void *context, uint32_t item, uint32_t timeout_ms, uint64_t *token); /* item 只会是 ACCOUNT 或 ANONYMOUS，timeout_ms 是 20000；0 是已持有（把令牌写进 *token），1 是超时，其他值是失败。未置 SHARED 时可为 NULL */
  void (*unlock)(void *context, uint32_t item, uint64_t token); /* 释放 lock 拿到的锁。未置 SHARED 时可为 NULL */
  void (*release)(void *context); /* 可为 NULL；只在注册成功后、最后一次使用结束时调用恰好一次 */
} msime_client_session_store;
/* 回调的参数与内存：Rust 给的 capacity 就是该项的上限，两个会话项和身份项是 65536，密钥串是 4096。buffer 和 bytes 只在本次调用期间有效，不要保留。 */
/* 读写语义：save 对 ACCOUNT、CLIPBOARD_KEYSTORE、ANONYMOUS 是原子整体替换，其他进程只能看到旧值或新值；对 ANONYMOUS_IDENTITY 是「不存在才写」，已经有值时不覆盖、返回 1，Rust 随即重新 load 并采用读到的那份（尚未迁移的进程可能在锁外发布身份，原子覆盖会造出两个身份和一个孤儿账号）。load 必须原样返回最后一次成功 save 写入的字节；置了 SHARED 时「最后一次」指任何进程的最后一次，Android 的 SharedPreferences 跨进程不满足这一条，不能被两个进程当作同一个 store。编解码、布局和合法性都由 Rust 校验，解码失败、超过上限或不合法是 account_storage。 CLIPBOARD_KEYSTORE 的 save 本身仍是整体替换，但 Rust 只在持有 ACCOUNT 槽的锁时、先 load 并确认里面没有另一个设备身份之后才调用它（有就不写、改用读到的那份，见 docs/cloud-clipboard-e2e.md 的「宿主的职责」）；宿主自己也写这一项时（Android 的键盘进程）要遵守同样的规则。 */
/* 线程：load、save、remove、lock、unlock、release 可能在任意线程上被并发调用，包括 Rust 自己起的线程。JNI 宿主要缓存 JavaVM，每次回调先 GetEnv，当前线程没有 attach 时 AttachCurrentThread 并在本次回调返回前 DetachCurrentThread，返回前检查并清掉挂起的 Java 异常、返回失败码；不能直接用调用 msime_client_* 的那个线程的 env。Apple 宿主的回调是同步调用，不能 hop 到 MainActor，也不能 DispatchQueue.main.sync。每次调宿主的 lock 最多等 20 秒，持锁期间最多一个 30 秒以内的 HTTP；但同一进程里排在别的线程那次刷新后面的调用要先等 Rust 进程内的锁，这一段没有超时，前面每个线程的 20 秒加 30 秒都会累加上来。所以所有经过 store 的 msime_client_* 调用都不能在主线程上调用。回调里不能重入任何 msime_client_*，也不能 unwind：C++ 异常自己 catch，Swift 不能抛出。AI 皮肤的出图线程遇到 401 时会刷新令牌，所以 msime_client_ai_skin_generate 进行中这些回调也会在 Rust 的线程上发生。 */
/* 锁：lock(item) 只需要保证跨进程互斥（本进程里的并发由 Rust 自己的锁排斥），但必须用 flock(2)，每次加锁新开一个 fd 做 flock(LOCK_EX)，这样同一进程里尚未迁移、同样用 flock 的代码也被排斥；不能用 fcntl 或 lockf（按进程计锁，同进程第二个 fd 直接成功，关任意一个 fd 会释放本进程在该文件上的所有锁），也不能用 Java 的 FileChannel.lock（底层是 fcntl，与 flock 互不可见）。宿主按 item 选锁文件。Rust 的加锁顺序固定为：进程内按 (store, item) 的锁 → lock(item) → Rust 会话锁 → load/save/remove；Rust 只持会话锁、不持宿主锁时也会调 load。所以 load、save、remove 不得等待刷新锁，也不得等待 lock 回调在等待期间持有的任何宿主锁（例如 Android 不能用同一个 monitor 既实现 lock 又包住 load）。Rust 只在一次刷新、登录、退出、改资料或补登录 ID 期间持宿主锁，其中最多一个 HTTP 请求（30 秒以内）；持锁期间不会对同一个 store 的任何 item 再加锁，同一线程嵌套加锁时直接返回 account_unavailable。每次加锁成功后，Rust 在同一线程上用同一个 item 和 token 调用 unlock 恰好一次，出错或 panic 时也一样。iOS 上挂起时还持有 App Group 文件锁的进程会被系统杀掉，宿主要在持锁期间保持后台执行：App 里用 UIApplication.beginBackgroundTask，键盘扩展里改用 ProcessInfo.performExpiringActivity。 */
/* 共存：同一份会话在新旧实现共存期间必须共用同一把锁。凡是不经锁回调就能刷新同一份会话的尚未迁移代码（例如 Android 的 FutureTask 单飞），必须和 Rust 在同一次迁移里一起切换，不能共存。 */
/* 注册：store 不能为 NULL。version 不是 1、load/save/remove 任一为 NULL、layout 不是 0 或 1、flags 有未知位、置了 SHARED 却没有 lock 或 unlock、client_agent 不合法时 error 是 invalid_session_store；HTTP 客户端建不出来时是 account_unavailable。这两种情况下 Rust 都没有接管 context，也不会调用 release。成功时结构体被复制，Value 是 {session_store: id}，id 是小于 2^53 的正整数。每个 store 在进程里有一个登录会话、一个匿名会话和一组社区服务，共用一个 HTTP 客户端；同一个 store 的 google_login 和 google_cancel 作用在同一个会话对象上。同一份存储在一个进程里只注册一次：每次注册各有一套进程内的锁和会话缓存，两次注册同一份不置 SHARED 的存储（例如同一个 Android SharedPreferences）会各自缓存，可能拿同一个会轮换的刷新令牌各刷新一次，服务端随即吊销整个会话。不联网。 */
char *msime_client_session_store_register(const msime_client_session_store *store);
/* 注销 msime_client_session_store_register 给的 id，Value 是它是否曾经注册。之后的新请求拿到的是「请求不合法」码；正在进行的调用照常结束，最后一个结束时调用 release。release 可能在任意线程上，也可能就在本函数或某次 msime_client_* 调用返回之前、在同一线程上同步执行，所以它不能重入 msime_client_*，也不能去拿宿主调用本函数时手里持有的锁；Rust 调用 release 时不持有任何 Rust 锁。注销前会先结束该 store 上进行中的 google_login。不联网。 */
char *msime_client_session_store_unregister(uint64_t session_store);
/* directory is an absolute UTF-8 directory path of length bytes; maximum 4096. Registers the device's anonymous MSIME account at https://api.msime.app unless anonymous-session.json already exists there, keeping the identity in anonymous-account.json and the session in anonymous-session.json (both owner-only). Blocks on the network: call off the input thread. Value is true once a session exists. A failure leaves the identity for the next call, so call again on a later start. */
char *msime_client_ensure_anonymous_account(const uint8_t *directory, size_t length);
/* 与 msime_client_ensure_anonymous_account 相同，只是宿主可以自报注册请求的 User-Agent（#8201 的决定 1，服务端「我的设备」据此显示平台和设备）：request 是 length 字节的 UTF-8 JSON {"directory": 绝对路径, "client_agent": 可选}，最多 32768 字节，directory 最多 16384 字节。client_agent 的形状与 msime_client_account 的 client_agent 相同（msime-<平台>/<版本> (<设备>; <系统>; edition=<id>)），不合法时是 account_invalid、不碰目录；不给时发出的请求与 msime_client_ensure_anonymous_account 相同（MSIME/Android）。已有会话时只读一次会话文件、不联网。Value 是 true；失败时 error 是账号错误码（与 msime_client_account 的 ensure_anonymous 相同），不是旧入口那样的说明文字。旧入口不变。 */
char *msime_client_ensure_anonymous_account_with_agent(const uint8_t *request, size_t length);
/* 拼出宿主自报的 User-Agent（#8201 迁移计划 P11），各宿主不再各写一份：request 是 length 字节（最多 16384）的 UTF-8 JSON {platform, version, device?, system?, edition}，都是字符串，device 和 system 可以不给或为 null（与空串相同）。Value 是 msime-<platform>/<version> (<device>; <system>; edition=<edition>)，总能当 msime_client_account、msime_client_account_chat_stream、msime_client_ensure_anonymous_account_with_agent 和 msime_client_session_store 的 client_agent 用。platform 必须是 android、ios、macos、windows、linux、harmony 之一（区分大小写）；device 是设备型号（Android 的 Build.MODEL），system 是系统名加版本（Android 传 "Android " 加 Build.VERSION.RELEASE），edition 是 shared/contracts/editions.json 的版本 ID。device、system 和 edition 只清洗、不拒绝：去掉首尾空白和控制字符，去掉不是可打印 ASCII 的字符和 ( ) ;，保留 64 个字符后再去一次首尾空白，为空时 device 和 system 是 unknown、edition 是 full；version 只保留 [0-9A-Za-z.+_-]、最多 32 个，为空时是 0。Android 照上面传参时与原来的 BackendAccount.loginUserAgent 逐字相同，只有三处不同：version 里有别的字符或超过 32 个（原来那样的串服务端认不出），Build.VERSION.RELEASE 为空（原来是 Android unknown，这里是 Android），以及 RELEASE 去掉字符后开头留下空白、或与 "Android " 合起来超过 64 个字符（原来 RELEASE 单独清洗、单独截断）。用例在 shared/contracts/client-agent/cases.json。platform 不认识、JSON 不合法、缺字段、有多余字段、超过上限或 request 为 NULL 时 error 是 account_invalid。不联网、不读文件，可以在任意线程上调用。 */
char *msime_client_client_agent(const uint8_t *request, size_t length);
/* 输入法进程取账号服务的访问令牌。request 是 length 字节的 UTF-8 JSON {"directory": 绝对路径, "rejected_token": 可选}，最多 16384 字节；directory 是设置应用和输入法共用的账号目录（Windows 是 %LOCALAPPDATA%\<本版本的用户目录>\account，与 msime_client_ensure_anonymous_account 相同）。设置应用登录的 account-session.json 优先，刷新时与设置应用共用 account-refresh.lock；没有登录时用 anonymous-session.json。rejected_token 是服务端刚以 401 拒绝的令牌，传入后强制刷新。每次调用都重新读文件，登录、退出和换号下一次调用即生效。也可以用 {"session_store": msime_client_session_store_register 给的 id, "rejected_token": 可选} 代替 directory：登录会话和匿名会话都在宿主注入的存储里，同样登录的优先，刷新时持宿主的 lock(ACCOUNT) 或 lock(ANONYMOUS)。Value {access_token, user_id, anonymous}；没有任何会话时 error 是 account_unauthorized。响应含凭据，不要写进日志。刷新会阻塞网络：不要在输入线程调用。 */
char *msime_client_account_access_token(const uint8_t *request, size_t length);
/* 设置窗口自己的云剪贴板请求，与云剪贴板面板发的相同。request 是 length 字节（最多 131072）的 UTF-8 JSON {directory: 账号目录的绝对路径, device_name?, action}；directory 是共享应用登录时写 account-session.json 的目录（Windows 是 %LOCALAPPDATA%\<本版本的用户目录>\account），只用登录的会话，不退回匿名账号。device_name 是可选的、端到端加密登记本机时给其他设备看的名称（例如鸿蒙的 marketName），规则与 msime_client_relay_cloud_clipboard 的相同：不给、为 null、只有空白、超过 256 字节或含控制字符时改用计算机名或平台名，不算请求不合法，登记前驱动还会再清理一遍；只在本机还没登记时用到。也可以用 {session_store: id, device_name?, action} 代替 directory，id 来自 msime_client_session_store_register，用该 store 的登录会话（注册时建好的那一个，不每次新建）；两者都给、都不给或 id 没有注册时 error 是 invalid_cloud_clipboard。action 是下面之一：{operation:"list",search}（search 不超过 1024 字节、不含控制字符，在本机解密后匹配）→ {enabled, retention_days, items:[{id,text,updated_at,pinned,device}], e2e:{status,code,pending_devices,undecryptable}}；{operation:"add",text}（非空、不超过 4000 个 UTF-16 单元、不含控制字符，换行除外）→ 新条目；{operation:"delete",id}（条目的 id，64 位小写十六进制；不超过 256 字节的其他 ASCII 标识符不发出请求，error 是 account_invalid）→ {}；{operation:"set_enabled",enabled} → {enabled}；{operation:"pin",id,pinned}（id 是 64 位小写十六进制，pinned 是布尔）→ {}；{operation:"set_retention",days}（days 是 0、1、7、30 之一，0 表示一直保留）→ {retention_days}；{operation:"clear"} 删掉云端全部条目、开关不变 → {}；端到端加密的设备管理 {operation:"devices"}、{operation:"approve"|"reject"|"remove",device_id}（device_id 是 32 位小写十六进制）、{operation:"reset"} → 驱动 status 的结果 {enabled,status,device_id,code,devices:[{device_id,name,created_at,approved,current,code}]}（见 docs/cloud-clipboard-e2e.md 的「共享面板的动作」）。action 不合法时 error 是 invalid_cloud_clipboard，不联网；没有登录或会话已失效时是 account_unauthorized，驱动给出的码原样交回（例如 clipboard_device_pending、clipboard_sync_disabled），其他失败是相应的 account_* 代码（置顶的条目已经不在时服务端回 404，是 account_unavailable）。按 directory 时刷新令牌与共享应用共用 account-refresh.lock；按 session_store 时持宿主的 lock(ACCOUNT)。阻塞网络：放在工作线程上调用。 */
/* 可选 preferences_directory 绑定账号路径的偏好来源；当前已配置中继时返回 clipboard_source_changed，不发账号请求。可选 create_identity 为 false 时只用本机已有的设备身份：本机还没有驱动解析得了的密钥串时，经驱动的动作（list、add、devices、approve、reject、remove、reset）答 clipboard_identity_missing，不生成新身份、不登记设备、不联网，其余动作照常；不给或为 true 时与以前相同，第一次使用时生成并登记。给与另一个进程共用同一份身份的宿主用（鸿蒙键盘扩展沿用设置应用经共享沙箱镜像过来的密钥串，同一台设备只登记一次）。 */
char *msime_client_account_cloud_clipboard(const uint8_t *request, size_t length);
/* 云剪贴板该走哪条路。directory 是 length 字节（最多 16384）的共享偏好目录绝对路径。偏好里的 cloud_clipboard_relay 配置了中继、或者 cloud_clipboard_webdav 配置了 WebDAV 网盘（docs/cloud-clipboard-webdav.md，客户端在本机扮演中继）时 Value 是 {source:"relay", backend:"relay"|"webdav"}，否则是 {source:"account"}；两者都带不含凭据的偏好 revision，供窗口绑定来源。网盘也报 source "relay"：下面两个中继入口按配置自己分派，宿主不必分辨，只在要给用户说明是哪一种时看 backend。配置了中继或网盘就只走它，不再用水杉账号。偏好读不了时 error 是 storage，配置不合规（包括中继和网盘同时配置）时是 relay_not_configured。不联网。 */
char *msime_client_cloud_clipboard_source(const uint8_t *directory, size_t length);
/* 经自建中继（协议见 docs/cloud-clipboard-relay.md）答一个云剪贴板动作。request 是 length 字节（最多 131072）的 UTF-8 JSON {preferences_directory: 共享偏好目录的绝对路径, directory: 本机放中继密钥串 cloud-clipboard-relay-keystore 的绝对路径目录, device_name: 可选，登记设备时给其他设备看的名称, action}。中继地址和令牌从偏好的 cloud_clipboard_relay 读；配置的是 cloud_clipboard_webdav 时经 WebDAV 网盘答同样的动作，日志的本机缓存 cloud-clipboard-webdav-state（只有密文和公开数据）和锁文件放在 directory 里。都没有配置时 error 是 relay_not_configured。可选 expected_revision 与读取配置的同一快照比较，不符时 error 是 clipboard_source_changed，不联网。可选 create_identity 为 false 时只用 directory 里已有的、绑定这个中继和空间的密钥串：没有时经驱动的动作答 clipboard_identity_missing（在 cloud-clipboard-relay.lock 里检查），不生成新身份、不登记设备、不联网，其余动作照常（与 msime_client_account_cloud_clipboard 的同名字段相同）。directory 在同一台设备上的输入法、设置和面板之间要相同，否则会登记成几台设备；密钥串退出水杉账号时不删除。action 与 msime_client_account_cloud_clipboard 的相同，两个入口接受同样的十二个动作，结果形状也相同：list、add、delete、set_enabled、pin、set_retention、clear，以及端到端加密的 devices、approve、reject、remove、reset（各自的字段和结果见 msime_client_account_cloud_clipboard）。action 不合法时 error 是 invalid_cloud_clipboard；中继拒绝令牌或网盘拒绝账号密码时是 relay_unauthorized，连不上是 relay_unavailable，密钥串读写失败是 relay_storage，网盘一直限流是 webdav_rate_limited，网盘上的日志比本机读到过的旧或被改写过是 webdav_rollback，其余是驱动给出的码（例如 clipboard_device_pending）。阻塞网络：放在工作线程上调用。 */
char *msime_client_relay_cloud_clipboard(const uint8_t *request, size_t length);
/* 向自建中继发端到端加密驱动（msime_client_cloud_clipboard_e2e）给出的一个请求，给自己跑驱动、自己保存密钥串的宿主用：Android 把中继的密钥串交给 AndroidKeyStore 加密，不经上面那个由 Rust 把密钥串存成明文文件的入口跑驱动。request 是 length 字节（最多 131072）的 UTF-8 JSON {url: 中继基地址, token: 中继令牌, method: GET|PUT|POST|DELETE, path: 驱动给的路径（必须在 /v1/users/me/clipboard 之下）, body: 可选，驱动给的 JSON 正文文本}。url 和 token 是宿主在这次操作开始时从偏好的 cloud_clipboard_relay 读到的那一份，整次操作都传同一份，中途改了偏好也不会发到另一个中继；校验规则与偏好相同，不合规时 error 是 relay_not_configured。Value 是 {status, body}，状态码和正文原样交回驱动。不读偏好、不读写任何密钥串。WebDAV 网盘改用 {backend:"webdav", url: 文件夹地址, username, password: 应用密码, directory: 放日志缓存 cloud-clipboard-webdav-state 和锁文件的绝对路径目录, method, path, body}，三项凭据来自偏好的 cloud_clipboard_webdav；directory 在同一台设备上要与 msime_client_relay_cloud_clipboard 的 directory 相同（Android 是 noBackupFilesDir/cloud-clipboard-relay），缓存里只有密文和公开数据；每个请求持有目录里的 cloud-clipboard-relay.lock。backend 缺省是 relay，每种后端只收自己的字段，多给或少给时 error 是 invalid_cloud_clipboard。请求不合规时 error 是 invalid_cloud_clipboard（不发出去），中继拒绝令牌或网盘拒绝账号密码时是 relay_unauthorized，连不上或响应过大是 relay_unavailable，网盘限流是 webdav_rate_limited，日志被回退是 webdav_rollback。阻塞网络：放在工作线程上调用。 */
char *msime_client_relay_cloud_clipboard_exchange(const uint8_t *request, size_t length);
/* WebDAV 设置的「测试连接」：保存 cloud_clipboard_webdav 之前在用户选的文件夹里试一遍日志要用的网盘行为（建探测子目录、独占创建两次第二次必须失败、刚写的马上列得出、四路并发独占创建恰好一个成功，最后删掉探测目录；见 docs/cloud-clipboard-webdav.md）。request 是 length 字节（最多 131072）的 UTF-8 JSON {url: 文件夹地址, username, password: 应用密码, directory: 可选，本机中继密钥串所在的绝对路径目录}，用的是设置窗口里还没保存的草稿。通过时 Value 是 {}；给了 directory 时还删掉那里的日志缓存，防回退从网盘现在的样子重新开始。地址或凭据不合规时 error 是 relay_not_configured，网盘不合格是 webdav_unsupported（不要保存），账号或密码不对是 relay_unauthorized，连不上是 relay_unavailable，限流是 webdav_rate_limited。只证明语义，证明不了任何负载下都是原子的。大约二十个请求，阻塞网络：放在工作线程上调用。 */
char *msime_client_cloud_clipboard_webdav_probe(const uint8_t *request, size_t length);
/* 「我的词条」经用户的 WebDAV 网盘同步（协议和网盘上的布局见 docs/dictionary-webdav-sync.md）。同步的是「我的词条」列表里的词和它们的删除，身份是（词库类别、编码、词），权重、候选顺序、主词库和集合元数据都不同步；网盘只见到密文，密钥来自词库自己的密钥串（云剪贴板端到端加密协议的另一个实例，放在网盘的 msime-dictionary/keyring/ 下），新设备要由已获准的设备批准，与云剪贴板分开批准。request 是 length 字节（最多 2 MiB）的 UTF-8 JSON {options: 宿主传给 msime_client_create 的 HostOptions 文档（要有绝对路径的 preferences_directory，Engine 的词库路径也从它来）, directory: 放本机同步状态的绝对路径目录, device_name: 可选，登记设备时给其他设备看的名称, create_identity: 可选，见下文, action}。网盘的地址、账号和应用密码从偏好的 dictionary_sync_webdav 读（规则与 cloud_clipboard_webdav 相同；应用密码在凭据清单里，整组不随设置导出、不进账号设置同步和 MCP，诊断里换成 <redacted>），没有配置时 error 是 dictionary_sync_not_configured。directory 在同一台设备上的输入法、设置应用和面板之间要相同（否则会登记成几台设备），里面有 dictionary-sync-state（同步状态，含词条明文，只放应用私有目录、不备份不导出）、cloud-clipboard-relay-keystore（词库密钥串，与云剪贴板的那份不是同一个文件）、cloud-clipboard-webdav-state（密钥串的日志缓存）和锁文件 dictionary-sync.lock、cloud-clipboard-relay.lock；整次同步持有前一把，密钥串的操作在里面再持有后一把。action 是下面之一：{operation:"run", trigger: "local_change"|"opened"|"periodic"|"manual", confirm_deletions?: bool, restore_deletions?: bool} 跑一次同步：local_change 在用户改词之后马上调（设置页、导入、MCP、候选栏删除之后），记录本机改动，通常不联网、按防抖稍后发送；但发送已经到期时（本机改动从第一处没发出去的算起已满 10 分钟，即一直在改、一分钟的防抖总被推迟时的上限，并且离上一次发送已满 5 分钟），这一次就联网拉取并发送，所以它也要当作联网的调用（工作线程、需要用户授权的平台要先授权）；opened 是打开设置应用或词库页；periodic 是宿主按上一次给的 next_check_ms 定时再调；manual 是用户点「立即同步」，不受防抖和间隔限制。是否联网由 Rust 按额度决定（每台设备每 30 分钟一共最多 60 个请求，其中自动同步自己发的最多 40 个；manual，以及本机还没加入共享词库时不论哪个触发的那一轮，是用户的操作，只占 60 个的总数，见 docs/dictionary-webdav-sync.md 的「请求量」）。Value 是 {network, next_check_ms, status, report?|error?}：next_check_ms 是宿主下一次该在多少毫秒后用 periodic 再调；report 是 {captured, received_events, applied, deferred, published_ops, pending_key, compacted, rejoined}，deferred 为真表示输入法正占着词库、远端改动这一轮没写进去（不联网的重试由 next_check_ms 安排）；同步失败时没有 report，error 是下面的错误码之一，status 照样给出。confirm_deletions 和 restore_deletions 是用户对被挡下的大批删除（status.held_deletions）的选择：同步删除，或者从网盘把这些词写回本机。{operation:"status"} 不联网的概况 status：{device_id, code, approved, joined, words, pending_upload, pending_apply, held_deletions, clock_skew, last_success_ms, last_error, budget_wait_until}，device_id 和 code 是本机在词库密钥串里的编号和核对码，还没用过网盘时为 null；budget_wait_until 是自动同步因请求额度在等时什么时候继续（Unix 毫秒，本机额度不够一轮或被限流之后都算，没在等时为 null），期间 local_change、opened、periodic 不联网、改词照常记下，宿主据此显示「额度用完，将于某时继续同步」，manual 不受它限制。{operation:"devices"}、{operation:"approve"|"reject", device_id} 是词库密钥串的设备管理，结果形状与云剪贴板的 devices 相同 {enabled,status,device_id,code,devices:[{device_id,name,created_at,approved,current,code}]}；device_id 是 32 位小写十六进制。{operation:"remove", device_id} 移除一台设备并换钥（其他设备要重新批准），随后同步一次、把共享词库用新密钥重写，Value 是 {devices, run}，run 与 run 动作的 Value 相同；{operation:"reset"} 换一把只给本机的新密钥，同样答 {devices, run}；{operation:"restart"} 以本机为准重新开始（持有旧密钥的设备都不在了时由用户选择，其他设备的词随后合并进来，不删词），Value 是 status。{operation:"probe", url, username, password, trust_remote?: bool} 是保存 dictionary_sync_webdav 之前的「测试连接」，用设置页里还没保存的草稿，不读 options 里的偏好（options 照样要给）：与云剪贴板的测试连接相同的几项网盘行为，探测目录建在 msime-dictionary/keyring/ 下、测完删掉；通过时 Value 是 {}，默认不改本机同步状态、待发改动或防回退记录。只有用户明确确认恢复、传 trust_remote:true 时，才让 directory 里的本机状态从网盘现状重新开始（忘掉同步位置和待发改动，下一次按第一次加入，本机的词全部保留，但未上传删除可能重新合并回来）；这是用户自己清理过网盘文件夹之后、本机报 dictionary_sync_rollback 时的恢复办法。action 不合法时 error 是 invalid_dictionary_sync，不联网。其余错误码：dictionary_sync_unauthorized（网盘拒绝了账号或应用密码）、dictionary_sync_unavailable（连不上、超时或网盘答得不对）、dictionary_sync_rate_limited（网盘一直限流或本机额度用完）、dictionary_sync_too_large（响应超过上限）、dictionary_sync_rollback（网盘上的日志比本机读过的旧或被删改过）、dictionary_sync_unsupported（网盘上的格式是更新的版本，或测试连接不合格）、dictionary_sync_corrupt（日志被写坏了）、dictionary_sync_storage（本机状态读写失败），以及密钥串驱动给的码（例如 not_found、clipboard_key_conflict）。写本机词库一次一批（最多 128 项），每批之前取 Engine 的独占维护权、写完就放，从不跨网络请求持有；拿不到时 Linux、macOS、Windows 在 manual、opened、remove、reset、restart 时请输入法让出词库（50 毫秒间隔、2500 毫秒预算、租约 30 秒，写完这一轮就撤），其余情况和移动端直接留到下一次。除 status 外都会阻塞联网：放在工作线程上调用，不要在输入线程上等它。可选 create_identity 为 false 时只用 directory 里已有的、绑定这个网盘地址和账号的词库密钥串（与云剪贴板入口的同名字段相同）：一个安装只有一个设备身份，只由设置应用在第一次配置时生成。本机还没有时 run 答 {network: false, error: "clipboard_identity_missing", ...}（不建 directory、不记录本机改动、不联网），devices、approve、reject、remove、reset、restart 的 error 是 clipboard_identity_missing，同样不生成身份、不登记、不联网；status 和 probe 照常。不给或为 true 时与以前相同，第一次联网时生成并登记。给与设置应用共用同一个 directory 的输入法进程、键盘扩展用。 */
char *msime_client_dictionary_sync(const uint8_t *request, size_t length);
/* 输入法进程里的「我的词条」自动同步：起一个后台线程，替宿主调上面的 run，宿主不用自己排定时器。macOS 的 IMK 进程、Linux 的 IBus 和 Fcitx5 宿主在进程里起一次，退出时 msime_client_dictionary_sync_stop。request 是 length 字节（最多 64 KiB）的 UTF-8 JSON {options_path: 运行时配置文件 runtime-options.json 的绝对路径, directory: 放本机同步状态的绝对路径目录，与设置应用的相同（macOS 是 ~/Library/Application Support/<设置应用 bundle id>/dictionary-sync，Linux 是共享状态目录下的 dictionary-sync）}。Value 是 {}；request 不合法时 error 是 invalid_dictionary_sync，线程起不来是 dictionary_sync_unavailable。已经在跑时换成这一次的请求。后台线程每一轮重读 options_path 和偏好：没配置 dictionary_sync_webdav，或者本机还没有设置应用生成的设备身份时，不读词库、不联网，每 5 分钟再看一次（只用已有身份，与 create_identity: false 相同，从不生成身份）。能同步时起来先调一次 periodic，之后按每次结果的 next_check_ms 调 periodic；每 5 秒看一次个人词库文件（Engine 日志库和它的 WAL）的大小和修改时间，变了之后等它安静一轮（一直在变时最多一分钟）再读一遍「我的词条」，与上一次同步开头核对过的相比真的加了或删了词才调 local_change，打字学到的权重和选择次数不算。不请输入法让出词库（输入会话占着词库时远端改动留到打开设置时写进来），另一个进程（设置应用）正在同步时最多等 5 秒，等不到 30 秒后再来。不在调用线程上做任何文件或网络操作。 */
char *msime_client_dictionary_sync_start(const uint8_t *request, size_t length);
/* 叫停上面的后台同步：不再开始新的一步，正在跑的那一次跑完为止（每一步都写盘，进程中途退出也不丢不重）。最多等 timeout_ms 毫秒让后台线程退出，UINT32_MAX 一直等到它退出；返回它是否已经退出，没有在跑时为 true。没等到时线程照样在那一次跑完后自己结束。代码可能被卸载的宿主（Fcitx5 插件）要一直等。 */
bool msime_client_dictionary_sync_stop(uint32_t timeout_ms);
/* 同一台设备上两个各有一份 Engine 日志的进程之间交换「我的词条」，让两份收敛成同一组词条：HarmonyOS 从 API 12 起设置应用和输入法扩展各在自己的沙箱里、各有一份个人词库，两边经共享沙箱里的发布文件交换（设计见 platforms/harmony/README.md「我的词条只有一份」）。「我的词条」与 msime_client_dictionary_sync 说的是同一组：日志里 user_inserted 而且没删的词条，身份是（词库类别、编码、词），权重和学习记录不交换。request 是 length 字节（最多 HostOptions 上限加 64 KiB）的 UTF-8 JSON {operation: "sync"|"publish", side: "settings"|"keyboard"（本方是谁）, options: 宿主传给 msime_client_create 的 HostOptions 文档（Engine 的词库路径从它来）, directory: 本方私有的绝对路径目录（放状态 user-words-state.json 和锁 user-words.lock，不存在时建出来）, theirs: 可选，对方发布文件的绝对路径（只有 sync 读它；不存在算没有）, publish: 可选，本方发布文件的绝对路径（给了就在存盘后原子替换它，上级目录不存在时建出来；不能与 theirs 相同）, max_changes: 可选，这次最多往本机词库写几项（1 到 4096，缺省 128）}。每个词条在本方记一个「最后写入者胜」的寄存器（混合逻辑时钟的时间戳、在不在）和本机词库里实际有没有它。sync 依次：把本机词库相对上次的变化记成带新时间戳的寄存器（side 是 keyboard 时，一次少了 50 个以上、并且至少是原有词条一半时不记删除，只记成本机没有，对方照样留着，算进 held_deletions：输入法里没有一次删这么多词的操作，多半是日志坏了被换成空的；设置应用一侧的大批删除来自用户的操作或 WebDAV 同步，照常记）；合并 theirs：对方的时间戳更大（相同时「在」胜过「不在」）就采用，本机词库与它不一致时写进去，每批最多 128 项、每批之前取 Engine 的独占维护权、写完就放，拿不到维护权或者到了 max_changes 时剩下的留到下一次（deferred），Engine 不收的照实记下、不再重试（rejected）；回收对方已经确认见过的墓碑（对方发布里的 merged 不小于墓碑的时间戳），墓碑超过 20000 个时去掉最早的；存盘，再发布。第一次运行时本方的词都记成新加的，合并的结果是两边的并集，从不删词；状态文件读不懂时同样从头开始。publish 只做第一步、存盘和发布，不读 theirs、不写本机词库、不要维护权，输入会话开着时也能调。Value 是 {captured, applied, rejected, deferred, held_deletions, words, theirs: "read"|"absent"|"unreadable", published}：captured 是新记下的本机改动数，applied 是写进本机词库的对方改动数，words 是本方寄存器里「在」的词条数；theirs 读不懂（格式不对、发布方与 side 相同、超过 32 MiB）时这次不合并，照样记录和发布。错误码：invalid_user_words_mirror（请求不合法）、user_words_busy（同一个 directory 正被另一次调用占着，稍后再试）、user_words_unavailable（读不出或写不进本机词库）、user_words_storage（状态或发布文件读写失败）、user_words_too_large（寄存器超过 100000 个或文件超过 32 MiB，什么都不存，两份词库各自不变）。不联网。会在一个只读事务里读整份日志，sync 还要写本机词库：设置应用放在工作线程上调；输入法扩展在没有输入会话开着的时候（建会话之前）调 sync，会话开着时只调 publish。 */
char *msime_client_user_words_mirror(const uint8_t *request, size_t length);
/* Anonymous usage reporting to https://api.msime.app/v1/telemetry/events (no credentials). Every request is UTF-8 JSON of length bytes; maximum 16 KiB (256 KiB for record_crash). directory is an absolute directory the host owns for its telemetry files (the queue telemetry.json, telemetry-state.json with the random install id, the session marker and telemetry-crashes/). platform is windows, macos, linux, android, ios or harmony (aliases such as win, darwin, ohos are mapped); version is the real app version. Consent: enabled (the usage_reporting switch, default on) or, for hosts on the shared preferences, preferences_directory to read usage_reporting from; with reporting off, begin and flush clear everything and send nothing.
 * begin {directory, platform, version, enabled?|preferences_directory?}：宿主启动时调用一次。为每个留下了崩溃记录、尚未结束的会话补报 session_crash（只留下标记的不算崩溃；同目录里几个进程各自的会话按 id 区分），把崩溃记录排成 crash 事件（路径只留文件名），排进当天的 active，写下新会话的标记。Value {enabled, crash_record_path?, session_id?, previous_session_crashed?, crashes?}。不联网。
 * end {directory, session?}：宿主正常退出，排进 session 事件。session 是 begin 返回的 session_id，给了就只结束这个会话，不给则结束标记里记着的会话（标记可能已属于同目录里后来启动的别的进程）。有对应会话在运行时 Value 为 true。不联网。
 * record_crash {directory, message, stack, session?}：给可以分配内存的崩溃处理（C++ terminate handler）用，只写会话的崩溃记录；message 第一行是摘要，stack 是 module+offset 或符号形式的帧。session 是 begin 返回的 session_id，给了就写这个会话的记录，不给则写标记里记着的会话的记录。没有会话在运行或记录已存在时 Value 为 false。async-signal 处理函数改为自己写 begin 返回的 crash_record_path：open(O_WRONLY|O_CREAT|O_EXCL, 0600)、摘要行、'\n'、各帧。
 * record_crash {directory, message, stack, pid}：系统事后报告的崩溃（iOS 的 MetricKit 诊断）。pid 是崩溃进程的进程号：它有尚未结束的会话时写成那个会话的记录，下次启动连同 session_crash 补报；该进程这次崩溃已由它自己的处理程序记下时不写，Value 为 false；没有会话时写成单独的记录，只报 crash。
 * record_crash {directory, message, stack, outside_session: true}：系统事后报告的强制结束（SIGKILL：看门狗、内存上限、挂起时持有文件锁），不算会话崩溃，总是写成单独的记录，只报 crash。带 pid 或 outside_session 的两种会加锁，不能在崩溃处理程序里调用；session、pid、outside_session 三者互斥。
 * flush (same request as begin): queues today's active and sends the queue oldest first; 400 drops an event, 429/5xx/network keep it (Retry-After honoured across processes). Value {enabled, sent, dropped, remaining, deferred}. Blocks on the network: background thread only.
 * clear {directory}: usage reporting was just turned off. */
/* begin 的可选 capture_only:true 为设置或辅助进程准备独立崩溃记录，不触碰输入会话；component 是只含字母数字、点、下划线和连字符的组件名（默认 input）。成功时另外返回 crash_record_header：新宿主在信号路径中把它原样写在摘要之前，或把它与 session 一起传给 record_crash。系统事后诊断也可以传原构建的记录头；不带 session 的普通写入拒绝记录头，避免静默忽略归属。事后诊断可另带长度不超过 256 字节的 diagnostic_id，重复送达沿用同一个事件 id；不得传入原始描述、路径或输入内容。 */
char *msime_client_telemetry_begin(const uint8_t *request, size_t length);
char *msime_client_telemetry_end(const uint8_t *request, size_t length);
char *msime_client_telemetry_record_crash(const uint8_t *request, size_t length);
char *msime_client_telemetry_flush(const uint8_t *request, size_t length);
char *msime_client_telemetry_clear(const uint8_t *request, size_t length);
/* 持续上报同意：{directory:偏好目录绝对路径,edition?:包内产品版本}，返回 {enabled:false} 或 {enabled:true,epoch,since_unix_ms}。首次使用原子保存本机代次，edition 仅在首次宿主准备前保持本版本的偏好默认值；读写失败保持错误，不清已有报告。begin/flush 可带 consent_epoch 与 preferences_directory，不能同时给 enabled；旧代次返回 enabled:false，不清新代次队列。系统 record_crash 同样可带这两项及 diagnostic_begin_unix_ms、diagnostic_end_unix_ms，仅接收完全位于该次持续同意中的诊断批次，并要求原构建记录头。知道原捕获 UUID 的系统补录可带 diagnostic_capture 和 outside_session:true，与现场记录共用原身份；它不与 pid 或 diagnostic_id 混用，原输入会话仍按 UUID 归属。clear 可带 preferences_directory，仅在开关当前仍关闭时清理，防旧回调删除重开后的新报告。故障处理器不调用 consent，也不读偏好，只写预备的记录头。 */
/* 只读授权来源：consent {authority_directory} 不建目录或锁；权威设置进程发布时另带 directory 与 publication:{policy_allowed,policy_identity}。暂停返回 {enabled:false,suspended:true}，不能当作撤回清队列。begin/flush/系统 record_crash/条件 clear 可用 authority_directory 代替 preferences_directory，两者互斥；前三者仍必须带 consent_epoch，每条发送前短暂核对共享授权，网络期间不持读锁。 */
char *msime_client_telemetry_consent(const uint8_t *request, size_t length);
/* Notices from GET https://api.msime.app/v1/notices (no credentials). request {directory, platform, channel?} (channel app by default); directory is an absolute directory for notices.json. Value {items:[{id,title,body,html,targets,channels,published_at}]}, newest first, without the ones dismissed; html is body rendered as by msime_client_markdown_to_html. The feed is fetched at most once a minute (the cached copy otherwise, and when the request fails). Blocks on the network: call off the input thread when the settings window or app home opens, never on a timer in the input method. */
char *msime_client_notices(const uint8_t *request, size_t length);
/* request {directory, id}: remember that the user dismissed notice id. */
char *msime_client_notice_dismiss(const uint8_t *request, size_t length);
/* 对照 github.com/metasequoiaime/msime 的发布列表检查更新（不带凭据）。request 为 {platform, current_version, edition?, arch?, channel?}：platform 是发布 tag 前缀（windows、linux、macos、harmony 等），current_version 是当前运行的版本号，edition 是版本 id（缺省为 full），arch 是 Rust 的架构名，channel 是更新通道 stable（缺省）、beta 或 nightly（其他值让请求不合法；windows7 一律按 stable）。stable 只认正式发布；beta 另外接受 tag 恰为 <platform>-vX.Y.Z-beta 的预发布；nightly 再接受滚动的 nightly 发布里本平台的构建（另外 GET .../releases/tags/nightly）。同一版本号按发布 workflow 的运行 ID 比较新旧，当前构建的运行 ID 是编进库里的 MSIME_BUILD_RUN，没有时只比版本号。返回值为 {status:"available"|"current", update:{version:{display,parts}, release_url, installer_name, installer_sha256, signed, channel, build_run}}，该平台还没有发布时为 {status:"none"}。channel 是这个构建所属的通道（stable、beta、nightly），build_run 是它的运行 ID（没有时为 null）。在 linux 和 windows 上 installer_name 和 installer_sha256 是本版本唯一的安装包（没有、有不止一个、或没有摘要时为 null）；signed 在这两个平台为 false，其他平台为 null。会在网络上阻塞，每个请求最多十秒，不要在 UI 线程或输入线程上调用。 */
char *msime_client_update_check(const uint8_t *request, size_t length);
/* 后台检查更新。request 为 {directory, platform, current_version, edition?, arch?, channel?}：在 msime_client_update_check 的请求上加 directory，宿主状态目录的绝对路径，记录 update-check.json 和锁 update-check.lock 放在这里，同一宿主的 msime_client_update_pending 要用同一个目录。距上次成功检查不满一天、距上次失败不满六小时、另一个进程正在检查时不联网；通道和记录的不同时立刻检查。开发构建（debug_assertions）或构建时、运行时设置了 MSIME_DISABLE_UPDATE_CHECK（非空且不是 0）时什么也不做。返回值为 {status:"disabled"|"not_due"|"unavailable"|"checked", notify}：notify 为 null，或者是这次该提醒的构建（形状同 msime_client_update_check 的 update），同一个构建（版本号和运行 ID）只给一次，返回时已经记成提醒过。没到时间时只读一个小文件，可以每小时调用；到时间时会在网络上阻塞，不要在 UI 线程或输入线程上调用。 */
char *msime_client_update_background_check(const uint8_t *request, size_t length);
/* 后台检查已经找到、仍比当前运行的构建新的版本，关于页打开时先显示。request 与 msime_client_update_background_check 相同。返回值为 {status:"available", update}，没有记录、记录按别的通道检查、或已经不新时为 null。只读记录，不联网、不拿锁。 */
char *msime_client_update_pending(const uint8_t *request, size_t length);
/* text is UTF-8 Markdown of length bytes; maximum 256 KiB. Value is HTML for a rich-text view: raw HTML in the source is escaped, only http, https and mailto links are kept, images become links and are never loaded. Open links externally. */
char *msime_client_markdown_to_html(const uint8_t *text, size_t length);
/* Community moderation for hosts that send community requests through their own HTTP stack. Request is UTF-8 JSON of length bytes; maximum 64 KiB. Publishing is post-moderated: an item is public at once and moderators may remove it. To learn the state of the user's own items add fields=moderation to scope=mine lists and to the detail request (skins, candidate-skins, plugins, resources); each own item then carries moderation "approved"|"pending"|"removed". Show only a 已下架 badge for removed and treat pending as approved; never show a reason. The one exception is a plugin of kind script, which is pre-moderated: until an administrator approves it only its owner sees it (everyone else gets 404 from list, search, detail, download, rating, save and report), so for an own script item pending means 审核中 and should be shown as such. Without fields=moderation the responses are unchanged.
 * {operation:"reasons"}: value is the report reasons in dialog order, each the exact string to send: ["侵权/抄袭","色情低俗","违法违规","垃圾广告","恶意插件","其他"].
 * {operation:"report",kind,item_id,reason,detail?}: kind is skins, candidate-skins, plugins, dictionaries or replies; item_id the item's UUID; reason one of the reasons; detail optional, at most 1000 characters. Value {method:"POST",path:"/v1/community/reports",body}: send body as JSON with the signed-in session's bearer (the device's anonymous account counts). A bad field is error community_invalid.
 * {operation:"error",status,body?}: status and body of a failed community response. Value {code,message,retry}: code is account_blocked_content (422 blocked_content: the text must change; never say the service is down), account_screening_unavailable (503 screening_unavailable: honour Retry-After), account_banned (403 account_banned) or the generic account_* code; message is the Chinese sentence to show for the first three and null otherwise; retry says whether the same request may be sent again later. */
char *msime_client_community_moderation(const uint8_t *request, size_t length);
/* Google 登录：由宿主自己监听回环端口、打开系统浏览器、申请 challenge 并提交授权码（鸿蒙）。这里给出与桌面端相同的判定；申请 challenge 和提交授权码可以经自己的 HTTPS 栈，也可以用 msime_client_account 的 google_challenge {target} 和 google_code_login。Request 是 UTF-8 JSON，最多 32 KiB。
 * {operation:"target",port}: port 是宿主在 127.0.0.1 上监听的端口（>=1024）。Value {target}: 向 POST /v1/auth/challenges 发 {provider:"google",target,purpose:"login"} 时用的回跳地址。
 * {operation:"plan",authorization_url,target,expires_in}: 后端返回的 authorization_url 与 expires_in。Value {state,wait_ms,max_request_bytes,io_timeout_ms}: 链接通过校验，可以用系统浏览器打开；最多等 wait_ms 毫秒；每个连接最多读 max_request_bytes 字节、最多 io_timeout_ms 毫秒拿到请求头，超出就以 head:null 回复并关掉。链接不是指向这个监听的 Google 授权链接、或 challenge 太短时返回 account_unavailable，不要打开它。
 * {operation:"reply",head,state,return_link?}: head 是一个连接读到的请求头（到第一个空行为止，含），没读到完整请求头时为 null；state 是 plan 给的；return_link 可省略，是宿主注册的、能把应用拉回前台的 `scheme://路径` 链接（如鸿蒙的 msime://auth/google），回跳页据此画「回到水杉输入法」按钮，形状不合格时不画，回复照常。Value {outcome,code?,error?,response}: 把 response 原样写回这个连接再关闭。outcome 为 ignored 时继续等下一个连接；code 时用 POST /v1/auth/login {challenge_id,credential:code} 完成登录；failed 时结束登录，error 是 account_cancelled（用户在 Google 页面拒绝）或 account_unavailable。 */
char *msime_client_google_loopback(const uint8_t *request, size_t length);
/* 原生设置窗口的账号操作，与共享应用（Tauri）的十四个 account_* 命令和 Windows 的四个 account_preferences_* 设置同步命令相同，用同一份本用户会话文件。Request 是 length 字节（最多 4 MiB）的 UTF-8 JSON {directory, legacy_directory?, client_agent?, action}：directory 是共享应用登录时写 account-session.json 的目录的绝对路径（Windows 是 %LOCALAPPDATA%\<本版本的用户目录>\account，与 msime_client_account_access_token 相同）；legacy_directory 可选，是旧版本放会话的目录（Windows 是 %LOCALAPPDATA%\<tauri identifier>），本进程第一次用到 directory 时，directory 还没有会话而这里有，就把它搬过去。client_agent 可选（#8201 的决定 1），是这个宿主自报的 User-Agent，形状与 msime_client_session_store 的 client_agent 相同（msime-<android|ios|macos|windows|linux|harmony>/<版本> (<设备>; <系统>; edition=<id>)，括号部分可省略，最多 256 字节可打印 ASCII，用 msime_client_client_agent 拼出），不合法时是 account_invalid；这次调用的请求（包括登录、绑定和匿名账号的注册与重新登录，服务端「我的设备」按登录时的 User-Agent 显示平台和设备）都以它发出，不给时与以前一样是 MSIME/Android。每个 directory、每个 client_agent 在进程里只有一个会话对象：同一个宿主的各次调用要么都带同一个 client_agent、要么都不带，google_cancel 才结束得了同一个对象上的 google_login；会话文件每次都重新读，刷新令牌时与共享应用和输入法共用 account-refresh.lock。也可以用 {session_store, action} 代替 {directory, legacy_directory?, client_agent?}：session_store 是 msime_client_session_store_register 给的 id，会话在宿主注入的存储里，用注册时建好的会话对象；directory 和 session_store 恰好给一个，legacy_directory 和 client_agent 只能与 directory 一起给（注入存储的 User-Agent 是注册时给的那个），否则是 account_invalid。响应不含令牌。目录模式读 account-session.json 时三种布局都认：Native、Apple，以及鸿蒙 ArkTS 桥接以前写的扁平布局 {access_token,refresh_token,token_type,expires_at(Unix 毫秒),user}（只读，expires_in 按剩余时长四舍五入补出、至少 1，没有 session_id；第一次认证请求前补上 session_id、之后每次保存都写成 Native）。鸿蒙上 directory 里的 account-session.json 和 cloud-clipboard-keystore 带属组读写位也照样读（@ohos.file.fs 没有 chmod，ArkTS 按应用 umask 写），与 anonymous-session.json 相同，其他用户位仍然拒绝；其他平台仍要求只有属主可读写。
 * action 是下列之一，value 与共享账号页（packages/ui/src/account/account-page.tsx）读到的形状相同；user 是 {id,displayName,createdAt,email?,avatarUrl?,avatarUploaded?}，avatarUrl 只在是本客户端会去取的头像地址时出现，avatarUploaded 只在头像是用户上传的（可以移除）时为 true：
 * {operation:"status"} → {user|null}，只读本地文件。{operation:"providers"} → {email,phone,apple,google,huawei,apple_web}：都是布尔值，是服务端 GET /v1/auth/providers 对应的开关（phone 也认服务端的 sms），没有的算 false；apple_web 是 Apple 网页登录（没有 Apple 原生登录的 Android 用，见下面的 apple_web_start），与 Apple 原生登录的 apple 分开；huawei 是华为账号登录（鸿蒙的 Account Kit，见下面的 huawei_challenge）。
 * {operation:"request_code",provider:"email"|"phone",target} → {challengeId,expiresIn}。{operation:"login",challenge_id,code}（code 是 6 位数字）→ {user}。
 * {operation:"google_login"} → {user}：在 127.0.0.1 上监听回环端口、申请 Google challenge、用系统浏览器打开校验过的授权链接（只在 Windows 上；其他平台是 account_unavailable），等浏览器回跳后登录。最多阻塞约 5 分钟；用户在 Google 页面拒绝或调用了 google_cancel 时是 account_cancelled。{operation:"google_cancel"} → null，立即返回，结束同一 directory 上进行中的 google_login。两者都只有 operation 一个字段，多带的（例如 purpose）是 account_invalid。
 * 宿主自己弹登录界面的登录（#8201），挑战和凭据都经这里，宿主不用自己发 /v1/auth 请求；登录成功返回 {user}，与 login 一样存进 directory 或 session_store 的登录会话。nonce、授权链接和 codeVerifier 只给宿主自己用，不要交给网页、不要写进日志。
 * Apple 原生登录（iOS、macOS 的 ASAuthorizationAppleIDProvider）：{operation:"apple_challenge"} → {challengeId,expiresIn,nonce}，nonce 原样放进 ASAuthorizationAppleIDRequest 的 nonce、challengeId 放进 state，回来的凭据 state 不等于 challengeId 时丢掉；{operation:"apple_login",challenge_id,identity_token}（identity_token 是凭据 identityToken 的 UTF-8 文本，非空、最多 16 KiB、不含控制字符）→ {user}。每个 challenge 只能用一次，失败或取消后重新申请。
 * Google ID token（iOS 的 GoogleSignIn、Android 的 Credential Manager）：{operation:"google_challenge"} → {challengeId,expiresIn,nonce}，nonce 原样交给 Google；{operation:"google_token_login",challenge_id,id_token}（id_token 是三段非空 base64url 用 . 连成的 JWT，最多 12000 字节）→ {user}。
 * 华为账号（鸿蒙设置应用的 Account Kit）：{operation:"huawei_challenge"} → {challengeId,expiresIn,nonce}，nonce 原样放进 AuthorizationWithHuaweiIDRequest 的 nonce（scopes 为 ["openid"]、permissions 为 ["idtoken"]）；{operation:"huawei_login",challenge_id,id_token}（id_token 是凭据 idToken，形状要求与 google_token_login 相同）→ {user}。服务端按 ID token 的 sub（UnionID）认人。两者都没有 purpose 字段，带了是 account_invalid。
 * Google 回环（自己监听回环端口的宿主，鸿蒙）：{operation:"google_challenge",target}（target 是 msime_client_google_loopback 的 target 给的回跳地址）→ {challengeId,expiresIn,authorizationUrl}，授权链接已校验过是指向 target 的 Google 授权链接，否则是 account_unavailable；再交给 msime_client_google_loopback 的 plan，等到回跳的 code 后 {operation:"google_code_login",challenge_id,code}（code 非空、最多 2048 个可打印 ASCII）→ {user}。target 不是 http://127.0.0.1:<端口>/callback 或 http://[::1]:<端口>/callback（端口 1024–65535）时是 account_invalid。
 * Apple 网页登录（没有 Apple 原生登录的 Android）：{operation:"apple_web_start",app}（app 是发起登录的 applicationId，小写字母开头的点分段，最多 128 字节；服务端只认 shared/contracts/editions.json 里各版本的 android.application_id，其余是 account_invalid）→ {authorizationUrl,expiresIn,codeVerifier}：Rust 生成 32 字节随机数的 codeVerifier，只把它的 S256 摘要发给 POST /v1/auth/apple/web；authorizationUrl 已校验过是 https://appleid.apple.com/auth/authorize、只带一个 state，用浏览器打开它；codeVerifier 由宿主私下保存（浏览器打开期间进程可能被杀），回调 <app>://auth/apple?grant=… 到达后 {operation:"apple_web_login",grant,code_verifier}（grant 是 32 字节的无填充 base64url，43 个字符）→ {user}。服务端不认这个 grant（不认识、已用过、过期或不属于这个 verifier）时是 account_invalid，这时保留 codeVerifier 等真正的回调，其余失败可以丢掉它；回调带 error=… 时不用调这里。
 * 给已登录的账号添加登录方式（#8201 的决定 9，Android 的「添加登录方式」）：邮箱、Google ID token 和 Apple 网页登录的申请和完成两步都可以带 purpose，取 "login"（缺省，与不带相同）或 "link"，其他值是 account_invalid。{operation:"request_code",provider:"email",target,purpose:"link"}（只能是 email）→ {challengeId,expiresIn}，再 {operation:"login",challenge_id,code,purpose:"link"}；{operation:"google_challenge",purpose:"link"}（不能带 target，回环流程不能绑定）→ {challengeId,expiresIn,nonce}，再 {operation:"google_token_login",challenge_id,id_token,purpose:"link"}；{operation:"apple_web_start",app,purpose:"link"} → {authorizationUrl,expiresIn,codeVerifier}，回调到达后 {operation:"apple_web_login",grant,code_verifier,purpose:"link"}，宿主要和 codeVerifier 一起记住这次是绑定。Apple 原生登录、华为账号登录、Google 回环、google_code_login 和 google_login 没有 purpose 字段，带了是 account_invalid。两步都只用登录会话（directory 的 account-session.json 或该 store 的 ACCOUNT 项）并带着它的令牌请求：快过期时先刷新；申请时令牌被 401 拒绝就刷新一次、在同一个账号里重试，完成时不重试（验证码不对也是 401）；没有登录时是 account_unauthorized，不联网，不会用匿名账号。完成后 Value 是 {user}：服务端把这种登录方式绑到该账号，并为它签发一个新会话，Rust 在刷新锁里用它替换本地的登录会话（同一个账号、新的登录 ID，旧会话在服务端留到过期）；完成请求进行中本地退出、换了账号或重新登录时是 account_cancelled，绑定在服务端已经生效，新会话丢掉。服务端要求登录会话是 10 分钟内建立的（刷新令牌不算），否则两步都是 account_recent_login_required，宿主让用户以 purpose:"login" 重新登录同一个账号后再从申请开始；这种登录方式已经绑在别的账号上是 account_conflict；验证码不对、令牌失效或完成时的账号不是发起绑定的那个是 account_unauthorized；Apple 的 grant 不认识、用过、过期或属于另一个账号发起的绑定是 account_invalid（保留 codeVerifier，与登录相同），完成这一步的其他失败都已经用掉了 grant，要从 apple_web_start 重新开始。失败按下面 #8201 新操作的规则带细节：限流是带 retry_after_seconds 的 account_rate_limited，没拿到响应是 account_network。
 * {operation:"profile"} → {user,providers:[...]}（providers 是已绑定的登录方式，去重）。{operation:"rename",display_name}（非空、首尾无空白、最多 64 个字符）→ 同 profile。
 * {operation:"avatar"} → null 或 "data:<image/png|image/jpeg|image/webp>;base64,<...>"（未登录或没有头像时为 null）：类型按下载到的文件头定，下载时声明接受 WebP，没上传过头像的 Google 登录用户的头像在 Google 的图片主机上，它按 Accept 内容协商，多半回 WebP，宿主要能解 WebP；上传仍只收 PNG 和 JPEG。{operation:"upload_avatar",path}：path 是宿主自己的文件对话框选中的 PNG 或 JPEG 的绝对路径，最多 1 MiB → 同 profile；不是这样的文件时是 account_invalid。{operation:"remove_avatar"} → 同 profile。
 * {operation:"logout",all} → null（all 为 true 时退出所有设备；本地会话总会先清掉）。{operation:"delete"} → {voice_samples}（注销账号，成功后清掉本地会话；voice_samples 见下面的 account_delete，以前 value 是 null）。{operation:"forget"} → null（只清掉本地会话，用于会话已失效时）。
 * {operation:"ensure_anonymous"} → true：本机匿名账号还没有可用的会话时注册它。带 directory 时与 msime_client_ensure_anonymous_account(directory) 相同（anonymous-account.json 和 anonymous-session.json 在 directory 里）；带 session_store 时身份在宿主的 ANONYMOUS_IDENTITY 项、会话在 ANONYMOUS 项，身份按「不存在才写」在锁外确定，登录时持 lock(ANONYMOUS)。已有会话时只读一次存储、不联网；否则阻塞网络。失败时 error 是账号错误码，不是 msime_client_ensure_anonymous_account 那样的说明文字。
 * 设置同步（共享账号页的设置同步卡片）：{operation:"preferences_schema"} → {fields,maximumBytes,updateMode,revisionRequired}，可以原样交给 msime_client_account_settings_export、msime_client_account_settings_apply 和 iOS、鸿蒙的设置导出与应用：它们的 schema 既认服务端的 snake_case（maximum_bytes、update_mode、revision_required），也认这里的 camelCase，同一个键两种写法都给时按请求不合法处理。{operation:"preferences_load"} → 云端设置文档 {revision,settings}。{operation:"preferences_upload",preferences_directory}：preferences_directory 是偏好目录的绝对路径；本机各平台共有的 input.* 设置按本目录所属的版本过滤、只留字段表收录的键，合并进云端文档后上传，返回上传后的云端文档；一个键都没有时是 account_unavailable，云端文档已被别处更新时是 account_conflict。{operation:"preferences_apply",preferences_directory,user_id,preferences}：preferences 是刚由 preferences_load 下载的文档，user_id 是下载时的账号（期间退出、换号或重新登录时是 account_cancelled）；按版本过滤后把其中的 input.* 键应用到本机偏好，按读到时的修订号比较并交换写回（关掉剪贴板历史时与 msime_client_save_preferences 一样清掉已存的历史），返回保存后的 PreferencesSnapshot。写回失败时 error 是 conflict（本机设置已在别处更新）或 storage，与共享应用相同。
 * 以下是 #8201 新增的操作，value 用服务端的 snake_case 形状（chat_models 除外，与共享账号页相同）。令牌从哪来：account_delete、phrases_*、preferences_put、sessions_list、session_revoke、data_*、download_link 只用登录账号（注销成功后、data_summary 和 data_delete 的 voice 分区另见下）；chat、chat_models、translate、feedback、diagnostics_upload 登录账号优先，没有登录时用本机匿名账号（带 directory 时是同一 directory 里的 anonymous-session.json，directory 还不存在时当作没有；带 session_store 时是该 store 的 ANONYMOUS 项；Rust 不会自动注册匿名账号，宿主要先调 ensure_anonymous）；voice_contribution 和 voice_delete 只用本机匿名账号，登录了也一样；diagnostics_state、diagnostics_regenerate_token、diagnostics_delete 绑定到上传时返回的 user_id，登录会话和匿名会话都不是它时是 account_identity_changed，不会退回另一个账号。服务端以 401 拒绝令牌时带着它刷新一次、只在同一个账号里重试。这些操作失败时 error 之外可能还有 retry_after_seconds，是服务端 Retry-After 的整数秒，只在已知时出现：{"ok":false,"error":"account_rate_limited","retry_after_seconds":3600}。请求发出去以后没有拿到响应（连不上、DNS 或 TLS 失败、超时、连接在响应头到达之前断开）时是 account_network（请求在本地就建不出来是 account_invalid，重定向被拒是 account_unavailable，都不算断网），服务端回了 5xx 时是 account_unavailable（503 server_busy 是带 retry_after_seconds 的 account_rate_limited），宿主据此分开「网络连接失败」和「服务暂不可用」；令牌要先刷新、而刷新请求本身没拿到响应时仍是 account_unavailable。
 * {operation:"chat",messages:[{role:"system"|"user"|"assistant",content}],model} → {content}：最多 16 条、每条非空白且最多 16 KiB，序列化后的请求体超过 65536 字节是 account_too_large；一次交回整段回复，不能取消，最长阻塞 125 秒，要边生成边显示或能停下来时用 msime_client_account_chat_stream。{operation:"chat_models"} → {data:[{id}],defaultModel}。两者被服务端限流时是 account_rate_limited，带 retry_after_seconds（超过每分钟的请求数时服务端给 60）；请求进行中给出令牌的那个账号退出或换了号时是 account_cancelled，回答不交出去。
 * {operation:"account_delete"} → {voice_samples}：注销账号，与已有的 delete 相同（DELETE /v1/users/me，成功后清掉本地会话），失败时按上面的规则带细节：会话不是 10 分钟内登录的是 account_recent_login_required，这时本地会话留着，宿主让用户退出并重新登录后再注销。delete 的失败码不变，遇到需要最近登录时仍是 account_unauthorized。两者注销成功以后都再删一次本机匿名账号名下的语音样本（与 voice_delete 相同，可能多一次匿名重新登录），尽力而为：这一步失败不撤销、也不让注销失败，ok 仍是 true，结果在 voice_samples 里，形状与响应的信封相同：{ok:true}（删掉了，或本机从来没有匿名身份、没有样本可删）或 {ok:false,error,retry_after_seconds?}，error 是 voice_delete 会给的码；宿主据此提示语音样本没有删掉，稍后用 voice_delete 重试。注销失败时不做这一步。
 * {operation:"translate",texts,source_lang,target_lang,timeout_ms?} → {translations:[string|null]}：texts 1–32 条，每条非空白、最多 8192 字节；语言代码 2–16 个 [A-Za-z-]，发出时转成大写；timeout_ms 1000–30000，缺省 10000；序列化后的请求体超过 65536 字节是 account_too_large。响应的 code 必须是整数 200、data 必须与 texts 等长，否则是 account_unavailable；每一项把制表符和换行折成空格、去掉首尾空白，含其他控制字符、为空、与原文相同或规范化后超过 4096 字节（msime_client_apply_translations 每条释义的上限，超过一条它就整批拒绝）的那一项是 null，不让整批作废；单项超过服务端的 65536 字节仍是 account_unavailable。
 * {operation:"phrases_load"} → {revision,phrases:[{id,text,group,position}]}（新用户是 revision 0 和空列表）。{operation:"phrases_save",revision,phrases} → 保存后的同一形状：最多 500 条；id 是 1–64 个 [A-Za-z0-9_-] 且唯一；text 是 1–2000 个 UTF-16 单元、不含 NUL，只有空白也合法；group 最多 32 个 UTF-16 单元、不含控制字符；position 在 0..=1000000；请求体超过 256 KiB 是 account_too_large；revision 与云端不符是 account_conflict，不重试。
 * {operation:"preferences_put",document,revision,user_id?} → 上传后的云端设置文档 {revision,settings}：整份替换云端的设置文档（PUT /v1/users/me/preferences，服务端的 update_mode 是 replace），按 revision 比较并交换。document 是文档的 settings（键是 1–128 个 [A-Za-z0-9._-]，值只能是布尔、整数、有限的数或最多 1 MiB 的字符串，最多 512 个键），revision 是读到它时云端的修订号，通常就是设置导出入口给的 merged 的 revision 和 settings；服务端只在云端的修订号仍是 revision 时写入，写入后修订号加 1，已被别处更新时是 account_conflict，不重试，宿主重新读、重新合并。上传前不按字段表过滤，键不在服务端字段表里或类型、范围不对时服务端整份拒绝（account_invalid）。revision 为负、键或值不合规是 account_invalid，请求体超过 1 MiB 是 account_too_large，都不联网。user_id 是读文档时的账号（status 给的 user.id，1–128 字节）：给了时只写这个账号，会话已经退出、换成别的账号，或者请求期间退出、换号、重新登录时是 account_cancelled（请求期间变的，服务端可能已经写进了原来那个账号）；不给时写当前登录的账号。
 * {operation:"sessions_list"} → {sessions:[{id,platform,name,app_version,created_at,last_active,current}]}：最多 50 条；网页登录的 platform 是 web，User-Agent 认不出时 platform、name、app_version 是空串，都保留；类型不对的行跳过。{operation:"session_revoke",id}（id 是 64 位小写十六进制）→ {signed_out}：撤销的是当前会话时等同 logout{all:false}，本地会话同时清掉，signed_out 为 true；否则为 false。会话不存在是 account_not_found。
 * {operation:"data_summary"} → {bytes,sections:[{id,bytes,items}],anonymous_voice}，服务端新增的分区原样保留。语音贡献只用匿名账号，所以登录账号的汇总之后再用本机匿名账号请求一次 GET /v1/users/me/data，把它的 voice 分区的 bytes、items 加进 voice 分区（登录账号的汇总里没有时整个加进去），总 bytes 跟着加；anonymous_voice 说明这一步：included 是已经合进去，none 是本机既没有匿名会话也没有匿名身份、没有可合的，unknown 是这一次没有成功（读不了匿名会话或身份、断网、被拒等，或者身份还在而会话已经过期清掉：服务端的会话 30 天后过期，名下的样本还在，汇总不为此重新登录），这时 voice 只是登录账号的，宿主显示「语音样本数暂不可知」。这一步的失败不让汇总失败；登录账号的汇总失败时整个失败，不做这一步。{operation:"data_export",destination}：destination 是写入 zip 的绝对路径，Rust 不检查扩展名（宿主自己取名，建议以 .zip 结尾），父目录不能是符号链接 → {path,bytes,filename}，filename 是服务端建议的文件名；流式写进同目录的临时文件，读得出 zip 的中央目录才换到 destination，失败时 destination 上原有的文件不动；最长约 130 秒；每天 3 次，超了是 account_rate_limited 带 retry_after_seconds。{operation:"data_delete",sections}：sections 非空、不重复，取自 preferences、dictionary、phrases、clipboard、voice → 不含 voice 时是 null，含 voice 时是 {voice_samples}；会话不是 10 分钟内登录的是 account_recent_login_required。sections 含 voice 时，删完登录账号的分区以后还删本机匿名账号名下的语音样本（voice_contribution 只用匿名账号），与 voice_delete 相同；登录账号这一步失败（包括需要最近登录、没有登录时的 account_unauthorized）时整个操作以它的码失败，不碰匿名账号，也就不会白耗一次匿名重新登录。匿名这一步失败时登录账号的分区已经删掉，操作仍然成功，失败在 voice_samples 里，形状与 account_delete 的相同；宿主用 voice_delete 重试这一步，不要重做整个 data_delete（词库会再写一条 reset）。删除后云端的设置是 {}、常用语是 []、词库有一条 reset，这个操作不碰本机的同步状态。{operation:"voice_delete"} → null：删掉本机匿名账号名下的语音样本（DELETE /v1/users/me/data 的 voice 分区），不要求登录，给从没登录过的用户和上面两处要重试的宿主。先用现有的匿名会话删，服务端要求最近登录（匿名会话也一样）或令牌已失效时，用保存的匿名身份（directory 里的 anonymous-account.json 或该 store 的 ANONYMOUS_IDENTITY 项）重新登录同一个匿名账号、换一个新会话再删，新会话替换存储里的旧会话；重新登录计入服务端按地址每天的匿名登录限额，超了是 account_rate_limited。本机从来没有匿名身份时没有样本可删，直接成功、不联网；有会话却没有身份、换不了新会话时交回那次失败。只删得到这台设备的匿名账号。
 * {operation:"feedback",type:"bug"|"suggestion"|"dictionary",text,platform,app_version,edition,diagnostics?,screenshots?} → {id,status}：text 去掉首尾空白后码点数和 UTF-16 单元数都不超过 500，控制字符只允许 \n 和 \t；platform 是 android、ios、macos、windows、linux、harmony 之一；app_version 是 1–64 个码点、不含控制字符；edition 是 0–32 个 [a-z0-9._-]；diagnostics 是字符串到字符串的对象，键只留 device、os、app_version、edition、scheme、keyboard_layout、skin、ime_enabled、ime_default，控制字符换成空格、按字节截到 256、空值丢掉；screenshots 最多 3 个 PNG 或 JPEG 文件的绝对路径，只读文件头和尺寸、不重新编码，一张超过 1 MiB、单边超过 8192 或总像素超过 8388608 是 account_too_large。以 multipart 发送，最长 60 秒。
 * {operation:"diagnostics_upload",platform,app_version,ttl:"one_hour"|"one_day"|"seven_days",bundle:{state_root,include,sources}} → {id,mcp_url,token,expires_at,counts,user_id,anonymous}：bundle 与 msime_client_diagnostic_bundle 的请求相同，只是没有 destination；platform 和 app_version 是 [A-Za-z0-9][A-Za-z0-9._+-]{0,63}；一个分类都没勾选是 account_invalid。上传前按服务端的规则重建：事件只留服务端认的 10 种 kind，只留 t_ms、kind、duration_ms，每类最多 5000 条；崩溃记录最多 200 条，at 重新格式化成 RFC 3339，解析不了的整条丢掉；配置快照超过 8 层的子树换成 "<truncated>"、条目满 2000 后不再加入、字符串截到 1024 字节，像凭据的键的值换成 "<redacted>"；整份超过 2 MiB 时先截掉最旧的事件，去掉全部事件仍超过时是 account_too_large。counts 是重建后各类保留、丢弃和截掉的条数，形状与 msime_client_diagnostic_bundle 的相同；user_id 和 anonymous 是上传所用的账号，之后的三个诊断操作要带上这个 user_id；token 只出现这一次，不要写进日志。读本机记录失败时 error 是 diagnostics_* 码。服务端每小时最多 6 次。
 * {operation:"diagnostics_state",user_id} → {snapshot:null|{id,created_at,expires_at,bytes,sections,token_hint},accesses:[{at,tool,arguments,result_count,bytes}]}。{operation:"diagnostics_regenerate_token",user_id} → {token}：旧令牌立即失效，没有有效快照时是 account_not_found。{operation:"diagnostics_delete",user_id} → null，没有快照也算成功。
 * {operation:"download_link",platform}（platform 是 windows、macos、linux、harmony-pc、ios、android、harmony 之一）→ {sent_to}：服务端把这个平台的下载页链接（固定模板）发到账号已验证的邮箱（邮箱登录身份优先，其次是已验证的 Google 邮箱），收件地址由服务端决定；sent_to 是打码后的收件地址（例如 u***@example.com），服务端已经发出、但这个字段缺失或不合规（空、超过 320 字节、含控制字符）时为 null。没有已验证邮箱是 account_no_verified_email，宿主改为提示复制链接；邮件没配置或发送失败是 account_unavailable；每个用户每小时 3 次、每天 10 次。
 * {operation:"voice_contribution",language,provider,transcript,app_version,audio} → {id,duration_ms}：只在用户打开语音贡献、且隐私判断允许时调用。audio 是 WAV 文件的标准 base64，解码后最多 2 MiB（超了是 account_too_large），按服务端的 RIFF 结构规则校验（恰好一个 fmt 和一个非空 data，PCM 8/16/24/32 位或浮点 32/64 位，1–8 声道，8000–192000 Hz）；duration_ms 由 Rust 从 data 大小和字节率算出（向下取整），要在 1–60000 之间，宿主不用传；不收 Ogg。language、provider、app_version 是 [A-Za-z0-9][A-Za-z0-9._+-]{0,63}；transcript 去掉首尾空白后发送，非空、最多 2000 个码点，控制字符只允许 \n 和 \t。以 multipart 发送，最长 60 秒；只用本机匿名账号，没有匿名会话时是 account_unauthorized（不退回登录账号）；服务端每个账号每小时 30 次，样本挂在这台设备的匿名账号下、保存 180 天，voice_delete、data_delete 的 voice 分区和注销登录账号（delete、account_delete）成功以后都会删掉它们，data_summary 的 voice 分区算上它们。
 * 打字统计「上传到云端」和「绑定 QQ 群助手」，给用 Rust 账号会话的原生宿主（Windows 原生设置），语义与共享设置应用的 typing_statistics_upload_status、set_typing_statistics_upload、upload_typing_statistics、account_qq_link* 命令相同。只用登录账号，不退回匿名账号。statistics_directory 是打字统计目录的绝对路径（msime_client_typing_statistics 的 directory），day 是宿主的本地日期 YYYY-MM-DD，platform 是 windows、macos、linux、android、ios、harmony 之一，不合法时是 account_invalid、不联网。{operation:"typing_upload_status",statistics_directory} → {enabled,last_upload_ms,signed_in}：last_upload_ms 是上次上传成功的 Unix 毫秒，还没有时为 null；signed_in 说明本机是否登录了账号（没登录时开关不能打开），不联网。{operation:"typing_upload_set",statistics_directory,enabled,day,platform} → 同上：打开要求已登录（否则 account_unauthorized，开关不动），在本地打开后立刻上传最近有字数的至多 7 天（PUT /v1/users/me/typing-statistics），上传失败时开关保持打开、error 是那次上传的码，下一次自动上传再试；关闭时先在本地关掉，再 DELETE /v1/users/me/typing-statistics 删掉云端副本，删除失败时开关仍是关的、error 是删除的码，宿主据此提示云端副本没删掉。{operation:"typing_upload",statistics_directory,day,platform,force?} → 同上：打开了上传、已登录、离上次上传超过十分钟（force 为 true 时不看间隔）才发请求，否则什么也不发；没登录时不发也不报错，开关保持打开，登录后下一次照常上传。宿主在设置窗口打开和打字统计页显示时各调一次，不用自己计时。同一进程里的 typing_upload_set 和 typing_upload 互相等待，关掉上传后的删除不会和还在路上的上传交错。本机统计目录读写失败时是 account_storage。{operation:"qq_links"} → {links:[{id,created_at}]}：本人绑定的 QQ 群友，新的在前，服务端只存群和成员标识的摘要，所以只有编号和绑定时间。{operation:"qq_link_redeem",code} → {id,created_at}：code 是群友在 QQ 群里 @机器人 发 /bind 拿到的 8 位数字，前后的空白和中间的空格、连字符先去掉，其余不是 8 位数字时是 account_invalid、不联网；码不存在、过期或用过是 account_qq_link_code_not_found，账号已经绑了 10 个群友是 account_qq_link_limit（码不作废），每小时 10 次、猜错的也算，超了是 account_rate_limited。{operation:"qq_link_delete",id} → null：id 是 qq_links 给的编号（1–19 位数字，不是时 account_invalid），别人的或不存在的是 account_not_found。
 * {operation:"session_adopt",path?,identity_path?,item:"account"|"anonymous",from:"settings"|"keyboard"} → {outcome:"adopted"|"unchanged"|"cleared",local_unreadable?:true}：只能带 directory（带 session_store 是 account_invalid），给鸿蒙设置应用与键盘两个沙箱之间的会话镜像。path 是另一边发布的会话文件的绝对路径，不给、或文件（连同所在目录）不存在，表示发布方没有这份会话；读它时不跟随符号链接、最多 64 KiB；鸿蒙上文件属主必须是本进程的有效 uid（不看所在目录的属主：发布目录由发布方自己建，看有效 uid 是为了拒收别的 uid 先建好目录再放进的文件；前提是设置应用和键盘是同一个应用的两个进程、有效 uid 相同），只拒绝其他用户位；其他平台与 directory 里的会话文件相同，属主与所在目录相同、只能属主可读写，Native、Apple 和鸿蒙扁平布局都认，读不了或不是合法会话时是 account_storage，本地不动。item 选 directory 里的 account-session.json（登录会话）或 anonymous-session.json（匿名会话）。identity_path 只能和 item:"anonymous" 一起给（和 item:"account" 一起是 account_invalid），是另一边发布的匿名账号身份文件 anonymous-account.json 的绝对路径，读的规则与 path 相同，文件（连同所在目录）不存在表示发布方没有身份，读不了或不是合法身份时是 account_storage，本地不动；给了时身份跟着匿名会话走：结果是 adopted 时，在同一把锁里、写会话之前把 directory 里的 anonymous-account.json 换成发布方的身份，发布方没有身份而采用的会话换了账号（本地原来没有匿名会话、读不了，或 user.id 不同）时删掉本地身份，同一个 user.id 时不动，会话和身份始终属于同一个匿名账号；写会话失败时尽力还原身份；结果不是 adopted 时身份不动。不给 identity_path 时身份文件不动，与加这个字段之前相同。整个决定在 directory 的 account-refresh.lock 里做，与刷新、登录、退出互斥，不联网。两种 item 只看 user.id 和 expires_at，不看 session_id（有没有、相不相同、v4 还是 v8 都一样），规则除下面注明的两处外相同：同一个 user.id 时 expires_at 更晚的胜，from:"settings" 和 from:"keyboard" 都一样，相同时 unchanged；不同的 user.id（换号）时 from:"settings" 直接替换、from:"keyboard" 是 unchanged，设置应用里的换号不会被键盘的旧副本撤销；本地没有会话或读不了时：item:"account" 与换号相同（读不了而没有采用时 outcome 仍是 unchanged，另带 local_unreadable:true：这个结论取决于本地此刻读不读得了，本地恢复可读以后同一份发布物可能就该采用，宿主不能把它记成同一份发布物不用再试；其他结果不带这个字段）；item:"anonymous" 两个方向都是 adopted，用户没打开过设置应用就先启用了键盘时，设置应用第一次启动采用键盘注册的那个匿名账号，不再注册第二个、多用一次注册额度（带 identity_path 时连同键盘的身份）。expires_at 是拿到令牌的时刻加服务端给的固定时长，两个沙箱用同一台设备的时钟，后签发的会话一定过期得更晚，所以刷新过的那份胜过刷新前的、同一个账号重新登录的那份胜过上一次登录留下的：只要另一边给的是更新的那份，session_adopt 不会留下、也不会换回用过的刷新令牌（中间把系统时钟往回调过时除外）。发布方没有会话时：item:"account" 且 from:"settings" 清掉本地会话，与 forget 相同（连同云剪贴板密钥串），本地原来就没有时是 unchanged；item:"account" 且 from:"keyboard" 是 unchanged；item:"anonymous" 两个方向都是 unchanged、本地不动，匿名账号没有退出登录，清掉只会逼着重新注册、用掉每个 IP 每天 5 次的注册额度，所以 cleared 只出现在 item:"account"。采用的会话按 Native 布局原样写进 directory（session_id 照抄，没有的在第一次认证请求前补上）。两个沙箱各用各的锁，两边各自拿同一枚刷新令牌刷新的风险这里消除不了。
 * 其余失败时 error 是账号错误码：account_invalid（请求或参数不合法）、account_unauthorized（未登录或会话失效）、account_rate_limited、account_storage、account_cancelled、account_conflict、account_unavailable、account_blocked_content、account_screening_unavailable、account_banned；只有上面新增的操作和 purpose:"link" 的几步还会返回 account_too_large、account_not_found、account_recent_login_required、account_identity_changed、account_no_verified_email 和 account_network（msime_client_account_chat_stream 另有 account_busy；已有操作的码不变，例如 delete 遇到需要最近登录时仍是 account_unauthorized，断网时仍是 account_unavailable）。除 status、google_cancel、forget、session_adopt 外都会阻塞网络（session_adopt 不联网，但可能等另一个进程的刷新最多 20 秒）：放在工作线程上调用。 */
char *msime_client_account(const uint8_t *request, size_t length);
/* 昵称草稿的纯计算，不创建会话、不读文件、不联网。请求最多 16384 字节的 UTF-8 JSON {name, id}，id 是账号 UUID，编辑草稿时可传空串；未知字段、字段类型错误、无效缓冲区和 UTF-8 都返回 account_invalid，不回显输入。Value 为 {normalized, preferred, valid, maximum_chars, maximum_utf16_units}：normalized 按既有设置界面的 JavaScript trim 去首尾空白（含 BOM，不含 U+0085）；valid 直接复用账号改名的校验（规范化后非空、最多 64 个 Unicode 码点、没有控制字符）；preferred 在 normalized 为空时是「水杉小鹿·」加 id 前六个字符的大写，否则就是 normalized。空白或不合法的草稿仍成功返回，只把 valid 设为 false；宿主只可提交 valid 为 true 时的 normalized。maximum_utf16_units 是容纳 maximum_chars 个码点所需的 UTF-16 单元上限（当前 128），给控件限制输入长度；实际合法性只看 valid。 */
char *msime_client_account_name(const uint8_t *request, size_t length);
/* 流式对话（#8201 的决定 15）：与 msime_client_account 的 chat 相同的请求、预检和会话选择，只是以 stream:true 请求 POST /v1/chat/completions，回复每到一段就交给 on_delta，并且能取消。request 最多 1 MiB：{directory?|session_store?, legacy_directory?, client_agent?, request_id, messages, model}，directory、legacy_directory、client_agent、session_store 与 msime_client_account 相同（恰好给 directory 或 session_store 之一）；request_id 是宿主为这次对话取的 1–96 个 [A-Za-z0-9_-]，取消时用它；messages、model 与 chat 相同，序列化后超过 65536 字节是 account_too_large。登录账号优先，没有登录时用本机匿名账号，与 chat 相同；服务端以 401 拒绝令牌时刷新一次、在同一个账号里重试，被拒的那次没有交出增量。
 * on_delta 可为 NULL；否则每收到一段非空的增量就在调用本函数的线程上调用一次：text 是这段增量的 UTF-8 字节（length 个，不以 NUL 结尾，不会把一个字符拆在两段里），只在回调期间有效；按到达顺序依次调用，不会并发，本函数返回以后不会再调用。回调里不得重入 msime_client_*，唯一的例外是 msime_client_account_chat_cancel（调用回调时 Rust 不持有登记表的锁，也不持有会话锁）。增量可能只有空白；所有增量连起来就是成功时的 content。
 * 成功时 Value 是 {content}，整段回复，最多 16 KiB，与 chat 相同。不认识 stream 的旧后端回 400 invalid_chat_request 时退回非流式请求，整段回复作为一段增量交出；服务端回了 200 却不是 text/event-stream 时同样按非流式的 JSON 读。失败时 error 是账号错误码，与 chat 相同的规则：开始输出之前的失败带状态码的细节（被限流是 account_rate_limited，带 retry_after_seconds；503 server_busy 也是带 retry_after_seconds 的 account_rate_limited；没拿到响应是 account_network；5xx 是 account_unavailable）。开始输出以后：服务端发来上游失败的事件（data: {"error":...}）、回复超过 16 KiB、含换行、回车和制表符以外的控制字符、或只有空白就结束时是 account_unavailable；没等到 data: [DONE] 连接就断开、读到一半出错或到了 125 秒的总时限时是 account_network；这两种情况下已经交出的增量不收回，宿主自己决定留不留。同一个 request_id 正在进行时是 account_busy，原来那次不受影响。回答完整以后与 chat 一样核对给出令牌的账号，期间退出或换了号时是 account_cancelled，这时增量已经交出去了，宿主要丢掉它们。
 * 取消：msime_client_account_chat_cancel 之后本函数立即以 account_cancelled 返回（等服务端第一个字节、或两段增量之间的等待都会立刻结束），不再调用 on_delta；在别的线程上取消时，正在进行的那一次回调照常结束。连接在后台关闭：Rust 的 HTTP 客户端不能从别的线程打断一次读取，读取线程在服务端下一次发来数据时（最晚到 125 秒的总时限）发现调用方已经走了，断开连接，期间只占这一个线程和这一条连接。最长阻塞 125 秒；放在工作线程上调用。 */
typedef void (*msime_client_account_chat_delta_callback)(void *context, const uint8_t *text, size_t length);
char *msime_client_account_chat_stream(const uint8_t *request, size_t length, msime_client_account_chat_delta_callback on_delta, void *context);
/* request 最多 4096 字节：{request_id}。取消那次流式对话，Value 是当时是否有一个在进行。没有在进行的记下这个 id（只记最近 64 个），随后用同一个 request_id 开始的 msime_client_account_chat_stream 立即以 account_cancelled 返回：宿主在调用 msime_client_account_chat_stream 之后、Rust 登记 id 之前取消，也不会丢。所以取消过的 request_id 不要再拿来开始新的对话。任意线程上调用，立即返回，也可以在 on_delta 里调用。请求不合法时 error 是 account_invalid。 */
char *msime_client_account_chat_cancel(const uint8_t *request, size_t length);
/* 账号释义的 sans-IO 一半，给在按键路径上自己发 HTTP 的宿主（Windows 的 curl、Linux）：不联网、不读会话，bearer 由宿主自己加。Request 是 length 字节（最多 4 MiB）的 UTF-8 JSON。{operation:"request",texts,source_lang,target_lang} → {method:"POST",path:"/v1/translate",body}：body 是要发的 JSON 文本，校验与 msime_client_account 的 translate 相同（序列化后超过 65536 字节是 account_too_large）。{operation:"parse",texts,status,body,retry_after?} → {translations:[string|null]}：texts 是请求时的原文，status 和 body 是收到的状态码和正文，retry_after 是 Retry-After 头的原文；2xx 按 translate 的规则解读（包括超过 4096 字节的一项是 null，结果可以直接交给 msime_client_apply_translations），其余按状态码和服务端的错误码映射成账号错误码，带 retry_after_seconds（只认整数秒）。失败时 error 是账号错误码（account_invalid、account_too_large、account_unavailable、account_unauthorized、account_rate_limited 等）。可以在任意线程上调用。 */
char *msime_client_account_gloss(const uint8_t *request, size_t length);
/* 原生设置窗口的社区操作，与共享应用（Tauri）在 Windows 上的社区命令相同：候选窗皮肤社区（candidate_skin_community_* 十四个）、插件社区（plugin_community_* 七个）、社区词包与回复模板（community_resource_* 七个）和举报（community_report）。经 msime_client_account 里同一个账号会话对象认证（同一份 account-session.json，刷新时共用 account-refresh.lock）；keyboard_skin_download、keyboard_skin_install 和 report 登录账号优先，没有登录时用本机匿名账号（带 directory 时是同一 directory 里的 anonymous-session.json，带 session_store 时是该 store 的 ANONYMOUS 项），两个都没有时仍是 community_unauthorized，其余要登录的操作只认登录账号。这一点与共享应用的 community_report 不同，那里仍只认登录账号。Request 是 length 字节（最多 4 MiB）的 UTF-8 JSON {directory, legacy_directory?, skins_directory?, state_root?, exclude_ai_generated?, action}（只有 keyboard_skin_publish 可以到 9,000,000 字节，设计里可能带着照片背景）：directory 与 legacy_directory 同 msime_client_account，也可以同样用 session_store 代替它们（经该 store 的登录会话认证，两者都给、都不给或 id 没有注册时是 community_invalid）；skins_directory 是候选窗皮肤根目录的绝对路径（Windows 是 <状态目录>\skins，与共享应用相同），下面标了「要 skins_directory」的操作必需；state_root 是放 plugins\ 的状态根目录的绝对路径（与 msime_client_plugins 的 state_root 相同），标了「要 state_root」的操作必需。exclude_ai_generated 可选，缺省 false；为 true 时 keyboard_skin_list、keyboard_skin_detail、keyboard_skin_download 和 keyboard_skin_install 只要来源为 user 的皮肤，AI 生成的和来源不明的都不要：四个操作的请求都带 exclude=ai，列表和详情另带 fields=origin；服务端的成功响应没有 X-MSIME-Skin-Origin: v1，或者答 400（旧服务端不认 fields=origin）时是 community_unavailable；列表里 origin 不是 "user" 的条目被去掉，这样的详情和被服务端排除的作品是 community_not_found；HarmonyOS 在 AI 生成类资质到位之前总是带上它，其他操作不看它。id 都是 UUID 字符串；发布时的 id 由宿主随机生成，同一次发布重试时沿用，服务端据此去重。
 * 候选窗皮肤：{operation:"candidate_skin_list",offset,search,mine,category?} → {skins:[...],has_more}（category 不给时列出全部分类；mine 要登录）。{operation:"candidate_skin_detail",id} → 一项。{operation:"candidate_skin_preview",id} → {dataUrl}（PNG 或 JPEG 的 data: URL，类型和文件头已校验）。{operation:"candidate_skin_install",id,replace}（要 skins_directory）→ 下载、装进皮肤根目录并记下来源（同步不会把别人的作品传进自己的库），返回重新扫描的 {directory,packages,issues}；replace 为 false 而同名文件夹已存在时 error 是 candidate_skin_exists，先问用户再以 true 重试。{operation:"candidate_skin_pack_preview",skin_id,visibility?:"public"|"private"}（要 skins_directory，visibility 缺省 public；私有的可以没有素材许可）→ {suggestedName,license:{code,assets,source},fileCount,size}，不上传。{operation:"candidate_skin_add_preview",skin_id,image}（要 skins_directory）：image 是宿主为没有预览图的已装皮肤画好的 PNG 或 JPEG（最多 256 KiB）的标准 base64，写进包并加进清单 → 重新扫描的目录。{operation:"candidate_skin_add_license",skin_id,assets}（要 skins_directory）：把用户选的素材许可写进清单 → 重新扫描的目录。{operation:"candidate_skin_publish",skin_id,id,name,description,visibility?,category?}（要 skins_directory）→ 发布后的一项；同步已放进库里的同一个包就地更新，不会留下第二行。{operation:"candidate_skin_rate",id,stars:1..5} → {stars}。{operation:"candidate_skin_set_visibility",id,visibility} / {operation:"candidate_skin_set_category",id,category} → 更新后的一项。{operation:"candidate_skin_unpublish",id}（要 skins_directory）→ {deleted:true}，本地文件夹保留，同步状态里忘掉这一行。{operation:"candidate_skin_sync"}（要 skins_directory）→ {uploaded,downloaded,deleted_local,deleted_cloud,skipped:[{package_id,code}],stopped}；downloaded 或 deleted_local 非空时皮肤根目录变了，宿主用 msime_client_skin_catalog 重新扫描。未登录时 sync 在读本地会话这一步就以 community_unauthorized 结束，不发请求。
 * 插件：桌面（Windows、macOS、非 ohos 的 Linux）上 kind 可以是 script（脚本插件），鸿蒙和移动端只有八种数据类插件，列表永远不会出现 script。脚本插件在社区里先审后发：发布后要经管理员审核，审核通过前只有作者看得到，作者自己的条目带 moderation "pending" 时显示「审核中」（数据类插件的 pending 仍当作已通过）；条目另带 permissions（清单申请的能力名，按清单顺序，可能有本客户端还不认识的名字），供宿主在安装前用授权页的白话列出；plugin_install 装好的脚本包默认不运行，要在脚本插件授权页启用并勾选能力，宿主应在安装后把用户带到那里。服务端还不接受脚本插件时 plugin_publish 的失败码是 plugin_community_script_unsupported。{operation:"plugin_list",offset,search,kind?,mine?} → {plugins:[...],has_more,skipped}。{operation:"plugin_detail",id} → 一项。{operation:"plugin_pack_preview",kind,plugin_id}（要 state_root）→ {suggestedName,suggestedDescription,version,license,fileCount,size}，不上传。{operation:"plugin_publish",kind,plugin_id,id,name,description}（要 state_root）→ 发布后的一项；归档是确定的，同一 id 重试时内容相同。{operation:"plugin_install",id,kind,plugin_id}（要 state_root）：kind 与 plugin_id 是列表里那一项的，下载到的若是别的包就拒绝；整个替换已装的同 kind 同 id 的包（先问用户）→ 装好的 PluginSummary，与 msime_client_plugins 的 catalog 里一项相同。{operation:"plugin_rate",id,stars} → {stars}。{operation:"plugin_delete",id} → {deleted:true}，已装的包保留。
 * 词包与回复模板：{operation:"resource_list",kind:"dictionary"|"reply"|"phrase",scope:""|"mine"|"saved",search,offset} → {items:[...],has_more}。{operation:"resource_detail",id} → 一项。{operation:"resource_publish",id,kind,name,description,content,revision} → {id,revision}。{operation:"resource_apply",id,resource_revision} → {revision,imported,resource_revision}（把词包导入账号云端词库）。{operation:"resource_save",id,saved}、{operation:"resource_rate",id,stars}、{operation:"resource_unpublish",id} → null。
 * 键盘皮肤（与移动端 Tauri 的 community_skin_* 命令相同）：{operation:"keyboard_skin_list",offset,search,mine?,category?} → {skins:[...],has_more}（mine 缺省 false，为 true 时要登录；category 不给时列出全部分类）。{operation:"keyboard_skin_detail",id} → 一项。{operation:"keyboard_skin_download",id} → TouchKeyboardSkinDesign。{operation:"keyboard_skin_install",id,name}（要 state_root）→ {skin,trial}：下载后开始试用并导入皮肤库，导入失败时结束试用、不保留，与 msime_client_community_skin_install 相同，本机这一步的失败码也相同（community_storage、community_trial_format、community_skin_library_full、community_skin_invalid_name、community_skin_duplicate_name、community_not_found、community_skin_library_format）。{operation:"keyboard_skin_rate",id,stars:1..5} → null。{operation:"keyboard_skin_publish",id,name,description,design,category?} → null（category 不给时由服务端归入默认分类）。{operation:"keyboard_skin_set_category",id,category} → 更新后的一项。{operation:"keyboard_skin_unpublish",id} → null。
 * 举报：{operation:"report",kind:"skins"|"candidate-skins"|"plugins"|"dictionaries"|"replies"|"phrases",id,reason,detail?} → null；reason 是 msime_client_community_moderation 的 reasons 之一，detail 最多 1000 个字符。
 * 屏蔽作者（只认登录账号，不用匿名账号；屏蔽之后服务端在所有社区目录里不再给这个账号列出那位作者的作品，宿主刷新列表即可）：{operation:"block",kind 同举报,id: 作品的 UUID} → {id,name,created_at}，id 是这条屏蔽的 UUID，name 是作者现在的显示名，created_at 是 RFC 3339 时间；再屏蔽同一位作者答原来那条。屏蔽自己的作品是 community_invalid，超过 500 位是 community_conflict，作品不可见是 community_not_found。{operation:"block_list"} → [{id,name,created_at}]，最近的在前，最多 500 条。{operation:"unblock",id: 屏蔽的 UUID} → null，已经取消过的也答 null。
 * 失败时 error 是社区错误码：community_invalid（请求或参数不合法、缺了要的目录）、community_unauthorized、community_forbidden、community_not_found、community_rate_limited、community_cancelled、community_storage、community_conflict、community_unavailable、community_blocked_content、community_screening_unavailable、community_account_banned；本机打包、安装、补图、补许可的规则失败是 candidate_skin_* 或 storage（皮肤），或插件的码（插件），插件失败时可能另有 detail，是违反的那条规则的中文说明。除 pack_preview、add_preview、add_license 外都会阻塞网络；全部放在工作线程上调用。 */
char *msime_client_community(const uint8_t *request, size_t length);
/* AI 键盘皮肤，与移动端 Tauri 的 ai_skin_generate 相同；登录账号优先，没有登录时用本机匿名账号（带 directory 时是同一 directory 里的 anonymous-session.json，带 session_store 时是该 store 的 ANONYMOUS 项），两个都没有时是 account_unauthorized，Tauri 的 ai_skin_generate 仍只用登录账号。一次生成从头到尾用开始时选中的那个账号；插画任务归创建它的账号，用匿名账号建的任务在登录以后 get、delete 是 account_not_found。msime_client_ai_skin_generate 和 msime_client_ai_skin_job 按 msime_client_account 的方式选会话：{directory, legacy_directory?} 或 {session_store}，恰好给一个，否则是 ai_skin_invalid。失败时 error 是 ai_skin_invalid（请求、请求 id 或提示词不合法，也包括服务端或模型返回的方案、插画不合规，与 Tauri 相同，两种情况共用这个码）、ai_skin_busy（同一个 request_id 正在生成）、ai_skin_cancelled、ai_skin_unavailable（进程内的请求登记表不可用，只在之前有线程 panic 时出现）或账号错误码，账号错误码与 msime_client_account 的 #8201 新操作相同：account_unauthorized、account_rate_limited（服务端限流，带 retry_after_seconds：{"ok":false,"error":"account_rate_limited","retry_after_seconds":7200}）、account_unavailable（服务端 5xx，例如没开 AI 皮肤或任务繁忙的 503，后者带 retry_after_seconds）、account_network（请求没有拿到响应）、account_not_found（任务不存在或已过期）等。Tauri 的 ai_skin_generate 不变，那里的账号错误码与以前相同。除 cancel 外都会阻塞网络：放在工作线程上调用。 */
typedef void (*msime_client_ai_skin_progress_callback)(void *context, uint32_t completed);
/* request 最多 65536 字节：{directory?|session_store?, legacy_directory?, request_id, prompt}。request_id 是宿主为这次生成取的 1–96 个 [A-Za-z0-9_-]，取消时用它；prompt 是 1–500 个字符、不含控制字符。Value 是三套 [{name,description,design,artworkPrompt,artwork:{b64_json,mime_type,width,height}}]，与 Tauri 的 AiSkinProposal 相同，大约 30 MiB。progress 可为 NULL；否则每完成一张插画就在调用本函数的线程上调用一次，completed 从 1 到 3 递增，不会并发，本函数返回以后不会再调用。回调里不得重入 msime_client_*，唯一的例外是 msime_client_ai_skin_cancel（调用回调时 Rust 不持有登记表的锁，也不持有会话锁）。出图线程遇到 401 时会刷新令牌，这时注入存储的回调在 Rust 自己的线程上发生，按 msime_client_session_store 的「任意线程」约定处理。最长约 5 分钟。 */
char *msime_client_ai_skin_generate(const uint8_t *request, size_t length, msime_client_ai_skin_progress_callback progress, void *context);
/* request 最多 4096 字节：{request_id}。取消那次生成，Value 是当时是否有一个在跑；生成随后以 ai_skin_cancelled 结束，已经开始的插画任务在返回前删掉。没有在跑的记下这个 id（只记最近 64 个），随后用同一个 request_id 开始的生成立即以 ai_skin_cancelled 结束：宿主在调用 msime_client_ai_skin_generate 之后、Rust 登记 id 之前取消，也不会丢。所以取消过的 request_id 不要再拿来开始新的生成。任意线程上调用，立即返回，也可以在进度回调里调用。 */
char *msime_client_ai_skin_cancel(const uint8_t *request, size_t length);
/* 单个插画任务，给自己轮询的宿主。request 最多 65536 字节：{directory?|session_store?, legacy_directory?, action}，action 是 {operation:"create",prompt}（prompt 去掉首尾空白后 1–1200 个码点、不含 NUL）、{operation:"get",id} 或 {operation:"delete",id}（id 是 48 位小写十六进制）。create 和 get 的 Value 是 {id,state:"running"|"succeeded"|"failed",artwork?,reason?,expires_at?}：artwork 只在 succeeded 时有，形状同上；reason 是失败的原因码；expires_at 是任务过期的 RFC 3339 时间，服务端没给或不合法时没有。delete 的 Value 是 null，进行中的任务同时取消。服务端每个账号每天最多 10 个任务，超了是 account_rate_limited，retry_after_seconds 是到配额恢复的秒数。 */
char *msime_client_ai_skin_job(const uint8_t *request, size_t length);
/* 云词库面板的请求，与 Windows 设置应用的 cloud_dictionary_request 用同一份分发，会话是 msime_client_account 里的同一个对象（同一份 account-session.json，或注入的存储）。Request 是 length 字节（最多 4 MiB）的 UTF-8 JSON {directory, legacy_directory?, snapshot_directory?, options?, action}：directory 与 legacy_directory 同 msime_client_account，也可以同样用 session_store 代替它们；snapshot_directory 是本用户私有的临时目录的绝对路径（Windows 设置应用用 %LOCALAPPDATA%\<tauri identifier>\dictionary-snapshots），放预览下载和导出、恢复的中转文件，不能是下面的快照队列目录，所有 snapshot_* 操作和 export_file 必需；它在本进程里第一次用到时会清掉上次运行留下的 download-*.ndjson、export-*.ndjson、export-*.tsv、restore-*.ndjson 和原子写入的临时文件，所以不要让两个同时运行的进程共用一个；options 是本机的 HostOptions 文档（对象，user_data 必须是绝对路径），snapshot_preview、snapshot_enqueue、snapshot_status、snapshot_cancel 必需，快照队列放在 user_data 的上一级目录下的 dictionary-snapshots，暂存在 dictionary-snapshot-staging。
 * 账号云词库，action 与 cloud_dictionary_request 的相同，返回形状也相同：{operation:"list",kind,offset,search} → 一页 {entries,has_more,offset}；{operation:"catalog",kind,code,offset,scheme,profile} → {catalog_entries,has_more,offset,revision,normalized}；{operation:"changes",after,limit}；{operation:"add"|"update"|"edit_catalog"|"candidates"|"remove_candidate"|"fixed_positions"|"set_fixed_position"|"delete"|"import",...}；{operation:"rank",...} → {revision,changed,selection_count}；{operation:"export",kind,format} → {text,filename}；{operation:"export_file",kind,format,path} → {path,filename,bytes}（#8201 的 P7）：与 export 相同的请求，正文最多 384 MiB，不放进响应，而是边下载边写进 snapshot_directory 里的 export-*.tsv（不进内存，要求与 export 相同：非空、UTF-8、不含 NUL），通过以后才经 path 旁边的临时文件 .msime-export-* 复制过去换入 path；预检和清理与下面的 snapshot_export_file 相同：path 是宿主选的保存位置的绝对路径，上级目录必须已经存在、不经过符号链接，否则在下载之前就是 account_invalid；未登录、联网失败、正文不合格或写入失败时 path 上原有的文件原样保留，中转文件总会删掉；filename 是与 export 相同的建议文件名 dictionary-<kind>.tsv，bytes 是写入的字节数；要带 snapshot_directory。kind 是 pinyin、wubi、wubi98、quick、english 之一，format 是 standard 或 windows。
 * 完整快照：{operation:"snapshot_status"} → 快照队列状态（不含账号 id），面板轮询时它也会处理当前账号排队中的请求，最多每 60 秒一次；{operation:"snapshot_preview"} → {previewToken, snapshot}：下载云端整份词库到 snapshot_directory 并校验；{operation:"snapshot_enqueue",token} → 队列状态：预览之后账号没换、云端也没改动时入列，并马上请输入会话放开词库、在本进程里准备并激活新的一代，放不开时留在队列里；{operation:"snapshot_cancel"} → 队列状态，丢掉预览并取消当前账号排队中的请求。整份快照可能有 512 MiB，所以备份与恢复走文件：{operation:"snapshot_export_file",path} → {path,filename,snapshot}：path 是宿主选的保存位置的绝对路径（上级目录必须已经存在、不经过符号链接，否则在下载之前就是 account_invalid），先下载到 snapshot_directory 校验，通过以后才经 path 旁边的临时文件 .msime-export-* 复制过去换入 path；未登录、联网失败、校验不过或写入失败时 path 上原有的文件原样保留；进程在复制中途被杀时留在 path 旁边的临时文件，下次往同一个目录导出时删掉（只删超过一小时没有改动的）；{operation:"snapshot_restore_preview_file",path} → {snapshot,expectedRevision}：path 是宿主选的备份文件（普通文件、不跟随符号链接、最多 512 MiB 的 UTF-8），复制进 snapshot_directory 校验，再读云端当前的 revision；{operation:"snapshot_restore_file",path,expected_sha256,revision} → {revision,reset}：expected_sha256 是预览给出的 snapshot.sha256，文件在预览之后变了是 account_invalid，revision 是预览给出的 expectedRevision，云端在这之后被改过是 account_conflict。snapshot 是快照元数据，不含 fileSha256 和 engineRecords。文本版的 snapshot_export、snapshot_restore_preview、snapshot_restore，以及 macOS 的 snapshot_restore_native、snapshot_restore_cancel、snapshot_choose_restore 都是 snapshot_unavailable。
 * 失败时 error 是 invalid_cloud_dictionary（请求或参数不合法、缺了要的目录或 options、directory 与 session_store 不是恰好给一个、id 没有注册），会话问题是 account_* 码，快照问题是 snapshot_busy、snapshot_conflict、snapshot_invalid、snapshot_unavailable。会阻塞网络、读写文件，「应用到本机」还要等输入会话放开词库：放在工作线程上调用。 */
char *msime_client_cloud_dictionary(const uint8_t *request, size_t length);
/* options is a readable UTF-8 buffer of length bytes; maximum 1 MiB.
 * Object: api_version=1, resources/user_data/cache/dictionaries (absolute paths),
 * preferences={scheme, candidate_page_size, learning, chinese_punctuation,
 *              shuangpin_profile?, shuangpin_custom_profile?, wubi_profile?}。缺省 shuangpin_profile 为 xiaohe，
 * 可选 xiaohe、ziranma、shoudao、microsoft、smart_abc、custom。custom 读 shuangpin_custom_profile（{initials, finals, zero_initials}，各是单位到键的对象），表缺失或不合法时按 xiaohe 建会话。缺省 wubi_profile 为 wubi86，可选 wubi86、wubi98。不认识的取值一律拒绝。
 * Optional preferences_directory is bootstrap metadata for host file monitoring;
 * session creation itself does not monitor or load it.
 * Optional phrase_preedit=true asks for a half-composed phrase to stay in the
 * composition: picking a candidate that covers only part of the input leaves the
 * chosen piece in view.phrase_prefix instead of committing it, and the whole
 * phrase commits at once when the composition ends. The host must draw that
 * field ahead of editing_text - it is separate because caret_position is an
 * offset into editing_text in the host's own string unit. Omitted means each
 * piece is committed as it is picked, as before.
 * Optional selection_statistics=false stops this session from recording candidate positions and commit efficiency in <preferences_directory>/typing-statistics.json. It is for a mirror session that repeats selections another session already made: the Windows TIP's in-process session follows the Server's choice, and only the Server's session records it. Omitted means the session records them.
 * The host must prepare and validate its dictionary generation before creation.
 * Creation acquires cooperative shared access to user_data and dictionaries until
 * destroy. It fails immediately while a participating maintenance writer holds
 * exclusive access. Existing sessions are never cancelled for maintenance.
 * Linux hosts may provide absolute online_provider_socket and
 * translation_provider_socket paths for user-managed Unix-socket services;
 * translation may reuse the online socket when omitted.
 * preferences_directory also names the plugins root, <preferences_directory>/plugins: the
 * installed plugin packs, the enabled command tables the "/" mode reads and the "@" mode's
 * mentions.json. They are read at creation, on preference updates and when a field gains
 * focus, and only while the mode that uses them is on. Optional sound_packs is the absolute
 * path of the bundle's built-in sound packs; absent means sound-packs beside resources.
 * Optional plugin_packs is the absolute path of the official plugin packs the desktop
 * installers ship (<kind>/<id>/ folders: the uuid script plugin, the latex-symbols phrase
 * table); absent means plugin-packs beside the built-in sound packs, and nothing when that
 * directory is not there (mobile, development builds without them).
 * 可选 helpcode_fallback_resources 是随包资源的绝对根目录。仅主资源缺少内置辅助码表时才从这里读取 helpcodes/；已有表、自定义表和插件表继续优先。resources 换成下载的词库代次时，msime_client_prepare_host 自己写这个键；文档没有它时，建会话会从词库更新记录里找随包目录，宿主一般不必设置。宿主自己注入时可仅放在内存配置中，避免旧版宿主读取不认识的持久化键。
 * Do not delete .msime-dictionary-access.lock files. Legacy/external writers do
 * not participate; preparation/upgrades still require stopped sessions.
 */
char *msime_client_create(const uint8_t *options, size_t length);
/* Management JSON (<=2248576 bytes), trusted native caller only:
 * {options: <same HostOptions as create>, action: {operation:"list",offset:0,limit:100}}
 * List 的 kind 与 query 可选：ASCII 查询匹配编码前缀，含非 ASCII 字符时匹配词条字面内容。无查询时列个人词条；指定 kind 与非空查询时包含内置行，quick_phrase 空查询也包含内置行。个人行优先；user_only:true 始终只列个人词条，在整库筛选后分页。
 * or action:{operation:"edit",previous:null|Entry,replacement:null|Entry,request_id:"..."}.
 * Batch import: action:{operation:"import",kind:"pinyin"|"wubi"|"wubi98"|"quick_phrase"|"english",
 * format:"standard"|"windows"|"rime"|"hans",text:"word<TAB>code<TAB>weight\\n",request_id:"..."}.
 * Standard rows are word, code, weight; Windows rows are code, word, weight. Rime rows are
 * word, code, optional weight and may include YAML front matter between --- and ...; Rime's
 * metadata weights (for example c=3 d=0.12) use the default 10000. Omitted weights use 10000.
 * The hans format is pinyin-only and accepts one pure Han phrase per line; Engine resolves
 * each phrase to its highest-ranked canonical pinyin and uses weight 10000.
 * Import accepts at most 1000 rows and returns {applied}; rows are committed
 * with deterministic receipt IDs derived from request_id so retries are safe.
 * Export: action:{operation:"export",kind,format:"standard"|"windows",offset,limit} returns
 * {text,has_more}; use pages of at most 1000 rows. Standard output is word, code, weight.
 * Entry:{kind:"pinyin"|"wubi"|"wubi98"|"quick_phrase"|"english",key,value,weight,source?:"user"|"bundled"}.
 * List returns {entries,has_more} and sets source on every entry; edit returns {applied:true}. Errors are redacted.
 * A bundled entry passed back as previous can only be re-weighted (replacement with the same kind, key and value) or deleted (replacement null); anything else fails with "bundled dictionary entry is read-only". Export of pinyin also carries the weights set or learned for bundled words and omits single characters; the other kinds export user words only.
 * 清除学习记录：action:{operation:"reset"} 取独占维护权（会话开着时报 dictionary maintenance busy），换回随包的工作词库和英文词库、清空日志（调频、置顶、固定位置、选词计数、整句联想、拼写纠错和用户自己的词），偏好、常用语、剪贴板和打字统计不动；options 带 preferences_directory 时先删掉那里还没合并的输入记录和输入习惯（pending-learning-merge 和 pending-habits-merge 的待合并、已认领和失败计数文件，删不掉时报 learned-data reset rejected、什么也没清），重置后再把已启用命名词库的词重新排进它们的待发送队列（`DictionaryCollectionsStore::requeue_enabled_after_reset`），宿主随后照常送出。返回 {reset:true,collections_requeued:n}；重置成功而重排失败时照样是 reset:true，collections_requeued 为 null，另带 collections_error:"learned-data reset collections unavailable"。云同步的待上传标记由宿主自己打（Android 用 msime_client_cloud_sync 的 dictionary_reset）。
 * 计数（只读）：action:{operation:"count",kind?,user_only?:bool} 返回 {count,kinds:{kind:n},complete}；user_only:true 只数用户自己的词，否则拼音还会数上学到或设过权重的随包词（也就是导出的那些行）。扫描在 1,000,000 行处停下时 complete 为 false。
 * 快照导出：action:{operation:"export_snapshot",destination:绝对路径,include_learning?:bool} 把用户的词写成与 GET /v1/users/me/dictionary/snapshot 相同的 NDJSON msime-dictionary-snapshot v1 文档（header、每个词一条 entry 和一条 overlay、带正文 SHA-256 的 footer；revision 为 1），用云端格式自己的校验器检查后返回那份元数据加上 path。不需要账号。include_learning 缺省为 false，这时不含位置和选择，输出与这个参数加入之前逐字节相同，云同步上传用的就是它；为 true 时（本地备份用）在用户的词之后再写输入记录：学习调权写成 user_inserted:false 的 overlay、删除记录写成 deleted:true 的 overlay、固定位置写成 position、选词计数写成 selection（截到 0..10），格式装不下的行跳过，返回值多出 learning 和 learning_skipped 两个计数；日志整体读不出来时照样导出词，learning 为 0，原因在 learning_error。
 * 输入记录：action:{operation:"learning_count"} 只读，返回 {count}，即本机日志里学习调权、删除记录、固定位置和选词计数的条数（不含用户自己的词）。action:{operation:"queue_learning_merge",source:绝对路径} 校验快照文件 source，把其中的输入记录另存成 <preferences_directory>/pending-learning-merge.ndjson（替换已有的一份），返回 {queued,learning}；没有输入记录时 queued 为 false、什么也不写。action:{operation:"merge_pending_learning"} 在没有会话时合并这份文件（要独占维护权，忙时报 dictionary maintenance busy、文件留着）：本机已有的词、固定的词和被占用的位置保留本机，选词计数取较大的那个，返回 {merged:false}（没有待合并的）或 {merged:true,entries,positions,selections,kept,skipped}；合并期间新排的一份不受影响，连续失败三次放弃并删掉文件（learning merge abandoned）。
 * 本地备份的校验与输入习惯：action:{operation:"inspect_snapshot",source:绝对路径} 只读地按云端格式完整校验一份词库快照并返回它的元数据。action:{operation:"export_habits",destination:绝对路径} 把日志里的输入习惯（整句联想的二元/三元计数、连续选词对、拼写纠错计数、自动纠错抑制、置顶的候选）写成 NDJSON（header format:"msime-learning-habits" version:1、每条一行、footer 带条数和正文 SHA-256），装不下的行跳过，返回 {path,habits,skipped}。action:{operation:"inspect_habits",source} 只读校验，返回 {habits}。action:{operation:"queue_habits_merge",source} 校验后存成 <preferences_directory>/pending-habits-merge.ndjson，返回 {queued,habits}；merge_pending_learning 随后在空闲时一并合并它（计数取大、置顶保留本机，写完压回各表上限），结果多一个 habits 字段。learning_count 另返回 habits（输入习惯行数，读不出来时为 null）。
 * 五笔查码（只读）：action:{operation:"wubi_codes",text} 返回 {phrase,characters,truncated}。characters 按 text 里第一次出现的顺序列出每个不同的汉字（最多 64 个，超出时 truncated 为 true），每项是 {text,wubi86:[...],wubi98:[...]}，编码短码在前、码表里没有时为空数组；phrase 是整段文字作为一个词的同样一项，只在整段都是汉字、2 到 16 个字、且至少一张表有它时给出，否则为 null。读会话用的同一份五笔码表，不拿维护权；text 超过 4096 字节时报 invalid wubi lookup。msime_client_personal_dictionary_request 对它给出同样的回答，不进队列。
 * Native host owns/authorizes paths; never accept arbitrary webview paths or log payloads.
 * Run on a worker thread. Edit returns busy until all participating sessions are
 * destroyed, then holds exclusive access; recreate sessions after success.
 * Do not automatically cancel user input to obtain access. Retry ambiguous writes
 * with the identical nonempty request ID and content.
 */
char *msime_client_dictionary(const uint8_t *request, size_t length);
/* The same {options, action} as msime_client_dictionary, as a settings page sends it: an edit,
 * import or reset first has the input hosts let go of the dictionaries (the Windows Server
 * over its auxiliary pipe) and is retried until they have; an import of a whole file (up to
 * 48 MiB of text) is sent in batches and answered with one report; and a refusal's error is
 * the settings pages' code: dictionary_busy, dictionary_too_large, dictionary_import_rejected,
 * dictionary_read_rejected, dictionary_pinyin_unavailable, dictionary_unavailable,
 * dictionary_reset_rejected, dictionary_bundled_readonly, dictionary_invalid_entry,
 * dictionary_invalid_word or storage. {operation:"reset"} restores the bundled dictionaries
 * and clears all learned data, returning {reset:true}. Blocks: call off the UI thread. */
char *msime_client_settings_dictionary(const uint8_t *request, size_t length);
/* Smart punctuation follow-up gestures. The host holds the snapshots: they belong
 * to its editor, not to Engine, and a session rebuilt while the keyboard was away
 * must not carry a gesture across the gap. The switches that gate them live in the
 * applied preferences, so the session answers rather than the host keeping a copy.
 * arm: {ascii, commit, timestamp_ms, editor_generation, auto_closed_pair} ->
 *      {repeat: {ascii, committed, timestamp_ms, editor_generation}|null,
 *       space: {chinese, ascii, editor_generation}|null}
 * decide: {character, preceding, timestamp_ms, editor_generation, repeat, space} ->
 *      {replace_with: "，"|null, space_ascii: 46|null}
 * `preceding` is what the editor holds before the caret at the moment of the press;
 * both decisions re-read it and decline when it disagrees with the arming, so a
 * stale snapshot can never rewrite the wrong character. A non-null space_ascii means
 * replace the preceding mark with it and swallow the space. Maximum 4096 bytes. */
char *msime_client_smart_punctuation_arm(uint64_t handle, const uint8_t *request, size_t length);
char *msime_client_smart_punctuation_decide(uint64_t handle, const uint8_t *request, size_t length);
/* Pure Engine validation/normalization for one Entry object. No paths or
 * session are required and no dictionary state is changed. */
char *msime_client_dictionary_validate(const uint8_t *request, size_t length);
/* Plain Chinese words, one per line, answered as the pinyin entries the "hans" import format would produce: {entries:[Entry]}. Reads only the packaged main dictionary under resources, read-only; needs no prepared host or session and changes no dictionary state. */
char *msime_client_dictionary_hans_entries(const uint8_t *text, size_t text_length,
                                           const uint8_t *resources, size_t resources_length);
/* 搜狗细胞词库（.scel）转成标准 TSV 文本：请求 {bytes_base64}（文件不超过 8 MiB），回答 {name, category, entries, text}。text 每个词一行（词<TAB>音节'音节，不带权重列），读不出读音或词的那一行必定被导入拒绝，所以导入报告里的行号就是词序；宿主把 text 当作 standard 格式交给自己现有的导入。纯函数：不需要宿主选项，不碰任何词库状态，不记录词条内容。错误：dictionary_scel_unsupported（不是细胞词库或版本不支持）、dictionary_scel_malformed（不完整或已损坏）、dictionary_scel_empty、dictionary_too_large、dictionary_scel_invalid（请求或 base64 坏了）。 */
char *msime_client_dictionary_scel_text(const uint8_t *request, size_t length);
/* A dictionary file ({kind, format, text}, as the "import" dictionary action) answered as the words the personal dictionary queue accepts, with the import report: {entries:[Entry], applied, failed, truncated, swapped, first_failures}. Invalid rows are counted with their line, repeated words appear once, and at most 128 words are returned. Reads only the packaged main dictionary under resources; changes no dictionary state. Maximum 1,200,000 bytes. */
char *msime_client_dictionary_import_entries(const uint8_t *request, size_t request_length,
                                             const uint8_t *resources, size_t resources_length);
/* 个人词库队列同步：请求是与 msime_client_create 相同的 HostOptions。
 * 一次最多应用 128 项，返回 {synchronized,pending_count,failed_count,snapshot_error,applied_count}；applied_count 是 Engine 确认已应用的 Written 请求数，含幂等重试，不计 AlreadyPresent 或失败项。
 * 整批先取得维护租约；词库仍有活跃会话时返回 dictionary maintenance busy，队列保持原样，稍后重试。 */
char *msime_client_personal_dictionary_sync(const uint8_t *request, size_t length);
/* Snapshot lifecycle. Version is a redacted SHA-256 binding the canonical
 * resource/user/cache/dictionary paths and one consistent Engine journal.
 * Prepare/discard are native-only; a prepared handle is not active until a
 * future activation transaction publishes it. The prepare callback returns
 * one UTF-8 JSON record into the supplied buffer, 0 only at verified EOF, and
 * a negative value for cancellation, truncation, or checksum failure. */
typedef intptr_t (*msime_client_snapshot_next)(void *context, uint8_t *buffer, size_t capacity);
char *msime_client_snapshot_version(const uint8_t *options, size_t length);
/* Validates a complete host-private NDJSON file and returns bounded metadata only. */
char *msime_client_snapshot_inspect(const uint8_t *path, size_t length);
/* Revalidates the exact host-private file, then streams it to the account service.
 * request: {revision,expected_sha256,access_token}; expected_sha256 identifies
 * the complete file returned by inspect, not only the checksummed snapshot body.
 * Blocking network and file I/O: call only from a worker thread. */
char *msime_client_snapshot_restore(const uint8_t *request, size_t request_length,
                                    const uint8_t *path, size_t path_length);
/* Crash-safe queue state, enqueue, cancellation and idle processing. */
/* {operation:"import_android_v1", directory} → {imported, state}：把 Android 旧版 Java 队列（DictionarySnapshotQueue.java）的 state.bin 就地转成本队列的 state.json，在 state.lock 的 flock 里做（等正在持锁的激活做完）；目录里已经没有 state.bin 时不取锁直接返回，转换过以后这一步不会排在激活后面，快照文件两边同名（<id>.ndjson），原样留着、不复制。先原子地写好 state.json、对目录 fsync，再删 state.bin，中途中断时下一次接着做，队列不丢。目录不存在（不会创建）、没有 state.bin、或者已经有 state.json（只补删留下的 state.bin）时 imported 为 false，什么也不改；state 是转换以后的队列状态，形状同其他操作。state.bin 读不懂（截断、魔数或版本不对、字段不合法）是 snapshot_invalid，两份文件都不动。其余会改队列的操作持锁时也先做同样的转换，还没转换时只读的读取直接解码 state.bin，所以 Android 的主进程和 :ime 谁先用到队列都不会把旧队列当成空的。{operation:"process", directory, staging_root, options, account_id?} 的 account_id 是当前账号，不知道当前账号是谁（读不到会话或未登录）时不给：属于某个账号的请求这时原样留着、下次再处理，属于别的账号时取消；账号是 local-backup 的请求（Android 本地备份恢复排的，与 Java 的 DictionarySnapshotQueue.LOCAL_RESTORE_OWNER 相同，转换时原样保留）不属于任何账号，不给 account_id 时照样认领和激活，也从不因账号不同被取消，只有 cancel 给的 account_id 正好是 local-backup 时才取消。state 和 process 可以带 skip_when_idle（默认 false）：为 true 且队列里没有排队或准备中的请求时只读状态返回，不扫用户词库算本机版本、不发布它，也不创建队列目录；鸿蒙键盘这样查，因为它的队列没有生产者，发布出去的本机版本没人读。 */
char *msime_client_snapshot_queue(const uint8_t *request, size_t length);
char *msime_client_snapshot_prepare(const uint8_t *request, size_t length,
                                     msime_client_snapshot_next next, void *context);
char *msime_client_snapshot_discard(uint64_t handle);
char *msime_client_snapshot_activate(uint64_t handle, const uint8_t *expected_version, size_t length);
/* The shared preference defaults as a JSON document. A host patching one key of
 * a nested preference object needs that object's other fields: the object itself
 * is optional, its members are not.
 */
char *msime_client_default_preferences(void);
/* 触控板设置元数据：defaults 为共享默认值，horizontal_dp 和 vertical_dp 各含 minimum / maximum。纯计算，返回标准 JSON 结果，用 string_free 释放。 */
char *msime_client_text_trackpad_options(void);
/* 「重置所有设置」：directory 是绝对的偏好目录（UTF-8，<=16384 字节）。在偏好锁里读出当前文档，换成本目录所属版本的默认偏好（服务凭据与 fuzzy_pinyin.seeded 保留），再按 expected_revision 比较并交换写回，返回新的 PreferencesSnapshot。修订号不符时失败且不写入。词库、统计不受影响；默认关闭剪贴板历史，因此与 msime_client_save_preferences 一样清空已存的历史。会读写文件，请在工作线程调用。 */
char *msime_client_restore_default_preferences(const uint8_t *directory, size_t length,
                                               uint64_t expected_revision);
/* 「导出设置」：directory 是绝对的偏好目录（UTF-8，<=16384 字节）。返回值 value 是设置文件全文（JSON 字符串，format 为 app.msime.client.preferences），不含语音、AI 辅助和翻译服务的配置与密钥、诊断日志、使用统计和剪贴板历史开关。宿主负责让用户选位置并写盘。会读文件，请在工作线程调用。 */
char *msime_client_export_settings(const uint8_t *directory, size_t length);
/* 「导入设置」：document 是用户选的设置文件内容（<=1048576 字节）。换算成偏好（本机的服务配置与密钥、诊断日志、使用统计和剪贴板历史开关保留，按本目录所属的版本收窄输入方案；不会清空剪贴板历史）后按 expected_revision 比较并交换写回，返回新的 PreferencesSnapshot。失败时 error 为 settings_document_invalid（不是设置文件）、settings_document_macos（macOS 原生设置窗口导出的文件）、settings_document_unsupported（更新版本的文件或读不懂的设置）、settings_conflict（设置已在别处更新）之一，或存储错误的说明；失败时不写入。会读写文件，请在工作线程调用。 */
char *msime_client_import_settings(const uint8_t *directory, size_t directory_length,
                                   uint64_t expected_revision, const uint8_t *document,
                                   size_t document_length);
/* The transcription provider and optional rewrite this device is configured for, read from an
 * absolute preferences directory. Response value: {provider:{...}|null, polish:{...}|null, punctuationMode}; both
 * absent means nothing is configured and the host uses whatever it falls back to. punctuationMode 是 voice_input.punctuation_mode 归一后的取值（auto、omit_trailing、omit_all、space，不认识的值给 auto），宿主把它交给 msime_client_voice_punctuation。 Contains
 * credentials: never log the response; release with msime_client_string_free. provider.requestFormat 是请求格式："multipart"（OpenAI 兼容的 /audio/transcriptions）、"chat_audio"（Chat Completions 带 input_audio，阿里云百炼）、"doubao_websocket" 或 "local"；宿主按它挑请求构造，不按 provider 名字判断。 */
char *msime_client_mobile_voice_configuration(const uint8_t *directory, size_t length);
/* 全局主题选择器：{themes:[{id,title,platforms,appearance,preview,candidate,keyboard},...],default:"system"}。只列出本库编译到的平台提供的主题，按选择器顺序：system, native（仅 iOS）, shuishan, light, paper, night, ink, custom。platforms 为提供该主题的宿主（如 ["ios"]），null 表示所有宿主。appearance is "light"|"dark"|null; preview {background,panel,accent,text}, candidate and keyboard are the built-in palettes and are null for system, native and custom. Keys are snake_case. Hosts keep no copy of the ids, titles or colours. */
char *msime_client_theme_catalog(void);
/* 解析所选全局主题的颜色。JSON 请求（<=1048576 字节，拒绝未知键）：{global_theme:string, custom_theme?:Preferences.custom_theme, dark:bool, layout:"horizontal"|"vertical", skins_directory?:皮肤根目录的绝对路径 | package?:candidate_skin_catalog 中的一项}。global_theme 必须是八个主题 id 之一（system、native、shuishan、light、paper、night、ink、custom），其他 id 请求失败；native 不在本平台目录里时也接受，解析结果与 system 相同，只是 id 为 native。custom_theme 按偏好同样校验。dark 是宿主当前的明暗模式，它决定自定义主题取哪个槽位：深色模式取 custom_theme.candidate_skin_dark，没设时取 custom_theme.candidate_skin，浅色模式取 custom_theme.candidate_skin。皮肤包只在它清单 base 的明暗下绘制：base 是固定明暗的内置主题（shuishan、night、ink 为深色，light、paper 为浅色）且与 dark 不符时不画，base 为 system 的包两种模式都画；设过皮肤却在这个模式下没有可画的包时，custom_theme.base 也只在属于当前明暗时作底，否则按 system。layout 是正在绘制的候选窗：皮肤包只在清单声明的排列和模式下绘制，只有画了它时 candidate_skin 才非空，所以宿主正好在 candidate_skin 非空时画包的装饰和最小宽度。skins_directory 给扫描皮肤根目录的宿主（除 Linux 外都是）；package 是 Linux candidate_skin_catalog 中当前模式槽位指名的那一项，其他任何东西（包括 msime_client_skin_catalog 的 SkinSummary）都让请求失败。响应 value：{id,source:"system"|"builtin"|"custom",appearance:"light"|"dark"|null, candidate:{surface,border,text,number,secondary,accent,selected,selected_text,selected_number,hover,show_selected_bar}|null, keyboard:{background,key,function_key,text,secondary,accent,on_accent}|null, candidate_skin:string|null}。appearance 非空时是返回的界面所处的明暗。keyboard.accent 是触屏候选条选中候选的文字色（无填充）；回车键保留平台强调色。on_accent 为黑或白，能在强调色填充上读清。每个颜色都是 #RRGGBB 或 #RRGGBBAA。为 null 的调色板或槽位表示用宿主自己的原生 token，从不表示透明。根目录里没有、磁盘上无效、或不是当前槽位指名的包会被略过而不是让调用失败。skins_directory 会读取皮肤包：在主题、明暗或皮肤包变化时解析，不要在绘制时调用。 */
char *msime_client_resolve_theme(const uint8_t *request, size_t length);
/* 应用主题（Android 本地设置里的 app_theme，不在共享偏好里）选择器：{app_themes:[{id,title,season,seasonal,light:{accent,accent_soft,on_accent,background,card,hair,candidate_accent},dark:{...同上}}],default:"siji"}。id 按选择器顺序为 siji、chunya、xiayin、qiushan、dongxue；season 是固定的那一季（"spring"|"summer"|"autumn"|"winter"），siji 为 null 且 seasonal 为 true，它的颜色固定画秋杉，目录因此不随时钟变化。颜色为 #RRGGBB 或 #RRGGBBAA；accent_soft 是强调色加 22（浅色）或 40（深色）透明度，浅色 on_accent 为 #FFFFFF，深色 on_accent 是强调色与黑色 25% 比 75% 的混合；candidate_accent 是触屏候选条首选的字色，浅色是同一色相加深到 WCAG AA 的强调色，深色等于 accent。纯计算。 */
char *msime_client_app_theme_catalog(void);
/* 应用主题在宿主当前月份与明暗模式下的颜色。JSON 请求（<=4096 字节，拒绝未知键）{app_theme:"siji"|"chunya"|"xiayin"|"qiushan"|"dongxue", month?:1-12, dark:bool}；month 是宿主本地日历的月份，只影响 siji，省略时用 UTC 月份，超出 1-12 时请求失败。返回 {id, season, accent, accent_soft, on_accent, background, card, hair, candidate_accent}，season 是实际画的那一季。纯计算，可在主线程调用。 */
char *msime_client_resolve_app_theme(const uint8_t *request, size_t length);
/* Per-key double-pinyin hint text for one profile name, as a JSON object mapping
 * an uppercase key to "initials / finals" - or to whichever side that key carries.
 * Read out of the Engine's own profile tables so a keyboard face never carries a
 * second copy of the keymap. An unknown profile name yields an empty object
 * rather than the default profile's hints. */
char *msime_client_shuangpin_key_hints(const uint8_t *profile, size_t length);
/* The double-pinyin codes of the whole zero-initial syllables for one profile name, as a JSON object mapping each syllable to its two-key code, e.g. {"a":"aa","ang":"ah",...}. Read out of the Engine's own profile tables. An unknown profile name yields an empty object. */
char *msime_client_shuangpin_zero_initials(const uint8_t *profile, size_t length);
/* 九键双拼方案在宿主看来的样子。request 是 JSON {"profile": "migala2"|"migala3"|"gushi2_low"|"custom", "table"?: {initials, finals, zero_initials}}，table 是 custom 用的表（与 nine_key_shuangpin_custom_profile 同形，键是数字）。Value 是 {effective, table, problem, keys}：effective 是输入法实际用的方案（custom 的表不能用时是默认的 migala3，与建会话时的回退相同），table 是它的表，problem 是 custom 表的问题（形状同自定义双拼表的问题，只有 unknown_unit、duplicate_unit、missing_unit、invalid_key 四种，能用时为 null），keys 按 1 到 9、0 的顺序列出每个用到的数字键的 {key, initials, finals, zero_initial}，键面从这里取，宿主不另存键位。不留存任何东西，编辑表时每改一格都可以调用。不认识的方案名或多余的字段报错。 */
char *msime_client_nine_key_shuangpin_layout(const uint8_t *request, size_t length);
/* 规范化中文触屏快捷符号：{symbols: string[]}，最多 64 KiB。标准响应 value 为去重的单字素列表，最多十二项；无有效项时返回共享默认值。不落盘，用 string_free 释放。 */
char *msime_client_touch_quick_symbols(const uint8_t *value, size_t length);

/* Load PreferencesStore from an absolute UTF-8 directory, without a session.
 * May block on disk/file lock: use a worker thread. Returns PreferencesSnapshot.
 * Missing file returns shared defaults; malformed/future files return errors.
 * Creates the directory/lock file if absent, never overwrites preference contents.
 */
char *msime_client_load_preferences(const uint8_t *directory, size_t length);
/* Android 本机旧布局迁移：{directory:绝对路径,revision:u64,long_space:bool,number_row:bool,nine_key_symbols?:[string],nine_key_digit_symbols?:[string],nine_key_swipe?:string}。
 * 两张九键左侧符号表是本机旧键里不等于出厂表的自定义表，缺省或空表表示不迁；共享偏好里这一项缺键或是空数组时才写入。
 * nine_key_swipe 是本机旧设置 platform.android.nine_key_swipe 的原值（off/up/down），本机没有时不带。
 * 在偏好锁内按原始字段决定迁移并校验修订号；未知样式原样保留。返回 PreferencesSnapshot。
 * 请求最多 64 KiB，工作线程调用，结果用 msime_client_string_free 释放。 */
char *msime_client_migrate_android_key_layout(const uint8_t *request, size_t length);
/* 可重绑切换热键（keybindings.toggle_english_candidates、toggle_full_width、toggle_punctuation、toggle_voice_input）按共享 KeyChord 契约解码。请求是 JSON {"keybindings": object}（<=65536 字节）；keybindings 缺省或为 null 时按出厂值，不是 object 时返回错误。value 是 {"english_candidates","full_width","punctuation","voice_input"}，每项是 null（写法为空或不合法：不绑定）或 {"key": 主键规范名, "ctrl": bool, "alt": bool, "shift": bool, "meta": bool}；规范名是 client-core KeyChord 的 Display 输出（KeyA-KeyZ、Digit0-Digit9、F1-F12、Space、Semicolon、Equal、Comma、Minus、Period、Slash、Backquote、BracketLeft、Backslash、BracketRight、Quote），宿主按规范名认键，写法的解析只在 Rust 一侧。结果用 msime_client_string_free 释放。 */
char *msime_client_toggle_hotkey_chords(const uint8_t *request, size_t length);
// 录键试写：{"field": 槽位, "chord": 写法, "keybindings": 偏好, "voice_ctrl_f9": bool}。
// value 为 {"ok": true, "chord": 规范写法}，或 {"ok": false, "kind": "invalid"|"conflict", "owner"?: 占用者}。
// 请求最多 64 KiB，不落盘；返回值通过 msime_client_string_free 释放。
char *msime_client_check_toggle_hotkey(const uint8_t *request, size_t length);
/* 私有的聚合打字统计。JSON 请求（<=65536 字节）：
 * {directory:绝对路径,action:{operation:"load"|"reset"}}
 * {directory,action:{operation:"set_enabled",enabled:bool}}
 * {directory,action:{operation:"set_retention",retention,day:"YYYY-MM-DD"}}
 *   retention 取 forever|30d|90d|180d|365d，其他值一律按 forever 读，绝不当成更短的时间窗。`day` 是调用方的本地日期。
 * {directory,action:{operation:"record",text,source,day:"YYYY-MM-DD",hour?:0-23}}。
 * record 在内存里给上屏文本分类，只持久化聚合计数；文本既不返回也不存储。可能阻塞在磁盘或文件锁上：请在工作线程调用。`hour` 是这次上屏的本地小时，必须与 `day` 取自同一时刻；拿不准就省略，不要猜，这一天的计数照样保留，只是不按小时拆分。record 返回 {recorded, milestone}：milestone 是总数刚刚越过的成就计数（100、1000……），没有越过时为 null。只有本进程里有会话开着成就音效时才计算它，而在桌面宿主上此时提示音已经排进队列。record 不再每次上屏都写文件：计数攒在本进程里（按 directory 分开），距上次读写这个目录的统计文件满 30 秒后的下一次 record、flush，或这个 directory 上的任何其他操作才在文件锁内重读文件、合并写出（期间别的进程改了保留期就合并进它写下的文件；清空了统计或关掉了统计就丢掉这批，清空之前打的字不会写回清空后的文件）；recorded 按上次读到的开关回答。进程被杀时最多丢 30 秒的计数。
 * {directory,action:{operation:"flush"}} 把本进程为 directory 攒着的上屏写进统计文件，value 为 null；没有攒着的上屏时不碰磁盘。宿主在键盘收起、输入框失焦和进程或服务退出前，在平时调统计的那个工作线程上调用；旧版本的库不认识 flush 会回错误，宿主忽略即可。会话失焦、msime_client_destroy 和 msime_client_flush_all 也会写出本进程攒着的全部上屏（macOS、Linux 的失焦和移动宿主的失焦与销毁在后台线程上写，不阻塞调用线程）。
 * {directory,action:{operation:"record_keys",day:"YYYY-MM-DD",keys:{"KeyA":3,...}}}
 *   把各键的按键次数加到 `day` 上，`day` 是这些按键发生的本地日期（跨过午夜的一批先按旧日期写出）。每个键只存每日按键次数：不存顺序、时间或文本。键 id 是 W3C KeyboardEvent.code 名加上软键盘 id（Nine0-Nine9、SoftPunctuation、SoftSymbol、SoftLayer、SoftLanguage、SoftGlobe、SoftEmoji、SoftVoice）；完整列表是 client-services typing_statistics.rs 里的 KEY_IDS。只要有一个未知 id 或零计数就拒绝整批；绝不要自造 id。返回 {recorded:n}，统计关闭时为 0（什么都不写）。宿主在内存里攒批，从工作线程调用，绝不每按一个键调用一次。
 * {directory,action:{operation:"summary",day:"YYYY-MM-DD",user_words?:n}} 返回调用方本地 `day` 的派生指标（总览、习惯、按键、成就）；统计开着时，新解锁的成就在锁内写下。user_words 是词库 "count" 加 user_only:true 的结果。隐私模式不影响它：它只读。
 * {directory,action:{operation:"record_voice",day,milliseconds}} 记一次语音输入的时长；{directory,action:{operation:"record_skin",id}} 记一次用到的皮肤。两者都返回 {recorded}。
 * 「上传到云端」，给自己发 HTTP 的宿主（Android、iOS、鸿蒙）：开关、设备标识和上次上传的时间存在 directory 下的 typing-statistics-upload.json 里，构建和十分钟的节流都在这里，宿主只负责发请求。{directory,action:{operation:"upload_status"}} → {enabled,lastUploadMs}（lastUploadMs 还没有上传过时为 null）。{directory,action:{operation:"set_upload_enabled",enabled}} → 同上：只改本机状态、不联网；打开时第一次生成设备标识，关闭时清掉上次上传的时间（设备标识保留，重新打开时服务端仍认作同一台设备）。宿主打开前要求用户已登录，打开后接着做一次 force 的 upload_request；关闭后自己发 DELETE /v1/users/me/typing-statistics 删掉云端副本，删除失败时开关仍是关的。{directory,action:{operation:"upload_request",day,platform,force?}} → 要发时 {method:"PUT",path:"/v1/users/me/typing-statistics",device,body}，body 是请求体对象（{device,platform,days:[{day,characters,active_seconds,hours?,categories?,sources?}],selections?}），宿主带登录账号的令牌、以 application/json 发出；不发时 {skip}，取 disabled（没打开上传）、not_due（离上次上传成功不到十分钟，force 为 true 时不看）或 nothing（最近 60 天没有字数）。day 是宿主的本地日期，platform 是 android、ios、harmony 等服务端认的平台 id（不认识的平台发出去是空串）。这一步不记任何东西：发送失败时什么也不用做，下一次照样交回请求。{directory,action:{operation:"upload_succeeded",device}} → {enabled,lastUploadMs}：服务端回 204 以后调用，device 是上一步交回的 device；期间用户关掉了上传或设备标识变了时不记。宿主要把 upload_request、发送、upload_succeeded 和关闭后的 DELETE 放在同一个串行队列里，免得关掉后的删除和还在路上的上传交错。
 * 请求顶层可加 status:true（只用于 load、set_enabled、set_retention 和 reset，用于别的操作时整个请求被拒绝）：回答换成设置页读的状态 {statistics,availability:"ready"|"neverWritten",lastWrittenMs:n|null}，availability 和 lastWrittenMs 来自统计文件的修改时间，与 Tauri 外壳的 load_typing_statistics 相同。
 */
char *msime_client_typing_statistics(const uint8_t *request, size_t length);
/* Read only the aggregate-statistics master switch from an absolute UTF-8
 * directory. Returns 1 when enabled, 0 when disabled/missing, and -1 for an
 * invalid directory or unreadable document. Intended for native capture gates:
 * call on activation or a settings-change notification, never per keystroke. */
int32_t msime_client_typing_statistics_enabled(const uint8_t *directory, size_t length);
/* Read or update 背单词 wordbooks and review progress under an absolute UTF-8
 * application data directory. Every action answers with the whole status
 * -- {wordbooks,settings,due,answeredToday,introducing,remaining,queue} -- so a
 * host keeps one request in flight and never follows a change with its own read.
 * Every request also carries `resources`, the staging root that holds both
 * `EngineResources/` and the `wordbooks/` sibling with the bundled books (中考/高考/CET-4/CET-6/考研/雅思/
 * 托福/GRE, built by scripts/fetch_wordbooks.py). A host that stages none simply
 * offers the imported books; a bundled book is read-only and cannot be deleted.
 * {directory,resources,day:"YYYY-MM-DD",action:{operation:"load"}}
 * {directory,resources,day,action:{operation:"answer",wordbook,word,known}}
 * 作答必须携带显示卡片时的来源 wordbook；缺少、为空或已不存在时拒绝，不推断当前选择。
 *   known is the 认识 button; false is 不认识 and returns the card to the same day.
 * {directory,resources,day,action:{operation:"set_settings",wordbook,new_per_day,session_limit}}
 * {directory,resources,day,action:{operation:"import",name,text}}
 *   text is a CSV/TXT word list; the library mints the id and selects the book.
 * {directory,resources,day,action:{operation:"remove",wordbook}}
 * {directory,resources,day,action:{operation:"remove_personal_word",word}} 把一个词移出生词本（id 为 personal、带 personal:true 的那本书），复习进度保留；移走最后一个词时生词本随之删除。
 * {directory,resources,day,action:{operation:"reset"}} clears progress, keeps the books.
 * 可选的 plugins 是插件目录的绝对路径：其中的单词本插件按 pack-<插件 id> 列在内置书之后、导入的书之前，带 pack:true；对它们的 remove 失败（在插件页卸载），卸载后复习进度保留。不传 plugins 的宿主只有内置和导入的书。
 * `day` is the caller's local day and is required by every action: the counts and
 * the queue are per-day and this layer cannot resolve the host's timezone.
 * 请求最多 17 MiB，为原始 8 MiB 词表的常规 JSON 转义和目录等元数据留出空间；解码后的
 * 词表仍受共享解析器的 8 MiB 上限约束。会取文件锁并读取词书，只能在工作线程调用。 */
char *msime_client_vocabulary_review(const uint8_t *request, size_t length);
/* 候选窗的「加入生词本」：{directory,candidate,gloss,pronunciation?}，directory 是与背单词相同的绝对应用数据目录，candidate 是候选原文，gloss 是候选旁显示的释义（几种语言时用换行或 U+2028 分行，义项用 ; 或 ；分隔），pronunciation 是随释义显示的读音（几行时只用第一行，可省略）。词头取第一行的第一个义项，释义是候选原文；同一个词已在生词本里时合并释义。
 * 成功时 value 是 {outcome,notice}：outcome 为 added、merged、already_present、full（生词本已满）或 invalid（释义做不成卡片），notice 是给用户看的一句提示，宿主原样显示。只有请求或存储本身出错时 ok 为 false。
 * 会取词书目录的文件锁并读写生词本：宿主在用户点了菜单之后调用一次，不在每次按键时调用。请求最多 64 KiB。 */
char *msime_client_personal_wordbook_add(const uint8_t *request, size_t length);
/* 候选菜单的「朗读」和背单词卡片的喇叭要读的英文：{candidate,gloss?}，gloss 是候选旁显示的释义（分行与义项分隔同 msime_client_personal_wordbook_add），背单词卡片把词头当作 candidate、不给 gloss。成功时 value 是要读的文字（只含 ASCII 字母、空格、-、'、.，以字母开头，最多 64 个字符），没有能读的时是 null，宿主据此不出「朗读」。取每行释义的第一个义项，第一个是英文的就是答案；没有英文释义时候选本身是英文就读候选。不读写文件，可以在打开菜单时调用。请求最多 64 KiB。 */
char *msime_client_read_aloud_text(const uint8_t *request, size_t length);
/* 今日待复习的张数，给菜单栏、托盘菜单这类用户自己点开才看得到的地方用；组字期间不显示，也不进候选窗。请求是 {directory,resources,day:"YYYY-MM-DD",plugins?}，含义与 msime_client_vocabulary_review 相同；手里只有 HostOptions 文档的宿主（Linux）可以不给 resources，改给 host_options:<整份 HostOptions>，内置词书的资源根按设置应用同样的规则（随包资源目录，词库在线更新后是原安装包目录）求出，两者恰好给一个。请求最多 1 MiB。返回值与 msime_client_vocabulary_review 应答里的 due 是同一个数（包括今天要出的新卡），但只读进度文档和选中的那一本书，不读书目、不组装队列。从没开过背单词、没选书或选中的书已不在时返回 0，且不创建任何文件或目录；请求不合法、进度文档损坏或选中的书读不出来时返回 -1，宿主应隐藏这一项。会取进度文件锁并读一本书（内置书最大约 1 MB）：在打开菜单时或工作线程上调用，不要每个按键都调。 */
int32_t msime_client_vocabulary_due_count(const uint8_t *request, size_t length);
/* 鸿蒙设置应用导入键盘发布的生词本：{directory,published,previous?}，三个都是绝对路径。API 12 起输入法扩展用独立沙箱，键盘把「加入生词本」写进自己沙箱里的一本（以键盘的某个目录为 directory 调 msime_client_personal_wordbook_add），经共享沙箱镜像发布；published 是这次发布的那本生词本文件（vocabulary-wordbooks/personal.json 的副本），previous 是设置应用存下的、上次已经并过的那一版（没有时省略，文件不在或读不懂时当作没有）。只并进上次以后新出现或改过的词条，键盘还没清走交过的词时，设置这边移出的词不会因为键盘那本里还留着而回来；键盘清走交过的词以后（msime_client_personal_wordbook_drain）宿主不再传 previous。两份文件都按单本书的上限（8 MiB）读，不跟随符号链接。成功时 value 是 {added,merged,unchanged,overLimit,failed,firstFailures,truncated}，overLimit 大于 0 表示设置应用那本已满、有词没并进来，宿主应当下次再试而不是记成已并过。published 不是有效的生词本文件时 ok 为 false。会取词书目录的文件锁：同步调用会等工作线程上正在写词书的请求做完。请求最多 64 KiB。 */
char *msime_client_personal_wordbook_merge(const uint8_t *request, size_t length);
/* 鸿蒙键盘一侧：{directory,published}，都是绝对路径。设置应用确认已经并过键盘发布的 published 那一版以后，把那一版里的词条从 directory 下键盘自己的生词本里清走：只清与之完全相同的词条，之后又改过释义的和新加的留着；一个不剩时生词本文件随之删除。成功时 value 是 {removed}。published 不是有效的生词本文件时 ok 为 false。会取词书目录的文件锁。请求最多 64 KiB。 */
char *msime_client_personal_wordbook_drain(const uint8_t *request, size_t length);
/* Scan an absolute UTF-8 skin root and return the catalog the settings page sees: {packages:[...],issues:[...]}. Reads the directory: use a worker. An unreadable root is an empty catalog; an invalid package becomes an issue and is never returned as renderable. Presenters must still check that a package supports the layout and theme before adopting its colors. Keys are camelCase, the same document the settings page consumes. Besides the colours (drawn through msime_client_resolve_theme, where a palette's translation becomes secondary) a package carries what a presenter draws itself: minWidthDip; cornerRadiusDip (0-32, null keeps the host radius); decorationTopDip, decorationWidthDip and decorationImage (package-relative, null unless decorated) placed by decorationAlign ("left"|"center"|"right"), with decorationMotion ("none"|"float"|"sway"|"breathe"|"bounce", an idle motion of the whole image while the window is on screen), decorationMotionPeriodMs (1200-8000) and decorationFrames (0 without a decoration image, 1 for a still image, the animation frame count of an APNG, GIF or WebP otherwise; a host that cannot play frames draws the first); background null or {image,fit:"cover"|"contain"|"stretch",opacity:0-1}, drawn over the surface and under the candidates, clipped to the card outline; toolbar {cornerRadiusDip|null, light, dark} where each mode is {background,border,handle,divider,icon,hover}, each #RRGGBB, #RRGGBBAA or null for the host's own. A package may also carry notices, non-fatal strings about manifest keys it parsed but does not apply here (canvas-only keys on a non-canvas base, or a canvas package's nine-slice panel and widgets, which no msime host draws); the key is absent when there are none and the package stays usable. Image paths are already confined to the package and are images; read them with msime_client_skin_resource. */
char *msime_client_skin_catalog(const uint8_t *directory, size_t length);
/* Scan an absolute Engine resource directory for optional custom helper-code tables. The
 * response value is an array of {schema,file_stem,name,name_en}; missing or unreadable
 * helpcodes/custom is an empty array. Display metadata comes from leading # name: and
 * # name_en: comments, while the Engine remains the owner of table parsing. */
char *msime_client_helpcode_schemas(const uint8_t *resources, size_t length);
/* JSON {directory:absolute skin root,id:package folder}. Validates that one package with the same loader as msime_client_skin_catalog and returns one of its camelCase packages entries; an invalid, built-in, symlinked or missing package is {ok:false,error} with the loader's reason. Reads the package: resolve on a skin or appearance change, never while drawing. */
char *msime_client_skin_package(const uint8_t *request, size_t length);

/* 图片工具栏的无 I/O 布局。请求 {toolbar: 扫描缓存中的 toolbarImage,
   shown: [[按钮键, 当前状态], ...]}，最多 7 个当前显示按钮；皮肤只能进一步隐藏。
   返回 {widthDip,heightDip,buttons:[{key,xDip,yDip,widthDip,heightDip,iconSizeDip,
   background,image}]}，尺寸覆盖背景与全部按钮。失败不能继续使用旧命中区。
   UTF-8 JSON 最多 64 KiB；结果用 msime_client_string_free 释放。 */
char *msime_client_skin_toolbar_image_layout(const uint8_t *request, size_t length);
/* 同步解码有界内存中的 PNG/APNG 默认图或 GIF/JPEG/WebP 首帧，不读文件。
 * 输入最多 8 MiB，边长最多 2048。成功时只调用一次回调，rgba 为非预乘 RGBA8，
 * length = width * height * 4。字节只在回调期间有效，宿主必须当场复制；回调不得抛异常。
 * 只有回调返回 1 时整个调用返回 1；其他回调值返回 0。
 * 坏图、空指针和超限返回 0，且不调用回调。
 * 只能在皮肤资源变化时加载，不能在逐键绘制中解码；这不是动画播放接口。 */
typedef int32_t (*msime_client_skin_image_callback)(void *context, const uint8_t *rgba,
                                                   size_t length, uint32_t width, uint32_t height);
int32_t msime_client_skin_image_decode(const uint8_t *bytes, size_t length,
                                      msime_client_skin_image_callback on_frame, void *context);
/* 同步解码整张 PNG/APNG、GIF、JPEG、WebP，全部验证成功后才按序回调。
 * 每帧为完整画布，像素格式、借用期和线程要求与单帧接口一致；最多 120 帧、800 万像素。
 * index 从 0 开始，count 为总帧数；delay_ms 是该帧持续时间，静态图为 0。
 * play_count 是完整播放次数，0 表示无限；有限播放结束保持末帧。
 * GIF 无循环声明时播放一次；APNG/GIF <=10ms 归一化为 100ms；WebP 多帧固定 100ms 无限。
 * 坏图、空指针和超限返回 0 且无回调。任一回调不返回 1 时停止并返回 0，宿主须丢弃已收帧。
 * 宿主必须对同一包的不同图片累计像素预算；不能在逐键绘制中调用。 */
typedef int32_t (*msime_client_skin_animation_callback)(void *context, const uint8_t *rgba,
    size_t length, uint32_t width, uint32_t height, uint32_t index, uint32_t count,
    uint32_t delay_ms, uint32_t play_count);
int32_t msime_client_skin_animation_decode(const uint8_t *bytes, size_t length,
    msime_client_skin_animation_callback on_frame, void *context);
/* JSON {directory:absolute path,id:skin id,relative:package asset,kind:"image"|"font"}.
 * Returns {contentType,bytes}; the manifest and package containment are
 * revalidated for every call and the requested kind must match the asset.
 * With the optional encoding:"base64" it returns {contentType,bytesBase64} instead: the bytes array costs about 64 bytes of heap per resource byte while the response is built, so every msime caller asks for base64. Any other encoding is an error. */
char *msime_client_skin_resource(const uint8_t *request, size_t length);
/* JSON {directory:absolute path,id:skin id}; returns a nullable stylesheet
 * string from the manifest, after revalidating the package and path. */
char *msime_client_skin_toolbar_stylesheet(const uint8_t *request, size_t length);
/* JSON {source:absolute picked folder,directory:absolute skin root}. Copies the
 * folder under its own name, replacing a skin of that name whole; returns {id}.
 * The error message is skin_name, skin_manifest or storage. Touches the disk. */
char *msime_client_skin_import(const uint8_t *request, size_t length);
/* The shared settings page's 「制作皮肤」, ruled by client-services skin::candidate_editor. JSON {operation:"check",package_id,manifest,files} reads the draft with the catalog loader in a scratch folder, without touching the skin root, and returns the camelCase package entry msime_client_skin_catalog would list (a template manifest such as base = "wechat" with no files resolves that look's colours). {operation:"create",directory:absolute skin root,package_id, manifest,files,replace} installs it with the installer community downloads and MCP use and returns {id,replaced}. files maps package-relative image paths to standard base64. A refusal is {ok:false,error,detail?}: error is a candidate_skin_* code or storage (candidate_skin_exists when the id is taken and replace is false), detail the loader's reason when the manifest did not load. Requests may be up to 4 MiB. Decodes images and touches the disk: use a worker. */
char *msime_client_candidate_skin_editor(const uint8_t *request, size_t length);
/* The queued personal dictionary, for a host that cannot take the Engine's
 * maintenance lock when the request arrives. Same request shape as
 * msime_client_dictionary - {options,action} - but the operations act on
 * <preferences_directory>/PersonalDictionary instead of the Engine, and the
 * keyboard applies them at its next session start. Use this for import_personal
 * in particular: "maintenance busy" is not an answer to "add these words", and
 * the user is as likely to import with the keyboard up as with it down. */
char *msime_client_personal_dictionary_request(const uint8_t *request, size_t length);
/* JSON {directory:absolute state root} reads the named custom touch-keyboard
 * designs; adding action:{operation:"create"|"rename"|"update"|"delete",...}
 * applies one change first. Both answer with the whole library, because every
 * caller redraws the list. Takes the library's file lock and rewrites it
 * atomically: use a worker. Failures carry the shared community_* codes the
 * settings pages already have wording for, not a Display string. */
char *msime_client_custom_skin_library(const uint8_t *request, size_t length);

/* 共用键盘设计操作，不写磁盘。请求 operation 为 catalog、reset、template（index）、
 * background（index、design）或 optimize_contrast（design）。返回普通 {ok,value} JSON；
 * catalog 返回 default、templates、backgrounds，其余返回完整设计。未知字段、无效设计、
 * 越界索引和超过 1 MiB 的请求返回 keyboard_skin_invalid。结果由 msime_client_string_free 释放。 */
char *msime_client_keyboard_skin_design(const uint8_t *request, size_t length);
/* JSON {directory:absolute state root,id:publication uuid,name,design}. Starts
 * a skin trial and imports the design into the library, answering
 * {skin,trial}. One call rather than two: the trial is what remembers the skin
 * being replaced, so a failed import has to end it or the user wears a design
 * that was never saved. The download itself is the caller's, because only the
 * surrounding platform's HTTPS stack can fetch it. Writes preferences and two
 * locked files: use a worker. */
char *msime_client_community_skin_install(const uint8_t *request, size_t length);
/* JSON {directory,action:{operation:"finish",id,keep}} or
 * {operation:"restore_pending"}. Declining a trial puts the previous skin back;
 * restore_pending is the crash recovery and is safe with no trial pending.
 * Answers {revision} so a caller holding the document can tell whether what it
 * is showing is still what is on disk. Writes preferences: use a worker. */
char *msime_client_keyboard_skin_trial(const uint8_t *request, size_t length);
/* JSON {file:absolute CommunityLibrary.json,action:{operation:"load"}} or
 * {operation:"save_reply",item} or {operation:"remove",id}. The reply templates
 * the user explicitly kept, which is the one thing the settings surface and the
 * keyboard process share about the community. Every operation answers with the
 * whole library. Takes the library's file lock: use a worker. */
char *msime_client_community_resource_library(const uint8_t *request, size_t length);
/* The decisions in AI skin generation, for a host whose HTTP must go through
 * the surrounding platform and so performs the four requests itself.
 * {operation:"compose",prompt,model} returns {path,body} carrying the shared
 * system prompt; {operation:"parse",text} returns the three validated plans or
 * refuses; {operation:"artwork",artwork} says whether a returned image is one
 * this client will show. The instruction and the parser must not be separated:
 * the prompt names the exact document the parser accepts. */
char *msime_client_ai_skin_plan(const uint8_t *request, size_t length);
/* Absolute staged engine resources; returns {profile,sourceCommit} from the
 * packaged msime-dictionary-manifest.json. Two fields only: the page is asking what
 * dictionary is installed and where it came from, not for journal modes or
 * third-party references. Missing or unreadable is reported, never guessed -
 * showing the wrong dictionary version is worse than showing none. */
char *msime_client_dictionary_manifest(const uint8_t *resources, size_t length);
/* Read saved history only; disabled preferences return an empty entries array. */
char *msime_client_load_clipboard_history(const uint8_t *directory, size_t length);
/* JSON {directory,text}; removes exact saved entry, not the system clipboard. */
char *msime_client_remove_clipboard_history(const uint8_t *request, size_t length);
/* JSON {directory,text}; capture under the shared preference/history locks. */
char *msime_client_capture_clipboard_history(const uint8_t *request, size_t length);
/* 桌面剪贴板历史，供「剪贴板」设置页使用：与 Tauri 外壳的剪贴板命令读写同一个 directory/clipboard_history.json。request 是 length 字节（最多 131072）的 UTF-8 JSON {directory: 偏好目录的绝对路径, action}，action 是 {operation:"list"|"sync"|"clear"}、{operation:"set_pinned",text,pinned} 或 {operation:"remove",text}；text 按内容找条目，非空且不超过 12000 字节。sync 只在 clipboard_history 开着时读系统剪贴板（只有 Windows 读，带密码管理器等隐私标记的内容不记录，末尾换行去掉），关着时 error 是 clipboard_history_disabled，其他平台读不到时是 clipboard_history_unavailable。clear、set_pinned 和 remove 不看开关。每个操作都返回 {enabled, entries:[{text,timestampMs,pinned}]}，是操作后的历史，先固定的、再按时间从新到旧；开关关着时 entries 为空。请求不合法时 error 是 clipboard_history_invalid，读写失败时是 clipboard_history_unavailable。读写磁盘、sync 还要打开剪贴板：放在工作线程上调用。 */
char *msime_client_desktop_clipboard_history(const uint8_t *request, size_t length);
/* 移动端的结构化剪贴板历史，与桌面的自动记录偏好无关。请求 JSON 为 {directory: 历史根目录的绝对路径, preferences_directory?: 偏好目录的绝对路径, action}；省略 preferences_directory 时从 directory 读取偏好，供原有宿主继续使用。action 是 {operation:"load"|"clear"}、{operation:"capture"|"remove",text}、{operation:"remove_many",texts}、{operation:"restore_many",entries}、{operation:"set_pinned",text,pinned} 或 {operation:"replace",text,replacement}。remove_many 原子返回 removed 完整条目，restore_many 原子恢复其文字、时间戳和固定状态；已有同文条目不覆盖。replace 按内容找到原条目原地改字，时间戳和固定状态不变；新文字与另一条相同时两条合并，固定状态取两者之或；返回 replaced、merged 和 reason（"not_found"|"invalid"|null）。历史存在 directory/MSIME/clipboard_history.json。 */
char *msime_client_mobile_clipboard_history(const uint8_t *request, size_t length);
/* Same validation as load_preferences; ok:true,value:null means lock busy.
 * Does not wait for the writer lock. Disk I/O may still block: use a worker.
 * Busy is not missing/corrupt and must not reset preferences to defaults. */
char *msime_client_try_load_preferences(const uint8_t *directory, size_t length);
/* Repair a preferences.json that is not well-formed JSON (truncated, empty, overwritten). May block on disk and the writer lock: use a worker. Backup first: the damaged bytes are copied verbatim to <directory>/preferences.json.corrupt-YYYYMMDD-HHMMSS (UTC, -N suffix when taken) and nothing is rewritten if that copy fails. Then every setting and service key the schema still accepts is carried onto the defaults. A valid or missing document is a no-op: {recovered:false, snapshot}. A repair returns {recovered:true, snapshot, backup_path, backup_name, salvaged}. A well-formed document the schema rejects (unknown fields, newer format_version) is most likely a newer build's and returns the load error unchanged; the settings page repairs it explicitly with msime_client_repair_preferences. Storage errors are returned and never lead to a rewrite. */
char *msime_client_recover_preferences(const uint8_t *directory, size_t length);
/* The settings page's explicit 修复配置文件: the same repair for any document load_preferences rejects, a newer build's (unknown fields, newer format_version) included. Same backup, salvage, result shape and worker requirement as msime_client_recover_preferences. Call it only when the user asks: what this schema does not accept takes its default. */
char *msime_client_repair_preferences(const uint8_t *directory, size_t length);
/* Compare-and-swap save of PreferencesSnapshot.preferences. The snapshot's
 * format_version is validated; expected_revision must match the store.
 * A disabled clipboard-history save clears the default history file if history
 * is still disabled. Cleanup errors may be returned after preferences are saved.
 * Directory <=16384 bytes; snapshot <=1048576 bytes, enough for a custom skin photo. */
char *msime_client_save_preferences(const uint8_t *directory, size_t directory_length,
                                    uint64_t expected_revision,
                                    const uint8_t *snapshot, size_t snapshot_length);
/* 在会话线程传入 PreferencesSnapshot JSON（最多 1048576 字节）：{format_version:1, revision, preferences:{...}}。修订号按会话递增；相同快照可重试，旧版或冲突快照会被拒绝。返回 {revision, deferred, view, diagnostic?, missing_resource_pack?}；仅首选方案无法运行并回退时带 diagnostic。首选方案因本机缺它的词库而回退时另带 missing_resource_pack（"wubi" 或 "language-dictionaries"），宿主可经 msime_client_resource_pack_install 下载，装好后下一次焦点刷新即换回首选方案。组字期间的 Engine 偏好应用延后到派发或焦点调用使会话空闲，但关闭云候选会立即清除现有云行和旧查询，AI 配置或令牌变化也会立即清除旧 AI 行和查询；释义开关、目标语言或翻译服务变化会清掉旧释义并推进候选代次，旧释义回包将被拒绝。宿主应使用返回的新 view。较新快照替换待应用快照。构建失败时保留旧会话和待应用快照供重试，派发通过 diagnostic 报告重试错误，不丢失已完成输入。本接口不读写偏好文件，宿主提供已加载的快照。 */
char *msime_client_update_preferences(uint64_t session, const uint8_t *snapshot, size_t length);
/* 获得焦点（true）只重置旧组字，包括 phrase_preedit 暂存的词段，不返回旧组字的提交；失焦（false）按当前方案结束组字，并返回应送出的提交，包括已经选好的暂存词段。 */
char *msime_client_focus(uint64_t session, bool focused);
/* Clear the current session's Engine candidate cache and refresh its view. */
char *msime_client_reset_cache(uint64_t session);
/* Release process-level model caches without a session handle (memory pressure): the Japanese dictionary, personal context models no session uses, and the plugin tables kept for the next session. Also releases the calling thread's candidate-gloss caches, as msime_client_release_gloss_caches does. */
void msime_client_release_process_cache(void);
/* 关掉调用线程缓存的候选释义词库连接（发布词库、学到的释义库、离线释义库），丢掉读好的自定义译文；下一次查询重新打开，结果不变。不动日文词典这类重新加载很贵的模型，宿主可以在每次键盘收起时调用。只清调用线程自己的那份，要在查询释义的那个线程上调。 */
void msime_client_release_gloss_caches(void);
/* 语音设置服务目录。纯函数，无文件、网络或设备访问；响应 value 为 {asr:[preset],polish:[preset],doubao_stream_endpoints:[{id,label,title,endpoint}]}。
 * preset 为 {id,title,endpoint,model,models:[string],documentation}，仅含公开预设。
 * 数组顺序即设置页顺序；不含平台特有的 system/local，宿主按能力追加。
 * models 仅作建议，不限制用户自定义模型。宿主复制字段后用 msime_client_string_free 释放返回的 JSON 缓冲区。 */
char *msime_client_voice_provider_catalog(void);
/* 设置页识别服务行的状态文案。provider 为 system、local、云端服务 id 或空串；resolved
 * 表示共享解析已得到可用云端配置，platform_available 是宿主是否有系统识别服务，后两个
 * 参数只在 provider 为 local 时使用。纯计算，返回 {status:string|null}。 */
char *msime_client_mobile_voice_status(const uint8_t *provider, size_t provider_length,
                                       bool resolved, bool platform_available,
                                       bool model_ready, bool runtime_ready);
/* 设置页润色行的状态文案；enabled 是润色开关，configured 表示共享解析已有可用配置，
 * has_token 表示当前服务或槽位存在凭据。纯计算，返回 {status:string|null}。 */
char *msime_client_mobile_voice_polish_status(bool enabled, bool configured, bool has_token);
/* Android 语音设置的服务分类和豆包鉴权模式。provider 与 auth_mode 是 <=256 字节 UTF-8，返回
 * {network:boolean,legacy_auth:boolean}，纯计算，不读取偏好或设备状态。 */
char *msime_client_mobile_voice_policy(const uint8_t *provider, size_t provider_length,
                                       const uint8_t *auth_mode, size_t auth_mode_length);
/* 切换语音服务的纯函数；请求 <=1 MiB：{service:"voice.asr"|"voice.polish",provider:string,
 * current:{provider?,endpoint?,model?,token?,tokens?:{id:string}},field_policy?:"fill_defaults"|"clear"}。
 * current 缺少的字段按空值处理。field_policy 缺省为 fill_defaults，只替换空白或出厂值；clear 在换服务时清空地址和模型。
 * value 为 {provider,token,tokens,endpoint?,model?}：省略地址/模型表示保留原值，空串表示清空。
 * 不读写偏好、不联网；宿主负责把补丁字段映射到 asr_ 或 polish_ 并保存。
 * 请求和返回含凭据，禁止记录；返回缓冲区用 msime_client_string_free 释放。 */
char *msime_client_voice_provider_update(const uint8_t *request, size_t length);
/* 语音设置接口预检；输入是 <=64 KiB 的 UTF-8 地址，websocket 为 true 时按 wss 校验，否则按 https 校验。
 * 首尾空白与运行时解析一样去除，空地址表示使用默认值；非空地址复用运行时的 2048 字节和 URL 形状规则，并保留移动宿主的小写协议、无 @ 限制。
 * value 为 {accepted:boolean,issue:string|null}；失败提示固定，不回显输入，不访问偏好或网络。
 * 空引用、超限和非法 UTF-8 返回错误；返回缓冲区用 msime_client_string_free 释放。 */
char *msime_client_voice_endpoint_issue(const uint8_t *endpoint, size_t length, bool websocket);
/* 纯语音凭据预检。空值或空白值表示删除并接受；其他值若含控制字符、超过 16 KiB，或豆包握手凭据不是 ASCII 则拒绝。
 * 返回 {accepted}，绝不回显凭据。 */
char *msime_client_voice_credential_issue(const uint8_t *provider, size_t provider_length,
                                          const uint8_t *value, size_t value_length);
/* 流式 wss 地址的传输描述；输入是 <=64 KiB 的 UTF-8 地址，实际地址遵循共享 2048 字节与 URL 形状规则。
 * value 为 {socket_host:string,port:uint16,host_header:string,request_target:string}；保留显式端口 0，缺省端口为 443。
 * socket_host 不含 IPv6 方括号，host_header 保留方括号与非默认端口，request_target 使用 URL 库序列化的路径与查询。
 * 不访问网络，失败不回显输入；返回缓冲区用 msime_client_string_free 释放。 */
char *msime_client_voice_websocket_endpoint(const uint8_t *endpoint, size_t length);
char *msime_client_voice_start(uint64_t session);
char *msime_client_voice_cancel(uint64_t session);
/* Capture bounded mono 16 kHz samples. The JSON result is transient audio and
 * must never be logged or persisted. */
char *msime_client_voice_capture(uint32_t milliseconds);
/* The microphones a settings page offers under 「可用录音设备」 (the Tauri settings shell's
 * list_voice_capture_devices on Windows). Value [{backend:"windows",id,label}], at most 128,
 * [] when there is none: id is "wasapi:" then the WASAPI endpoint id as four lowercase hex
 * digits per UTF-16 unit, the value voice_input.capture_device holds with capture_backend
 * "windows". Other targets fail with "voice capture devices unavailable". Enumerates through
 * COM: call off the UI thread. Ids and labels may identify the user's hardware: never log them. */
char *msime_client_voice_capture_devices(void);
#ifdef _WIN32
/* 原生语音审阅面板复用 Server 控制协议。create 不录音，返回 {handle:uint64}，至多 32 个活动句柄。
 * recognize 消费句柄一次，在工作线程阻塞到识别结束，返回 {text}；不自动输入或修改剪贴板。
 * language 为 1..64 字节 ASCII 语言标签。progress 可空，非空时在调用线程借用 UTF-8 JSON
 * {phase:connecting|recording|recognizing|processing|complete|cancelled|failed,level:0..1,text?:string}。
 * 回调不能展开异常，可重入 control；不保留其借用缓冲区，也不记录或落盘音频／文本。
 * control 可从任何线程调用：0 请求结束录音，1 取消，2 关闭并取消；关闭后句柄永不复用。
 * 执行中的调用继续持有自己的取消信号；宿主关闭窗口时先 control(2)，再等待 recognize 返回。
 * 完成／失败不隐式关闭句柄，调用方必须 control(2)。所有返回字符串用 string_free 释放。 */
char *msime_client_windows_voice_session_create(void);
char *msime_client_windows_voice_session_control(uint64_t handle, uint32_t operation);
char *msime_client_windows_voice_session_recognize(uint64_t handle, const uint8_t *language, size_t length,
    void (*progress)(const uint8_t *, size_t, void *), void *context);
#endif
char *msime_client_voice_apply(uint64_t session, uint64_t generation,
                               const uint8_t *text, size_t length);
/* On-device speech models and user-dictionary hotwords. JSON request buffers of length bytes; standard responses. Error text of the model calls is a stable code beginning with "local_model_".
 * voice_hotwords: {options: HostOptions as msime_client_dictionary, limit?: 200} -> {hotwords:[{text,pinyin}]}, the user's own pinyin words (two or more Chinese characters), heaviest first. Worker thread; fails with "dictionary maintenance busy" while maintenance holds the store. <=1 MiB.
 * voice_hotword_correct: {text, hotwords:[{text,pinyin}]} -> {text}. Pinyin-similarity replacement for models whose msime-model.json has "hotwords":"pinyin". Pure. The transcript is limited to 10000 Unicode scalar values, with at most 1000 hotwords; each hotword text is <=256 bytes and pinyin <=1024 bytes. The JSON request is <=1048576 bytes.
 * voice_local_models: {root: absolute dir} -> {models:[{id,title,description,languages,streaming,default,desktop_only,installed,path,installed_size,archive_size,memory,license_spdx,license_source,license_terms,license_notice,hotwords,import_files:[{name,url,size}]}], default: id}. path is <root>/<id>, the value for voice_input.asr_model_path. import_files 是不联网安装时要用户自己下载的文件（压缩包和带地址的附加文件），url 是上游下载地址。
 * voice_local_model_install: {root, id, mirror?: "https://..." prefix, files?: [absolute path]} -> {path}。阻塞到安装结束，只能在工作线程调用。progress 可为空，在调用线程收到 {id,stage:"download"|"import"|"verify"|"extract"|"done",downloaded,total}，返回前需复制缓冲区。登记后、访问文件或网络前先回调 download（导入为 import）的零字节进度，total 为目录里的归档字节数；回调可重入 voice_local_model_cancel 取消，本次安装随即报 "local_model_cancelled"。每个 id 同时只允许一次安装（"local_model_install_running"）。带 files（至多 16 个）时不联网，用这些本地文件安装：按长度和 SHA-256 认文件、不看文件名，进度阶段 download 报成 "import"，缺文件时失败为 "local_model_import_missing: <name>"，所选文件打不开或读出错时为 "local_model_import_unreadable: <detail>"。
 * voice_local_model_cancel: {id} cancels that install, NULL/0 or {} cancels all; value is whether one was running. Any thread.
 * voice_local_model_remove: {root, id} -> null. Only catalog ids; refused while that id is installing.
 * voice_model_setting_check: {asr_model_mirror?, asr_model_path?} (<=64 KiB) -> {asr_model_mirror?: bool, asr_model_path?: bool}, one key per field asked: whether the preferences store accepts that value (the same checks as saving). Pure. */
typedef void (*msime_client_voice_local_model_progress_callback)(const uint8_t *json, size_t length,
                                                                 void *context);
char *msime_client_voice_hotwords(const uint8_t *request, size_t length);
char *msime_client_voice_hotword_correct(const uint8_t *request, size_t length);
/* voice_normalize_numbers: {text, kind} -> {text}. 本地模型最终文本里汉字写的数字改成数字（二零零二年 → 2002年，百分之五十七 → 57%），一个、十分有趣这类词不动。只改写不自带 ITN 的 kind（online_transducer、offline_transducer），其他 kind 原样返回，宿主对每个本地模型的最终文本都可以调用；只用于最终文本，不用于中间结果。Pure. text 上限 10000 个 Unicode 标量，kind 上限 64 字节，请求 <=1048576 字节。 */
char *msime_client_voice_normalize_numbers(const uint8_t *request, size_t length);
/* voice_punctuation: {text, mode} -> {text}. 语音标点：按 mode 处理最终识别结果，auto 原样返回，omit_trailing 去掉句末标点，omit_all 去掉所有标点，space 把每串标点换成一个空格；不认识的 mode 按 auto。只动识别器写出的句读（，。、；：？！…～ 和半角 , . ; : ? !），数字和网址里的点、数字之间的冒号和逗号保留，引号和括号保留。整段只有标点时返回空串。宿主在所有识别服务和润色之后、上屏之前调用，mode 取 msime_client_mobile_voice_configuration 的 punctuationMode。Pure. text 上限 10000 个 Unicode 标量，mode 上限 64 字节，请求 <=1048576 字节。 */
char *msime_client_voice_punctuation(const uint8_t *request, size_t length);
char *msime_client_voice_local_models(const uint8_t *request, size_t length);
char *msime_client_voice_local_model_install(
    const uint8_t *request, size_t length,
    msime_client_voice_local_model_progress_callback progress, void *context);
char *msime_client_voice_local_model_cancel(const uint8_t *request, size_t length);
char *msime_client_voice_local_model_remove(const uint8_t *request, size_t length);
char *msime_client_voice_model_setting_check(const uint8_t *request, size_t length);
/* 按需资源包存放在 <state_root>/resource-packs/<id>/；state_root 是 HostOptions 的 preferences_directory（Android: files/bootstrap/state），会话下次取得焦点时读取新发布的包。
 * 包 ID：`japanese`、`language-dictionaries`、`handwriting`、`settled-model`、`grammar-model-wanxiang`、`grammar-model-baguwen`、`grammar-model-baguwen-compact`、`offline-glosses`、`voice-runtime`、`wubi`、`voice-runtime-macos`（锁文件固定了重签的 macOS 运行库之后才有）。请求为 `length` 字节的 JSON（<=16384），响应使用标准封装。`install` 和 `adopt` 是阻塞调用，只能放在工作线程。
 * 错误为 "invalid resource pack request"、"invalid state root"、"invalid source"、"resource_pack_unknown"、"resource_pack_busy"（本进程正在安装、校验、收编或卸载该包）或 "local_model_*"。
 * resource_packs: {state_root} -> [{id, state:"missing"|"installed"|"outdated", size, schemes:[...], import_files:[{name,url,size}]}]，每包一项；size 为下载字节数，import_files 为离线安装所需文件（voice-runtime 使用归档），url 为上游下载地址。
 * resource_pack_install: {state_root, pack, sources?:["https://mirror/", ...], files?:[absolute path]} -> {path}。
 * sources 是依次尝试的镜像前缀（最多 8 个，跳过空项），之后尝试项目镜像和来源 URL；固定资源按锁中的大小和 SHA-256 校验，语法模型按官方在线目录的大小和 SHA-256 校验。固定资源未完成文件在下次调用时用 HTTP Range 续传。channel 可选 stable、beta、nightly，语法模型优先选同通道条目，缺失时回退。
 * files（最多 16 个绝对路径）表示改从本地安装，按大小和 SHA-256 匹配而非文件名，忽略 sources，下载阶段改报 "import"；缺文件返回 "local_model_import_missing: <name>"，不可读返回 "local_model_import_unreadable: <detail>"。
 * progress 可为空，在调用线程收到 (context, phase "download"|"import"|"verify"|"done", done, total)，阶段字符串仅在回调期间有效。
 * 首次进度在任务登记后、任何文件或网络 I/O 前发送（done=0，total=资源包大小），回调可重入 resource_pack_cancel；取消后返回 "local_model_cancelled"。
 * resource_pack_cancel: pack 是原始 UTF-8 id（不是 JSON）；NULL/0 取消本进程全部资源包安装。可在任何线程调用，立即返回，不触发进度回调。
 * resource_pack_remove: {state_root, pack} -> {removed: bool}。阻塞卸载已发布的资源包，先摘除引擎侧共享映射缓存；未安装、未发布目录和符号链接返回 false，活跃会话仍持有映射导致的删除失败如实返回错误；本进程正在安装、校验、收编或卸载同一个包时返回 "resource_pack_busy" 且不删除，卸载期间的安装同样返回 "resource_pack_busy"。只能放在工作线程。
 * resource_pack_adopt: {state_root, pack, source: absolute dir} -> {path}。在同一文件系统内把包文件从 source 重命名移入（不复制），按编译锁校验后发布；失败则全部移回，不发布任何内容。已有相同字节的安装时直接返回，不改动 source。 */
typedef void (*msime_client_resource_pack_progress_callback)(void *context, const char *phase,
                                                             uint64_t done, uint64_t total);
char *msime_client_resource_packs(const uint8_t *request, size_t length);
/* 主动检查语法模型目录，不下载：{state_root,channel?} -> [{pack,source_commit,size}]。 */
char *msime_client_resource_pack_check_grammar_updates(const uint8_t *request, size_t length);
char *msime_client_resource_pack_install(const uint8_t *request, size_t length,
                                         msime_client_resource_pack_progress_callback progress,
                                         void *context);
void msime_client_resource_pack_cancel(const uint8_t *pack, size_t length);
char *msime_client_resource_pack_remove(const uint8_t *request, size_t length);
/* 重新校验资源包的全部固定文件，不写入：{state_root, pack} -> {path}。 */
char *msime_client_resource_pack_verify(const uint8_t *request, size_t length);
char *msime_client_resource_pack_adopt(const uint8_t *request, size_t length);
/* 自定义翻译请求描述器（不做网络 I/O）：api.deepl.com / api-free.deepl.com 用 DeepL v2 格式（DeepL-Auth-Key 鉴权、text 数组，没有密钥时报错），其他地址用 DeepLX 兼容格式。Request <=16 KiB:
 * {config:{enabled,endpoint,api_key},text,source_language,target_language}.
 * Returns null if disabled; otherwise {url,method,headers,body,timeout_ms,max_response_bytes}.
 * Descriptor can contain credentials: never log it. Host enforces timeout/size,
 * rejects redirects and checks HTTP status before parsing. Text <=256 scalars. */
char *msime_client_custom_translation_http_request(const uint8_t *request, size_t length);
/* Pure visible-page plan <=64 KiB: {target_language,candidates:[{text,source}]}.
 * Returns [{text,key,source_language,target_language}]; at most nine candidates. */
char *msime_client_custom_translation_plan(const uint8_t *request, size_t length);
/* Pure signed TMT descriptor <=64 KiB. Input: {config:{enabled,secret_id,
 * secret_key,region},texts:[string],source_language,target_language,timestamp}.
 * Send body_utf8 bytes unchanged. Never log this credential-bearing descriptor. */
char *msime_client_tencent_translation_http_request(const uint8_t *request, size_t length);
/* Pure NiuTrans v2 descriptor <=64 KiB. Input: {config:{enabled,app_id,apikey},
 * text,source_language,target_language,timestamp}. */
char *msime_client_niutrans_translation_http_request(const uint8_t *request, size_t length);
/* OpenAI 兼容的 AI 翻译描述器和解析器。请求不超过 64 KiB：
 * {config: AiAssistantPreferences, input:{text,source_language,target_language}}.
 * It reuses the configured AI endpoint and token, and returns one JSON string or null. */
char *msime_client_ai_translation_http_request(const uint8_t *request, size_t length);
char *msime_client_parse_ai_translation_response(const uint8_t *body, size_t length);
bool msime_client_ai_translation_reply_failed(const uint8_t *body, size_t length);
/* A settings page's 「测试配置」 check, the one the Tauri settings shell runs on Windows and
 * macOS. Request <=64 KiB: {service,config}, service one of translation.tencent ({secret_id,
 * secret_key,region}), translation.niutrans ({app_id,apikey}), translation.custom ({endpoint,
 * api_key}), voice.asr, voice.polish or ai.assistant, config the object the shared page sends.
 * Value {ok,message}: message is the Chinese result line to show; it never carries the
 * provider's reply or the credential. An incomplete config is answered without the network;
 * otherwise this sends one request and blocks up to about 15 s: call off the UI thread.
 * The request carries credentials: never log it. */
char *msime_client_test_api_credential(const uint8_t *request, size_t length);
/* The same check for a settings page without its own copy of the provider defaults (the Android native app). Request <=64 KiB: {preferences_directory, service}, service voice.asr or voice.polish; the configuration is resolved from the stored preferences exactly as the keyboard resolves it. Value {ok,message} as above; a provider with nothing to test (system, local, unknown) is answered without the network. Blocks up to about 15 s: call off the UI thread. */
char *msime_client_test_voice_credential(const uint8_t *request, size_t length);
/* Pure check of a custom translation endpoint (UTF-8, <=64 KiB), for the settings page. Value
 * {issue,accepted}: issue is the Chinese hint the shared page shows under the endpoint field
 * (translationEndpointIssue: empty endpoint, too long, control characters, not a URL,
 * credentials or fragment, remote http), "" when there is none; accepted is whether the
 * preferences store saves it (empty, https, or http to localhost/127.0.0.1/[::1]). No I/O. */
char *msime_client_translation_endpoint_issue(const uint8_t *endpoint, size_t length);
/* Pure check of an AI assistant endpoint (UTF-8, <=64 KiB), for the AI settings page. Value
 * {origin,problem,hint}: origin is the "scheme://host:port" key ai_assistant.tokens stores the
 * token under (aiCredentialOrigin), null when the endpoint is not usable; problem is null,
 * "invalid" or "cleartext_public" (http to a host that is not this machine or the local network);
 * hint is the Chinese line the shared page shows under the field (aiEndpointHint), "" when usable.
 * No I/O. */
char *msime_client_ai_endpoint_check(const uint8_t *endpoint, size_t length);
/* The AI settings page's 「获取模型」, the Tauri settings shell's ai_models. Request <=256 KiB:
 * {endpoint,token}, the token being ai_assistant.tokens[origin]. GETs the /models sibling of the
 * endpoint without following redirects (http only to this machine or the local network, never
 * through a proxy). Value: up to 128 model ids, in the service's order. Error: a stable code,
 * ai_invalid (request or endpoint/token rejected, nothing sent), ai_models_unavailable (no
 * connection, timeout or error status) or ai_models_invalid (reply too large or no ids). Blocks
 * up to about 15 s: call off the UI thread. The request carries the token: never log it. */
char *msime_client_ai_models(const uint8_t *request, size_t length);
/* AI 公开预设数组；含 id/title/endpoint/model/models/documentation/desktop_only/uses_token，不做 I/O。 */
char *msime_client_ai_provider_catalog(void);
/* 纯计算切换补丁。请求 <=16 KiB：`{provider,current:{enabled,provider,endpoint,model,endpoints?,models?}}`。
 * `endpoints`/`models` 是按服务商 `id` 记忆自定义地址/模型的字符串映射，缺省为空映射；切换前保存当前自定义值，再按目标槽位回填，缺键或空字符串使用目录默认值。
 * 返回 `{enabled,provider,endpoint,model,endpoints,models}`；宿主须将两个映射随补丁保存。不传凭据，错误码 `ai_invalid`。不保留请求缓冲区。 */
char *msime_client_ai_provider_update(const uint8_t *request, size_t length);
/* The AI settings page's 「测试」, the Tauri settings shell's ai_test. Request <=256 KiB:
 * {endpoint,model,prompt,token,text}: model non-empty <=256 bytes; prompt (may be empty) and
 * text (non-empty) <=16 KiB each, newlines and tabs allowed. POSTs one non-streaming chat
 * request (prompt as system, text as user) to the endpoint without following redirects. Value:
 * the trimmed reply text. Error: ai_invalid, ai_test_unavailable or ai_test_invalid, as for
 * msime_client_ai_models. Blocks up to about 15 s: call off the UI thread. The request carries
 * the token and the user's text: never log it or the reply. */
char *msime_client_ai_test(const uint8_t *request, size_t length);
// Pure AI descriptor from {config:AI preferences,input:{segmented_pinyin,context,candidate_limit}}.
// Input <=64KiB. Result contains credentials; never log it. Host must forbid redirects.
char *msime_client_ai_http_request(const uint8_t *request, size_t length);
// Successful HTTP body <=1MiB, limit 1..10. Returns a string array or null.
char *msime_client_parse_ai_response(const uint8_t *body, size_t length, uint8_t limit);
// Worker-thread disk I/O. JSON <=64KiB: {directory:absolute private user path,
// action:lookup|remember,target_language,generation,items:[{text,direction,translation?}]}.
// At most 9 items, directions english_to_chinese/chinese_to_english. Remember
// requires translation; lookup forbids it. Returns {generation,translations,saved}.
// Never pass packaged resources. Learned text is private; never log requests.
// A malformed batch makes no writes; an I/O failure can leave earlier items saved.
// New writes use Engine translation-glosses.db; old JSON records remain read-only fallback.
char *msime_client_learned_translation_request(const uint8_t *request, size_t length);
/* Returns [string|null] with exact expected count (1..9), or null for invalid body. */
char *msime_client_parse_tencent_translation_response(const uint8_t *body, size_t length, size_t expected);
/* Provider body <=1 MiB. Returns a formatted translation string or null. */
char *msime_client_parse_niutrans_translation_response(const uint8_t *body, size_t length);
/* Host-produced gloss <=64 KiB UTF-8. Returns it formatted like provider replies (whitespace collapsed, ends trimmed) or null when empty or it has a control char. */
char *msime_client_format_translation_gloss(const uint8_t *text, size_t length);
/* Provider body <=1 MiB. True when the reply reports a failure (malformed, non-200 code, errorCode/errorMsg) rather than an answer, empty or not. Only answers may be negative-cached. */
bool msime_client_custom_translation_reply_failed(const uint8_t *body, size_t length);
bool msime_client_niutrans_translation_reply_failed(const uint8_t *body, size_t length);
/* Provider body <=1 MiB. Returns translation string <=4096 bytes or null when
 * malformed/no result. No session mutation; host validates original identity. */
char *msime_client_parse_custom_translation_response(const uint8_t *body, size_t length);
/* 同代次的非空 JSON [{"text":"candidate","translation":"gloss","gloss":false}] 合并候选释义。
 * [] 主动清空；宿主异步无结果时跳过调用，旧代次不应用。gloss 可省略，缺省为 false；翻译查询 gloss_query 的回答标 true。
 * 候选词翻译范围是「仅当前高亮」时，不带 gloss 标记的回答只画在高亮行（候选的 translation_reserved 为真的那一行），带标记的离线释义每一行都画。 */
char *msime_client_apply_translations(uint64_t session, uint64_t generation,
                                      const uint8_t *translations, size_t length);
/* Resolve copied candidates against the packaged offline English dictionary.
 * JSON request: {generation,candidates:[{text,source}],user_data?,target_language?,state_root?}; the generation is echoed for the host to pass to apply_translations on the session thread. This function owns no session handle and may run on a worker thread.
 * target_language absent or "en" reads msime-english.db and the user's glosses. fr/ja/es/ru/de/ko read only offline-glosses/zh-<lang>.db beside resources, or, when that is absent and state_root (absolute; pass the translation query's state_root) is given, the same file in the downloaded <state_root>/resource-packs/offline-glosses pack; they ignore user_data. When neither is installed the result is {generation,translations:[]}, not an error. Any other value is an invalid request. Only Chinese candidates get a non-English gloss. */
char *msime_client_candidate_gloss_request(const uint8_t *request, size_t request_length,
                                           const uint8_t *resources, size_t resources_length);
/* 不持有会话地查询候选释义行的读音，可在宿主工作线程调用。
 * JSON 请求：{generation,items:[{text,language}],supplementary_resources?}；language 为 "en"、"fr"、"ja"、"es"、"ru"、"de"、"ko" 之一，是这一行释义的目标语言，其他值整批拒绝。
 * en：text 为英文候选或一行英文释义；只读首个词组，忽略开头的 to/a/an/the，任一词未知则不返回该组；读音为 "/…/" IPA。读 pronunciations/en-phonetic.db。
 * 其余语言：text 为这种语言的一行释义；只读第一个分隔符（; ； , ， 、 / ? ？）前的词条，去掉括号里的注释，整条查表（先原样、再小写），查不到不返回。法、西、俄、德语的读音是维基词典写的 IPA，带着它的 "/…/" 或 "[…]"；日文是赫本式罗马字（neko、chūgoku），韩文是文化观光部式罗马字（goyang-i、hakgyo），都不带斜线或括号，宿主按形状校验时只能要求它们非空。读 target-pronunciations/<language>-phonetic.db。
 * 响应为 {generation,pronunciations:[{text,language,pronunciation}]}，按请求顺序，没有读音的项省略；宿主按 (language,text) 对应回去。
 * 每种语言优先读 resources 旁的表；缺失时可从 supplementary_resources 下读取同名相对路径（资源包 pronunciations 与 target-pronunciations 的根）。
 * 补充目录为可选的绝对路径，最多 4096 字节；缺省保持旧行为。某种语言两处均未安装时它的项没有读音，不报错。 */
char *msime_client_pronunciation_request(const uint8_t *request, size_t request_length,
                                         const uint8_t *resources, size_t resources_length);
/* Break copied Chinese candidates into words with their English, without a session, for a line a host draws under the gloss lines (never a committable gloss column).
 * JSON request: {generation,texts:[...]}, at most 64 texts. Only all-Han texts of 2 to 32 characters are considered; each is cut left to right by longest match (up to 8 characters) against character-glosses/zh-en.db (single characters first) and word-glosses/zh-en.db beside resources, and each piece shows the first phrase of its gloss: "我 I · 喜欢 to like · 你 you". A text with fewer than two glossed pieces or more than eight pieces is omitted. The response is {generation,breakdowns:[{text,breakdown}]}; with neither table installed it is empty, not an error. May run on a worker thread. */
char *msime_client_gloss_breakdown_request(const uint8_t *request, size_t request_length,
                                           const uint8_t *resources, size_t resources_length);
/* 查询固定英文词库，不创建或改变输入会话。
 * JSON 请求：{prefix,limit,correction?:false,completions?:false}；limit 为 1..32，响应为 {prefix,items:[...]}。
 * 缺省按原有方式补全 ASCII 字母前缀；correction:true 时接受字母与撇号，第一行始终原文，最多附加一行纠错。
 * 纠错仅处理不超过 32 字节的输入，33..128 字节原样返回；缺词库也保留原文。
 * completions:true 只能与 correction:true 同用（单独使用报错），返回手机候选栏的合并展示，响应为 {prefix,items:[...],corrected}：
 * 有纠错时 items 为 [原文, 纠错]；原文已是完整的词（词库精确命中、固定词形或受保护的网络缩写）时只有 [原文]；否则原文后接至多 limit-1 个前缀补全，按大小写不敏感去重，原文首字母大写时补全首字母随之大写，其余照词库拼写。
 * corrected 当且仅当 items[1] 是纠错时为 true。不带 completions 的请求响应形状保持不变，不含 corrected。
 * resources 必须是资源分代目录的绝对路径；词库只读，检索结果可以缓存，可在工作线程调用。 */
char *msime_client_english_completions_request(const uint8_t *request,
                                               size_t request_length,
                                               const uint8_t *resources,
                                               size_t resources_length);
// Live per-session mode, not a persisted preference. Preserves composition and
// candidate generation; remains authoritative across preference replacement.
char *msime_client_set_chinese_punctuation(uint64_t session, bool enabled);
/* 即时设置来源 18 混合候选的中文片段输出，保留组字与候选身份；返回最新 View。
 * 不落盘，变体沿用最新共享偏好；后续偏好更新可替换此临时值。 */
char *msime_client_set_multilingual_output(uint64_t session, bool traditional);
char *msime_client_set_character_width(uint64_t session, bool fullwidth);
char *msime_client_set_english_mode(uint64_t session, bool enabled);
/* 引擎负责的九键模式，用于全拼九宫格或注音九键（注音下 View.nine_key_spellings 是目标音节的候选读音）。只在组字结束后调用。View.nine_key 和 View.nine_key_spellings 是权威状态。对其他方案开启、或在组字中切换模式都会被拒绝。等同于 msime_client_set_key_grid 的 1（开）和 0（关），所以关掉时 14 键也一并关掉。
 * 创建和重建会话时按偏好自动开启：全拼看 touch_keyboard_layout 是否为 nine_key（fourteen_key 则开 14 键）；注音还要求 touch_keyboard_schemes 选中（或在没有选中项时启用了）zhuyin_nine_key，这个方案只有 Android 写，桌面宿主为全拼九宫格写下的 nine_key 不会让注音会话离开大千键位。宿主设下的网格只保留到方案或 touch_keyboard_layout 改变为止。
 * View.touch_keyboard_layout 是已应用的宿主呈现偏好；日文九键宿主只用它，不开启引擎的九键模式。 */
char *msime_client_set_nine_key_mode(uint64_t session, bool enabled);
/* 切换引擎的组码网格：0 关闭，1 九键（同 msime_client_set_nine_key_mode），2 当前全拼、双拼或五笔方案的 14 键（QWERTY 上相邻两个字母一个键，L、M 单独一键），3 九键双拼（表按已应用的 nine_key_shuangpin_profile 取），4 十八键全拼，5 十八键双拼（Q WE RT Y U IO P / A SD FG H JK L / Z XC V BN M）。只在组字结束后调用；14 键只给全拼、双拼或五笔，九键双拼和网格 4 只给全拼，网格 5 只给双拼。对其他方案开启、在组字中切换都会被拒绝，其他取值报错，会话不变。返回值中的 View 反映结果。
 * View.key_grid 是权威状态："none"、"nine_key"、"fourteen_key"、"eighteen_key" 或 "nine_key_shuangpin"；两种十八键均为 "eighteen_key"，按 scheme 区分。View.nine_key 只在九键下为真。Android 和 iOS 双拼会话在创建和重建时按 touch_shuangpin_layout 选择 26、14 或 18 键，五笔会话按 touch_wubi_layout 选择 26 或 14 键（鸿蒙的五笔 14 键由宿主自己调用本函数开启网格 2），touch_keyboard_layout 为 eighteen_key 的全拼会话在这两个宿主上开启网格 4；其他宿主创建会话时不据这两个 18 键偏好开网格（鸿蒙的全拼 18 键由宿主自己按偏好调用本函数开启网格 4）。字母双拼网格从已应用的 shuangpin_profile 及自定义表派生，分号与 L 共键。五笔 14 键不经九宫格会话：组码由五笔方案按组展开码表，没有拼音选择、单字和笔画筛选，nine_key_spellings 为空，nine_key_reading 是首选编码的前几位（同 preedit），候选按五笔照常选择；View.key_grid 仍是 "fourteen_key"，按 scheme 区分。其余几种网格共用 View.nine_key_spellings、nine_key_reading、nine_key_single_character、nine_key_strokes，以及 msime_client_choose_nine_key_spelling 和 msime_client_set_nine_key_filter；字母网格的分词键照九键送 '\''。字母网格下数字不是输入，照常是选词键。九键双拼的数字从 msime_client_character 进来（方案用到的数字，可以有 1 和 0），和九键一样不当选词键，没有英文九键；创建和重建会话时 touch_keyboard_layout 为 nine_key_shuangpin 的全拼会话自动开启它。View.grid_composing 说这次组字是不是在网格里打的（九宫格会话或五笔 14 键在组字）：网格开着时硬件键盘的字母照样走 26 键，这时它为假，没有组字时也为假。宿主要按当前组字实际走哪种退格、哪种编辑来画（例如光标隔着读音行里的显示分隔画在哪一侧）时看它，不看 View.key_grid。 */
char *msime_client_set_key_grid(uint64_t session, uint8_t grid);
/* 字母网格的一键：letter 是这一组里的任一小写字母，引擎归成这一组，宿主约定送当前网格各组的首字母。仅十八键双拼另接受 A-Z，表示同一网格里的精确小写字母；editing_text 保留大写标记用于编辑和恢复，它不是 Shift 或英文字母上屏。返回与 msime_client_character 相同的输入响应。不在 14 或 18 键下、方案不支持当前网格、在本地模式或专用英文里、26 键正在组字、或 letter 不在上述范围（双拼另接受分号）时 handled=false，会话不变。
 * 五笔 14 键最多四码：四码之后的一键先把首选顶上屏，再开始下一组编码，组码里没有 z，所以 z 通配、z 拼音反查和混输拼音都不作用于它；四码唯一时与 26 键一样按偏好自动上屏。
 * 硬件键盘的字母照常经 msime_client_character 走当前方案的 26 键：字母网格组字进行中它们 handled=false，同九键。 */
char *msime_client_grid_key(uint64_t session, uint8_t letter);
/* 标出隐私会话（隐私模式、不允许学习的输入框）：不记录选词位置和上屏效率，并立即禁止、清除上屏后联想；宿主须重新读取 View，以丢弃旧候选代次。用户单独关掉学习不算隐私会话；学习本身仍由 learning 偏好决定。Value 是设下的布尔值。 */
char *msime_client_set_private_session(uint64_t session, bool enabled);
/* Android 的 next_word_prediction 偏好开启时，View.prediction=true 标记上屏后的空闲联想：editing_text、preedit 和 reading 为空；只有显式选择候选才上屏。空格、回车、数字、导航和失焦不提交联想；普通状态省略 prediction 字段，宿主缺省按 false 读取。 */
/* 中日英循环切换的状态机（`crates/client-core/src/language_cycle.rs`），一个键在 中 → 日 → 英 → 中 之间转圈。顺序来自旧版 Windows 正式版 `engine/contracts/trilingual_input.h` 的 `Next()`：中文 → 日文 → 英文 → 中文；参与循环的是「切换中英文」那一组快捷键，不是 Ctrl+Shift+J。
 * 参数：enabled 是 keybindings.trilingual_cycle；mode 是按下循环键那一刻的模式，0 中文、1 日文、2 英文；scheme 是实际在跑的方案，编号同 View.scheme；last_chinese_scheme 是要回去的中文方案（0/1/2/5/6/9），-1 表示还没记下；has_composition 是宿主会话里还有组字（已选词段或未上屏的拼音）。
 * Value 是 {mode, scheme, commit_raw}：mode 同上编号；scheme 是进入中文时要切到的方案，null 表示只换中英模式；commit_raw 为真时先原样上屏正在组的内容（已选词段保留，剩下没选的拼音串照原样出去），再换模式。value 为 null 表示这一键不循环（开关关着，这一组快捷键仍是二态切换），宿主照旧自己处理这个键。非法编号返回错误。
 * 有组字时上的屏是原始拼音串，不是高亮的候选：旧版在改名或缩写时按住循环键是为了保住已经打出来的字母，而不是替用户选一个他没要的词。 */
char *msime_client_language_cycle(bool enabled, uint8_t mode, uint8_t scheme, int8_t last_chinese_scheme,
                                  bool has_composition);
/* 报告大写锁定状态：偏好 caps_lock_ascii_punctuation 打开时，大写锁定期间没有组字的标点键（经 msime_client_character、msime_client_punctuation 或 msime_client_punctuation_with_context 送来）改走字面 ASCII 路线，和英文模式一样；punctuation_lock 为 chinese 时仍是中文标点。会话状态而非偏好，会话重建后由宿主重新报告。Value 是设下的布尔值。接上这个调用的宿主在 HostCapabilities.caps_lock_punctuation 里声明。 */
char *msime_client_set_caps_lock(uint64_t session, bool enabled);
char *msime_client_set_paired_punctuation(uint64_t session, bool enabled);
char *msime_client_set_punctuation_lock(uint64_t session, uint8_t lock);
/* 每页候选数的会话内覆盖，size 为 1–10；本平台排不下那么多时（Windows、iOS、鸿蒙最多 9）按平台上限截断，view.page_size 是实际值。组字中调用时等组字结束才生效。 */
char *msime_client_set_candidate_page_size(uint64_t session, uint8_t size);
char *msime_client_character(uint64_t session, uint8_t ascii, bool shift);
/* 触屏 26 键双拼的字母键几何，只有触屏宿主调用；桌面宿主和实体键盘永远不调用它，实体键盘的字母照常走 msime_client_character。只在布局变化时调用（旋转、分离、单手、键高调整、数字行变化、微软双拼 ';' 显隐），不要每键都发。json 是 length 字节的 UTF-8 JSON（<=16384，拒绝未知键）：{"id":u32,"keys":[[cx,cy,w,h] x 26，按 a..z],"semicolon":[cx,cy,w,h] 或 null,"duplicates":[["g",cx,cy,w,h],...]}。id 是宿主分配的非零代次，布局一变就换一个；semicolon 是微软双拼显示的 ';' 键；duplicates 是分离键盘里重复出现的字母键。坐标系由宿主自定，但必须是没有经过键距挪动、不受按下缩放影响的键区坐标，引擎按字母键键宽、键高的中位数归一。数值不是有限数、宽高不为正、不是 26 个字母键或归一后的键心离原点超过一万个键时整份拒绝，原来的几何不变。length 为 0 或 id 为 0 清除几何。几何由会话持有，宿主空闲时重建引擎也不用重发。返回 View。 */
char *msime_client_set_key_geometry(uint64_t session, const uint8_t *json, size_t length);
/* 触屏 26 键的一键，带上按下点（touch-down，不是抬起点）：x、y 与注册的几何同一个坐标系，geometry_id 是那份几何的 id。ascii 仍以宿主的命中测试为准（含键距归属），是用户实际按到的键。只在 26 键双拼的字母键和微软双拼的 ';' 上用它；辅助码 Shift、本地模式、专用英文、14/18 键、九键数字、无障碍点击和实体键盘照常用 msime_client_character。引擎把落点用于给双拼纠错计价，坐标只留在会话内存里，不落盘、不进统计。geometry_id 与当前注册的几何不符、坐标不是有限数或当前不是 26 键双拼时，结果与 msime_client_character(session, ascii, shift) 逐字节相同。返回与 msime_client_character 相同的输入响应。 */
char *msime_client_touch_character(uint64_t session, uint8_t ascii, bool shift, uint32_t geometry_id, float x,
                                   float y);
// Explicit native punctuation: finish the highlighted composition, then translate.
// Invalid non-punctuation bytes fail without modifying the session.
char *msime_client_punctuation(uint64_t session, uint8_t ascii);
/* Resolve native punctuation using the immediately preceding Unicode scalar
 * supplied by the platform editor. Zero means unavailable. Only the scalar is
 * inspected and no document text is retained. */
char *msime_client_punctuation_with_context(uint64_t session, uint8_t ascii,
                                            uint32_t preceding);

/* 无会话、无 I/O 的跨次空格策略。请求含 config（enabled/excluded_apps）、application、context（scheme/local_mode），可带 text、preceding Unicode 标量、secure、full_width。返回 {enabled,text}；先询问 enabled，再读取必要的前文。最多 65536 字节；结果按通常的 ok/value 信封返回，由 msime_client_string_free 释放。 */
char *msime_client_cross_commit_spacing(const uint8_t *request, size_t length);
/* Balance Engine nesting after the host emitted a paired closing mark. Only
 * the ASCII book-title opening '<' is accepted. */
char *msime_client_balance_paired_punctuation_after_auto_close(uint64_t session,
                                                               uint8_t opening);
// Finish the highlighted composition, then append the literal ASCII mark.
// Hosts use this for platform smart-punctuation decisions based on editor
// context; invalid non-punctuation bytes fail without modifying the session.
char *msime_client_punctuation_ascii(uint64_t session, uint8_t ascii);
/* View.scheme and commit_context.scheme: 0 quanpin, 1 shuangpin, 2 wubi, 3 japanese, 4 korean (preferences scheme "korean"). Korean is a Dubeolsik Hangul automaton: send every letter through msime_client_character with its case (Shift+Q/W/E/R/T/O/P type ㅃ ㅉ ㄸ ㄲ ㅆ ㅒ ㅖ); View.reading and View.preedit hold the composing Hangul to mark inline with the caret at its end, while editing_text holds the key letters of the open syllable and is non-empty exactly while composing. A transition may carry a commit together with a new composition (the previous syllable finished when a new one started) and a commit with handled=false (Space, Enter, a caret key, Delete or a digit ended the syllable): always insert the commit first, then let an unhandled key do its normal work in the application. Punctuation is always half-width ASCII and never converted to full width. With no Hanja list open there are no candidates: MSIME_BACKSPACE removes one jamo; MSIME_CANCEL discards the open syllable; msime_client_focus(false) commits it, while msime_client_focus(true) discards it so a syllable left open in one client never reaches the next.
 * Hanja: candidates appear only after MSIME_CONVERT_HANJA while a syllable is composing, so a host reads scheme 4 with a non-empty candidate list as "the Hanja list is open". The list holds the Hanja of the composing syllable (a syllable already committed is not converted), each candidate's annotation is its 훈음 when it has one, and its code is the key letters, which a host should not draw. MSIME_CONVERT_HANJA answers handled=false when the composition has no Hanja (a lone jamo) or nothing is composing; a host should then swallow its trigger key while composing rather than pass it on. Sending it again closes the list. While the list is open the candidate commands work as for any list (MSIME_COMMIT_CANDIDATE, digits 1-9 on the visible page, paging and MSIME_NEXT/PREVIOUS_CANDIDATE), and choosing commits the Hanja with handled=true; send MSIME_COMMIT_CANDIDATE for Return as well, since only the session knows the highlight. MSIME_CANCEL and MSIME_BACKSPACE only close the list and keep the syllable composing. A letter closes the list and composes as usual. Punctuation, MSIME_FINISH_COMPOSITION and msime_client_focus(false) close the list and commit the Hangul, never a Hanja, whatever is highlighted; msime_client_focus(true) still discards the syllable. */
/* View.scheme 和 commit_context.scheme 续：8 tibetan（藏文，偏好方案 "tibetan"），在拉丁字母键盘上按 EWTS（扩展威利转写）输入，不需要词库，所有宿主都提供，和越南文一样默认不启用。威利转写区分大小写，大写字母（T D N Sh A I U M H 等）是拼写：带大小写原样送进 msime_client_character，不要当成 Shift 命令。组字保存当前音节串的威利原文，View.preedit 和 editing_text 显示转换后的藏文，候选列表始终为空。View.spelling_symbols 空闲时为 "'/"，组字时为 "'+-./"，这些键要作为字符交给引擎，尤其 '/' 不能由宿主自己插入 ASCII 斜杠。组字时空格（字符 ' ' 或 MSIME_COMMIT_CANDIDATE）上屏藏文加音节点 U+0F0B，'/' 上屏藏文加垂符 U+0F0D，两者都是 handled=true；没有组字时 '/' 单独上屏垂符，空格和数字交回宿主。MSIME_COMMIT_RAW（回车）只上屏藏文、不加音节点，handled=true。MSIME_BACKSPACE 删掉一个原文按键；第一次 MSIME_CANCEL 把显示退回威利原文，第二次丢弃组字。数字、光标键、Delete、Tab 和 MSIME_COMMIT_READING 按显示上屏后 handled=false：先插入 commit，再让按键在应用里照常生效。其他标点先上屏藏文再跟半角 ASCII 标点，从不转成中文或全角标点。msime_client_focus(false) 上屏组字。数字保持原样，不转成藏文数字。 */
/* View.scheme and commit_context.scheme, continued: 5 cantonese (Jyutping), 6 zhuyin (Dachen bopomofo), 7 vietnamese (Telex or VNI), 9 stroke (笔画: five-stroke order 横竖撇点折 typed as the letters h s p n z, with x matching any one stroke), each the preferences scheme of the same name. Every host offers them, and Cantonese, Zhuyin and Stroke only with their language dictionaries installed; where a scheme cannot run, a preferences document naming it runs the last Chinese scheme or quanpin instead and msime_client_update_preferences says so in its diagnostic.
 * Stroke: only h s p n z start a composition; x and other letters return handled=false while idle, so a host inserts them as usual. While composing x appends a wildcard, other letters are swallowed with handled=true, and digits 1-9 pick candidates. View.reading and View.preedit hold the stroke glyphs 一丨丿丶乛＊, one per typed letter, which a host draws inline; editing_text and caret_position count the ASCII letters. MSIME_COMMIT_RAW commits the typed letters, MSIME_COMMIT_CANDIDATE the highlighted character (the letters when there is none), and focus loss does not commit.
 * MSIME_OPEN_CANDIDATE_LIST is MSIME_CONVERT_HANJA under the name that says what it does in every scheme: open the active scheme's candidate list. Korean opens and closes its Hanja list as described above; Zhuyin opens the candidate list of its composition; every other scheme answers handled=false. */
/* View.display_reading 是宿主画在行内的读音。日文罗马字（scheme 3，非本地模式、非英文）组字时末尾单个 n 还没定成ん，这里留作字母（kan 是 かn，kann 才是 かん），与微软、Google 日文输入法一致；其余方案和状态下与 View.reading 相同。View.reading 仍是 MSIME_COMMIT_READING 上屏的读音，末尾的 n 在那里已是ん。宿主不经引擎结束组字区时：光标被移走、编辑器仍报告着组字区，应把组字区换成 View.reading 再结束；确认不了组字区还在（编辑器报告没有组字区、会话停下）时只结束组字，不要把 View.reading 写进可能已被应用清空或改写的输入框，这时文档里可能留下画出的读音。只读 View.reading 的宿主行为不变。 */
/* 整句改字（全拼、双拼）：桌面宿主把不带修饰键的左右方向键发成 MSIME_CONVERSION_LEFT / MSIME_CONVERSION_RIGHT（macOS 不进改字，发 MSIME_MOVE_LEFT / MSIME_MOVE_RIGHT）；Shift+左右和 Ctrl+左右在所有方案里都发 MSIME_MOVE_LEFT_SEGMENT / MSIME_MOVE_RIGHT_SEGMENT，按音节移动拼音光标。偏好 navigation.left_right 打开时，横排候选窗里不带修饰键的左右键可能被换成移动候选（高亮不在首选，或首选上光标已在末尾时按右键），分段命令从不被换，所以高亮在哪个候选上它们都编辑拼音。首选是一个字对一个完整音节、覆盖整个组字、至少两个字的句子时，MSIME_CONVERSION_LEFT 进入改字：View.conversion 是整句，光标在 View.conversion_focus_start 个字（Unicode 标量）之前，conversion_focus_start..conversion_focus_end 是候选所替换的那一段，候选换成这一段（先是光标所在的词，再是更短、更长的）的字和词。之后左右键每次移一个字，最远到句末（focus_start 等于字数，没有候选）。选中候选（数字、MSIME_COMMIT_CANDIDATE、msime_client_select）把这段钉住，句子其余部分围绕它重新转换，光标移到这段后面，不上屏；光标在句末时 MSIME_COMMIT_CANDIDATE 上屏整句。MSIME_COMMIT_RAW、MSIME_FINISH_COMPOSITION、标点和失去焦点上屏改好的整句，不是拼音字母。MSIME_CANCEL 和 MSIME_BACKSPACE 只退出改字回到拼音（组字保留，钉住的段丢掉）；键入字母或撇号、Shift/Ctrl+左右、Home/End 先退出改字再照常处理。进不了改字时（读音不完整、单字、光标在拼音中间、其他方案、本地模式）这两条命令就是 MSIME_MOVE_LEFT / MSIME_MOVE_RIGHT。改字期间宿主把 View.phrase_prefix 加 View.conversion 画在行内代替拼音，不管预编辑样式选的是什么；改字的候选不能置顶、删除或固定位置。 */
/* 移动光标后编辑光标后的部分（偏好 caret_edit_suffix，默认关，只在三个桌面宿主上生效）：全拼、双拼的组字光标停在拼音中间（MSIME_MOVE_LEFT 等字母光标命令、分段命令）时，光标前那段原文固定为它当前的最佳转换，候选换成光标后那段的候选。此时 View.caret_edit_suffix 为 true，View.caret_edit_prefix 是固定的那段文字，首选（id.index 为 0）的 text 已经以它开头，是整句；其余候选是光标后那段开头的词。宿主在候选窗右侧显示「候选编辑：光标后」，能分段画候选的把首选开头 caret_edit_prefix 的那几个字（按 Unicode 标量数）画淡。选中一条只消耗它自己的音节：固定的那段和这一条进 View.phrase_prefix 暂存（需要 phrase_preedit），剩下的拼写接着在本模式里转换，整段完成时一起上屏。光标在开头或末尾、其他方案、本地模式和专用英文里两项都不出现，行为与关着时相同。模式里不混入英文、emoji、符号、日文、快捷短语等行，也不发云候选和 AI 查询。 */
enum MsimeCommand {
    MSIME_BACKSPACE = 0, MSIME_COMMIT_CANDIDATE = 1, MSIME_COMMIT_RAW = 2,
    MSIME_CANCEL = 3, MSIME_MOVE_LEFT = 4, MSIME_MOVE_RIGHT = 5,
    MSIME_MOVE_HOME = 6, MSIME_MOVE_END = 7, MSIME_DELETE_FORWARD = 8,
    MSIME_FINISH_COMPOSITION = 9,
    MSIME_CYCLE_KANA_VARIANT = 10, MSIME_COMMIT_READING = 11,
    MSIME_BACKSPACE_SEGMENT = 12, MSIME_MOVE_LEFT_SEGMENT = 13, MSIME_MOVE_RIGHT_SEGMENT = 14,
    MSIME_COMMIT_RAW_WITHOUT_LEARNING = 15,
    MSIME_CONVERT_HANJA = 16,
    MSIME_OPEN_CANDIDATE_LIST = 16,
    MSIME_CONVERSION_LEFT = 17, MSIME_CONVERSION_RIGHT = 18,
    /* 宿主已经自己把文本写进了文档，再丢弃组字：对会话的作用同 MSIME_CANCEL（一次，组字没结束时照旧再发一次），区别只在生命周期里结束原因记 HostCommit、不出取消音。 */
    MSIME_CANCEL_AFTER_HOST_COMMIT = 19,
    /* 焦点即将离开或会话即将关闭，宿主替用户结束组字（macOS IMK 的 commitComposition:，宿主为词库维护关掉会话之前）：对会话的作用同 MSIME_FINISH_COMPOSITION（只有一处不同：高亮的是脚本插件在 / 指令里给的行，即来源 14 时，上屏用户打的原文，例如 /uuid，因为这时用户没在看候选窗），生命周期里上屏来源记 FocusLoss，同 msime_client_focus(false) 的上屏：不可改写、不出提交音、不画上屏特效，回复里 feedback 的 typing_effect 为 0。用户按键或点选引起的结束组字仍发 MSIME_FINISH_COMPOSITION。 */
    MSIME_FINISH_ON_FOCUS_LOSS = 20,
    MSIME_NEXT_PAGE = 100, MSIME_PREVIOUS_PAGE = 101,
    MSIME_NEXT_CANDIDATE = 102, MSIME_PREVIOUS_CANDIDATE = 103,
    MSIME_FIRST_CANDIDATE = 104, MSIME_LAST_CANDIDATE = 105,
    /* 候选窗显示时的 Home 键：高亮不在首选时同 MSIME_FIRST_CANDIDATE；已经在首选、输入串非空、没有打开的候选列表（韩文汉字、注音）且不在整句改字时同 MSIME_MOVE_HOME；整句改字时一直同 MSIME_FIRST_CANDIDATE，不退出改字。宿主只在 Home 键上发它；需要「无条件回到首选」的地方（日文转换循环候选）仍发 MSIME_FIRST_CANDIDATE。 */
    MSIME_FIRST_CANDIDATE_OR_HOME = 106
};
char *msime_client_command(uint64_t session, uint32_t command);
/* Re-rank the visible candidates with the settled model, once the host's typing pause elapses.
 * The host owns the clock: it is the only side that knows whether a keystroke arrived while the
 * pass was being decided. Answers {"moved": false} when the order did not change, or
 * {"moved": true, "view": ...} after a reorder. The false case lets the host leave the candidate
 * window alone without serializing a view it will discard. Inert, and immediately false, when no
 * settled model is installed. */
char *msime_client_rerank_settled(uint64_t session);
/* 宿主等待输入停顿后，在后台启动模型评分。
 * 无需改动或只做呈现整理时返回 {"status":"done","moved":false,"current":true}；
 * 呈现整理改变顺序时返回 {"status":"done","moved":true,"current":true,"view":...}。
 * 后台评分返回 {"status":"pending","token":N}，经 msime_client_poll_settled_rerank 轮询。
 * 每会话只运行一个任务，重复启动返回相同令牌；评分期间可以继续处理按键。 */
char *msime_client_start_settled_rerank(uint64_t session);
/* token 来自启动接口；评分未完成时返回 {"status":"pending","token":N}，
 * 完成后返回 {"status":"done","moved":...,"current":...}。
 * current 为真表示当前组字及配置已评分；按键、开关或模型改变，或 accept=false 时为假。
 * 未知令牌返回 {"status":"done","moved":false,"current":false}。
 * 宿主已离开原组字时传 accept=false，只归还仍适用的模型。必须在会话线程调用。 */
char *msime_client_poll_settled_rerank(uint64_t session, uint64_t token, bool accept);
/* Pass the generation and global index from the displayed candidate's id. */
char *msime_client_select(uint64_t session, uint64_t generation, size_t index);
/* Select any entry copied by msime_client_all_candidates for this exact
 * generation. Normal select remains restricted to the current View page. */
char *msime_client_select_any_candidate(uint64_t session, uint64_t generation, size_t index);
char *msime_client_pin_candidate(uint64_t session, uint64_t generation, size_t index);
char *msime_client_remove_candidate(uint64_t session, uint64_t generation, size_t index);
/* Fix a dictionary candidate to slot 1..5 for the current input context. */
char *msime_client_fix_candidate_position(uint64_t session, uint64_t generation, size_t index,
                                          uint8_t position);
/* Clear a previously fixed dictionary candidate position. */
char *msime_client_clear_candidate_position(uint64_t session, uint64_t generation, size_t index);
/* 从 View.nine_key_spellings 里选一项，generation 拒绝过期的界面。全拼下把这个拼写锁进数字；注音下只钉住目标音节的读音（不上屏），nine_key_spellings 随后换成下一个有歧义的音节。 */
char *msime_client_choose_nine_key_spelling(uint64_t session, uint64_t generation, size_t index);
/* 全拼九键组字时筛选候选，作用到这次组字结束：single_character 只留单字；strokes 是 length 字节的笔顺前缀（h 横、s 竖、p 撇、n 点、z 折，<=64），只留首字笔顺以它开头的候选，length 为 0 时不按笔画筛选（strokes 可为 NULL）。当前状态见 View.nine_key_single_character 和 View.nine_key_strokes。没有九键组字或 strokes 含其他字节时 handled=false；笔画字典不可用时 handled=true 并带 diagnostic LANGUAGE_DICTIONARY_UNAVAILABLE，筛选不变。
 * View.nine_key_spellings 在全拼九键下的含义：先是完整音节；再是下一个数字键上能起头一个音节的字母，大写（M N O），选它限定下一个音节的首字母；最后是这个数字本身（6），只在没有锁定的音节时出现，选它直接上屏这个数字。数字全部锁定后它是最后一次锁定时的选项，选其中一项就换掉那次锁定。这时退格先撤销最后一次锁定，其余情况退格删数字。
 * 14 键下含义相同，只是没有最后那一项「原样上屏」：完整音节，再是下一键上能起头一个音节的大写字母（Q W）。preedit 是组码字母（bugao），不是拼音，读音行用 View.nine_key_reading。 */
char *msime_client_set_nine_key_filter(uint64_t session, bool single_character,
                                       const uint8_t *strokes, size_t length);
/* 滑行输入：手指一笔滑过字母键，由引擎解码成最可能拼出的全拼字母，像打字一样写到组字的光标处（与已有字母之间用 ' 隔开）。request 是 length 字节的 UTF-8 JSON（<=65536，拒绝未知键）：{"keys":[[x,y] x 26],"key_width":w,"key_height":h,"points":[[x,y] 或 [x,y,ms]，2..1024 个]}，keys 是 a..z 各键中心（按此次序），key_width/key_height 是一个字母键的尺寸，全部在宿主自选的同一个坐标系里；ms 是距笔画开始的毫秒数，有了它，手指在键上停一下就能确认那个键。返回与 msime_client_character 相同的输入响应。方案不是全拼、在本地模式或专用英文里、九键数字正在组字，或者没有音节跟得上这一笔时 handled=false：宿主丢掉这一笔，不得把它经过的键当作按键输入。在会话线程上、手指抬起时调用；一次触摸是滑行还是点按只由宿主判断。 */
char *msime_client_glide(uint64_t session, const uint8_t *request, size_t length);
enum MsimeCandidateEdge { MSIME_FIRST_HAN = 0, MSIME_LAST_HAN = 1 };
/* Engine selects one Han character and clears composition on success.
 * A candidate without Han text is unhandled and keeps composition; no fallback
 * punctuation or candidate commit is manufactured. Uses the same current-page
 * identity checks as select. Invalid edge values fail before state changes.
 */
char *msime_client_select_edge(uint64_t session, uint64_t generation, size_t index, uint8_t edge);
/* On-demand {session,generation,preedit,candidates:[Candidate...]}. Unlike View,
 * candidates contains the complete cached Engine generation with global IDs. */
char *msime_client_all_candidates(uint64_t session);
/* {session,generation,preedit,reading,total,candidates:[Candidate...]}：座位 start 起最多 count 行，到 total（这一代已放出的候选数）为止，身份与 msime_client_all_candidates 同一座位的那一行相同，可交给 msime_client_select_any_candidate。每键重画的横向候选条只取它这次建得出来的一批，不必整代复制。引擎暂扣的尾部（首字母只先给 24 行）只在 start 到了 total 时才放出；放出来会重排座位并换一代，宿主看到 generation 变了要按新的一代从头取。同一代里 candidates 为空表示已经没有更多。 */
char *msime_client_candidate_range(uint64_t session, size_t start, size_t count);
/* 宿主按自己的排版显示了与固定分页不同的一组候选（macOS 横排按宽度自适应）时，报出这一代里这些候选的全局序号
 * （来自 all_candidates 的 id.index，<=256 个）。二三候选键、五笔自定义选词键和翻译查询都按看得见的这一组；
 * count 为 0 时清掉。只对 generation 这一代有效，组字推进后自动失效。
 * Value {"applied": bool}：代次过期或序号越界时为 false，原有显示页不变。 */
char *msime_client_set_display_page(uint64_t session, uint64_t generation, const size_t *indices, size_t count);
/* Return read-only English completions for a bounded ASCII prefix. */
char *msime_client_english_completions(uint64_t session, const uint8_t *prefix,
                                       size_t prefix_length, size_t limit);
/* View.local_mode 是 Engine 自己的模式，不是从预编辑前缀猜出来的；View.microsoft_shuangpin 报告 Engine 已应用的配置，不是尚未生效的新偏好；它为真表示当前是双拼且韵母或零声母编码用 `;` 作第二键，宿主要把 `;` 当字母键交给 msime_client_character。宿主结合模式、编辑文本和光标使用它。取值为 none、unicode、date_time、quick_phrase、emoji、kaomoji、super_jianpin、temporary_english、temporary_japanese、expression、command、mention、url、chaizi（拆字反查，Shift+C 进入；候选注释是字的带调读音，由 Engine 给出）、symbol（符号模式，Shift+S 进入；输入分组码 xl / sx / szq / jt 或符号名 alpha、infinity），未知取值视为不可用状态。
 * View.spelling_symbols 列出 Engine 在当前状态下当作输入的非字母键：当前模式的拼写（expression 的数字和运算符、unicode 的数字、url 的数字和网址符号）；没有组字时打开模式的键（`/` 和 `@`）；全拼、双拼、五笔组字中原文恰好是 www、http、https、ftp 时打开网址模式的键（`.` 或 `:`）。宿主要把它们当字符发送；列在里面的数字是输入，不是选候选的快捷键。转换结果的 commit_context.typing_statistics 对 expression、command、mention 模式生成的文本为 false，这些文本不计入打字统计。
 * 中文标点下 '\'' 与 '"' 出哪种引号、'[' ']' 出哪种方头括号由偏好 quote_style、bracket_style 决定（契约 shared/contracts/punctuation 的 styles 段）。View.punctuation_marks 报告引擎已应用的样式：{"quotes": "curly"|"corner", "brackets": "solid"|"hollow", "enumeration_comma_backslash": bool, "enumeration_comma_slash": bool}，corner_when_traditional 已按繁体输出开关解析成 curly 或 corner；两个顿号字段报告契约 enumerationComma 段里 '\\' 和 '/' 哪个键出顿号（至少一个为真）。宿主做成对补全、跨过后半个、空格转换、英文模式下的中文标点和画触屏键面时读它，不自己解析偏好；改了偏好要等空闲重建后它才变，和引擎出字同步。
 */
char *msime_client_view(uint64_t session);
/* 在线候选查询包含 cloud_request（GET/POST、请求头、JSON 请求体、超时、响应格式和可选缓存回包）。
 * 描述符可能含自定义服务凭据，只交给宿主传输，不写日志或上传诊断。
 * 各接收 OnlineQuery 的接口最多接受 128 KiB UTF-8 JSON；候选列表与回包沿用各自上限。
 * 云与 AI 共用这份查询，宿主在后台执行后原样交回，组字或来源变化会拒绝迟到结果。 */
#define MSIME_ONLINE_QUERY_MAX_BYTES 131072
char *msime_client_online_query(uint64_t session);
/* Build a validated AI HTTP descriptor for a copied OnlineQuery. The host
 * resolves credentials from its current preferences; the query must still
 * match the active AI configuration. */
char *msime_client_ai_request_for_query(uint64_t session,
                                        const uint8_t *query,
                                        size_t query_length);
/* 把偏好之外（例如 iOS 钥匙串）的 AI 凭据交给会话；仅覆盖当前端点的令牌，
 * 不持久化或回传，也不会随偏好切到别的端点。长度为零表示撤销。
 * 凭据确实变化时，当前组字的旧 AI 候选和旧在线查询失效；重复设置同一凭据不变。 */
char *msime_client_set_ai_credential(uint64_t session, const uint8_t *token,
                                     size_t token_length);
/* Return null or {generation,target_language,candidates:[{text}], provider:"none"|"account"|"tencent"|"niutrans"|"custom", translation_account:bool, custom_translation:{enabled,endpoint,api_key}|null, tencent_tmt:{enabled,secret_id,secret_key,region}|null, niutrans:{enabled,app_id,apikey}|null} for visible candidates.
 * provider 是偏好里选中的服务，即使它的配置还不完整；传输层只能问这个服务或者谁都不问，不能退回到别的服务。凭据只为选中且可用的服务返回。这些字段供宿主自己的传输层使用；不要记录查询内容。已有的 target_language 对两种服务都适用。translation_account 只在用户明确选了水杉账号、candidate_translations 打开、而且没有用户自己的服务适用时为 true；宿主的账号释义路径完全由它决定，为 false 时什么也不能发。offline_gloss_languages 只在非空时出现：按偏好顺序列出离线词典已装在资源旁或已下载到 offline-glosses 资源包里的非英文目标语言；对每一种都用那个 target_language 调 msime_client_candidate_gloss_request。state_root 只在其中某种来自资源包时出现；在那个请求里原样传下去。candidate_pronunciation 只在用户打开读音时出现（为 true）：这时宿主把它画出的释义行连同各自的目标语言交给 msime_client_pronunciation_request 去读（有系统日文分词器的宿主可以自己把日文行转成罗马字），读音只作显示，从不上屏。gloss_breakdown 只在候选词翻译或离线英文释义打开、而且英语是目标语言之一时出现（为 true），与 macOS 请求逐词拆解的条件相同：这时宿主带着 resources 为整句候选调 msime_client_gloss_breakdown_request，只要 gloss_breakdown 出现，查询里就一定带着 resources。
 * While the composition is a /fy request (command mode) the query is that request alone, whatever the gloss switches say: sentence:true, target_language "zh", one candidate {text, online_gloss:false} holding the English typed after the command, no resources, user_data or offline_gloss_languages, and null when no service of the user's own (tencent, niutrans or custom) is selected. Ask only that service, without the offline dictionary or the gloss cache, and hand the answer back through msime_client_apply_translations, which shows it as the first row and commits the translation.
 * 候选词翻译范围（candidate_translation_scope）是 "highlighted" 且候选词翻译开着时，candidates 只有当前高亮的那一个候选（高亮不在这一页时没有这一份），翻译服务和只为候选词翻译查的离线词典都只为它工作。离线释义开关（candidate_english_gloss）要查释义时，查询另带 gloss_query：一份完整的查询文档，覆盖这一页全部候选，provider 为 "none"、translation_account 为 false、各服务配置为 null，宿主用处理普通查询的同一条路径算它（不会联网），把它的每条回答标上 "gloss":true 交给 msime_client_apply_translations（同一代次，可以和主查询的回答拼成一个数组；分开交时不要交空数组）。高亮不在这一页、只剩离线释义要查时，返回的就是那份不带服务的文档本身。范围是 "all" 时没有 gloss_query，查询与以前相同。
 */
char *msime_client_translation_query(uint64_t session);
/* Google 的默认预算；cloud_request 存在时按描述符中的毫秒预算执行。 */
#define MSIME_CLOUD_CONNECT_TIMEOUT_MS 2000
#define MSIME_CLOUD_REQUEST_TIMEOUT_MS 2000
/* 兼容 GET 云候选的 URL 入口；POST 宿主直接执行 OnlineQuery.cloud_request。 */
char *msime_client_cloud_request_url(const uint8_t *query, size_t query_length);
/* 共享解析器处理宿主取得的云响应，返回 {applied,view}；空或无效响应不改变组字。
 * 查询最多 128 KiB，响应最多 256 KiB。过期、关闭或切换来源的查询不应用候选。 */
char *msime_client_apply_cloud_response(uint64_t session,
                                      const uint8_t *query, size_t query_length,
                                      const uint8_t *body, size_t body_length);
/* The #if !defined(_WIN32) blocks below mirror #[cfg(unix)] exports, so a
 * Windows host fails at compile time instead of with unresolved externals. */
#if !defined(_WIN32)
/* Linux: perform one bounded request to a user-owned Unix-socket provider.
 * Call from a worker thread with a copied query; returns null value when no
 * candidate is available. Credentials and network policy stay in that service. */
char *msime_client_online_provider_request(const uint8_t *query,
                                           size_t query_length,
                                           const uint8_t *socket_path,
                                           size_t socket_length);
/* Linux: forward one validated account-backed dictionary operation to a
 * user-owned Unix-socket provider. The provider owns credentials and sync. */
char *msime_client_cloud_dictionary_provider_request(const uint8_t *request,
                                                     size_t request_length,
                                                     const uint8_t *socket_path,
                                                     size_t socket_length);
/* Linux: forward one validated cloud clipboard operation to a user-owned
 * Unix-socket provider. The provider owns credentials and retention policy. */
char *msime_client_cloud_clipboard_provider_request(const uint8_t *request,
                                                    size_t request_length,
                                                    const uint8_t *socket_path,
                                                    size_t socket_length);
#endif
/* 云剪贴板端到端加密的 sans-IO 驱动，所有平台都有。输入 {start,keystore,device_name} 开始一个操作，或 {resume,response} 交回上一步请求的响应；value 是 {request,state,keystore}、{done,keystore} 或 {error,keystore}。宿主发请求并保存 keystore，驱动不访问会话、不联网。协议见 docs/cloud-clipboard-e2e.md。请求最大 2 MiB。 */
char *msime_client_cloud_clipboard_e2e(const uint8_t *request, size_t request_length);
/* Persist {target_language,translations:[{text,translation}]} in an existing
 * absolute user-data directory. Only short changed English-target glosses are
 * saved. Candidate gloss requests may include user_data to read this overlay.
 * Maximum request size 128 KiB; path 4096 bytes. Does not access a session. */
char *msime_client_translation_gloss_save(const uint8_t *request, size_t request_length,
                                         const uint8_t *user_data, size_t user_data_length);
/* TranslationQuery accepts the optional sentence:true flag for an explicit
 * single-item sentence request (up to 512 Unicode characters); ordinary
 * candidate gloss requests retain their normal limits. */
#if defined(_WIN32)
/* Windows 原生手写面板，阻塞调用，只能放在工作线程，不依赖 WebView。
 * 请求为 <=262144 字节的 JSON；{operation:"status"} 返回 {system_recognizer:bool}。
 * {operation:"recognize",request:{language?:"zh-CN",strokes:[{points:[{x,y},...]}]},model?:绝对路径}
 * 使用 420×420 DIP 坐标；系统识别优先，失败或无结果时可用可信模型回退。
 * 返回标准封装 {candidates:[...]}；错误码 invalid_stroke、no_chinese_recognizer、
 * handwriting_unavailable、invalid_handwriting_model；本安装包不提供手写时为 handwriting_not_offered。
 * 不访问输入会话、不联网、不保留请求；调用方用 msime_client_string_free 释放响应。 */
char *msime_client_windows_handwriting(const uint8_t *request, size_t length);
#endif
#if !defined(_WIN32)
char *msime_client_translation_provider_request(const uint8_t *query,
                                                size_t query_length,
                                                const uint8_t *socket_path,
                                                size_t socket_length);
/* Linux handwriting panel adapter. The query is a bounded JSON object with
 * language and normalized stroke arrays; the user-owned socket returns
 * {candidates:[...]} and owns recognizer/model policy. */
char *msime_client_handwriting_provider_request(const uint8_t *query,
                                                size_t query_length,
                                                const uint8_t *socket_path,
                                                size_t socket_length);
/* Run the optional offline Engine recognizer against a trusted packaged model.
 * The model path must be absolute; response is {candidates:[...]} or an error. */
char *msime_client_handwriting_local_request(const uint8_t *query,
                                             size_t query_length,
                                             const uint8_t *model_path,
                                             size_t model_length);
/* Linux standalone emoji panel adapter. The query contains search/category
 * text and a bounded result limit; the socket returns {items:[...]}. */
char *msime_client_emoji_provider_request(const uint8_t *query,
                                          size_t query_length,
                                          const uint8_t *socket_path,
                                          size_t socket_length);
#endif
/* 从已校验的 msime-others.db 读取本地表情目录；resources 为含该文件的绝对目录。
 * 无需提供商套接字，Windows 和 Unix 宿主使用同一接口。
 * offset 为非负行偏移（缺省 0），limit 为 1..255。
 * group 筛选目录子类；list_groups:true 返回 {groups:[name,...]}。
 * list_symbol_groups:true 返回 {symbol_groups:[{parent,title},...]}。
 * parent 在分页前筛选符号父类，避免不同父类下的同名子类混淆。
 * list_plugin_symbol_groups:true 搭配绝对 plugins 目录，返回
 * {plugin_symbol_groups:[{pack,pack_name,tab,title,keywords,items:[text,...]},...]}。
 * 插件组独立于内置数据库；包按名称、组按清单顺序，不与内置目录去重。
 * cursor:true 返回 next_offset 和 complete；必须按 next_offset 推进，
 * 空页可能只是坏行。缺省模式按 limit 推进，每页按文本去重。 */
char *msime_client_emoji_catalog_request(const uint8_t *query,
                                         size_t query_length,
                                         const uint8_t *resources,
                                         size_t resources_length);
/* Shared Doubao authentication policy. Input (max 32768 bytes):
 * {auth_mode,app_id,token,resource_id}; an absent or empty mode means api_key.
 * Response value: {headers:[[name,value],...]}. Contains credentials: never
 * log/persist the response; release with msime_client_string_free. */
char *msime_client_doubao_auth_headers(const uint8_t *request, size_t length);
#if !defined(_WIN32)
/* Linux voice adapter. The user-owned socket captures audio and runs ASR,
 * returning {text}; the query contains language and the active generation. */
char *msime_client_voice_provider_request(const uint8_t *query,
                                          size_t query_length,
                                          const uint8_t *socket_path,
                                          size_t socket_length);
#endif
/* Decode one Doubao v1 response frame. The value contains either {last,payload}
 * for a UTF-8 JSON response or {error_code} for a type-0xF error frame. */
char *msime_client_doubao_decode_frame(const uint8_t *frame,
                                       size_t frame_length);
/* Build caller-owned Doubao v1 start and PCM/final frames. If capacity is too
 * small, output_length receives the required size and no bytes are written. */
bool msime_client_doubao_start_frame(bool enable_itn, bool enable_punc,
                                     bool enable_ddc,
                                     const uint8_t *boosting_table_id,
                                     size_t boosting_table_id_length,
                                     uint8_t *output, size_t output_capacity,
                                     size_t *output_length);
bool msime_client_doubao_audio_frame(int32_t sequence, const uint8_t *pcm,
                                     size_t pcm_length, bool final_chunk,
                                     uint8_t *output, size_t output_capacity,
                                     size_t *output_length);
#if !defined(_WIN32)
typedef void (*msime_client_voice_update_callback)(const uint8_t *text,
                                                   size_t text_length,
                                                   bool final,
                                                   void *context);
/* Stream newline-delimited provider updates. The callback runs on the caller
 * thread and receives bounded interim/final UTF-8 text; context is untouched. */
char *msime_client_voice_provider_stream(
    const uint8_t *query, size_t query_length, const uint8_t *socket_path,
    size_t socket_length, msime_client_voice_update_callback callback,
    void *context);
/* Optional phase notifications: 0=recording, 1=recognizing, 2=polishing.
 * Both callbacks run synchronously on the caller thread and must not throw. */
typedef void (*msime_client_voice_status_callback)(uint8_t phase, void *context);
char *msime_client_voice_provider_stream_events(
    const uint8_t *query, size_t query_length, const uint8_t *socket_path,
    size_t socket_length, msime_client_voice_update_callback callback,
    msime_client_voice_status_callback status_callback, void *context);
/* Normalized microphone level in [0, 1]; never transcript text or audio.
 * Callback runs synchronously on the caller thread and must not throw.
 * All three stream calls return {"ok":true,"value":{"text":...}} on success and value null when the provider gave no result. A provider that names a missing optional dependency returns {"ok":false,"error":"voice_dependency_missing:websockets"}, "voice_dependency_missing:recorder" or "voice_dependency_missing:local_asr". */
typedef void (*msime_client_voice_level_callback)(float level, void *context);
/* Return true to stop a streaming provider read. The callback runs on the
 * provider worker at least every 100ms and must not throw. */
typedef bool (*msime_client_voice_cancellation_callback)(void *context);
char *msime_client_voice_provider_stream_feedback(
    const uint8_t *query, size_t query_length, const uint8_t *socket_path,
    size_t socket_length, msime_client_voice_update_callback callback,
    msime_client_voice_status_callback status_callback,
    msime_client_voice_level_callback level_callback, void *context);
/* Cancellable variant used by detached hosts. The legacy feedback entry point
 * remains equivalent to passing a null cancellation callback. */
char *msime_client_voice_provider_stream_feedback_cancelled(
    const uint8_t *query, size_t query_length, const uint8_t *socket_path,
    size_t socket_length, msime_client_voice_update_callback callback,
    msime_client_voice_status_callback status_callback,
    msime_client_voice_level_callback level_callback,
    msime_client_voice_cancellation_callback cancellation_callback,
    void *context);
/* Request cancellation of a provider capture session by generation. */
char *msime_client_voice_provider_cancel(const uint8_t *socket_path,
                                         size_t socket_length,
                                         uint64_t generation);
/* Ask the provider to finish capture and deliver the final stream result. */
char *msime_client_voice_provider_stop(const uint8_t *socket_path,
                                       size_t socket_length,
                                       uint64_t generation);
#endif
/* Apply a UTF-8 cloud (source=0) or AI (source=1) result for a copied query. */
char *msime_client_apply_online_candidate(uint64_t session,
                                           const uint8_t *query,
                                           size_t query_length,
                                           const uint8_t *candidate,
                                           size_t candidate_length,
                                           uint8_t source);
/* Apply a JSON array of UTF-8 strings for one source (cloud=0, AI=1).
 * Buffers are borrowed for the call; at most 16384 bytes each. */
char *msime_client_apply_online_candidates(uint64_t session,
                                          const uint8_t *query,
                                          size_t query_length,
                                          const uint8_t *candidates,
                                          size_t candidates_length,
                                          uint8_t source);
/* Remove cached and visible rows for one source (source=0 cloud, 1 AI). */
char *msime_client_clear_online_candidates(uint64_t session, uint8_t source);
/* AI 回答为空、失败，或 apply_online_candidates 返回 applied:false 时宿主会调用它（source 只认 0 和 1，其余报错）。共享内核不再放 AI 占位行（候选 source 12），AI 回答到达时第一条直接插在第一页最后一格；这个入口为已发布的宿主留着，合法的调用一律返回 {applied:false, view}，不改候选、不换代。 */
char *msime_client_resolve_online_candidates(uint64_t session,
                                             const uint8_t *query,
                                             size_t query_length,
                                             uint8_t source);
/* Resolve a shared surface route ("settings", "settings:voice", "emoji", ...)
 * so a host launches the shared shell by name. Returns the canonical route and,
 * for panel surfaces, the window label, query and geometry. */
char *msime_client_parse_surface_route(const uint8_t *value, size_t length);
/* Convert UTF-8 Simplified Chinese to Traditional Chinese with the shared
 * OpenCC s2t tables (phrase-level, so 头发 -> 頭髮 and 发展 -> 發展). Unlike
 * the JSON calls, returns the converted NUL-terminated text directly because
 * hosts call it per candidate. Returns NULL for NULL, inputs over 1 MiB,
 * invalid UTF-8 or an embedded NUL; keep the original text then. Free with
 * string_free. The first call parses the tables (a few milliseconds in
 * release builds). */
char *msime_client_simplified_to_traditional(const uint8_t *text, size_t length);
/* 同上，但按 variant 选 OpenCC 配置：0 s2t、1 s2tw（臺灣字形）、2 s2twp
 * （臺灣字形加惯用词），与偏好 traditional_chinese_variant 的 "s2t"、"s2tw"、
 * "s2twp" 一一对应。未知的 variant 与其它非法输入一样返回 NULL。臺灣配置的
 * 附加词表在第一次用到时解析。Free with string_free. */
char *msime_client_convert_to_traditional(const uint8_t *text, size_t length,
                                          uint32_t variant);
/* Display-only font aliases. Input: JSON array, at most 33 names / 32 KiB.
 * Returns the standard response with an array value. Free with string_free.
 * Resolution failure retains the corresponding original family name. */
char *msime_client_resolve_font_families(const uint8_t *value, size_t length);
/* Installed font family names, the catalog the shared app's 刷新字体列表 reads.
 * Returns the standard response with a sorted array of distinct names, each
 * non-empty, at most 128 UTF-8 bytes and free of control characters; error
 * "unsupported" on a host without a catalog, "font_catalog" or
 * "font_catalog_limit" (over 16384 families) when enumeration fails.
 * Enumerates the system's fonts: call it off the UI thread. Free with
 * string_free. */
char *msime_client_list_font_families(void);
/* Describe what the named host ("windows"/"macos"/"linux"/"android"/"ios") can
 * do, so the shared UI renders from capabilities rather than the user agent. */
char *msime_client_host_capabilities(const uint8_t *platform, size_t length);
/* 调用方可执行文件旁的 msime-mcp，供桌面外壳以外的设置宿主使用。JSON 请求（<=65536 字节）{options:运行时选项的绝对路径|null}。返回 {command,installed,options,config,clients:[{id,path,configured,flags}]}，flags 是已写入条目带的权限参数。会读取助手的配置文件，请在工作线程调用。 */
char *msime_client_mcp_status(const uint8_t *request, size_t length);
/* 把 msime 条目写进一个助手的配置，保留其它所有键。JSON 请求 {options,client:"claude_desktop"|"cursor",flags?:["--allow-write"|"--allow-dictionary-read"],replace:bool}，flags 省略时写只读条目。返回 "added"|"updated"|"replaced"|"unchanged"；已有条目只差权限参数时直接更新，其它不同的 msime 条目在未设 replace 时以 mcp_entry_exists 失败。会写文件，请在工作线程调用。 */
char *msime_client_mcp_install(const uint8_t *request, size_t length);
/* Where this library reports a failure it recovers from by itself, such as a sound or music pack that does not load, an audio device that does not open, or a helpcode pack the Engine falls back from. Without a sink each report is one line on stderr prefixed "msime: "; with one, sink receives the same line without the prefix, as NUL-terminated UTF-8 valid only during the call. Each line is "<category>: <detail>": the category is a fixed English phrase saying what failed, such as "sound pack not loaded", and holds no colon, so the text before the first colon names no pack, file or cause; the detail names packs, files and error causes. Neither holds typed text, candidates or credentials; a host whose log must not keep the detail keeps only the category. Pass NULL to go back to stderr; a later call replaces the sink. A report already under way on another thread may still call the sink it read after the call that replaced or cleared it has returned, so every sink ever registered must stay callable for the rest of the process: do not unload or free its code. sink may be called on any thread, including the audio player's own and several at once, and from inside other msime_client calls: it must not block, must not call any msime_client function, and must return normally. Register it once at startup, before the first session. */
typedef void (*msime_client_diagnostic_sink)(const char *line);
void msime_client_set_diagnostic_sink(msime_client_diagnostic_sink sink);
/* Effect sounds and background music, played by this library on macOS, Windows and Linux from the session's preferences.plugins. The three calls below are for the key path: they return whether a request was queued, never block, decode or read files, and need no free. False means nothing is switched on, the session handle is unknown or on another thread, the queue is full, this platform does not play (iOS, Android, HarmonyOS), or sound failed earlier in this process, which turns it off until the process restarts with one line through msime_client_set_diagnostic_sink (stderr without one). Nothing starts - no thread, no audio device - until a call finds something switched on, so a process that never calls them (the Windows TSF DLL) pays nothing; the device is let go again after 30 s without a sound or playing music. Do not call them for keys typed into a secure (password) field.
 * key_sound: key_class 0 any other key, 1 space, 2 enter, 3 backspace; anything else queues nothing. Plays the key pack's sample for the class, or the melody pack's next note in melody mode.
 * commit_sound: call when a transition commits text. Plays the key pack's commit sample when the commit sound is on, and the next note of a melody that advances on commits.
 * music_set_active: true while the input method is active in a field that is not a secure one, false when it deactivates or a secure field gains focus; music plays only in between.
 * Achievement sounds need no call: msime_client_typing_statistics record plays one when the count passes a milestone. */
bool msime_client_key_sound(uint64_t session, uint32_t key_class);
bool msime_client_commit_sound(uint64_t session);
bool msime_client_music_set_active(uint64_t session, bool active);
/* 生命周期核心（桌面宿主选择加入；移动端宿主不调用，行为和 transition JSON 都不变）。以下调用都在会话所在线程上进行，不落盘，不分配要释放的内存；未知会话、错线程、重入调用或内部 panic 时返回 0 或 false。
 * msime_client_set_lifecycle：让会话选择加入，或换一组位；返回实际启用的位，其余位被忽略。没调用过它的会话不推导任何事件。
 *   MSIME_LIFECYCLE_FEEDBACK：按键音、上屏音、打字特效和背景音乐改由核心驱动。按键音和按键特效来自 msime_client_key；上屏音和上屏特效（typing_effect 事件 4）来自核心的上屏事件，派发类调用的回复里多一个 "feedback":{"typing_effect":N} 键，N 的位布局同 msime_client_typing_effect，没有上屏时为 0；音乐由 msime_client_set_active 仲裁所有者。本会话的 msime_client_key_sound、msime_client_commit_sound、msime_client_typing_effect（事件 0-4）和 msime_client_music_set_active 随之变为空操作（返回 false 或 0）。typing_effect 事件 5（绕过按键音的删除）不经过 msime_client_key，核心看不到，仍由宿主调用。失焦上屏（msime_client_focus(false) 和 MSIME_FINISH_ON_FOCUS_LOSS）不出声也不画。字段属性带 SECURE 时没有任何声音和特效，音乐关闭；带 FULLSCREEN 时声音静音、特效照画（同 typing_effect 的位 9）。
 *   MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY：之后的上屏只能观察、不能改写，可随时切换。TIP 已经从自己的会话把文字写进文档的路径（Windows），Server 在派发前设上、派发后清掉。设着它时脚本插件的候选变换也停下：派发里不运行 candidates，旁路调用也不重放之前的重排、隐藏和注释，列表保持引擎和内置排序的顺序。TIP 在自己的会话里选词的列表（注音等 AlwaysInlinePreedit 方案）按那个会话的顺序选，Server 一侧重排或隐藏就会让显示的顺序和数字选中的对不上，所以这类列表的每一次派发都要设着它。
 *   MSIME_LIFECYCLE_HOST_HOLDS_PIECES：宿主自己暂存部分选词（Windows），补齐整个词时才把各段拼好写进文档。这样的会话里每次部分选词都是一次上屏，commit 事件按段投递，插件读到的 commit 只是这一段；这个词的各段，包括补齐它的那一段，before_commit 都不可改写。部分选词上屏的那一段还在宿主手里、没进文档，FEEDBACK 不为它出上屏音、不画上屏特效（transition 的 feedback 为 0），整段送出的那次上屏再出；生命周期事件照常发。
 * 顺序契约。轮次：宿主每把一个物理按下交给输入法，都要在为这个键调用任何派发之前先调一次 msime_client_key，它开启新的一轮；到下一次 msime_client_key 之前的所有派发和 focus 调用都属于这一轮，同一个键可能触发多次派发（如 Windows 对同一个键再发一次 MSIME_CANCEL）。指针点选和翻页先调 msime_client_key(session, MSIME_KEY_POINTER)。带 MSIME_KEY_PASSTHROUGH 的键（Windows 交给应用、由 TIP 经 Aux 管道异步上报的键）不开新的一轮，也不承诺先于应用处理。所以"打字前"只承诺先于本轮的第一次派发。
 *   过渡期的例外：msime_client_key 还没有"开一轮但不出声"的位，所以 Linux 的 IBus 和 Fcitx5 宿主只为会出按键音的打字键调用它。松开、单独的修饰键、带 Ctrl/Alt/Super 的组合键、录音中的键和 IBus 屏幕键盘面板送来的键都不开新的一轮，它们引起的派发（如组字中按 Ctrl 组合键发出的 MSIME_FINISH_COMPOSITION 或 MSIME_CANCEL、单按 Shift 切换中英文时的 MSIME_COMMIT_RAW）算在上一轮里。第 3 阶段按轮计 fuel 的脚本运行时不会因为这类没有先调 msime_client_key 的派发熔断；补上这个位之后，宿主再为这些按下也调用它，让它们各自成一轮。按住不放产生的自动重复不出声、不画（与 macOS 宿主一样），但每次重复都会派发，所以两个 Linux 宿主为它借 msime_client_key(session, MSIME_KEY_POINTER) 开新的一轮；macOS 宿主目前不为自动重复调用 msime_client_key，自动重复引起的派发算在第一次按下那一轮里。
 * 同一次调用内的事件顺序：input（派发前）→ selection_retracted → candidate_selected → before_commit → commit → composition_end 或 composition_start → composition_update → candidates_shown 或 candidates_hidden，每次调用最多一个候选事件。focus 调用没有 input，focus_in/focus_out 排在它自己的上屏和组字结束之后；失焦上屏一律不可改写。destroy 先发 composition_end{Destroyed}，最后是会话结束。组字状态只在调用边界前后各采样一次。一次 transition 同时带着上屏和新组字时（韩文音节延续）只发 commit{continues}，不发 end 和 start。apply_online_*、resolve_online_candidates、clear_online_candidates、apply_cloud_response、rerank_settled、apply_translations 这些旁路调用，以及 set_english_mode、set_chinese_punctuation、set_punctuation_lock、update_preferences、set_ai_credential 这些可能在组字中途改动组字或候选的设置调用，只给核心发视图类事件（composition_end{Other}、candidates_shown/hidden 等）。派发、focus 和上面这些调用报错时照样推导事件：runtime 可能在报错前已经结束或改动了组字，start 和 end 始终成对。事件不带文本，也不写进任何逐键记录。
 * Windows TIP 会话的约束：TIP 在应用进程里自己的会话只给 Server 转述，不调用以下任何一个函数；字段属性和激活由 Server 的会话报告。
 * msime_client_key：key 的位 0-6 是按键类别 0-3（同 key_sound），位 7（MSIME_KEY_POINTER）是指针调用，类别被忽略，不出声不画；位 8（MSIME_KEY_REPEAT）是自动重复，画但不计连击；位 10（MSIME_KEY_PASSTHROUGH）见上。其余位忽略，类别超出 0-3 时什么都不做。返回值是 FEEDBACK 下这个键的打字特效，位布局同 msime_client_typing_effect；没开 FEEDBACK 时为 0。
 * msime_client_set_field_traits：宿主在每个键之前比较一次字段属性，变了就先调它，再调 msime_client_key。没有 MSIME_TRAIT_KNOWN 位时属性按未知处理：上屏不可改写；但 SECURE 和 FULLSCREEN 位本身仍然生效，声音、特效和音乐照样按上面的规则关闭或静音。返回现在生效的属性（不认识的位被丢掉）；会话没选择加入时返回 0。
 * msime_client_set_active：输入法在这个会话里激活（true）或停用（false）。FEEDBACK 下，true 让调用者成为背景音乐的所有者，只有所有者能开关音乐，所有者被销毁时音乐自动关闭；实际的音乐状态是"是所有者、处于激活、字段既不是 SECURE 也不是 FULLSCREEN"。会话已选择加入时返回 true。 */
#define MSIME_LIFECYCLE_FEEDBACK 0x1u
#define MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY 0x4u
#define MSIME_LIFECYCLE_HOST_HOLDS_PIECES 0x8u
#define MSIME_KEY_POINTER 0x80u
#define MSIME_KEY_REPEAT 0x100u
#define MSIME_KEY_PASSTHROUGH 0x400u
#define MSIME_TRAIT_SECURE 0x01u
#define MSIME_TRAIT_PRIVATE 0x02u
#define MSIME_TRAIT_NO_SPELLCHECK 0x04u
#define MSIME_TRAIT_FULLSCREEN 0x08u
#define MSIME_TRAIT_VOICE_ACTIVE 0x10u
#define MSIME_TRAIT_HOST_ENGLISH 0x20u
#define MSIME_TRAIT_ELEVATED 0x40u
#define MSIME_TRAIT_KNOWN 0x80000000u
uint32_t msime_client_set_lifecycle(uint64_t session, uint32_t flags);
uint32_t msime_client_key(uint64_t session, uint32_t key);
uint32_t msime_client_set_field_traits(uint64_t session, uint32_t traits);
bool msime_client_set_active(uint64_t session, bool active);
/* 脚本插件（桌面）。msime_client_scripts_enable 是进程级开关，返回开关的实际状态；任何线程都可以调用，不分配要释放的内存。只由 4 个桌面入口在启动时、第一个会话之前调用 msime_client_scripts_enable(true)：macOS 输入法进程、Windows Server、IBus 引擎进程和 Fcitx5 addon。Windows TIP（与 Server 共用同一份库，被注入每个应用进程）、设置应用和 MCP 从不调用，所以解释器的代码在它们的进程里，但从不执行。iOS、Android 和 HarmonyOS 上恒返回 false。
 * 授权文件 <state root>/plugins/script-grants.json 在用户可写的目录下，不签名：同一用户下的进程能伪造授权。三个桌面平台的开发构建和 release 构建都运行脚本，这一残余风险写在授权页上：macOS 和 Linux 是共享设置页，Windows 是原生设置应用（经 msime_client_plugins 的 script_page 和三项授权操作）；Windows 的提权窗口（MSIME_TRAIT_ELEVATED）里从不运行脚本。
 * 脚本运行要同时满足：开关开着；会话经 msime_client_set_lifecycle 选择加入；字段属性带 MSIME_TRAIT_KNOWN，且不带 SECURE、PRIVATE、ELEVATED。属性变得不满足时正在跑的实例当场丢弃。每次 msime_client_focus(true) 之后属性都要重新确认：宿主在聚焦之前调过 msime_client_set_field_traits（属性没变也算），或者聚焦之后按了第一个不带 MSIME_KEY_PASSTHROUGH 的键（msime_client_key；交给应用的键在 Windows 上经 Aux 管道异步上报，和属性没有先后保证，不算确认）；确认之前不实例化、不投递，所以想让脚本收到 focus_in 的宿主要在 msime_client_focus(true) 之前报告属性。观察类事件（focus_in、focus_out、input、selection_retracted、candidate_selected、commit、composition_start/update/end、candidates_shown/hidden）来自派发类调用、focus 和 destroy；msime_client_key 的按键、字段属性、激活和旁路调用的视图事件不投递。修改类钩子只在派发类调用里运行：candidates 在候选刷新和展开之后，插件可以重排、隐藏、注释候选，视图和选择 ID 按重排之后的顺序，被隐藏的行不出现在视图、分页和 msime_client_all_candidates 里、任何选择都选不到，插件的注释放在候选的 "script_annotation" 键里（没有时不出现，不改 "annotation"，由宿主另行绘制）；设着 MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY 时 candidates 不运行也不重放；before_commit 在上屏之前，可改写时（选中候选、原样上屏或暂存段送出，本地模式 none 且算打字，不是以词定字，没设 MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY，不是 MSIME_LIFECYCLE_HOST_HOLDS_PIECES 下宿主暂存的一段）插件可以改写标点和顶字尾巴之前的那一段，回复里的 commit 就是改写之后的文本，引擎学习和 AI 上文仍按原文。旁路调用不运行脚本，只按候选身份重放最近一次的重排、隐藏和注释。command 钩子给插件自己的 / 指令出候选：选择加入时和每次 msime_client_focus(true) 时，会话把启用、授予了 provide_commands 的插件在清单里声明的触发词交给引擎（用户指令表赢、插件按执行顺序先到先得）；用户在 / 指令里打完某个触发词时，派发类调用只投递给第一个声明它的插件一次，事件带触发词、后面的字母和 64 字节系统随机数，插件给的行是来源 14 的候选（"script_annotation" 是「标题 注释」），由引擎按预编辑串缓存，旁路调用、移动高亮和翻页都不再投递；运行条件不满足或设着 MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY 时只有原样的那一行。脚本只能经授权的 import 在钩子调用期间读文本，读不到宿主的任何别的状态，也不能回调宿主。每次 msime_client_focus（两个方向）和 destroy 都丢弃这个会话的全部脚本实例。fuel 按 msime_client_key 开启的轮次给：没有先调 msime_client_key 的派发（修饰键组合、快捷键、失焦命令）和上一次按键共用那一轮的配额；本轮配额用完时插件跳过这次投递，不算失败；运行到一半被本轮配额截断的插件在这次聚焦里不再运行，这一次用掉的不到单个钩子自己上限的一半时同样不算失败，达到一半就和超出单个钩子自己的上限一样算一次失败（candidates 的单次上限和每轮配额相等，同一轮里 input 先花过 fuel 时它的死循环先撞上每轮配额，这样也照样熔断）。脚本的失败不影响上屏：一次失败只丢弃它这次的结果，trap 之后这个插件在这次聚焦里不再运行，连续失败按插件熔断；被停用的插件和停用的类别（invalid、init、failed、slow，不带次数、具体原因和任何文本）在失焦时写进 <user_data>/script-health.json。 */
bool msime_client_scripts_enable(bool on);
/* The typing effect of one key, for the host to draw beside the caret: the session's combo count and the resolved style: the selected effect pack's (preferences.plugins.effect_pack) when one is set, preferences.plugins.effect_style otherwise. Also for the key path: integer arithmetic on the session's own state, no lock, no allocation, no file, no free; it works on every platform, iOS, Android and HarmonyOS included. Call it once per key the host handles while effect_style is not "off", effect_pack is set or combo_counter is on, alongside key_sound, and once per commit; skip keys typed into a secure (password) field.
 * event bits 0-7: 0 any other key, 1 space, 2 enter, 3 backspace (the key_sound classes), 4 commit, 5 backspace by any other route (a delete that bypasses key_sound). 0-2 count one key; 3 and 5 end the combo; 4 counts nothing and only reports the state.
 * event bit 8 (0x100): the key is an auto-repeat of a held key; drawn, not counted. Bit 9 (0x200): sounds must stay quiet now (a full-screen foreground application on Windows); the tier-up sound is neither queued nor reported due. Other bits are ignored.
 * Return value, 0 when the resolved style is off and combo_counter is off, for an unknown session handle, a wrong thread, an event code above 5, or after a panic: bits 0-15 the combo count (saturating at 65535, always 0 while combo_counter is off); bit 16 this key moved the combo up a tier (it reached 10, 25, 50 or 100 keys); bits 17-19 the style, 0 off, 1 flash, 2 sparks, 3 power_mode; bit 20 the tier-up sound is due, set only with combo_counter and combo_tier_sound on and bit 9 clear. On macOS, Windows and Linux this library has already queued that sound (the key pack's commit sample raised 3 semitones per tier); a host that plays packs itself plays it. The combo also ends after 3000 ms without a counted key, and carries over a change of preferences. */
uint32_t msime_client_typing_effect(uint64_t session, uint32_t event);
/* The session's resolved typing effect, for drawing what msime_client_typing_effect reports. Standard response; value {pack: id|null, issue: string|null, style: "off"|"flash"|"sparks"|"power_mode", intensity: 0..100, colors: ["#RRGGBB", 0..4 entries], duration_ms: 60..1500|null, particles: 0..64|null, combo_counter: bool}. With no effect pack selected (pack null) style and intensity are preferences.plugins.effect_style and effect_intensity, colors is empty and duration_ms and particles are null: the host's own defaults for the style. With one selected, the pack's parameters replace them; a pack that cannot be loaded gives style "off" and issue says why, in Chinese for a log line. colors, duration_ms and particles are hints a host may ignore when its style has no such thing. Linux draws no effect and shows only the combo count. Reads one small manifest only when the selection or the pack changed: call it after msime_client_update_preferences and after a focus-in, not per key. Free with string_free. */
char *msime_client_typing_effect_settings(uint64_t session);
/* The validated files of one sound pack, for a host that plays packs itself (HarmonyOS). Request (<=65536 bytes): {state_root: absolute|null, sound_packs: absolute|null, pack: id}; state_root is the preferences directory holding plugins/, sound_packs the bundle's built-in pack root. Built-in ids ("default", "twinkle", "msime-typewriter", "msime-bubble", "msime-8bit", "msime-woodblock", "msime-pentatonic", "msime-canon", "msime-ode-to-joy") always resolve from sound_packs; any other id needs state_root and is refused without one. Value: {id, name, license, builtin, mode:"keys"|"sequence", sounds:{default, space, enter, backspace, commit, achievement: absolute path|null}, sequence:{sample: absolute path, semitones:[-24..24], advance:"key"|"commit"}|null, max_sample_millis, melody_idle_reset_millis}. Reads the pack from disk: not for the key path. */
char *msime_client_key_sound_pack(const uint8_t *request, size_t length);
/* The validated tracks of one music pack, for a host that streams music itself (HarmonyOS). Request (<=65536 bytes): {state_root: absolute|null, sound_packs: absolute|null, pack: id}, as for key_sound_pack. Built-in ids ("msime-music-lofi", "msime-music-ambient") resolve from sound_packs, which holds them beside the built-in sound packs; any other id needs state_root and is refused without one. Value: {id, name, license, tracks:[absolute path, in play order], max_track_seconds}; a host plays a track only when its duration is within max_track_seconds, then the next, starting over after the last. Reads the pack from disk: not for the key path. */
char *msime_client_music_pack(const uint8_t *request, size_t length);
/* 插件页的包仓库、设置页插件编辑器和 @ 名单，给桌面外壳以外的设置宿主：Windows 的原生设置应用、鸿蒙的插件页，以及 Android、iOS 设置里导入辅助码表。Request（<=8 MiB）为 {state_root: 绝对路径, sound_packs: 绝对路径|null, plugin_packs?: 绝对路径|null, script_page?: bool, user_data?: 绝对路径|null, action}；包和 mentions.json 放在 state_root/plugins 下，sound_packs 是安装包自带的内置音效包目录，plugin_packs 是桌面安装包自带的官方插件包目录（<kind>/<id>/，没给时取 sound_packs 旁边的 plugin-packs，同宿主选项的 plugin_packs）。script_page 为 true 表示宿主有脚本插件的授权页（列出、授权、排序，并写明同一用户下的其他程序能伪造授权），缺省为 false：Android、HarmonyOS 和只导入辅助码表的宿主不带它，永远看不到脚本插件。user_data 是输入法宿主选项里的 user_data 目录，只用于脚本授权的回答里的停用记录。action.operation：
 * "catalog"：value 为 {packages:[...], issues:[{kind, folder, reason}]}，即全部已安装的包、内置音效包和官方插件包（builtin 为 true，默认停用：脚本包要授权，短语表要在偏好里启用），与桌面外壳列出的相同；官方包的 id 在插件目录里有同名的旧副本时，那份副本列在 issues 里，不会被使用；不带 script_page 时去掉脚本插件（kind 为 script）的包和问题行。脚本包另有 abi、module、hooks（声明的钩子名）、permissions（申请的能力名）、memory_pages 和 content_hash（64 位小写十六进制的内容哈希，授权绑定在它上面）。
 * "import" {source: 包文件夹或 .zip 文件的绝对路径}：安装它，整包替换已安装的同 id 包；value 是装好的包。不带 script_page 时 release 构建拒绝脚本包（plugin_invalid，与移动端遇到 script 时的原因相同）；带 script_page 时接受，装好的脚本包默认不运行，要经授权。
 * "import_helpcode_table" {file_name, text}：把单个 .txt 辅助码表装成 kind 为 helpcode 的包。file_name 是用户选的文件名，只用于报错、显示名和 id；text 是码表正文，宿主必须先按 UTF-8 严格解码（解码失败就不要调用），原始字节不超过 1 MiB。码表按辅助码表包的严格规则校验（每行「字=码」，码是 1 到 2 个小写字母，同一个字只能出现一次，1 到 30000 条），不合格时是 plugin_invalid，detail 写明文件名、原因和行号。显示名取开头的 # name: 注释，没有时用去掉扩展名的文件名；id 是 custom-<小写文件名主干>，组不成合法 id 时是 custom-<12 位十六进制哈希>，同名文件再次导入时整包替换。value 是装好的包，其中 entries 是条数。宿主再把 id 写进 preferences.plugins.helpcode_pack_quanpin 或 helpcode_pack_shuangpin 来选用它。
 * "remove" {kind: "sound"|"music"|"command_table"|"effect"|"phrase_table"|"helpcode"|"wordbook"|"symbol_set"，桌面上另有 "script", id}：value 为 null；没装的包视为已删除。内置音效包和背景音乐包的 id 是 plugin_reserved；官方插件包的 id 只删插件目录里的同名旧副本，内置的那一份和它的授权都不动。
 * "load_mentions"：value 为 [{text, key, phone}]（phone 是联系人手机号，普通名单为空串），还没保存过名单时为空。保存时要把读到的 phone 原样带回去，否则它会被清掉。
 * "save_mentions" {entries:[{text, key, phone}]}：替换名单；value 为 null。
 * "authoring_load" {kind, id}：设置页插件编辑器读出用户插件目录里的一个包，value 是一份 AuthoredPack：{kind: "phrase_table"|"command_table"|"symbol_set"|"wordbook", id, name, version, license, author: string|null, description: string|null}，另加与 kind 对应的恰好一个内容字段："phrases":[{key, text}] | "commands":[{trigger, title, template}] | "groups":[{tab: "symbols"|"kaomoji", title, keywords: string|null, items:[string]}] | "words":[{word, phonetic: string|null, gloss}]（单词本读的是它的 .tsv 词表）。内置和官方包的 id、这四种以外的类型、以及没有装在 state_root/plugins 下或载入不了的包都是 plugin_not_editable，detail 说明是哪一种。
 * "authoring_save" {pack: AuthoredPack, replace: bool}：生成 plugin.toml（单词本另生成 words.tsv，一行一个「单词\t音标\t释义」，没有音标时两列）放进暂存目录，按 "import" 完全相同的规则校验并安装；value 是装好的包，与 "import" 的 value 同形。pack.id 为 null 时生成 my- 加 8 位小写十六进制随机数的 id，同类型下已被占用就再换一个；author、description、keywords 为 null 或全是空白时不写进清单。同类型、同 id 的包已经装着时，replace 为 false 是 plugin_exists，为 true 则整包替换，原包里的 .txt、.md 说明文件原样带进新包（从社区装来的包改过之后照样能保存，成为本机的副本）。内容不合规时是 plugin_invalid，detail 是清单或词表校验报告的规则（与导入同一条，例如「短语编码 DH 必须是 1 到 32 个小写字母」），另有两条只有编辑器会碰到的：单词的某一列含有制表符或换行、单词以 # 开头；内置 id 是 plugin_reserved；内容字段与 kind 不符也是 plugin_invalid。
 * 以下三项只在桌面（macOS、Windows、Linux）上有，别处是 invalid；value 都是整份授权 {revision, plugins:[{id, hash, enabled, granted:[能力名]}], health:[{id, hash, disabled: "invalid"|"init"|"failed"|"slow"}], commands:[{id, trigger, title, status: "active"|"shadowed_by_user_table"|"shadowed_by_plugin", conflict?, conflict_id?}]}，plugins 的顺序就是执行顺序，health 是输入法停用了的插件（来自 user_data/script-health.json，没给 user_data 时为空），commands 是启用、授予了 provide_commands、清单能载入的插件声明的 / 指令和状态（conflict 是冲突的触发词，conflict_id 是冲突的插件，用户指令表取 state_root 里偏好启用的指令表包），与桌面外壳的 script_grants 命令相同。
 * "script_grants"：读出授权。
 * "set_script_grant" {id, hash, enabled, granted:[能力名]}：授予、调整或撤销一个脚本插件的授权。hash 是页面展示给用户的内容哈希，包在这之后被换过时是 plugin_script_changed；granted 必须是清单申请的能力的子集，并满足依赖和 rewrite_commit 与 read_composition 互斥的规则；启用时同时启用超过 8 个是 plugin_script_limit，超出内存总账是 plugin_script_memory。撤销是 enabled 为 false、granted 为空。写下的 granted 里没有 persist_data 时（撤销了它，或者插件更新后重新授权时没有再给）同时删掉这个插件的持久存储；persist_data 与 rewrite_commit 也互斥。
 * "reorder_scripts" {ids:[id]}：ids 按给出的顺序排到最前，其余按原来的相对顺序；有重复或没有授权条目的 id 时整个拒绝。
 * 以下三项同样只在桌面上有；value 都是一个插件的状态 {id, values:{设置项 key: 值}, storage_bytes}，values 覆盖清单 [[script.settings]] 的每一项，类型同清单（bool 是 true/false，choice 是选项的 value，number 是整数，text 是字符串），存下的值不合规时是默认值；storage_bytes 是 persist_data 的存储文件占的字节数，没有时为 0。设置项的声明（title、description、type、options、min、max、max_length、default）在 "catalog" 里脚本包的 settings 字段。插件没装或清单载入不了是 plugin_script_missing。
 * "script_plugin_state" {id}：读出一个插件的设置值和存储大小。
 * "set_script_settings" {id, values:{key: 值}}：整份替换这个插件的设置值，没写的项用默认值。清单里没有的 key、类型不对或超出范围的值整个拒绝，是 plugin_script_setting_invalid（detail 说明是哪一项）；设置文件是更新的版本写的是 plugin_script_settings_format。改设置不作废授权，输入法在下一次聚焦时用新值重建插件。
 * "clear_script_data" {id}：删掉这个插件的持久存储（persist_data），输入法手上还没写盘的部分一并作废。
 * 失败时为 {ok:false, error: 失败码, detail?}：失败码与桌面外壳相同（invalid、storage、plugin_invalid、plugin_unsupported_source、plugin_archive、plugin_reserved、plugin_storage、plugin_exists、plugin_not_editable、plugin_script_missing、plugin_script_changed、plugin_script_grant_invalid、plugin_script_limit、plugin_script_memory、plugin_script_grants_format、plugin_script_setting_invalid、plugin_script_settings_format、mention_invalid、mention_format、mention_storage），detail 存在时是被拒的包或条目违反的规则，中文，给设置页显示。会读写文件，导入最多复制一个音乐包大小的数据：宿主有工作线程时在工作线程上调用。 */
char *msime_client_plugins(const uint8_t *request, size_t length);
/* 离线世界时钟；JSON 请求最多 16384 字节，返回值用 msime_client_string_free 释放。 */
char *msime_client_world_clock(const uint8_t *request, size_t length);
/* 不带编码的常用语（<=4 MiB）：{directory: 偏好目录的绝对路径, action:{operation, ...}}。
 * 正文操作：load、add{text,note?}、remove{id}、replace{id,text,note?}、move{id,index}、install_pack{resource}、remove_pack{id}。
 * 本机分类：add_category{name}、rename_category{id,name}、remove_category{id}、categorize{id,category:UUID|null}、set_expanded_category{category:UUID|"all"|""}。
 * 备份分类合并：import_categories{categories:[{id,name,texts:[正文]}],expanded_category:备份分类UUID|"all"|"",phrase_ids:[本次新增UUID]}，原子合并，保留已有正文归属。
 * Value 为 {phrases:[{id,text,pack,note?,category?}],packs:[{id,name,revision}],categories?:[{id,name}],expanded_category?:UUID|"",skipped?}；省略 expanded_category 表示 "all"。
 * 错误码：common_phrases_io、_corrupt、_invalid、_note_invalid、_duplicate、_limit、_too_large、_not_found、_category_invalid、_category_duplicate、_category_limit。
 * 设置进程和键盘进程都会调用，调用时锁住文件。在工作线程上调用。 */

char *msime_client_common_phrases(const uint8_t *request, size_t length);
/* 具名词库集合（<=17 MiB）：{options: 带 preferences_directory 的 HostOptions, action:{operation:"load"|"create"|"rename"|"delete"|"set_enabled"|"add_words"|"remove_words"|"import"|"install_community"|"queue_words"{entries}|"flush"|"synchronize"|"third_party_matches"{entries}, ...}}，格式见 client-services 的 dictionary::collections；import 的 format 为 "scel"（搜狗细胞词库）时文件按 base64 放在 bytes_base64（不超过 8 MiB），不带 text，结构损坏时是 import_malformed；queue_words 把不属于任何集合的词（本地备份恢复的个人词库，每次最多 20000 个）排进待发送队列，value 带 queued；third_party_matches 只读地核对最多 128 个准备发布到社区的词，value 的 third_party_matches 是其中出现在搜狗细胞词库导入的集合里的下标。词条经个人词库队列传过去，键盘在下一次会话时应用。synchronize 在 budget_millis（1–2000，默认 2000）毫秒软预算内逐批应用个人队列并补充待发项，每批个人队列至多 128 项，集合直接应用至多 4096 项，在整批结束后释放维护租约，返回 {synchronized,pending_count,failed_count,snapshot_error,sent,applied_count}，sent 和 applied_count 累计整个调用，后者累计个人队列的 Written 确认（含幂等重试）；pending_count 包括未发送的集合条目；普通操作的 Value 是集合视图；错误是固定的代码（collections_*、builtin_locked、unsupported_format、import_*、personal_dictionary_*）。在工作线程上调用。 */
char *msime_client_dictionary_collections(const uint8_t *request, size_t length);
/* 诊断包（<=65536 字节）：{state_root: 偏好目录, include:{crash_logs,performance_logs,input_events,config_snapshot}, sources:{crash_logs,performance_logs,input_events: 绝对路径|null}, destination: .zip 的绝对路径|null}。输入事件和性能记录只有恰好是 {t_ms, kind, duration_ms} 时才保留，kind 以 client-services diagnostics.rs 的 EVENT_KINDS 为准（按键、候选、面板、ime_start/ime_finish、input_failure_*、engine_session_*、engine_call、candidate_rendered、voice_engine_*、touch_layout_* 等），不在里面的种类整行丢掉，带其他任何键（text、key、candidate……）的整行都丢掉并计数。崩溃记录只保留异常类型和栈帧：异常说明可能带着正在输入的文字，message 只留冒号前的异常类型，stack 的说明行同样处理，路径只留文件名。input_failure_* 仅供本地诊断，云端上传器须按服务端白名单过滤。配置快照把所有凭据换成 "<redacted>"。给了 destination 时写出 zip，value 是 {path,bytes,counts}；没给时 value 是 {counts,sections}，sections 是上传请求体的对象（crash_logs、perf_trace、input_events、config_snapshot）。错误：diagnostics_invalid、diagnostics_source、diagnostics_preferences、diagnostics_write。在工作线程上调用。 */
char *msime_client_diagnostic_bundle(const uint8_t *request, size_t length);
/* 账号设置文档导出（<=4 MiB）：{preferences_directory, feedback?:{soundEnabled,hapticsEnabled,hapticStrength}|null, custom_keyboard_skins?: JSON 数组字符串|null, android_local?: {同步键: 值}|null, schema?: 偏好字段表|null, cloud?: {revision,settings}|null}。Value {settings, merged?}：settings 按本版本过滤（给了 schema 时再按它的字段过滤）；同时给了 schema 和 cloud 时，merged 是带着云端 revision 去 PUT 的整份文档。android_local 是 Android 本机设置文件里参与同步的值（应用主题、单手模式、按键细节、工具栏的常用语/方案/隐藏项、手写、离线语音），不认识的键和非法值不导出。凭据和诊断日志从不导出；隐私模式、开发者选项和语音贡献只在 Android 本机文件里，从不同步。 */
char *msime_client_account_settings_export(const uint8_t *request, size_t length);
/* 账号设置文档应用（<=4 MiB）：{preferences_directory, cloud:{revision,settings}, schema, feedback?: 宿主当前的按键反馈|null}。把文档应用到本机偏好，按读到的 revision 做比较并交换后保存。本机不认识或超出范围的取值只跳过那一个键；本机选着账号装不下的方案（粤拼、注音、越南文、藏文、笔画）时导出不写 input.schema，应用时也不拿云端的方案盖掉本机的方案。Value {preferences: 保存后的快照, feedback: 应用后的按键反馈|null（宿主自己存）, custom_keyboard_skins: 云端皮肤库 JSON|null（宿主合并）, android_local:{同步键: 值}（宿主写进本机设置文件）, skipped:[键]}。 */
char *msime_client_account_settings_apply(const uint8_t *request, size_t length);
/* iOS 的账号设置文档导出（<=4 MiB），与 Tauri iOS 的 account_preferences_upload 相同，但不联网、不读会话：{preferences_directory, ios_native, schema, cloud?}。ios_native 是 App Group 里的键盘设置，字段与 Tauri 插件的 loadKeyboardPreferences 相同（camelCase：inputScheme、traditionalChineseOutput、soundEnabled、hapticsEnabled、hapticStrength、englishSuggestions、candidatePaletteFollowsDesktop、inlinePreedit、hapticsAvailable、tabletFullKeys?、tabletSplitKeyboard?、dictionaryLearning、globalTheme、customKeyboardSkin）；schema 是服务端的偏好字段表，必需，因为服务端只要有一个键不在表里就拒绝整份文档，platform.ios.global_theme 和 platform.ios.custom_theme_base 目前不在表里，导出时会被去掉。Value {settings, merged?}：settings 按 preferences_directory 所属的版本过滤、只留字段表收录的键；给了 cloud（{revision,settings}）时 merged 是带着云端 revision 去 PUT 的整份文档。一个键都没有时 error 是 account_unavailable；ios_native 不合规是 account_storage，请求不合法是 account_invalid。 */
char *msime_client_account_ios_settings_export(const uint8_t *request, size_t length);
/* iOS 的账号设置文档应用（<=4 MiB），与 Tauri iOS 的 account_preferences_apply 相同，分两步，不读会话也不检查会话代次，下载文档以后账号有没有变由宿主负责：{preferences_directory, cloud:{revision,settings}, step:"plan"|"commit", ios_native?, saved_native?}。与导出不同，应用不带 schema、不按字段表过滤（与 Tauri iOS 的 apply 相同），请求里多出任何字段（包括 schema）都是 account_invalid。cloud 先按 preferences_directory 所属的版本过滤，再整份校验，任何一个值不合规都是 account_invalid。step 为 plan 时要带 ios_native（App Group 里当前的键盘设置），Value {ios_native: 要写进 App Group 的键盘设置}，什么也不写；ios_native 的方案是账号装不下的粤拼、注音、越南文、藏文或笔画时（导出不写方案），要写的键盘设置保留本机的方案。cloud 里有 input.wubi_schema 时总是把它写进共享偏好的五笔版本，不管 input.schema 是不是 wubi（与 Android、桌面和鸿蒙的映射相同），不认识的取值与其余的键一样是 account_invalid；没有这个键时保留本机的五笔版本。宿主写进 App Group 以后，用实际保存下来的值作 saved_native 调 step 为 commit：按读到的修订号比较并交换写回共享偏好，Value {preferences: 保存后的 PreferencesSnapshot}。commit 失败时宿主把 App Group 恢复成旧值，顺序与 Tauri 相同：先写 App Group，再写共享偏好，失败时回滚 App Group。写回失败时 error 是 conflict（本机设置已在别处更新）或 storage，映射失败是 account_invalid 或 account_storage。 */
char *msime_client_account_ios_settings_apply(const uint8_t *request, size_t length);
/* 鸿蒙的账号设置文档导出（<=4 MiB），原来是鸿蒙账号桥 preferences_upload 里 AccountPreferencePlan 的那部分，不联网、不读会话：{preferences_directory, feedback?:{soundEnabled,hapticsEnabled,hapticStrength}|null, schema, cloud?}。鸿蒙的本机设置就是 preferences_directory 里的共享偏好，与 Android 是同一套映射，平台键放在 platform.harmony.* 下（不写 platform.android.*，也不导出只有 Android 才有的工具栏、按键音包、语音语言、本地设置和皮肤库）；全拼 14 键不上传（云端保留原值），鸿蒙不提供的全局主题按 system、不认得的振动强度按 medium 上传。feedback 是宿主反馈文件里的值，不带时不导出那三个键。schema 是服务端的偏好字段表，必需，服务端收录 platform.harmony.* 之前只有 input.* 留下来。Value {settings, merged?}：settings 按 preferences_directory 所属的版本过滤、只留字段表收录的键；给了 cloud（{revision,settings}）时 merged 是带着云端 revision 去 PUT 的整份文档，别的设备写的键原样保留。一个键都没有时 error 是 account_unavailable；读不出共享偏好是 account_storage，请求不合法或合并时键不在字段表里、类型不对、超出字节上限是 account_invalid。 */
char *msime_client_account_harmony_settings_export(const uint8_t *request, size_t length);
/* 鸿蒙的账号设置文档应用（<=4 MiB），原来是鸿蒙账号桥 preferences_apply 里 AccountPreferencePlan 的那部分，不读会话也不检查会话代次，下载文档以后账号有没有变由宿主负责：{preferences_directory, cloud:{revision,settings}, schema, feedback?: 反馈文件里当前的值|null}。cloud 先按 preferences_directory 所属的版本过滤，再整份校验：字段表声明了的键类型不对、文档或字段表本身不合规时整份拒绝；只读 input.* 和 platform.harmony.*，本机不认识或超出范围的取值只跳过那一个键。导出不上传的两项应用时也不改：本机是全拼 14 键时不应用 platform.harmony.keyboard_layout，本机选着账号装不下的方案（粤拼、注音、越南文、藏文、笔画）时不应用 input.schema，否则上传后写回、或下载时会被云端留着的旧值改回去。文档和字段表里都有按键反馈的键而没带 feedback 时是 account_invalid。偏好有变化时按读到的修订号比较并交换写回整份快照。Value {preferences: 保存后的 PreferencesSnapshot, feedback: 应用后的按键反馈|null（不为 null 时宿主写回反馈文件，失败时不回滚已经写好的偏好）, skipped:[键]}。写回失败时 error 是 conflict（本机设置已在别处更新）或 storage，读不出共享偏好是 account_storage，映射失败是 account_invalid 或 account_unavailable（字段表不可信）。 */
char *msime_client_account_harmony_settings_apply(const uint8_t *request, size_t length);
/* 多分类自动云同步（设置、Android 的自定义键盘皮肤库、无编码常用语、个人词库）的编排，原来在 iOS 的 CloudSync 和 Android 的 home/CloudSync 各有一份。Request 是 length 字节（最多 4 MiB）的 UTF-8 JSON {directory?|session_store?, legacy_directory?, state_directory, preferences_directory?, options?, snapshot_queue_directory?, action}。会话的来源与 msime_client_account 相同（directory 和 session_store 恰好给一个），只用登录账号；peek、mark_dirty、dictionary_reset 和 import_legacy 只碰状态文件（import_legacy 还只读快照激活队列，dictionary_reset 还会取消队列里的请求），可以不带会话来源，也从不按会话对齐。state_directory 是绝对路径，状态在它下面的 cloud-sync/state.json（版本 2，最多 256 KiB，同目录 state.lock 用 flock 保护，基线和收不下的常用语只存 SHA-256）和 pending.json；iOS 传 App Group 的根目录，键盘扩展和应用读写同一份。preferences_directory 是共享偏好和常用语所在的偏好目录，run 和 commit 要带。options 是 Engine 的宿主选项（与 msime_client_dictionary 相同，要带 preferences_directory），snapshot_queue_directory 是词库快照激活队列的目录（Android 是 files/bootstrap/state/dictionary-snapshots），同步词库时两个都要带；sign_out 带上队列目录时取消旧账号排队中的快照；import_legacy 导入 Android 旧状态时也要带上队列目录（只读）。status、set_enabled 和 run 每次调用都按会话里的用户 id 对齐：账号变了（或已经退出）就清空进度、关掉开关；同一个账号重新登录保留进度。会话读不出来（钥匙串不可用）时不对齐。所以只有持有登录的那个进程（iOS 和鸿蒙的应用、Android 的主进程、桌面的设置程序）可以调 status、set_enabled、run、commit 和 sign_out；会话来源里看不到登录的进程（键盘扩展、Android 的 :ime、注入存储读出 0 字节的扩展）调 status 会被当成已经退出，把共享的同步进度清空、开关关掉，这些进程看开关一律用 peek，记改动用 mark_dirty。
 * action 是下列之一：
 * {operation:"status"} → 状态 S：{enabled, disabled_on_cloud_cleared, account_id|null, sections:{settings|skins|phrases|dictionary:{cursor|null, dirty, cleared, last_outcome?:{outcome,reason?}}}, last_attempt_ms, last_synced_ms, not_before_ms, pending_choice:"merge"|"use_cloud"|null, running, failure:{section,reason,status?,code?,message}|null, skins_trimmed, unheld_phrases, status_line}。status_line 是「我的」页同步行的一句话，措辞与两端原来的相同，只有两句是新的：等「云端被清空」的选择时是「云端数据已被清空，请选择合并、使用云端或关闭同步」，用户在那里选了「关闭同步」以后、再开关同步之前是「云端数据已被清空，已关闭同步，本机数据没有改动」，这时 disabled_on_cloud_cleared 为 true（打开、关闭同步和换账号、退出都把它清成 false）。
 * {operation:"peek"} → S：只读 state.json，不读会话、不对齐、不写任何东西（状态目录还不存在时是全新的默认值），给不持有登录的进程看开关；请求可以不带会话来源，带了也不用。
 * {operation:"sign_out"} → S 加 snapshots_cancelled：退出登录时调用，同步清空状态、代次加一，进行中的那一轮因此提交不了。它不读会话，但请求仍要带会话来源，按 session_store 时要在注销这个存储之前调用，否则是 cloud_sync_invalid、状态不清。
 * {operation:"set_enabled", enabled} → S：打开时要有登录的账号（否则是 cloud_sync_signed_out），并把全部分类标成待上传；关闭时保留游标。
 * {operation:"mark_dirty", section:"settings"|"skins"|"phrases"|"dictionary"} → {marked}：只读写状态文件，不读会话、不联网，键盘扩展和 Android 的 :ime 进程用户改了本机内容时调用；开关关着时不记（marked 为 false）。同步自己写本机不经过这里，宿主也不要为同步写的值再打标记。
 * {operation:"dictionary_reset"} → {marked}：本机用 msime_client_dictionary 的 reset 清除了学习记录以后调用，不读会话、不联网。带 snapshot_queue_directory 时先取消状态里绑定账号排在激活队列里、还没激活的云端快照，然后不管开关开没开都忘掉等激活的记录，再像 mark_dirty 一样给 dictionary 打标记（开关关着时 marked 为 false）。不忘掉的话，清除之前下载、因本机版本变了而激活不了的那份快照会让下一轮按合并把云端的旧词并回本机。取消失败时是 cloud_sync_storage，什么也没记，可以再调一次。
 * {operation:"run", trigger:"active"|"enable"|"choice"|"manual", choice?:"merge"|"use_cloud"|"disable", capabilities:{settings,skins_library,phrases,dictionary}, host_inputs?:{ios_native?, harmony?, feedback?, android_local?, custom_keyboard_skins?, settings_locked?, excluded_phrase_texts?}} → R：{outcome:"skipped"|"needs_choice"|"done"|"pending_commit"|"disabled", reason?, run_id?, host_writes?:{ios_native?, feedback?, android_local?, custom_keyboard_skins?, adopt_texts?}, sections?:{<分类>:{outcome:"synced"|"unchanged"|"skipped"|"failed"|"needs_choice", reason?}}（分类是 settings、phrases、dictionary，皮肤库没有动时还有 skins）, status: S}。active 是回到前台，按节流跑（距上次满 5 分钟，参与的分类有待上传标记时满 30 秒）；enable、manual 和这一次请求带了 choice 的不等节流，状态里记着的选择（另一轮在跑时记下的 pending_choice、词库合并没做完时留给词库的选择）不让下一轮跳过节流；所有触发都尊重服务端的 Retry-After（记成 not_before_ms，期间是 skipped/backoff）：超过 24 小时的按 24 小时记，离现在超过 24 小时的 not_before_ms（时钟往回拨以前记下的）不再挡、status 里是 null，所以手动同步最多被挡 24 小时。trigger 为 choice 时必须带 choice。capabilities 是宿主这一端同步哪几类：iOS 是 {settings, phrases}，Android 四类都开，鸿蒙不开 skins_library；skins_library 要和 settings 一起开，dictionary 要带 options 和 snapshot_queue_directory，否则是 cloud_sync_invalid。host_inputs 是宿主保管、Rust 读不到的本机值：带 ios_native（App Group 里的键盘设置，字段同 msime_client_account_ios_settings_export）时设置按 iOS 映射，harmony 为 true 时按鸿蒙映射（platform.harmony.*，同 msime_client_account_harmony_settings_export，按键反馈放在 feedback，不能同时带 ios_native、android_local、custom_keyboard_skins 或开 skins_library，否则是 cloud_sync_invalid），否则按 Android 映射（feedback、android_local、custom_keyboard_skins 同 msime_client_account_settings_export）；custom_keyboard_skins 不是 JSON 数组、或者云端的皮肤库读不懂（不是 JSON 数组，可能是更新的版本写的）时，上传的设置文档里皮肤库原样用云端的值，不删也不拿另一份替换它，设置照常同步，sections.skins 是 skipped，reason 为 skins_library_unreadable 或 cloud_skins_library_unreadable；settings_locked 为 true（iOS 社区皮肤试用中）时这一轮不同步设置；excluded_phrase_texts 是 Android 本机预置、还没被认领的示例常用语，不上传，也不会被当成本机多出来的删掉。
 * run 的 outcome：skipped 时 reason 是 signed_out、disabled、throttled、backoff、busy（另一轮还在跑；这时带的 choice 记成 pending_choice，下一轮用它，但下一轮照样等节流）、awaiting_choice（要问用户但 5 分钟内刚问过）、session_unavailable、dictionary_storage（同步词库时建不出 state_directory 下的 cloud-sync/work）或 stale_run（跑到一半账号变了、开关被关或另一处写了状态，这一轮作废）。needs_choice 时 reason 是 first_run（有分类第一次同步而云端已有数据）或 cloud_cleared（云端被清空，多半是「删除云端数据」；写入因修订号冲突而重读云端时读到被清空的云端也算，不拿旧基线合并），整轮什么都没写，宿主弹「合并 / 使用云端」（cloud_cleared 时多一个「关闭同步，保留本机」），用户选了再以 trigger:"choice" 调一次；choice 为 disable 只回答 cloud_cleared，只能和 trigger:"choice" 一起给（否则是 cloud_sync_invalid）：它不联网、不跑一轮，也不读写本机的设置、常用语和词库（所以可以不带 preferences_directory），只在状态锁里关掉这台设备的同步，R 是 {outcome:"disabled", status: S}；不等节流、Retry-After 和另一轮（在跑的那一轮因此提交不了，它在这之前写到云端的不撤回），也不记成 pending_choice，记着的选择（pending_choice、词库留下的选择）一起丢掉；清空的标记和游标留着，用户以后重新打开同步，第一轮读完云端照样先问，不会把本机传回被清空的云端。账号对齐与 run 相同；没有登录、开关关着、或者没有分类记着清空（包括首次同步的提示）时是 skipped，reason 为 signed_out、disabled 或 not_cloud_cleared，状态不变；会话读不出来是 skipped/session_unavailable。「使用云端」遇到从没上传过（revision 0）或为空的云端、而本机有数据时按合并处理，不清空本机；词库还没同步过时按云端词库现在的条数（GET /v1/users/me/data/summary）判断空不空，不管改动历史里有没有 reset，读不出条数时按不空处理。词库另有一条硬性约束：下载下来要整份替换本机的云端快照一个词都没有（entry 减去标成删除的 overlay）、而本机这一轮读到自己的词时，不管是「使用云端」、首次同步还是之后云端有改动的下载，都不替换，改按合并处理：两边的词都留着，sections.dictionary 照常是 synced，游标推进到快照的修订号，词库再标一次待上传，下一轮把本机的词传回云端（读不出条数时选了「使用云端」、或者在云词库面板里把词删光，都走这一条）。done 时各类的结果在 sections，一类失败不挡其他类，第一条失败记在 status.failure；冲突重读才发现云端被清空、而排在前面的分类这一轮已经写过时，整轮照常是 done 或 pending_commit，那一类在 sections 里是 needs_choice（cloud_cleared），状态里记下清空（status 显示清空的提示），下一轮读完云端再问。设置写回本机时按这一轮读本机时共享偏好的修订号比较并交换：期间另一个进程里用户改了设置时不覆盖它，sections.settings 是 skipped、reason 为 settings_changed，不算失败，游标和基线不动、设置再标一次待上传，下一轮按三方合并（这时云端可能已经写过）。pending_commit 时宿主先把 host_writes 里的值写进自己的存储（ios_native 写进 App Group；feedback（Android 和鸿蒙）、android_local 写回本机；custom_keyboard_skins 按 id 和更新时间合并进皮肤库；adopt_texts 里的示例常用语认领为用户自己的），再以 run_id 调 commit；游标、清标记和基线只在提交时推进，宿主不提交时 180 秒后租约过期，下一轮重新下载，结果相同。
 * {operation:"commit", run_id, host_results?:{saved_native?}} → S：提交 pending_commit 的那一轮。iOS 要带 saved_native（按 host_writes.ios_native 写进 App Group 以后实际保存下来的值），据此把设置文档应用到共享偏好，和 run 写回设置一样按这一轮读本机时共享偏好的修订号比较并交换；那一步失败时 error 是 conflict（共享偏好在这一轮读本机以后被别处改过，包括宿主写 App Group 期间用户在设置页改的）或 storage，这一轮作废，什么都不推进、设置的待上传标记留着，宿主把 App Group 恢复成旧值，下一轮按三方合并。期间有别的写入（sign_out、set_enabled、另一轮）或账号变了时是 stale_run，什么都不推进。
 * {operation:"import_legacy", ios_v1?: iOS 第 1 版的 CloudSync/state.json 对象, android?:{sync_v1: msime_sync_v1 偏好的键值, cloud_sync_status: msime_cloud_sync_status 偏好的键值}} → {imported, status: S}：两者恰好给一个。只在还没有任何进度（没有导入过、用户没有开关过同步、开关关着、没有游标）、而且状态没有绑定账号或绑定的就是旧状态里的账号时导入开关、账号、游标、改动代数、收不下的常用语和几个时间，免得迁移以后又弹首次选择，所以导入之前先调过 status 或 run 也不要紧；导入过一次、调过 set_enabled（打开或关闭）以后不再导入，退出、换账号也不重新开放，宿主可以每次启动都带着旧状态调用，用户关掉的同步不会被旧状态改回打开；在 set_enabled 之前调用。已经有进度、导入已经关上、绑定的是别的账号或读不懂时 imported 为 false。Android 的词库游标要对照 snapshot_queue_directory：旧版本下载词库时先把快照交给激活队列、同时就把游标记成云端修订号，队列里还有这个账号没应用的快照请求（在等激活，或者冲突、失败、取消而结束）时，把它记成等激活的快照，第一轮按等激活或没激活处理（没激活就按合并补上云端的词，不上传）；没带队列目录或读不出队列时不导入词库的游标，词库下一轮按首次同步处理。
 * 合并规则：设置按键三方合并（本机自上次同步以来改过的键用本机的值，其余用云端的），上传前按本轮读到的字段表过滤，字段表外和隐私相关的键既不上传也不写回本机；常用语按正文三方合并，一侧删掉的不会在另一侧复活，两边都有的沿用云端 id，合并后 id 冲突的换新 id，超过 500 条（phrases_over_limit）或请求体超过 256 KiB（phrases_too_large）时这一类失败、两边都不写；词库是整份快照，快照里的词是 entry 减去同一 (kind, code, word) 上标成删除的 overlay（不按 id 匹配：云词库面板改词时新旧两条 overlay 和改后的 entry 共用一个 id），合并时把本机还没有的云端词排进本机导入队列，等键盘应用以后下一轮再整份上传；只有云端的每个词都已经在本机（或本机排着删除、改掉它）时才算合并完，在这之前这一类失败，游标不推进、不上传，首次同步（或云端被清空以后）的选择只留给词库的下一轮（不进 pending_choice，设置和常用语以后遇到云端被清空照样要问），下一轮跳过已经在本机或已在队列里的词接着排：词还在排队、或队列一次排不下（最多 128 个没应用完的请求）时是 dictionary_merge_pending，云端有本机导入不了的词（本机新词的规则不收，或键盘应用时拒绝了）时是 dictionary_merge_rejected，被导入队列里别的失败请求挡住时是 dictionary_merge_blocked；键盘因为暂时拿不到词库而没应用的请求会重排，权重超出新词范围（1 到 100000000）的云端词收进范围后导入。合并没做完、或者键盘还没发布过本机词库版本（dictionary_unpublished，新装以后还没打开过键盘就打开了同步，下载的快照交不给激活队列）时，选择一样留给词库（dictionary_unpublished 而本机这一轮还没有自己的词时留成 merge：这时两个选择都是下载云端快照，留着 use_cloud 会在以后那一轮拿快照整份替换键盘期间学到的词），词库这一类还要退避；本机这一侧读写词库失败（dictionary_storage、dictionary_invalid 这类，不是联网失败）时也一样退避，只是选择不留。退避第一次 5 分钟，之后每多一轮没有进展就翻倍，最多 6 小时；这一轮往导入队列里排进了新的词、或者云端快照里已经在本机的词比上一次记下的多（键盘应用了排着的词）时从 5 分钟重新数，只重排了键盘因为暂时拿不到词库而没应用的请求不算进展；退避期间 active 跳过词库（sections 里词库是 skipped、reason 为 dictionary_backoff，这一轮不算同步成功，status.failure 留着上次的原因），词库的待上传标记也不把节流缩到 30 秒，manual、enable 和带 choice 的不等退避，状态里记着另一轮在跑时做的 pending_choice 时这一轮词库也不等退避（选择在这一轮用掉，词库跳过就会丢掉它；它仍不让这一轮跳过节流）。导入队列没清空（个人词库导入队列里还有没应用的请求，或者具名词库集合的待发送队列里还有没送进去的增删，例如 Android 本地备份恢复排下的大批词）、或下载的云端快照还在激活队列里等键盘激活时推迟上传（dictionary_pending），等激活期间词库这一类也不下载、不合并；下载的快照没有激活就结束了（冲突、失败或取消）时，下一轮改按合并处理（本机没有词时重新下载），不上传。
 * 错误：请求不合法是 cloud_sync_invalid，状态文件读写失败是 cloud_sync_storage，会话读不出来是 account_* 码。会联网（每个请求最长约 30 秒，词库快照最长 10 分钟），在工作线程上调用；不要在主线程上调用。 */
char *msime_client_cloud_sync(const uint8_t *request, size_t length);
char *msime_client_destroy(uint64_t session);
/* Write the selection counts held by every session on the calling thread and all queued personal-context learning, without ending any session. Call from the host's will-terminate hook (e.g. NSApplicationWillTerminateNotification) on the thread that owns the sessions; the C++ Engine did this from atexit. Returns null on success. */
char *msime_client_flush_all(void);
/* value must be NULL or a still-owned pointer returned by this library. */
void msime_client_string_free(char *value);

#ifdef __cplusplus
}
#endif
#endif
