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
/* Update check against the release list of github.com/metasequoiaime/msime (no credentials). request {platform, current_version, edition?, arch?}: platform is the release tag prefix (windows, linux, macos, harmony, ...), current_version the running version, edition the edition id (full when absent), arch Rust's architecture name. Value {status:"available"|"current", update:{version:{display,parts}, release_url, installer_name, installer_sha256, signed}} or {status:"none"} when the platform has no published release. installer_name and installer_sha256 are the edition's single package on linux and windows (null when there is none or more than one, or GitHub sent no digest); signed is false there and null elsewhere. Blocks on the network for up to ten seconds: call off the UI and input threads. */
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
 * {operation:"reply",head,state}: head 是一个连接读到的请求头（到第一个空行为止，含），没读到完整请求头时为 null；state 是 plan 给的。Value {outcome,code?,error?,response}: 把 response 原样写回这个连接再关闭。outcome 为 ignored 时继续等下一个连接；code 时用 POST /v1/auth/login {challenge_id,credential:code} 完成登录；failed 时结束登录，error 是 account_cancelled（用户在 Google 页面拒绝）或 account_unavailable。 */
char *msime_client_google_loopback(const uint8_t *request, size_t length);
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
 * Native host owns/authorizes paths; never accept arbitrary webview paths or log payloads.
 * Run on a worker thread. Edit returns busy until all participating sessions are
 * destroyed, then holds exclusive access; recreate sessions after success.
 * Do not automatically cancel user input to obtain access. Retry ambiguous writes
 * with the identical nonempty request ID and content.
 */
char *msime_client_dictionary(const uint8_t *request, size_t length);
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
/* Structured mobile history, independent of the desktop automatic-capture preference.
 * JSON {directory:absolute App Group root,action:{operation:"load"|"clear"}}
 * or action:{operation:"capture"|"remove",text} or
 * action:{operation:"set_pinned",text,pinned}. History lives in directory/MSIME/clipboard_history.json. */
char *msime_client_mobile_clipboard_history(const uint8_t *request, size_t length);
/* Same validation as load_preferences; ok:true,value:null means lock busy.
 * Does not wait for the writer lock. Disk I/O may still block: use a worker.
 * Busy is not missing/corrupt and must not reset preferences to defaults. */
char *msime_client_try_load_preferences(const uint8_t *directory, size_t length);
/* Repair a preferences.json that is not well-formed JSON (truncated, empty, overwritten). May block on disk and the writer lock: use a worker. Backup first: the damaged bytes are copied verbatim to <directory>/preferences.json.corrupt-YYYYMMDD-HHMMSS (UTC, -N suffix when taken) and nothing is rewritten if that copy fails. Then every setting and service key the schema still accepts is carried onto the defaults. A valid or missing document is a no-op: {recovered:false, snapshot}. A repair returns {recovered:true, snapshot, backup_path, backup_name, salvaged}. A well-formed document the schema rejects (unknown fields, newer format_version) is most likely a newer build's and returns the load error unchanged; the settings page repairs it explicitly. Storage errors are returned and never lead to a rewrite. */
char *msime_client_recover_preferences(const uint8_t *directory, size_t length);
/* Compare-and-swap save of PreferencesSnapshot.preferences. The snapshot's
 * format_version is validated; expected_revision must match the store.
 * A disabled clipboard-history save clears the default history file if history
 * is still disabled. Cleanup errors may be returned after preferences are saved.
 * Directory <=16384 bytes; snapshot <=1048576 bytes, enough for a custom skin photo. */
char *msime_client_save_preferences(const uint8_t *directory, size_t directory_length,
                                    uint64_t expected_revision,
                                    const uint8_t *snapshot, size_t snapshot_length);
/* Call on the session thread with a PreferencesSnapshot JSON buffer (<=1048576): {format_version:1, revision, preferences:{...}}. Revision order is per session; identical retries are allowed, older/conflicting snapshots are rejected. Returns {revision, deferred, view, diagnostic?}; diagnostic is present only when the preferred input scheme could not run and a fallback scheme was applied. Active composition defers application until a successful dispatch/focus leaves it idle. Newer snapshots replace pending ones. Build failure retains the old session and pending snapshot for retry; dispatch reports retry failure in diagnostic without losing completed input. Does not read/write preferences files; the host supplies an already loaded snapshot. */
char *msime_client_update_preferences(uint64_t session, const uint8_t *snapshot, size_t length);
char *msime_client_focus(uint64_t session, bool focused);
/* Clear the current session's Engine candidate cache and refresh its view. */
char *msime_client_reset_cache(uint64_t session);
char *msime_client_voice_start(uint64_t session);
char *msime_client_voice_cancel(uint64_t session);
/* Capture bounded mono 16 kHz samples. The JSON result is transient audio and
 * must never be logged or persisted. */
char *msime_client_voice_capture(uint32_t milliseconds);
char *msime_client_voice_apply(uint64_t session, uint64_t generation,
                               const uint8_t *text, size_t length);
/* On-device speech models and user-dictionary hotwords. JSON request buffers of length bytes; standard responses. Error text of the model calls is a stable code beginning with "local_model_".
 * voice_hotwords: {options: HostOptions as msime_client_dictionary, limit?: 200} -> {hotwords:[{text,pinyin}]}, the user's own pinyin words (two or more Chinese characters), heaviest first. Worker thread; fails with "dictionary maintenance busy" while maintenance holds the store. <=1 MiB.
 * voice_hotword_correct: {text, hotwords:[{text,pinyin}]} -> {text}. Pinyin-similarity replacement for models whose msime-model.json has "hotwords":"pinyin". Pure. The transcript is limited to 10000 Unicode scalar values, with at most 1000 hotwords; each hotword text is <=256 bytes and pinyin <=1024 bytes. The JSON request is <=1048576 bytes.
 * voice_local_models: {root: absolute dir} -> {models:[{id,title,description,languages,streaming,default,desktop_only,installed,path,installed_size,archive_size,memory,license_spdx,license_source,license_terms,license_notice,hotwords,import_files:[{name,url,size}]}], default: id}. path is <root>/<id>, the value for voice_input.asr_model_path. import_files 是不联网安装时要用户自己下载的文件（压缩包和带地址的附加文件），url 是上游下载地址。
 * voice_local_model_install: {root, id, mirror?: "https://..." prefix, files?: [absolute path]} -> {path}. Blocks for the whole download: worker thread only. progress (nullable) gets {id,stage:"download"|"verify"|"extract"|"done",downloaded,total} on the calling thread; copy the buffer before returning. One install per id at a time ("local_model_install_running"). 带 files（至多 16 个）时不联网，用这些本地文件安装：按长度和 SHA-256 认文件、不看文件名，进度阶段 download 报成 "import"，缺文件时失败为 "local_model_import_missing: <name>"，所选文件打不开或读出错时为 "local_model_import_unreadable: <detail>"。
 * voice_local_model_cancel: {id} cancels that install, NULL/0 or {} cancels all; value is whether one was running. Any thread.
 * voice_local_model_remove: {root, id} -> null. Only catalog ids; refused while that id is installing. */
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
/* On-demand resource packs under <state_root>/resource-packs/<id>/, where state_root is the HostOptions preferences_directory (Android: files/bootstrap/state); a session picks up a newly published pack at its next focus. Pack ids: japanese, language-dictionaries, handwriting, settled-model, offline-glosses, voice-runtime. JSON request buffers of length bytes (<=16384); standard responses. Install and adopt block: worker thread only. Errors are "invalid resource pack request", "invalid state root", "invalid source", "resource_pack_unknown", "resource_pack_busy" (that pack is already installing or adopting in this process) or a "local_model_*" code.
 * resource_packs: {state_root} -> [{id, state:"missing"|"installed"|"outdated", size, schemes:[...]}], one entry per pack; size is the download size in bytes.
 * resource_pack_install: {state_root, pack, sources?:["https://mirror/", ...]} -> {path}. Sources are mirror prefixes tried in order (at most 8, empty ones skipped), then the project's mirrors, then the URL in the compiled lock; integrity is always the compiled size and SHA-256. An unfinished file is resumed with an HTTP Range request next time. progress (nullable) gets (context, phase "download"|"verify"|"done", done, total) on the calling thread; the phase string is valid only during the call. Cancelled installs fail with "local_model_cancelled".
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
/* Pure DeepLX-compatible descriptor builder (no network I/O). Request <=16 KiB:
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
/* Apply JSON [{"text":"candidate","translation":"gloss"}] for a candidate generation. */
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
/* 引擎负责的九键模式，用于全拼九宫格或注音九键（注音下 View.nine_key_spellings 是目标音节的候选读音）。只在组字结束后调用。View.nine_key 和 View.nine_key_spellings 是权威状态。对其他方案开启、或在组字中切换模式都会被拒绝。
 * 创建和重建会话时按偏好自动开启：全拼看 touch_keyboard_layout 是否为 nine_key；注音还要求 touch_keyboard_schemes 选中（或在没有选中项时启用了）zhuyin_nine_key，这个方案只有 Android 写，桌面宿主为全拼九宫格写下的 nine_key 不会让注音会话离开大千键位。
 * View.touch_keyboard_layout 是已应用的宿主呈现偏好；日文九键宿主只用它，不开启引擎的九键模式。 */
char *msime_client_set_nine_key_mode(uint64_t session, bool enabled);
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
 * View.nine_key_spellings 在全拼九键下的含义：先是完整音节；再是下一个数字键上能起头一个音节的字母，大写（M N O），选它限定下一个音节的首字母；最后是这个数字本身（6），只在没有锁定的音节时出现，选它直接上屏这个数字。数字全部锁定后它是最后一次锁定时的选项，选其中一项就换掉那次锁定。这时退格先撤销最后一次锁定，其余情况退格删数字。 */
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
/* View.local_mode 是 Engine 自己的模式，不是从预编辑前缀猜出来的；View.microsoft_shuangpin 报告 Engine 已应用的配置，不是尚未生效的新偏好；它为真表示当前是双拼且韵母或零声母编码用 `;` 作第二键，宿主要把 `;` 当字母键交给 msime_client_character。宿主结合模式、编辑文本和光标使用它。取值为 none、unicode、date_time、quick_phrase、emoji、kaomoji、super_jianpin、temporary_english、temporary_japanese、expression、command、mention、url，未知取值视为不可用状态。
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
/* Hand the session an AI provider credential kept outside the preferences
 * (for example in the iOS Keychain). It overrides the current endpoint's
 * stored token for this session only and is never persisted or reported.
 * It remains bound to that endpoint until replaced or cleared; a preference
 * change to another endpoint cannot forward it. A zero length clears it. */
char *msime_client_set_ai_credential(uint64_t session, const uint8_t *token,
                                     size_t token_length);
/* Return null or {generation,target_language,candidates:[{text}], provider:"none"|"account"|"tencent"|"niutrans"|"custom", translation_account:bool, custom_translation:{enabled,endpoint,api_key}|null, tencent_tmt:{enabled,secret_id,secret_key,region}|null, niutrans:{enabled,app_id,apikey}|null} for visible candidates.
 * provider names the service selected in preferences even when its configuration is incomplete; a transport must ask that service or none, never fall back to another. Credentials are returned only for the selected usable provider. These fields are for host-owned transport; never log the query. The existing target_language applies to both providers. translation_account is true only when the user explicitly chose the MSIME account, candidate_translations is on and no service of the user's own applies; it is the whole decision for a host's account gloss path, which must send nothing when it is false. offline_gloss_languages is present only when non-empty: the non-English target languages, in preference order, whose offline dictionary is installed beside resources or in the downloaded offline-glosses resource pack; ask msime_client_candidate_gloss_request with that target_language for each. state_root is present only when one of them comes from the pack; pass it on in that request. candidate_pronunciation is present (true) only when the user turned pronunciation on: the host then pronounces the English and Japanese gloss lines it draws, English through msime_client_pronunciation_request, as display text that is never committed.
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
char *msime_client_voice_provider_stream_feedback(
    const uint8_t *query, size_t query_length, const uint8_t *socket_path,
    size_t socket_length, msime_client_voice_update_callback callback,
    msime_client_voice_status_callback status_callback,
    msime_client_voice_level_callback level_callback, void *context);
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
/* The settings page's pack store and @ name list, for a settings host other than the desktop shell (HarmonyOS). Request (<=2 MiB): {state_root: absolute, sound_packs: absolute|null, action}; packs and mentions.json live in state_root/plugins, sound_packs is the bundle's built-in sound pack root. action.operation:
 * "catalog": value {packages:[...], issues:[{kind, folder, reason}]}, every installed pack and the built-in sound packs, as the desktop shell lists them.
 * "import" {source: absolute path of a pack folder or .zip file}: installs it, replacing an installed pack of the same id whole; value is the installed pack.
 * "remove" {kind: "sound"|"music"|"command_table"|"effect"|"phrase_table"|"helpcode"|"wordbook"|"symbol_set", id}: value null; a pack that is not installed is already removed.
 * "load_mentions": value [{text, key}], empty before a list was saved.
 * "save_mentions" {entries:[{text, key}]}: replaces the list; value null.
 * A failure is {ok:false, error: code, detail?}: the codes are the desktop shell's (invalid, storage, plugin_invalid, plugin_unsupported_source, plugin_archive, plugin_reserved, plugin_storage, mention_invalid, mention_format, mention_storage) and detail, when present, is the rule a refused pack or entry broke, in Chinese for the page. Reads and writes files, and an import copies up to a music pack's size: use a worker thread where the host has one. */
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
