#ifndef MSIME_CLIENT_H
#define MSIME_CLIENT_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

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
 */
char *msime_client_prepare_host(const uint8_t *options, size_t length);
/* path is an absolute UTF-8 runtime options file path of length bytes; maximum 4096. When its dictionaries directory is not the installed resource generation (after a package upgrade), prepares that generation, replays the user dictionary into it and atomically rewrites resources/dictionaries, keeping every other key. It also keeps language_dictionaries in step with the Cantonese, Zhuyin and Stroke dictionaries installed beside the resources, whatever the generation; only an input method host may call it, because a host older than this library rejects that key. Value is true when the file was rewritten. Call before creating any session from the file. When the recorded resource directory does not match the compiled dictionary lock (downloaded dictionaries an upgrade did not replace) the error text begins with "dictionary_outdated:" and the file is left unchanged; the rest of that text may name private paths. */
char *msime_client_refresh_host(const uint8_t *path, size_t length);
/* directory is an absolute UTF-8 directory path of length bytes; maximum 4096. Registers the device's anonymous MSIME account at https://api.msime.app unless anonymous-session.json already exists there, keeping the identity in anonymous-account.json and the session in anonymous-session.json (both owner-only). Blocks on the network: call off the input thread. Value is true once a session exists. A failure leaves the identity for the next call, so call again on a later start. */
char *msime_client_ensure_anonymous_account(const uint8_t *directory, size_t length);
/* 输入法进程取账号服务的访问令牌。request 是 length 字节的 UTF-8 JSON {"directory": 绝对路径, "rejected_token": 可选}，最多 16384 字节；directory 是设置应用和输入法共用的账号目录（Windows 是 %LOCALAPPDATA%\<本版本的用户目录>\account，与 msime_client_ensure_anonymous_account 相同）。设置应用登录的 account-session.json 优先，刷新时与设置应用共用 account-refresh.lock；没有登录时用 anonymous-session.json。rejected_token 是服务端刚以 401 拒绝的令牌，传入后强制刷新。每次调用都重新读文件，登录、退出和换号下一次调用即生效。Value {access_token, user_id, anonymous}；没有任何会话时 error 是 account_unauthorized。响应含凭据，不要写进日志。刷新会阻塞网络：不要在输入线程调用。 */
char *msime_client_account_access_token(const uint8_t *request, size_t length);
/* 设置窗口自己的云剪贴板请求，与云剪贴板面板发的相同。request 是 length 字节（最多 131072）的 UTF-8 JSON {directory: 账号目录的绝对路径, action}；directory 是共享应用登录时写 account-session.json 的目录（Windows 是 %LOCALAPPDATA%\<本版本的用户目录>\account），只用登录的会话，不退回匿名账号。action 是 {operation:"list",search} → {enabled, items:[{id,text,updated_at}]}、{operation:"add",text}（非空、不超过 4000 个 UTF-16 单元、不含控制字符，换行除外）→ 新条目、{operation:"delete",id} → {} 或 {operation:"set_enabled",enabled} → {enabled}。action 不合法时 error 是 invalid_cloud_clipboard；没有登录或会话已失效时是 account_unauthorized，其他失败是相应的 account_* 代码。刷新令牌时与共享应用共用 account-refresh.lock。阻塞网络：放在工作线程上调用。 */
char *msime_client_account_cloud_clipboard(const uint8_t *request, size_t length);
/* 云剪贴板该走哪条路。directory 是 length 字节（最多 16384）的共享偏好目录绝对路径。偏好里的 cloud_clipboard_relay 配置了中继时 Value 是 {source:"relay"}，否则是 {source:"account"}；配置了中继就只走中继，不再用水杉账号。偏好读不了时 error 是 storage，中继配置不合规时是 relay_not_configured。不联网。 */
char *msime_client_cloud_clipboard_source(const uint8_t *directory, size_t length);
/* 经自建中继（协议见 docs/cloud-clipboard-relay.md）答一个云剪贴板动作。request 是 length 字节（最多 131072）的 UTF-8 JSON {preferences_directory: 共享偏好目录的绝对路径, directory: 本机放中继密钥串 cloud-clipboard-relay-keystore 的绝对路径目录, device_name: 可选，登记设备时给其他设备看的名称, action}。中继地址和令牌从偏好的 cloud_clipboard_relay 读，没有配置时 error 是 relay_not_configured。directory 在同一台设备上的输入法、设置和面板之间要相同，否则会登记成几台设备；密钥串退出水杉账号时不删除。action 与 msime_client_account_cloud_clipboard 的相同，结果形状也相同（list、add、delete、set_enabled，以及端到端加密的 devices、approve、reject、remove、reset），另外只在这里接受 {operation:"pin",id,pinned} → {}、{operation:"set_retention",days: 0|1|7|30} → {retention_days} 和 {operation:"clear"} → {}。action 不合法时 error 是 invalid_cloud_clipboard；中继拒绝令牌时是 relay_unauthorized，连不上是 relay_unavailable，密钥串读写失败是 relay_storage，其余是驱动给出的码（例如 clipboard_device_pending）。阻塞网络：放在工作线程上调用。 */
char *msime_client_relay_cloud_clipboard(const uint8_t *request, size_t length);
/* 向自建中继发端到端加密驱动（msime_client_cloud_clipboard_e2e）给出的一个请求，给自己跑驱动、自己保存密钥串的宿主用：Android 把中继的密钥串交给 AndroidKeyStore 加密，不经上面那个由 Rust 把密钥串存成明文文件的入口跑驱动。request 是 length 字节（最多 131072）的 UTF-8 JSON {url: 中继基地址, token: 中继令牌, method: GET|PUT|POST|DELETE, path: 驱动给的路径（必须在 /v1/users/me/clipboard 之下）, body: 可选，驱动给的 JSON 正文文本}。url 和 token 是宿主在这次操作开始时从偏好的 cloud_clipboard_relay 读到的那一份，整次操作都传同一份，中途改了偏好也不会发到另一个中继；校验规则与偏好相同，不合规时 error 是 relay_not_configured。Value 是 {status, body}，状态码和正文原样交回驱动。不读偏好、不读写任何密钥串。请求不合规时 error 是 invalid_cloud_clipboard（不发出去），中继拒绝令牌时是 relay_unauthorized，连不上或响应过大是 relay_unavailable。阻塞网络：放在工作线程上调用。 */
char *msime_client_relay_cloud_clipboard_exchange(const uint8_t *request, size_t length);
/* Anonymous usage reporting to https://api.msime.app/v1/telemetry/events (no credentials). Every request is UTF-8 JSON of length bytes; maximum 16 KiB (256 KiB for record_crash). directory is an absolute directory the host owns for its telemetry files (the queue telemetry.json, telemetry-state.json with the random install id, the session marker and telemetry-crashes/). platform is windows, macos, linux, android, ios or harmony (aliases such as win, darwin, ohos are mapped); version is the real app version. Consent: enabled (the usage_reporting switch, default on) or, for hosts on the shared preferences, preferences_directory to read usage_reporting from; with reporting off, begin and flush clear everything and send nothing.
 * begin {directory, platform, version, enabled?|preferences_directory?}: call once at host start. Closes the previous session (session_crash only when it left a crash record; a leftover marker alone is no crash), queues crash records as crash events with paths reduced to file names, queues today's active, writes a new marker. Value {enabled, crash_record_path?, previous_session_crashed?, crashes?}. No network.
 * end {directory}: the host is exiting normally; queues the session event. Value true when a session was running. No network.
 * record_crash {directory, message, stack}: from a crash handler that may allocate (C++ terminate handler). Writes the session's crash record only; first line of message is the summary, stack is frames as module+offset or symbol. Value false when no session runs or a record exists. An async-signal handler instead writes crash_record_path from begin itself: open(O_WRONLY|O_CREAT|O_EXCL, 0600), the summary line, '\n', the frames.
 * flush (same request as begin): queues today's active and sends the queue oldest first; 400 drops an event, 429/5xx/network keep it (Retry-After honoured across processes). Value {enabled, sent, dropped, remaining, deferred}. Blocks on the network: background thread only.
 * clear {directory}: usage reporting was just turned off. */
char *msime_client_telemetry_begin(const uint8_t *request, size_t length);
char *msime_client_telemetry_end(const uint8_t *request, size_t length);
char *msime_client_telemetry_record_crash(const uint8_t *request, size_t length);
char *msime_client_telemetry_flush(const uint8_t *request, size_t length);
char *msime_client_telemetry_clear(const uint8_t *request, size_t length);
/* Notices from GET https://api.msime.app/v1/notices (no credentials). request {directory, platform, channel?} (channel app by default); directory is an absolute directory for notices.json. Value {items:[{id,title,body,html,targets,channels,published_at}]}, newest first, without the ones dismissed; html is body rendered as by msime_client_markdown_to_html. The feed is fetched at most once a minute (the cached copy otherwise, and when the request fails). Blocks on the network: call off the input thread when the settings window or app home opens, never on a timer in the input method. */
char *msime_client_notices(const uint8_t *request, size_t length);
/* request {directory, id}: remember that the user dismissed notice id. */
char *msime_client_notice_dismiss(const uint8_t *request, size_t length);
/* 对照 github.com/metasequoiaime/msime 的发布列表检查更新（不带凭据）。request 为 {platform, current_version, edition?, arch?}：platform 是发布 tag 前缀（windows、linux、macos、harmony 等），current_version 是当前运行的版本号，edition 是版本 id（缺省为 full），arch 是 Rust 的架构名。返回值为 {status:"available"|"current", update:{version:{display,parts}, release_url, installer_name, installer_sha256, signed}}，该平台还没有发布时为 {status:"none"}。在 linux 和 windows 上 installer_name 和 installer_sha256 是本版本唯一的安装包（没有、有不止一个、或 GitHub 没给摘要时为 null）；signed 在这两个平台为 false，其他平台为 null。会在网络上阻塞最多十秒，不要在 UI 线程或输入线程上调用。 */
char *msime_client_update_check(const uint8_t *request, size_t length);
/* text is UTF-8 Markdown of length bytes; maximum 256 KiB. Value is HTML for a rich-text view: raw HTML in the source is escaped, only http, https and mailto links are kept, images become links and are never loaded. Open links externally. */
char *msime_client_markdown_to_html(const uint8_t *text, size_t length);
/* Community moderation for hosts that send community requests through their own HTTP stack. Request is UTF-8 JSON of length bytes; maximum 64 KiB. Publishing is post-moderated: an item is public at once and moderators may remove it. To learn the state of the user's own items add fields=moderation to scope=mine lists and to the detail request (skins, candidate-skins, plugins, resources); each own item then carries moderation "approved"|"pending"|"removed". Show only a 已下架 badge for removed and treat pending as approved; never show a reason. Without fields=moderation the responses are unchanged.
 * {operation:"reasons"}: value is the report reasons in dialog order, each the exact string to send: ["侵权/抄袭","色情低俗","违法违规","垃圾广告","恶意插件","其他"].
 * {operation:"report",kind,item_id,reason,detail?}: kind is skins, candidate-skins, plugins, dictionaries or replies; item_id the item's UUID; reason one of the reasons; detail optional, at most 1000 characters. Value {method:"POST",path:"/v1/community/reports",body}: send body as JSON with the signed-in session's bearer (the device's anonymous account counts). A bad field is error community_invalid.
 * {operation:"error",status,body?}: status and body of a failed community response. Value {code,message,retry}: code is account_blocked_content (422 blocked_content: the text must change; never say the service is down), account_screening_unavailable (503 screening_unavailable: honour Retry-After), account_banned (403 account_banned) or the generic account_* code; message is the Chinese sentence to show for the first three and null otherwise; retry says whether the same request may be sent again later. */
char *msime_client_community_moderation(const uint8_t *request, size_t length);
/* Google 登录：由宿主自己监听回环端口、打开系统浏览器、经自己的 HTTPS 栈申请 challenge 并提交授权码（鸿蒙）。这里给出与桌面端相同的判定。Request 是 UTF-8 JSON，最多 32 KiB。
 * {operation:"target",port}: port 是宿主在 127.0.0.1 上监听的端口（>=1024）。Value {target}: 向 POST /v1/auth/challenges 发 {provider:"google",target,purpose:"login"} 时用的回跳地址。
 * {operation:"plan",authorization_url,target,expires_in}: 后端返回的 authorization_url 与 expires_in。Value {state,wait_ms,max_request_bytes,io_timeout_ms}: 链接通过校验，可以用系统浏览器打开；最多等 wait_ms 毫秒；每个连接最多读 max_request_bytes 字节、最多 io_timeout_ms 毫秒拿到请求头，超出就以 head:null 回复并关掉。链接不是指向这个监听的 Google 授权链接、或 challenge 太短时返回 account_unavailable，不要打开它。
 * {operation:"reply",head,state,return_link?}: head 是一个连接读到的请求头（到第一个空行为止，含），没读到完整请求头时为 null；state 是 plan 给的；return_link 可省略，是宿主注册的、能把应用拉回前台的 `scheme://路径` 链接（如鸿蒙的 msime://auth/google），回跳页据此画「回到水杉输入法」按钮，形状不合格时不画，回复照常。Value {outcome,code?,error?,response}: 把 response 原样写回这个连接再关闭。outcome 为 ignored 时继续等下一个连接；code 时用 POST /v1/auth/login {challenge_id,credential:code} 完成登录；failed 时结束登录，error 是 account_cancelled（用户在 Google 页面拒绝）或 account_unavailable。 */
char *msime_client_google_loopback(const uint8_t *request, size_t length);
/* 原生设置窗口的账号操作，与共享应用（Tauri）的十四个 account_* 命令和 Windows 的四个 account_preferences_* 设置同步命令相同，用同一份本用户会话文件。Request 是 length 字节（最多 4 MiB）的 UTF-8 JSON {directory, legacy_directory?, action}：directory 是共享应用登录时写 account-session.json 的目录的绝对路径（Windows 是 %LOCALAPPDATA%\<本版本的用户目录>\account，与 msime_client_account_access_token 相同）；legacy_directory 可选，是旧版本放会话的目录（Windows 是 %LOCALAPPDATA%\<tauri identifier>），本进程第一次用到 directory 时，directory 还没有会话而这里有，就把它搬过去。每个 directory 在进程里只有一个会话对象，会话文件每次都重新读，刷新令牌时与共享应用和输入法共用 account-refresh.lock。响应不含令牌。
 * action 是下列之一，value 与共享账号页（packages/ui/src/account/account-page.tsx）读到的形状相同；user 是 {id,displayName,createdAt,email?,avatarUrl?,avatarUploaded?}，avatarUrl 只在是本客户端会去取的头像地址时出现，avatarUploaded 只在头像是用户上传的（可以移除）时为 true：
 * {operation:"status"} → {user|null}，只读本地文件。{operation:"providers"} → {email,phone,apple,google}。
 * {operation:"request_code",provider:"email"|"phone",target} → {challengeId,expiresIn}。{operation:"login",challenge_id,code}（code 是 6 位数字）→ {user}。
 * {operation:"google_login"} → {user}：在 127.0.0.1 上监听回环端口、申请 Google challenge、用系统浏览器打开校验过的授权链接（只在 Windows 上；其他平台是 account_unavailable），等浏览器回跳后登录。最多阻塞约 5 分钟；用户在 Google 页面拒绝或调用了 google_cancel 时是 account_cancelled。{operation:"google_cancel"} → null，立即返回，结束同一 directory 上进行中的 google_login。
 * {operation:"profile"} → {user,providers:[...]}（providers 是已绑定的登录方式，去重）。{operation:"rename",display_name}（非空、首尾无空白、最多 64 个字符）→ 同 profile。
 * {operation:"avatar"} → null 或 "data:<image/png|image/jpeg>;base64,<...>"（未登录或没有头像时为 null）。{operation:"upload_avatar",path}：path 是宿主自己的文件对话框选中的 PNG 或 JPEG 的绝对路径，最多 1 MiB → 同 profile；不是这样的文件时是 account_invalid。{operation:"remove_avatar"} → 同 profile。
 * {operation:"logout",all} → null（all 为 true 时退出所有设备；本地会话总会先清掉）。{operation:"delete"} → null（注销账号，成功后清掉本地会话）。{operation:"forget"} → null（只清掉本地会话，用于会话已失效时）。
 * 设置同步（共享账号页的设置同步卡片）：{operation:"preferences_schema"} → {fields,maximumBytes,updateMode,revisionRequired}。{operation:"preferences_load"} → 云端设置文档 {revision,settings}。{operation:"preferences_upload",preferences_directory}：preferences_directory 是偏好目录的绝对路径；本机各平台共有的 input.* 设置按本目录所属的版本过滤、只留字段表收录的键，合并进云端文档后上传，返回上传后的云端文档；一个键都没有时是 account_unavailable，云端文档已被别处更新时是 account_conflict。{operation:"preferences_apply",preferences_directory,user_id,preferences}：preferences 是刚由 preferences_load 下载的文档，user_id 是下载时的账号（期间退出、换号或重新登录时是 account_cancelled）；按版本过滤后把其中的 input.* 键应用到本机偏好，按读到时的修订号比较并交换写回（关掉剪贴板历史时与 msime_client_save_preferences 一样清掉已存的历史），返回保存后的 PreferencesSnapshot。写回失败时 error 是 conflict（本机设置已在别处更新）或 storage，与共享应用相同。
 * 其余失败时 error 是账号错误码：account_invalid（请求或参数不合法）、account_unauthorized（未登录或会话失效）、account_rate_limited、account_storage、account_cancelled、account_conflict、account_unavailable、account_blocked_content、account_screening_unavailable、account_banned。除 status、google_cancel、forget 外都会阻塞网络：放在工作线程上调用。 */
char *msime_client_account(const uint8_t *request, size_t length);
/* 原生设置窗口的社区操作，与共享应用（Tauri）在 Windows 上的社区命令相同：候选窗皮肤社区（candidate_skin_community_* 十四个）、插件社区（plugin_community_* 七个）、社区词包与回复模板（community_resource_* 七个）和举报（community_report）。经 msime_client_account 里同一个账号会话对象认证（同一份 account-session.json，刷新时共用 account-refresh.lock）。Request 是 length 字节（最多 4 MiB）的 UTF-8 JSON {directory, legacy_directory?, skins_directory?, state_root?, action}：directory 与 legacy_directory 同 msime_client_account；skins_directory 是候选窗皮肤根目录的绝对路径（Windows 是 <状态目录>\skins，与共享应用相同），下面标了「要 skins_directory」的操作必需；state_root 是放 plugins\ 的状态根目录的绝对路径（与 msime_client_plugins 的 state_root 相同），标了「要 state_root」的操作必需。id 都是 UUID 字符串；发布时的 id 由宿主随机生成，同一次发布重试时沿用，服务端据此去重。
 * 候选窗皮肤：{operation:"candidate_skin_list",offset,search,mine,category?} → {skins:[...],has_more}（category 不给时列出全部分类；mine 要登录）。{operation:"candidate_skin_detail",id} → 一项。{operation:"candidate_skin_preview",id} → {dataUrl}（PNG 或 JPEG 的 data: URL，类型和文件头已校验）。{operation:"candidate_skin_install",id,replace}（要 skins_directory）→ 下载、装进皮肤根目录并记下来源（同步不会把别人的作品传进自己的库），返回重新扫描的 {directory,packages,issues}；replace 为 false 而同名文件夹已存在时 error 是 candidate_skin_exists，先问用户再以 true 重试。{operation:"candidate_skin_pack_preview",skin_id,visibility?:"public"|"private"}（要 skins_directory，visibility 缺省 public；私有的可以没有素材许可）→ {suggestedName,license:{code,assets,source},fileCount,size}，不上传。{operation:"candidate_skin_add_preview",skin_id,image}（要 skins_directory）：image 是宿主为没有预览图的已装皮肤画好的 PNG 或 JPEG（最多 256 KiB）的标准 base64，写进包并加进清单 → 重新扫描的目录。{operation:"candidate_skin_add_license",skin_id,assets}（要 skins_directory）：把用户选的素材许可写进清单 → 重新扫描的目录。{operation:"candidate_skin_publish",skin_id,id,name,description,visibility?,category?}（要 skins_directory）→ 发布后的一项；同步已放进库里的同一个包就地更新，不会留下第二行。{operation:"candidate_skin_rate",id,stars:1..5} → {stars}。{operation:"candidate_skin_set_visibility",id,visibility} / {operation:"candidate_skin_set_category",id,category} → 更新后的一项。{operation:"candidate_skin_unpublish",id}（要 skins_directory）→ {deleted:true}，本地文件夹保留，同步状态里忘掉这一行。{operation:"candidate_skin_sync"}（要 skins_directory）→ {uploaded,downloaded,deleted_local,deleted_cloud,skipped:[{package_id,code}],stopped}；downloaded 或 deleted_local 非空时皮肤根目录变了，宿主用 msime_client_skin_catalog 重新扫描。未登录时 sync 在读本地会话这一步就以 community_unauthorized 结束，不发请求。
 * 插件：{operation:"plugin_list",offset,search,kind?,mine?} → {plugins:[...],has_more,skipped}。{operation:"plugin_detail",id} → 一项。{operation:"plugin_pack_preview",kind,plugin_id}（要 state_root）→ {suggestedName,suggestedDescription,version,license,fileCount,size}，不上传。{operation:"plugin_publish",kind,plugin_id,id,name,description}（要 state_root）→ 发布后的一项；归档是确定的，同一 id 重试时内容相同。{operation:"plugin_install",id,kind,plugin_id}（要 state_root）：kind 与 plugin_id 是列表里那一项的，下载到的若是别的包就拒绝；整个替换已装的同 kind 同 id 的包（先问用户）→ 装好的 PluginSummary，与 msime_client_plugins 的 catalog 里一项相同。{operation:"plugin_rate",id,stars} → {stars}。{operation:"plugin_delete",id} → {deleted:true}，已装的包保留。
 * 词包与回复模板：{operation:"resource_list",kind:"dictionary"|"reply"|"phrase",scope:""|"mine"|"saved",search,offset} → {items:[...],has_more}。{operation:"resource_detail",id} → 一项。{operation:"resource_publish",id,kind,name,description,content,revision} → {id,revision}。{operation:"resource_apply",id,resource_revision} → {revision,imported,resource_revision}（把词包导入账号云端词库）。{operation:"resource_save",id,saved}、{operation:"resource_rate",id,stars}、{operation:"resource_unpublish",id} → null。
 * 举报：{operation:"report",kind:"skins"|"candidate-skins"|"plugins"|"dictionaries"|"replies"|"phrases",id,reason,detail?} → null；reason 是 msime_client_community_moderation 的 reasons 之一，detail 最多 1000 个字符。
 * 失败时 error 是社区错误码：community_invalid（请求或参数不合法、缺了要的目录）、community_unauthorized、community_forbidden、community_not_found、community_rate_limited、community_cancelled、community_storage、community_conflict、community_unavailable、community_blocked_content、community_screening_unavailable、community_account_banned；本机打包、安装、补图、补许可的规则失败是 candidate_skin_* 或 storage（皮肤），或插件的码（插件），插件失败时可能另有 detail，是违反的那条规则的中文说明。除 pack_preview、add_preview、add_license 外都会阻塞网络；全部放在工作线程上调用。 */
char *msime_client_community(const uint8_t *request, size_t length);
/* options is a readable UTF-8 buffer of length bytes; maximum 1 MiB.
 * Object: api_version=1, resources/user_data/cache/dictionaries (absolute paths),
 * preferences={scheme, candidate_page_size, learning, chinese_punctuation,
 *              shuangpin_profile?, shuangpin_custom_profile?, wubi_profile?}。缺省 shuangpin_profile 为 xiaohe，
 * 可选 xiaohe、ziranma、shoudao、microsoft、custom。custom 读 shuangpin_custom_profile（{initials, finals, zero_initials}，各是单位到键的对象），表缺失或不合法时按 xiaohe 建会话。缺省 wubi_profile 为 wubi86，可选 wubi86、wubi98。不认识的取值一律拒绝。
 * Optional preferences_directory is bootstrap metadata for host file monitoring;
 * session creation itself does not monitor or load it.
 * Optional phrase_preedit=true asks for a half-composed phrase to stay in the
 * composition: picking a candidate that covers only part of the input leaves the
 * chosen piece in view.phrase_prefix instead of committing it, and the whole
 * phrase commits at once when the composition ends. The host must draw that
 * field ahead of editing_text - it is separate because caret_position is an
 * offset into editing_text in the host's own string unit. Omitted means each
 * piece is committed as it is picked, as before.
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
 * Do not delete .msime-dictionary-access.lock files. Legacy/external writers do
 * not participate; preparation/upgrades still require stopped sessions.
 */
char *msime_client_create(const uint8_t *options, size_t length);
/* Management JSON (<=2248576 bytes), trusted native caller only:
 * {options: <same HostOptions as create>, action: {operation:"list",offset:0,limit:100}}
 * List takes optional kind and query (a code prefix). Without them it lists the user's own words; with a kind and a nonblank query (quick_phrase needs none) it also finds the bundled words of that dictionary, user words first. user_only:true keeps any list to the user's own words, filtered by the kind and code prefix across the whole store.
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
/* A dictionary file ({kind, format, text}, as the "import" dictionary action) answered as the words the personal dictionary queue accepts, with the import report: {entries:[Entry], applied, failed, truncated, swapped, first_failures}. Invalid rows are counted with their line, repeated words appear once, and at most 128 words are returned. Reads only the packaged main dictionary under resources; changes no dictionary state. Maximum 1,200,000 bytes. */
char *msime_client_dictionary_import_entries(const uint8_t *request, size_t request_length,
                                             const uint8_t *resources, size_t resources_length);
/* Android personal-dictionary queue synchronization. The request contains the
 * same HostOptions object as msime_client_create. The caller must have no
 * Engine session using its user_data/dictionaries paths. */
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
 * absolute preferences directory. Response value: {provider:{...}|null, polish:{...}|null}; both
 * absent means nothing is configured and the host uses whatever it falls back to. Contains
 * credentials: never log the response; release with msime_client_string_free. provider.requestFormat 是请求格式："multipart"（OpenAI 兼容的 /audio/transcriptions）、"chat_audio"（Chat Completions 带 input_audio，阿里云百炼）、"doubao_websocket" 或 "local"；宿主按它挑请求构造，不按 provider 名字判断。 */
char *msime_client_mobile_voice_configuration(const uint8_t *directory, size_t length);
/* 全局主题选择器：{themes:[{id,title,platforms,appearance,preview,candidate,keyboard},...],default:"system"}。只列出本库编译到的平台提供的主题，按选择器顺序：system, native（仅 iOS）, shuishan, light, paper, night, ink, custom。platforms 为提供该主题的宿主（如 ["ios"]），null 表示所有宿主。appearance is "light"|"dark"|null; preview {background,panel,accent,text}, candidate and keyboard are the built-in palettes and are null for system, native and custom. Keys are snake_case. Hosts keep no copy of the ids, titles or colours. */
char *msime_client_theme_catalog(void);
/* 解析所选全局主题的颜色。JSON 请求（<=1048576 字节，拒绝未知键）：{global_theme:string, custom_theme?:Preferences.custom_theme, dark:bool, layout:"horizontal"|"vertical", skins_directory?:皮肤根目录的绝对路径 | package?:candidate_skin_catalog 中的一项}。global_theme 必须是八个主题 id 之一（system、native、shuishan、light、paper、night、ink、custom），其他 id 请求失败；native 不在本平台目录里时也接受，解析结果与 system 相同，只是 id 为 native。custom_theme 按偏好同样校验。dark 是宿主当前的明暗模式，它决定自定义主题取哪个槽位：深色模式取 custom_theme.candidate_skin_dark，没设时取 custom_theme.candidate_skin，浅色模式取 custom_theme.candidate_skin。皮肤包只在它清单 base 的明暗下绘制：base 是固定明暗的内置主题（shuishan、night、ink 为深色，light、paper 为浅色）且与 dark 不符时不画，base 为 system 的包两种模式都画；设过皮肤却在这个模式下没有可画的包时，custom_theme.base 也只在属于当前明暗时作底，否则按 system。layout 是正在绘制的候选窗：皮肤包只在清单声明的排列和模式下绘制，只有画了它时 candidate_skin 才非空，所以宿主正好在 candidate_skin 非空时画包的装饰和最小宽度。skins_directory 给扫描皮肤根目录的宿主（除 Linux 外都是）；package 是 Linux candidate_skin_catalog 中当前模式槽位指名的那一项，其他任何东西（包括 msime_client_skin_catalog 的 SkinSummary）都让请求失败。响应 value：{id,source:"system"|"builtin"|"custom",appearance:"light"|"dark"|null, candidate:{surface,border,text,number,secondary,accent,selected,selected_text,selected_number,hover,show_selected_bar}|null, keyboard:{background,key,function_key,text,secondary,accent,on_accent}|null, candidate_skin:string|null}。appearance 非空时是返回的界面所处的明暗。keyboard.accent 是触屏候选条选中候选的文字色（无填充）；回车键保留平台强调色。on_accent 为黑或白，能在强调色填充上读清。每个颜色都是 #RRGGBB 或 #RRGGBBAA。为 null 的调色板或槽位表示用宿主自己的原生 token，从不表示透明。根目录里没有、磁盘上无效、或不是当前槽位指名的包会被略过而不是让调用失败。skins_directory 会读取皮肤包：在主题、明暗或皮肤包变化时解析，不要在绘制时调用。 */
char *msime_client_resolve_theme(const uint8_t *request, size_t length);
/* 应用主题（Android 本地设置里的 app_theme，不在共享偏好里）选择器：{app_themes:[{id,title,season,seasonal,light:{accent,accent_soft,on_accent,background,card,hair},dark:{...同上}}],default:"siji"}。id 按选择器顺序为 siji、chunya、xiayin、qiushan、dongxue；season 是固定的那一季（"spring"|"summer"|"autumn"|"winter"），siji 为 null 且 seasonal 为 true，它的颜色固定画秋杉，目录因此不随时钟变化。颜色为 #RRGGBB 或 #RRGGBBAA；accent_soft 是强调色加 22（浅色）或 40（深色）透明度，浅色 on_accent 为 #FFFFFF，深色 on_accent 是强调色与黑色 25% 比 75% 的混合。纯计算。 */
char *msime_client_app_theme_catalog(void);
/* 应用主题在宿主当前月份与明暗模式下的颜色。JSON 请求（<=4096 字节，拒绝未知键）{app_theme:"siji"|"chunya"|"xiayin"|"qiushan"|"dongxue", month?:1-12, dark:bool}；month 是宿主本地日历的月份，只影响 siji，省略时用 UTC 月份，超出 1-12 时请求失败。返回 {id, season, accent, accent_soft, on_accent, background, card, hair}，season 是实际画的那一季。纯计算，可在主线程调用。 */
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
/* Load PreferencesStore from an absolute UTF-8 directory, without a session.
 * May block on disk/file lock: use a worker thread. Returns PreferencesSnapshot.
 * Missing file returns shared defaults; malformed/future files return errors.
 * Creates the directory/lock file if absent, never overwrites preference contents.
 */
char *msime_client_load_preferences(const uint8_t *directory, size_t length);
/* Private aggregate typing statistics. JSON request (<=65536 bytes):
 * {directory:absolute path,action:{operation:"load"|"reset"}}
 * {directory,action:{operation:"set_enabled",enabled:bool}}
 * {directory,action:{operation:"set_retention",retention,day:"YYYY-MM-DD"}}
 *   retention is forever|30d|90d|180d|365d; anything else is read as forever,
 *   never as a shorter window. `day` is the caller's local day.
 * {directory,action:{operation:"record",text,source,day:"YYYY-MM-DD",hour?:0-23}}.
 * Record classifies committed text in memory and persists only aggregate counts;
 * text is never returned or stored. May block on disk/file lock: use a worker.
 * `hour` is the commit's local hour and must come from the same instant as `day`;
 * omit it rather than guess, and the day keeps its counts with no hourly split.
 * Record answers {recorded, milestone}: milestone is the achievement count (100, 1000, ...)
 * the total just passed, or null. It is only computed while a session in this process has
 * achievement sounds on, and on the desktop hosts the jingle is then already queued.
 * {directory,action:{operation:"record_keys",day:"YYYY-MM-DD",keys:{"KeyA":3,...}}}
 *   adds per-key press counts to `day`, the local day the presses happened on
 *   (flush a batch that crossed midnight under the old day first). Only each
 *   key's daily press count is stored: no order, timing or text. Key ids are
 *   W3C KeyboardEvent.code names plus soft-keyboard ids (Nine0-Nine9,
 *   SoftPunctuation, SoftSymbol, SoftLayer, SoftLanguage, SoftGlobe, SoftEmoji,
 *   SoftVoice); the full list is KEY_IDS in client-core typing_statistics.rs.
 *   An unknown id or a zero count rejects the whole batch; never invent ids.
 *   Returns {recorded:n}, 0 when statistics are off (nothing is written).
 *   Hosts batch in memory and call this from a worker, never per key.
 * {directory,action:{operation:"summary",day:"YYYY-MM-DD",user_words?:n}} 返回调用方本地 `day` 的派生指标（总览、习惯、按键、成就）；统计开着时，新解锁的成就在锁内写下。user_words 是词库 "count" 加 user_only:true 的结果。隐私模式不影响它：它只读。
 * {directory,action:{operation:"record_voice",day,milliseconds}} 记一次语音输入的时长；{directory,action:{operation:"record_skin",id}} 记一次用到的皮肤。两者都返回 {recorded}。
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
 * {directory,resources,day,action:{operation:"answer",word,known}}
 *   known is the 认识 button; false is 不认识 and returns the card to the same day.
 * {directory,resources,day,action:{operation:"set_settings",wordbook,new_per_day,session_limit}}
 * {directory,resources,day,action:{operation:"import",name,text}}
 *   text is a CSV/TXT word list; the library mints the id and selects the book.
 * {directory,resources,day,action:{operation:"remove",wordbook}}
 * {directory,resources,day,action:{operation:"reset"}} clears progress, keeps the books.
 * 可选的 plugins 是插件目录的绝对路径：其中的单词本插件按 pack-<插件 id> 列在内置书之后、导入的书之前，带 pack:true；对它们的 remove 失败（在插件页卸载），卸载后复习进度保留。不传 plugins 的宿主只有内置和导入的书。
 * `day` is the caller's local day and is required by every action: the counts and
 * the queue are per-day and this layer cannot resolve the host's timezone.
 * Requests may be up to 8 MiB rather than the usual 64 KiB, because an imported
 * word list is a few hundred kilobytes of text. Takes a file lock and reads the
 * library: call on a worker, never on the input path. */
char *msime_client_vocabulary_review(const uint8_t *request, size_t length);
/* Scan an absolute UTF-8 skin root and return the catalog the settings page sees: {packages:[...],issues:[...]}. Reads the directory: use a worker. An unreadable root is an empty catalog; an invalid package becomes an issue and is never returned as renderable. Presenters must still check that a package supports the layout and theme before adopting its colors. Keys are camelCase, the same document the settings page consumes. Besides the colours (drawn through msime_client_resolve_theme, where a palette's translation becomes secondary) a package carries what a presenter draws itself: minWidthDip; cornerRadiusDip (0-32, null keeps the host radius); decorationTopDip, decorationWidthDip and decorationImage (package-relative, null unless decorated) placed by decorationAlign ("left"|"center"|"right"); background null or {image,fit:"cover"|"contain"|"stretch",opacity:0-1}, drawn over the surface and under the candidates, clipped to the card outline; toolbar {cornerRadiusDip|null, light, dark} where each mode is {background,border,handle,divider,icon,hover}, each #RRGGBB, #RRGGBBAA or null for the host's own. Image paths are already confined to the package and are images; read them with msime_client_skin_resource. */
char *msime_client_skin_catalog(const uint8_t *directory, size_t length);
/* Scan an absolute Engine resource directory for optional custom helper-code tables. The
 * response value is an array of {schema,file_stem,name,name_en}; missing or unreadable
 * helpcodes/custom is an empty array. Display metadata comes from leading # name: and
 * # name_en: comments, while the Engine remains the owner of table parsing. */
char *msime_client_helpcode_schemas(const uint8_t *resources, size_t length);
/* JSON {directory:absolute skin root,id:package folder}. Validates that one package with the same loader as msime_client_skin_catalog and returns one of its camelCase packages entries; an invalid, built-in, symlinked or missing package is {ok:false,error} with the loader's reason. Reads the package: resolve on a skin or appearance change, never while drawing. */
char *msime_client_skin_package(const uint8_t *request, size_t length);
/* JSON {directory:absolute path,id:skin id,relative:package asset,kind:"image"|"font"}.
 * Returns {contentType,bytes}; the manifest and package containment are
 * revalidated for every call and the requested kind must match the asset. */
char *msime_client_skin_resource(const uint8_t *request, size_t length);
/* JSON {directory:absolute path,id:skin id}; returns a nullable stylesheet
 * string from the manifest, after revalidating the package and path. */
char *msime_client_skin_toolbar_stylesheet(const uint8_t *request, size_t length);
/* JSON {source:absolute picked folder,directory:absolute skin root}. Copies the
 * folder under its own name, replacing a skin of that name whole; returns {id}.
 * The error message is skin_name, skin_manifest or storage. Touches the disk. */
char *msime_client_skin_import(const uint8_t *request, size_t length);
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
/* 移动端的结构化剪贴板历史，与桌面的自动记录偏好无关。请求 JSON 为 {directory: App Group 根的绝对路径, action}，action 是 {operation:"load"|"clear"}、{operation:"capture"|"remove",text}、{operation:"set_pinned",text,pinned} 或 {operation:"replace",text,replacement}。replace 按内容找到原条目原地改字，时间戳和固定状态不变；新文字与另一条相同时两条合并，固定状态取两者之或；返回 replaced、merged 和 reason（"not_found"|"invalid"|null）。历史存在 directory/MSIME/clipboard_history.json。 */
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
/* 在会话线程传入 PreferencesSnapshot JSON（最多 1048576 字节）：{format_version:1, revision, preferences:{...}}。修订号按会话递增；相同快照可重试，旧版或冲突快照会被拒绝。返回 {revision, deferred, view, diagnostic?}；仅首选方案无法运行并回退时带 diagnostic。组字期间的 Engine 偏好应用延后到派发或焦点调用使会话空闲，但关闭云候选会立即清除现有云行和旧查询，AI 配置或令牌变化也会立即清除旧 AI 行和查询；释义开关、目标语言或翻译服务变化会清掉旧释义并推进候选代次，旧释义回包将被拒绝。宿主应使用返回的新 view。较新快照替换待应用快照。构建失败时保留旧会话和待应用快照供重试，派发通过 diagnostic 报告重试错误，不丢失已完成输入。本接口不读写偏好文件，宿主提供已加载的快照。 */
char *msime_client_update_preferences(uint64_t session, const uint8_t *snapshot, size_t length);
char *msime_client_focus(uint64_t session, bool focused);
/* Clear the current session's Engine candidate cache and refresh its view. */
char *msime_client_reset_cache(uint64_t session);
/* Release process-level model caches without a session handle (memory pressure). */
void msime_client_release_process_cache(void);
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
char *msime_client_voice_apply(uint64_t session, uint64_t generation,
                               const uint8_t *text, size_t length);
/* On-device speech models and user-dictionary hotwords. JSON request buffers of length bytes; standard responses. Error text of the model calls is a stable code beginning with "local_model_".
 * voice_hotwords: {options: HostOptions as msime_client_dictionary, limit?: 200} -> {hotwords:[{text,pinyin}]}, the user's own pinyin words (two or more Chinese characters), heaviest first. Worker thread; fails with "dictionary maintenance busy" while maintenance holds the store. <=1 MiB.
 * voice_hotword_correct: {text, hotwords:[{text,pinyin}]} -> {text}. Pinyin-similarity replacement for models whose msime-model.json has "hotwords":"pinyin". Pure. The transcript is limited to 10000 Unicode scalar values, with at most 1000 hotwords; each hotword text is <=256 bytes and pinyin <=1024 bytes. The JSON request is <=1048576 bytes.
 * voice_local_models: {root: absolute dir} -> {models:[{id,title,description,languages,streaming,default,desktop_only,installed,path,installed_size,archive_size,memory,license_spdx,license_source,license_terms,license_notice,hotwords,import_files:[{name,url,size}]}], default: id}. path is <root>/<id>, the value for voice_input.asr_model_path. import_files 是不联网安装时要用户自己下载的文件（压缩包和带地址的附加文件），url 是上游下载地址。
 * voice_local_model_install: {root, id, mirror?: "https://..." prefix, files?: [absolute path]} -> {path}. Blocks for the whole download: worker thread only. progress (nullable) gets {id,stage:"download"|"verify"|"extract"|"done",downloaded,total} on the calling thread; copy the buffer before returning. One install per id at a time ("local_model_install_running"). 带 files（至多 16 个）时不联网，用这些本地文件安装：按长度和 SHA-256 认文件、不看文件名，进度阶段 download 报成 "import"，缺文件时失败为 "local_model_import_missing: <name>"，所选文件打不开或读出错时为 "local_model_import_unreadable: <detail>"。
 * voice_local_model_cancel: {id} cancels that install, NULL/0 or {} cancels all; value is whether one was running. Any thread.
 * voice_local_model_remove: {root, id} -> null. Only catalog ids; refused while that id is installing.
 * voice_model_setting_check: {asr_model_mirror?, asr_model_path?} (<=64 KiB) -> {asr_model_mirror?: bool, asr_model_path?: bool}, one key per field asked: whether the preferences store accepts that value (the same checks as saving). Pure. */
typedef void (*msime_client_voice_local_model_progress_callback)(const uint8_t *json, size_t length,
                                                                 void *context);
char *msime_client_voice_hotwords(const uint8_t *request, size_t length);
char *msime_client_voice_hotword_correct(const uint8_t *request, size_t length);
char *msime_client_voice_local_models(const uint8_t *request, size_t length);
char *msime_client_voice_local_model_install(
    const uint8_t *request, size_t length,
    msime_client_voice_local_model_progress_callback progress, void *context);
char *msime_client_voice_local_model_cancel(const uint8_t *request, size_t length);
char *msime_client_voice_local_model_remove(const uint8_t *request, size_t length);
char *msime_client_voice_model_setting_check(const uint8_t *request, size_t length);
/* On-demand resource packs under <state_root>/resource-packs/<id>/, where state_root is the HostOptions preferences_directory (Android: files/bootstrap/state); a session picks up a newly published pack at its next focus. Pack ids: japanese, language-dictionaries, handwriting, settled-model, offline-glosses, voice-runtime. JSON request buffers of length bytes (<=16384); standard responses. Install and adopt block: worker thread only. Errors are "invalid resource pack request", "invalid state root", "invalid source", "resource_pack_unknown", "resource_pack_busy" (that pack is already installing or adopting in this process) or a "local_model_*" code.
 * resource_packs: {state_root} -> [{id, state:"missing"|"installed"|"outdated", size, schemes:[...], import_files:[{name,url,size}]}], one entry per pack; size is the download size in bytes; import_files are the files an offline install needs (the archive for voice-runtime), url being the upstream download.
 * resource_pack_install: {state_root, pack, sources?:["https://mirror/", ...], files?:[absolute path]} -> {path}. Sources are mirror prefixes tried in order (at most 8, empty ones skipped), then the project's mirrors, then the URL in the compiled lock; integrity is always the compiled size and SHA-256. An unfinished file is resumed with an HTTP Range request next time. With files: [absolute path] (at most 16) it installs from those local files instead, matched by size and SHA-256 (never by name), sources ignored, the download phase reported as "import"; a missing file fails with "local_model_import_missing: <name>", an unreadable one with "local_model_import_unreadable: <detail>". progress (nullable) gets (context, phase "download"|"import"|"verify"|"done", done, total) on the calling thread; the phase string is valid only during the call. Cancelled installs fail with "local_model_cancelled".
 * resource_pack_cancel: pack is the raw UTF-8 id (not JSON); NULL/0 cancels every pack install in this process. Any thread; returns at once.
 * resource_pack_adopt: {state_root, pack, source: absolute dir} -> {path}. Renames the pack's files out of source on the same filesystem (no copy), verifies them against the compiled lock and publishes the pack; on failure every file is renamed back and nothing is published. A pack already installed with the same bytes is returned without touching source. */
typedef void (*msime_client_resource_pack_progress_callback)(void *context, const char *phase,
                                                             uint64_t done, uint64_t total);
char *msime_client_resource_packs(const uint8_t *request, size_t length);
char *msime_client_resource_pack_install(const uint8_t *request, size_t length,
                                         msime_client_resource_pack_progress_callback progress,
                                         void *context);
void msime_client_resource_pack_cancel(const uint8_t *pack, size_t length);
char *msime_client_resource_pack_adopt(const uint8_t *request, size_t length);
/* 自定义翻译请求描述器（不做网络 I/O）：api.deepl.com / api-free.deepl.com 用 DeepL v2 格式（DeepL-Auth-Key 鉴权、text 数组，没有密钥时报错），其他地址用 DeepLX 兼容格式。Request <=16 KiB:
 * {config:{enabled,endpoint,api_key},text,source_language,target_language}.
 * Returns null if disabled; otherwise {url,method,headers,body,timeout_ms,max_response_bytes}.
 * Descriptor can contain credentials: never log it. Host enforces timeout/size,
 * rejects redirects and checks HTTP status before parsing. Text <=40 scalars. */
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
/* 同代次的非空 JSON [{"text":"candidate","translation":"gloss"}] 合并候选释义。
 * [] 主动清空；宿主异步无结果时跳过调用，旧代次不应用。 */
char *msime_client_apply_translations(uint64_t session, uint64_t generation,
                                      const uint8_t *translations, size_t length);
/* Resolve copied candidates against the packaged offline English dictionary.
 * JSON request: {generation,candidates:[{text,source}],user_data?,target_language?,state_root?}; the generation is echoed for the host to pass to apply_translations on the session thread. This function owns no session handle and may run on a worker thread.
 * target_language absent or "en" reads msime-english.db and the user's glosses. fr/ja/es/ru/de/ko read only offline-glosses/zh-<lang>.db beside resources, or, when that is absent and state_root (absolute; pass the translation query's state_root) is given, the same file in the downloaded <state_root>/resource-packs/offline-glosses pack; they ignore user_data. When neither is installed the result is {generation,translations:[]}, not an error. Any other value is an invalid request. Only Chinese candidates get a non-English gloss. */
char *msime_client_candidate_gloss_request(const uint8_t *request, size_t request_length,
                                           const uint8_t *resources, size_t resources_length);
/* Pronounce copied English texts without a session, for the line a host draws beside a gloss.
 * JSON request: {generation,items:[{text,language}]}; language must be "en". text is an English candidate or one line of an English gloss; only its first term is pronounced, without a leading to/a/an/the, and a term with any unknown word gets nothing. The response is {generation,pronunciations:[{text,language,pronunciation}]} with "/…/" IPA, omitting texts that have none. The table is pronunciations/en-phonetic.db beside resources; when it is not installed the result is {generation,pronunciations:[]}, not an error. May run on a worker thread. */
char *msime_client_pronunciation_request(const uint8_t *request, size_t request_length,
                                         const uint8_t *resources, size_t resources_length);
/* Break copied Chinese candidates into words with their English, without a session, for a line a host draws under the gloss lines (never a committable gloss column).
 * JSON request: {generation,texts:[...]}, at most 64 texts. Only all-Han texts of 2 to 32 characters are considered; each is cut left to right by longest match (up to 8 characters) against character-glosses/zh-en.db (single characters first) and word-glosses/zh-en.db beside resources, and each piece shows the first phrase of its gloss: "我 I · 喜欢 to like · 你 you". A text with fewer than two glossed pieces or more than eight pieces is omitted. The response is {generation,breakdowns:[{text,breakdown}]}; with neither table installed it is empty, not an error. May run on a worker thread. */
char *msime_client_gloss_breakdown_request(const uint8_t *request, size_t request_length,
                                           const uint8_t *resources, size_t resources_length);
/* Query the packaged English dictionary without creating a session.
 * JSON request: {prefix,limit}; prefix is an ASCII-letter word fragment and
 * limit is 1..32. The response echoes prefix and returns {items:[...]}.
 * Resources must be an absolute generation directory. */
char *msime_client_english_completions_request(const uint8_t *request,
                                               size_t request_length,
                                               const uint8_t *resources,
                                               size_t resources_length);
// Live per-session mode, not a persisted preference. Preserves composition and
// candidate generation; remains authoritative across preference replacement.
char *msime_client_set_chinese_punctuation(uint64_t session, bool enabled);
char *msime_client_set_character_width(uint64_t session, bool fullwidth);
char *msime_client_set_english_mode(uint64_t session, bool enabled);
/* 引擎负责的九键模式，用于全拼九宫格或注音九键（注音下 View.nine_key_spellings 是目标音节的候选读音）。只在组字结束后调用。View.nine_key 和 View.nine_key_spellings 是权威状态。对其他方案开启、或在组字中切换模式都会被拒绝。等同于 msime_client_set_key_grid 的 1（开）和 0（关），所以关掉时 14 键也一并关掉。
 * 创建和重建会话时按偏好自动开启：全拼看 touch_keyboard_layout 是否为 nine_key（fourteen_key 则开 14 键）；注音还要求 touch_keyboard_schemes 选中（或在没有选中项时启用了）zhuyin_nine_key，这个方案只有 Android 写，桌面宿主为全拼九宫格写下的 nine_key 不会让注音会话离开大千键位。宿主设下的网格只保留到方案或 touch_keyboard_layout 改变为止。
 * View.touch_keyboard_layout 是已应用的宿主呈现偏好；日文九键宿主只用它，不开启引擎的九键模式。 */
char *msime_client_set_nine_key_mode(uint64_t session, bool enabled);
/* 换引擎的组码网格：0 关，1 九键（同 msime_client_set_nine_key_mode），2 全拼 14 键（QWERTY 上相邻两个字母一个键，L、M 单独一键），3 九键双拼（表按已应用的 nine_key_shuangpin_profile 取）。只在组字结束后调用；14 键和九键双拼只给全拼，对其他方案开启、在组字中切换都会被拒绝，其他取值报错，会话不变。Value 是 View。
 * View.key_grid 是权威状态："none"、"nine_key"、"fourteen_key" 或 "nine_key_shuangpin"。View.nine_key 只在九键下为真。几种网格共用 View.nine_key_spellings、nine_key_reading、nine_key_single_character、nine_key_strokes，以及 msime_client_choose_nine_key_spelling 和 msime_client_set_nine_key_filter；14 键的分词键照九键送 '\''。14 键下数字不是输入，照常是选词键。九键双拼的数字从 msime_client_character 进来（方案用到的数字，可以有 1 和 0），和九键一样不当选词键，没有英文九键；创建和重建会话时 touch_keyboard_layout 为 nine_key_shuangpin 的全拼会话自动开启它。 */
char *msime_client_set_key_grid(uint64_t session, uint8_t grid);
/* 14 键的一键：letter 是这一组里的任一小写字母，引擎归成这一组，宿主约定送首字母（q e t u o a d g j l z c b m）。返回与 msime_client_character 相同的输入响应。不在 14 键下、方案不是全拼、在本地模式或专用英文里、26 键正在组字、或 letter 不是 a-z 时 handled=false，会话不变。
 * 硬件键盘的字母照常经 msime_client_character 走全拼 26 键：14 键组字进行中它们 handled=false，同九键。 */
char *msime_client_grid_key(uint64_t session, uint8_t letter);
/* 标出隐私会话（隐私模式、不允许学习的输入框）：这个会话的选词位置和上屏效率不记入打字统计。用户在设置里关掉学习不算隐私会话。学习本身仍由偏好里的 learning 决定。Value 是设下的布尔值。 */
char *msime_client_set_private_session(uint64_t session, bool enabled);
/* 报告大写锁定状态：偏好 caps_lock_ascii_punctuation 打开时，大写锁定期间没有组字的标点键（经 msime_client_character、msime_client_punctuation 或 msime_client_punctuation_with_context 送来）改走字面 ASCII 路线，和英文模式一样；punctuation_lock 为 chinese 时仍是中文标点。会话状态而非偏好，会话重建后由宿主重新报告。Value 是设下的布尔值。接上这个调用的宿主在 HostCapabilities.caps_lock_punctuation 里声明。 */
char *msime_client_set_caps_lock(uint64_t session, bool enabled);
char *msime_client_set_paired_punctuation(uint64_t session, bool enabled);
char *msime_client_set_punctuation_lock(uint64_t session, uint8_t lock);
/* 每页候选数的会话内覆盖，size 为 1–10；本平台排不下那么多时（Windows、iOS、鸿蒙最多 9）按平台上限截断，view.page_size 是实际值。组字中调用时等组字结束才生效。 */
char *msime_client_set_candidate_page_size(uint64_t session, uint8_t size);
char *msime_client_character(uint64_t session, uint8_t ascii, bool shift);
// Explicit native punctuation: finish the highlighted composition, then translate.
// Invalid non-punctuation bytes fail without modifying the session.
char *msime_client_punctuation(uint64_t session, uint8_t ascii);
/* Resolve native punctuation using the immediately preceding Unicode scalar
 * supplied by the platform editor. Zero means unavailable. Only the scalar is
 * inspected and no document text is retained. */
char *msime_client_punctuation_with_context(uint64_t session, uint8_t ascii,
                                            uint32_t preceding);
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
/* 整句改字（全拼、双拼）：桌面宿主把不带修饰键的左右方向键发成 MSIME_CONVERSION_LEFT / MSIME_CONVERSION_RIGHT，Ctrl+左右发成 MSIME_MOVE_LEFT / MSIME_MOVE_RIGHT 编辑拼音字母。首选是一个字对一个完整音节、覆盖整个组字、至少两个字的句子时，MSIME_CONVERSION_LEFT 进入改字：View.conversion 是整句，光标在 View.conversion_focus_start 个字（Unicode 标量）之前，conversion_focus_start..conversion_focus_end 是候选所替换的那一段，候选换成这一段（先是光标所在的词，再是更短、更长的）的字和词。之后左右键每次移一个字，最远到句末（focus_start 等于字数，没有候选）。选中候选（数字、MSIME_COMMIT_CANDIDATE、msime_client_select）把这段钉住，句子其余部分围绕它重新转换，光标移到这段后面，不上屏；光标在句末时 MSIME_COMMIT_CANDIDATE 上屏整句。MSIME_COMMIT_RAW、MSIME_FINISH_COMPOSITION、标点和失去焦点上屏改好的整句，不是拼音字母。MSIME_CANCEL 和 MSIME_BACKSPACE 只退出改字回到拼音（组字保留，钉住的段丢掉）；键入字母或撇号、Ctrl+左右、Home/End 先退出改字再照常处理。进不了改字时（读音不完整、单字、光标在拼音中间、其他方案、本地模式）这两条命令就是 MSIME_MOVE_LEFT / MSIME_MOVE_RIGHT。改字期间宿主把 View.phrase_prefix 加 View.conversion 画在行内代替拼音，不管预编辑样式选的是什么；改字的候选不能置顶、删除或固定位置。 */
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
    /* 焦点即将离开或会话即将关闭，宿主替用户结束组字（macOS IMK 的 commitComposition:，宿主为词库维护关掉会话之前）：对会话的作用同 MSIME_FINISH_COMPOSITION，区别只在生命周期里上屏来源记 FocusLoss，同 msime_client_focus(false) 的上屏：不可改写、不出提交音、不画上屏特效，回复里 feedback 的 typing_effect 为 0。用户按键或点选引起的结束组字仍发 MSIME_FINISH_COMPOSITION。 */
    MSIME_FINISH_ON_FOCUS_LOSS = 20,
    MSIME_NEXT_PAGE = 100, MSIME_PREVIOUS_PAGE = 101,
    MSIME_NEXT_CANDIDATE = 102, MSIME_PREVIOUS_CANDIDATE = 103,
    MSIME_FIRST_CANDIDATE = 104, MSIME_LAST_CANDIDATE = 105
};
char *msime_client_command(uint64_t session, uint32_t command);
/* Re-rank the visible candidates with the settled model, once the host's typing pause elapses.
 * The host owns the clock: it is the only side that knows whether a keystroke arrived while the
 * pass was being decided. Answers {"moved": false} when the order did not change, or
 * {"moved": true, "view": ...} after a reorder. The false case lets the host leave the candidate
 * window alone without serializing a view it will discard. Inert, and immediately false, when no
 * settled model is installed. */
char *msime_client_rerank_settled(uint64_t session);
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
/* Return read-only English completions for a bounded ASCII prefix. */
char *msime_client_english_completions(uint64_t session, const uint8_t *prefix,
                                       size_t prefix_length, size_t limit);
/* View.local_mode 是 Engine 自己的模式，不是从预编辑前缀猜出来的；View.microsoft_shuangpin 报告 Engine 已应用的配置，不是尚未生效的新偏好；它为真表示当前是双拼且韵母或零声母编码用 `;` 作第二键，宿主要把 `;` 当字母键交给 msime_client_character。宿主结合模式、编辑文本和光标使用它。取值为 none、unicode、date_time、quick_phrase、emoji、kaomoji、super_jianpin、temporary_english、temporary_japanese、expression、command、mention、url、chaizi（拆字反查，Shift+C 进入；候选注释是字的带调读音，由 Engine 给出），未知取值视为不可用状态。
 * View.spelling_symbols 列出 Engine 在当前状态下当作输入的非字母键：当前模式的拼写（expression 的数字和运算符、unicode 的数字、url 的数字和网址符号）；没有组字时打开模式的键（`/` 和 `@`）；全拼、双拼、五笔组字中原文恰好是 www、http、https、ftp 时打开网址模式的键（`.` 或 `:`）。宿主要把它们当字符发送；列在里面的数字是输入，不是选候选的快捷键。转换结果的 commit_context.typing_statistics 对 expression、command、mention 模式生成的文本为 false，这些文本不计入打字统计。
 */
char *msime_client_view(uint64_t session);
/* Return a copied OnlineQuery JSON object, or null when the current composition
 * is not eligible for an online provider. Linux responses may include the
 * validated ai_assistant provider/model/prompt configuration (never its token).
 * The document also carries the validated cloud_candidates preference so a
 * user-owned provider can distinguish cloud suggestions from AI suggestions.
 * The caller may perform provider work off-thread and pass the unchanged
 * document back to apply_online_candidate. */
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
 * provider names the service selected in preferences even when its configuration is incomplete; a transport must ask that service or none, never fall back to another. Credentials are returned only for the selected usable provider. These fields are for host-owned transport; never log the query. The existing target_language applies to both providers. translation_account is true only when the user explicitly chose the MSIME account, candidate_translations is on and no service of the user's own applies; it is the whole decision for a host's account gloss path, which must send nothing when it is false. offline_gloss_languages is present only when non-empty: the non-English target languages, in preference order, whose offline dictionary is installed beside resources or in the downloaded offline-glosses resource pack; ask msime_client_candidate_gloss_request with that target_language for each. state_root is present only when one of them comes from the pack; pass it on in that request. candidate_pronunciation is present (true) only when the user turned pronunciation on: the host then pronounces the English and Japanese gloss lines it draws, English through msime_client_pronunciation_request, as display text that is never committed. gloss_breakdown is present (true) only when candidate translation or the offline English gloss is on and English is a target language, as macOS requests breakdowns: the host then asks msime_client_gloss_breakdown_request about the sentence candidates, with resources, which the query carries whenever gloss_breakdown is present.
 * While the composition is a /fy request (command mode) the query is that request alone, whatever the gloss switches say: sentence:true, target_language "zh", one candidate {text, online_gloss:false} holding the English typed after the command, no resources, user_data or offline_gloss_languages, and null when no service of the user's own (tencent, niutrans or custom) is selected. Ask only that service, without the offline dictionary or the gloss cache, and hand the answer back through msime_client_apply_translations, which shows it as the first row and commits the translation.
 */
char *msime_client_translation_query(uint64_t session);
/* How long a cloud candidate is worth waiting for: connecting, and in total.
 * Mirrors client-core's cloud::candidates, which takes them from the reference,
 * whose cloud worker gives every phase of the request 2000 ms. A host must not
 * choose its own: a reply arriving after a private deadline is one the
 * reference would have shown.
 */
#define MSIME_CLOUD_CONNECT_TIMEOUT_MS 2000
#define MSIME_CLOUD_REQUEST_TIMEOUT_MS 2000
/* Build the bounded HTTPS cloud URL for an eligible OnlineQuery. The native
 * host performs network I/O and applies the copied result separately. */
char *msime_client_cloud_request_url(const uint8_t *query, size_t query_length);
/* Parse a host-fetched response using the shared cloud parser. Returns
 * {applied,view}; no-result/malformed provider documents do not mutate input.
 * Query <=16 KiB, response <=256 KiB. Stale queries and disabled cloud
 * preferences (including a pending disable) cannot apply candidates. */
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
/* Query the verified local msime-others.db Emoji catalog. Resources is an absolute
 * generation directory containing msime-others.db; no provider socket is needed.
 * Optional offset is a nonnegative SQL row offset (default 0); limit is 1..255.
 * Optional group filters a catalog subdivision; list_groups:true returns
 * {groups:[name,...]} in catalog order instead of an item page.
 * list_symbol_groups:true returns {symbol_groups:[{parent,title},...]}.
 * Optional parent narrows symbols to a parent category before paging.
 * list_plugin_symbol_groups:true with plugins (absolute plugins directory) returns
 * {plugin_symbol_groups:[{pack,pack_name,tab:"symbols"|"kaomoji",title,keywords,items:[text,...]},...]}:
 * 已安装符号集插件的全部组，包按名字排序、组按清单顺序；keywords 没写时为空串。宿主把 symbols 组追加在内置符号之后、以 pack_name 为上级分类，
 * kaomoji 组追加在颜文字 All 之后，不与内置目录去重。没传 plugins 时为空列表。
 * Advance offset by limit, not returned item count: each page deduplicates text. */
char *msime_client_emoji_catalog_request(const uint8_t *query,
                                         size_t query_length,
                                         const uint8_t *resources,
                                         size_t resources_length);
#endif
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
/* AI 回答为空、失败，或 apply_online_candidates 返回 applied:false 时调用（source 只认 1）：
 * 这次查询在 AI 座位上的占位行（候选 source 12）换成「—」，留到组字变化为止。
 * 返回 {applied, view}；过期查询、AI 已关或座位上已经是 AI 候选时 applied 为 false。 */
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
 *   过渡期的例外：msime_client_key 还没有"开一轮但不出声"的位，所以 Linux 的 IBus 和 Fcitx5 宿主只为会出按键音的打字键调用它。松开、单独的修饰键、带 Ctrl/Alt/Super 的组合键、录音中的键和 IBus 屏幕键盘面板送来的键都不开新的一轮，它们引起的派发（如组字中按 Ctrl 组合键发出的 MSIME_FINISH_COMPOSITION 或 MSIME_CANCEL、单按 Shift 切换中英文时的 MSIME_COMMIT_RAW）算在上一轮里。第 3 阶段按轮计 fuel 的脚本运行时不会因为这类没有先调 msime_client_key 的派发熔断；补上这个位之后，宿主再为这些按下也调用它，让它们各自成一轮。
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
 * 脚本运行要同时满足：开关开着；会话经 msime_client_set_lifecycle 选择加入；字段属性带 MSIME_TRAIT_KNOWN，且不带 SECURE、PRIVATE、ELEVATED。属性变得不满足时正在跑的实例当场丢弃。每次 msime_client_focus(true) 之后属性都要重新确认：宿主在聚焦之前调过 msime_client_set_field_traits（属性没变也算），或者聚焦之后按了第一个不带 MSIME_KEY_PASSTHROUGH 的键（msime_client_key；交给应用的键在 Windows 上经 Aux 管道异步上报，和属性没有先后保证，不算确认）；确认之前不实例化、不投递，所以想让脚本收到 focus_in 的宿主要在 msime_client_focus(true) 之前报告属性。观察类事件（focus_in、focus_out、input、selection_retracted、candidate_selected、commit、composition_start/update/end、candidates_shown/hidden）来自派发类调用、focus 和 destroy；msime_client_key 的按键、字段属性、激活和旁路调用的视图事件不投递。修改类钩子只在派发类调用里运行：candidates 在候选刷新和展开之后，插件可以重排、隐藏、注释候选，视图和选择 ID 按重排之后的顺序，被隐藏的行不出现在视图、分页和 msime_client_all_candidates 里、任何选择都选不到，插件的注释放在候选的 "script_annotation" 键里（没有时不出现，不改 "annotation"，由宿主另行绘制）；设着 MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY 时 candidates 不运行也不重放；before_commit 在上屏之前，可改写时（选中候选、原样上屏或暂存段送出，本地模式 none 且算打字，不是以词定字，没设 MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY，不是 MSIME_LIFECYCLE_HOST_HOLDS_PIECES 下宿主暂存的一段）插件可以改写标点和顶字尾巴之前的那一段，回复里的 commit 就是改写之后的文本，引擎学习和 AI 上文仍按原文。旁路调用不运行脚本，只按候选身份重放最近一次的重排、隐藏和注释。脚本只能经授权的 import 在钩子调用期间读文本，读不到宿主的任何别的状态，也不能回调宿主。每次 msime_client_focus（两个方向）和 destroy 都丢弃这个会话的全部脚本实例。fuel 按 msime_client_key 开启的轮次给：没有先调 msime_client_key 的派发（修饰键组合、快捷键、失焦命令）和上一次按键共用那一轮的配额；本轮配额用完时插件跳过这次投递，不算失败；运行到一半被本轮配额截断的插件在这次聚焦里不再运行，这一次用掉的不到单个钩子自己上限的一半时同样不算失败，达到一半就和超出单个钩子自己的上限一样算一次失败（candidates 的单次上限和每轮配额相等，同一轮里 input 先花过 fuel 时它的死循环先撞上每轮配额，这样也照样熔断）。脚本的失败不影响上屏：一次失败只丢弃它这次的结果，trap 之后这个插件在这次聚焦里不再运行，连续失败按插件熔断；被停用的插件和停用的类别（invalid、init、failed、slow，不带次数、具体原因和任何文本）在失焦时写进 <user_data>/script-health.json。 */
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
/* 插件页的包仓库和 @ 名单，给桌面外壳以外的设置宿主：Windows 的原生设置应用、鸿蒙的插件页，以及 Android、iOS 设置里导入辅助码表。Request（<=2 MiB）为 {state_root: 绝对路径, sound_packs: 绝对路径|null, script_page?: bool, user_data?: 绝对路径|null, action}；包和 mentions.json 放在 state_root/plugins 下，sound_packs 是安装包自带的内置音效包目录。script_page 为 true 表示宿主有脚本插件的授权页（列出、授权、排序，并写明同一用户下的其他程序能伪造授权），缺省为 false：Android、HarmonyOS 和只导入辅助码表的宿主不带它，永远看不到脚本插件。user_data 是输入法宿主选项里的 user_data 目录，只用于脚本授权的回答里的停用记录。action.operation：
 * "catalog"：value 为 {packages:[...], issues:[{kind, folder, reason}]}，即全部已安装的包和内置音效包，与桌面外壳列出的相同；不带 script_page 时去掉脚本插件（kind 为 script）的包和问题行。脚本包另有 abi、module、hooks（声明的钩子名）、permissions（申请的能力名）、memory_pages 和 content_hash（64 位小写十六进制的内容哈希，授权绑定在它上面）。
 * "import" {source: 包文件夹或 .zip 文件的绝对路径}：安装它，整包替换已安装的同 id 包；value 是装好的包。不带 script_page 时 release 构建拒绝脚本包（plugin_invalid，与移动端遇到 script 时的原因相同）；带 script_page 时接受，装好的脚本包默认不运行，要经授权。
 * "import_helpcode_table" {file_name, text}：把单个 .txt 辅助码表装成 kind 为 helpcode 的包。file_name 是用户选的文件名，只用于报错、显示名和 id；text 是码表正文，宿主必须先按 UTF-8 严格解码（解码失败就不要调用），原始字节不超过 1 MiB。码表按辅助码表包的严格规则校验（每行「字=码」，码是 1 到 2 个小写字母，同一个字只能出现一次，1 到 30000 条），不合格时是 plugin_invalid，detail 写明文件名、原因和行号。显示名取开头的 # name: 注释，没有时用去掉扩展名的文件名；id 是 custom-<小写文件名主干>，组不成合法 id 时是 custom-<12 位十六进制哈希>，同名文件再次导入时整包替换。value 是装好的包，其中 entries 是条数。宿主再把 id 写进 preferences.plugins.helpcode_pack_quanpin 或 helpcode_pack_shuangpin 来选用它。
 * "remove" {kind: "sound"|"music"|"command_table"|"effect"|"phrase_table"|"helpcode"|"wordbook"|"symbol_set", id}：value 为 null；没装的包视为已删除。
 * "load_mentions"：value 为 [{text, key}]，还没保存过名单时为空。
 * "save_mentions" {entries:[{text, key}]}：替换名单；value 为 null。
 * 以下三项只在桌面（macOS、Windows、Linux）上有，别处是 invalid；value 都是整份授权 {revision, plugins:[{id, hash, enabled, granted:[能力名]}], health:[{id, hash, disabled: "invalid"|"init"|"failed"|"slow"}]}，plugins 的顺序就是执行顺序，health 是输入法停用了的插件（来自 user_data/script-health.json，没给 user_data 时为空），与桌面外壳的 script_grants 命令相同。
 * "script_grants"：读出授权。
 * "set_script_grant" {id, hash, enabled, granted:[能力名]}：授予、调整或撤销一个脚本插件的授权。hash 是页面展示给用户的内容哈希，包在这之后被换过时是 plugin_script_changed；granted 必须是清单申请的能力的子集，并满足依赖和 rewrite_commit 与 read_composition 互斥的规则；启用时同时启用超过 8 个是 plugin_script_limit，超出内存总账是 plugin_script_memory。撤销是 enabled 为 false、granted 为空。
 * "reorder_scripts" {ids:[id]}：ids 按给出的顺序排到最前，其余按原来的相对顺序；有重复或没有授权条目的 id 时整个拒绝。
 * 失败时为 {ok:false, error: 失败码, detail?}：失败码与桌面外壳相同（invalid、storage、plugin_invalid、plugin_unsupported_source、plugin_archive、plugin_reserved、plugin_storage、plugin_script_missing、plugin_script_changed、plugin_script_grant_invalid、plugin_script_limit、plugin_script_memory、plugin_script_grants_format、mention_invalid、mention_format、mention_storage），detail 存在时是被拒的包或条目违反的规则，中文，给设置页显示。会读写文件，导入最多复制一个音乐包大小的数据：宿主有工作线程时在工作线程上调用。 */
char *msime_client_plugins(const uint8_t *request, size_t length);
/* 不带编码的常用语（<=4 MiB）：{directory: 偏好目录的绝对路径, action:{operation:"load"|"add"{text}|"remove"{id}|"replace"{id,text}|"move"{id,index}|"install_pack"{resource}|"remove_pack"{id}}}。Value 是整份文档 {phrases:[{id,text,pack}],packs:[{id,name,revision}],skipped?}；错误是固定的代码（common_phrases_io、_corrupt、_invalid、_duplicate、_limit、_too_large、_not_found）。设置进程和键盘进程都会调用，调用时锁住文件。在工作线程上调用。 */
char *msime_client_common_phrases(const uint8_t *request, size_t length);
/* 具名词库集合（<=17 MiB）：{options: 带 preferences_directory 的 HostOptions, action:{operation:"load"|"create"|"rename"|"delete"|"set_enabled"|"add_words"|"remove_words"|"import"|"install_community"|"queue_words"{entries}|"flush", ...}}，格式见 client-core 的 dictionary::collections；queue_words 把不属于任何集合的词（本地备份恢复的个人词库，每次最多 20000 个）排进待发送队列，value 带 queued。词条经个人词库队列传过去，键盘在下一次会话时应用。Value 是集合视图；错误是固定的代码（collections_*、builtin_locked、unsupported_format、import_*、personal_dictionary_*）。在工作线程上调用。 */
char *msime_client_dictionary_collections(const uint8_t *request, size_t length);
/* 诊断包（<=65536 字节）：{state_root: 偏好目录, include:{crash_logs,performance_logs,input_events,config_snapshot}, sources:{crash_logs,performance_logs,input_events: 绝对路径|null}, destination: .zip 的绝对路径|null}。输入事件和性能记录只有恰好是 {t_ms, kind (key_down|key_up|candidate_shown|candidate_selected|commit|backspace|panel_open|panel_close|ime_start|ime_finish), duration_ms} 时才保留，带其他任何键（text、key、candidate……）的整行都丢掉并计数。崩溃记录只保留异常类型和栈帧：异常说明可能带着正在输入的文字，message 只留冒号前的异常类型，stack 的说明行同样处理，路径只留文件名。配置快照把所有凭据换成 "<redacted>"。给了 destination 时写出 zip，value 是 {path,bytes,counts}；没给时 value 是 {counts,sections}，sections 是上传请求体的对象（crash_logs、perf_trace、input_events、config_snapshot）。错误：diagnostics_invalid、diagnostics_source、diagnostics_preferences、diagnostics_write。在工作线程上调用。 */
char *msime_client_diagnostic_bundle(const uint8_t *request, size_t length);
/* 账号设置文档导出（<=4 MiB）：{preferences_directory, feedback?:{soundEnabled,hapticsEnabled,hapticStrength}|null, custom_keyboard_skins?: JSON 数组字符串|null, android_local?: {同步键: 值}|null, schema?: 偏好字段表|null, cloud?: {revision,settings}|null}。Value {settings, merged?}：settings 按本版本过滤（给了 schema 时再按它的字段过滤）；同时给了 schema 和 cloud 时，merged 是带着云端 revision 去 PUT 的整份文档。android_local 是 Android 本机设置文件里参与同步的值（应用主题、单手模式、按键细节、工具栏的常用语/方案/隐藏项、手写、离线语音），不认识的键和非法值不导出。凭据和诊断日志从不导出；隐私模式、开发者选项和语音贡献只在 Android 本机文件里，从不同步。 */
char *msime_client_account_settings_export(const uint8_t *request, size_t length);
/* 账号设置文档应用（<=4 MiB）：{preferences_directory, cloud:{revision,settings}, schema, feedback?: 宿主当前的按键反馈|null}。把文档应用到本机偏好，按读到的 revision 做比较并交换后保存。本机不认识或超出范围的取值只跳过那一个键。Value {preferences: 保存后的快照, feedback: 应用后的按键反馈|null（宿主自己存）, custom_keyboard_skins: 云端皮肤库 JSON|null（宿主合并）, android_local:{同步键: 值}（宿主写进本机设置文件）, skipped:[键]}。 */
char *msime_client_account_settings_apply(const uint8_t *request, size_t length);
char *msime_client_destroy(uint64_t session);
/* Write the selection counts held by every session on the calling thread and all queued personal-context learning, without ending any session. Call from the host's will-terminate hook (e.g. NSApplicationWillTerminateNotification) on the thread that owns the sessions; the C++ Engine did this from atexit. Returns null on success. */
char *msime_client_flush_all(void);
/* value must be NULL or a still-owned pointer returned by this library. */
void msime_client_string_free(char *value);

#ifdef __cplusplus
}
#endif
#endif
