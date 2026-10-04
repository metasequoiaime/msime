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
/* text is UTF-8 Markdown of length bytes; maximum 256 KiB. Value is HTML for a rich-text view: raw HTML in the source is escaped, only http, https and mailto links are kept, images become links and are never loaded. Open links externally. */
char *msime_client_markdown_to_html(const uint8_t *text, size_t length);
/* Community moderation for hosts that send community requests through their own HTTP stack. Request is UTF-8 JSON of length bytes; maximum 64 KiB. Publishing is post-moderated: an item is public at once and moderators may remove it. To learn the state of the user's own items add fields=moderation to scope=mine lists and to the detail request (skins, candidate-skins, plugins, resources); each own item then carries moderation "approved"|"pending"|"removed". Show only a 已下架 badge for removed and treat pending as approved; never show a reason. Without fields=moderation the responses are unchanged.
 * {operation:"reasons"}: value is the report reasons in dialog order, each the exact string to send: ["侵权/抄袭","色情低俗","违法违规","垃圾广告","恶意插件","其他"].
 * {operation:"report",kind,item_id,reason,detail?}: kind is skins, candidate-skins, plugins, dictionaries or replies; item_id the item's UUID; reason one of the reasons; detail optional, at most 1000 characters. Value {method:"POST",path:"/v1/community/reports",body}: send body as JSON with the signed-in session's bearer (the device's anonymous account counts). A bad field is error community_invalid.
 * {operation:"error",status,body?}: status and body of a failed community response. Value {code,message,retry}: code is account_blocked_content (422 blocked_content: the text must change; never say the service is down), account_screening_unavailable (503 screening_unavailable: honour Retry-After), account_banned (403 account_banned) or the generic account_* code; message is the Chinese sentence to show for the first three and null otherwise; retry says whether the same request may be sent again later. */
char *msime_client_community_moderation(const uint8_t *request, size_t length);
/* options is a readable UTF-8 buffer of length bytes; maximum 1 MiB.
 * Object: api_version=1, resources/user_data/cache/dictionaries (absolute paths),
 * preferences={scheme, candidate_page_size, learning, chinese_punctuation,
 *              shuangpin_profile?, wubi_profile?}. Missing profile defaults to xiaohe; allowed
 * profiles: xiaohe, ziranma, shoudao, microsoft. 缺省 wubi_profile 为 wubi86，可选 wubi86、wubi98。 Unknown values are rejected.
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
/* The transcription provider and optional rewrite this device is configured for, read from an
 * absolute preferences directory. Response value: {provider:{...}|null, polish:{...}|null}; both
 * absent means nothing is configured and the host uses whatever it falls back to. Contains
 * credentials: never log the response; release with msime_client_string_free. */
char *msime_client_mobile_voice_configuration(const uint8_t *directory, size_t length);
/* The global theme picker: {themes:[{id,title,appearance,preview,candidate,keyboard},...],default:"system"}. Ids in picker order: system, shuishan, light, paper, night, ink, custom. appearance is "light"|"dark"|null; preview {background,panel,accent,text}, candidate and keyboard are the built-in palettes and are null for system and custom. Keys are snake_case. Hosts keep no copy of the ids, titles or colours. */
char *msime_client_theme_catalog(void);
/* Resolve the colours for the selected global theme. JSON request (<=1048576 bytes, unknown keys rejected): {global_theme:string, custom_theme?:Preferences.custom_theme, dark:bool, layout:"horizontal"|"vertical", skins_directory?:absolute skin root | package?:one candidate_skin_catalog entry}. global_theme must be one of the seven catalog ids; any other id fails the request. custom_theme is validated like the preference. dark is the host's effective mode; it only matters for custom over a system base, because a built-in base (custom_theme.base, or the applied package's manifest base) fixes the mode and its package palette is the one for that mode. layout is the candidate window being drawn: a package is drawn only in a layout and a mode its manifest declares, and candidate_skin is set only when it is, so a host draws the package decoration and minimum width exactly when candidate_skin is not null. skins_directory is for hosts that scan the skin root (every host but Linux); package is one entry of the Linux candidate_skin_catalog, and anything else there (a msime_client_skin_catalog SkinSummary included) fails the request. Response value: {id,source:"system"|"builtin"|"custom",appearance:"light"|"dark"|null, candidate:{surface,border,text,number,secondary,accent,selected,selected_text,selected_number,hover,show_selected_bar}|null, keyboard:{background,key,function_key,text,secondary,accent,on_accent}|null, candidate_skin:string|null}. appearance, when not null, is the mode the returned surfaces are in. keyboard.accent is the touch strip's selected candidate text (no fill); the return key keeps the platform accent. on_accent is black or white, readable on an accent fill. Every colour is #RRGGBB or #RRGGBBAA. A null palette or null slot means the host's own native token, never transparent. A package missing from the root, invalid on disk or not the one custom_theme.candidate_skin names is left out rather than failing the call. skins_directory reads the package: resolve on a theme, appearance or package change, never while drawing. */
char *msime_client_resolve_theme(const uint8_t *request, size_t length);
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
 * packaged dictionary-manifest.json. Two fields only: the page is asking what
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
 * voice_local_models: {root: absolute dir} -> {models:[{id,title,description,languages,streaming,default,desktop_only,installed,path,installed_size,archive_size,memory,license_spdx,license_source,license_terms,license_notice,hotwords}], default: id}. path is <root>/<id>, the value for voice_input.asr_model_path.
 * voice_local_model_install: {root, id, mirror?: "https://..." prefix} -> {path}. Blocks for the whole download: worker thread only. progress (nullable) gets {id,stage:"download"|"verify"|"extract"|"done",downloaded,total} on the calling thread; copy the buffer before returning. One install per id at a time ("local_model_install_running").
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
 * JSON request: {generation,candidates:[{text,source}],user_data?,target_language?}; the generation is echoed for the host to pass to apply_translations on the session thread. This function owns no session handle and may run on a worker thread.
 * target_language absent or "en" reads english.db and the user's glosses. fr/ja/es/ru/de/ko read only offline-glosses/zh-<lang>.db beside resources and ignore user_data; when that file is not installed the result is {generation,translations:[]}, not an error. Any other value is an invalid request. Only Chinese candidates get a non-English gloss. */
char *msime_client_candidate_gloss_request(const uint8_t *request, size_t request_length,
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
/* Engine-owned quanpin nine-key mode. Call only after finishing composition.
 * View.nine_key and View.nine_key_spellings are authoritative. Enabling for
 * another scheme or changing mode during composition is rejected.
 * View.touch_keyboard_layout is the applied host presentation preference; a
 * Japanese nine-key host uses it without enabling Engine quanpin nine-key. */
char *msime_client_set_nine_key_mode(uint64_t session, bool enabled);
char *msime_client_set_paired_punctuation(uint64_t session, bool enabled);
char *msime_client_set_punctuation_lock(uint64_t session, uint8_t lock);
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
/* Select an entry from View.nine_key_spellings. The generation rejects stale UI. */
char *msime_client_choose_nine_key_spelling(uint64_t session, uint64_t generation, size_t index);
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
/* View.local_mode 是 Engine 自己的模式，不是从预编辑前缀猜出来的；View.microsoft_shuangpin 报告 Engine 已应用的配置，不是尚未生效的新偏好。宿主结合模式、编辑文本和光标使用它。取值为 none、unicode、date_time、quick_phrase、emoji、kaomoji、super_jianpin、temporary_english、temporary_japanese、expression、command、mention、url，未知取值视为不可用状态。
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
 * (for example in the iOS Keychain). It overrides the active provider's
 * stored token for this session only and is never persisted or reported.
 * A zero length clears it. */
char *msime_client_set_ai_credential(uint64_t session, const uint8_t *token,
                                     size_t token_length);
/* Return null or {generation,target_language,candidates:[{text}], provider:"none"|"account"|"tencent"|"niutrans"|"custom", translation_account:bool, custom_translation:{enabled,endpoint,api_key}|null, tencent_tmt:{enabled,secret_id,secret_key,region}|null, niutrans:{enabled,app_id,apikey}|null} for visible candidates.
 * provider names the service selected in preferences even when its configuration is incomplete; a transport must ask that service or none, never fall back to another. Credentials are returned only for the selected usable provider. These fields are for host-owned transport; never log the query. The existing target_language applies to both providers. translation_account is true only when the user explicitly chose the MSIME account, candidate_translations is on and no service of the user's own applies; it is the whole decision for a host's account gloss path, which must send nothing when it is false. offline_gloss_languages is present only when non-empty: the non-English target languages, in preference order, whose offline dictionary is installed beside resources; ask msime_client_candidate_gloss_request with that target_language for each.
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
/* Query the verified local others.db Emoji catalog. Resources is an absolute
 * generation directory containing others.db; no provider socket is needed.
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
/* Effect sounds and background music, played by this library on macOS, Windows and Linux from the session's preferences.plugins. The three calls below are for the key path: they return whether a request was queued, never block, decode or read files, and need no free. False means nothing is switched on, the session handle is unknown or on another thread, the queue is full, this platform does not play (iOS, Android, HarmonyOS), or sound failed earlier in this process, which turns it off until the process restarts with one line on stderr. Nothing starts - no thread, no audio device - until a call finds something switched on, so a process that never calls them (the Windows TSF DLL) pays nothing; the device is let go again after 30 s without a sound or playing music. Do not call them for keys typed into a secure (password) field.
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
char *msime_client_destroy(uint64_t session);
/* Write the selection counts held by every session on the calling thread and all queued personal-context learning, without ending any session. Call from the host's will-terminate hook (e.g. NSApplicationWillTerminateNotification) on the thread that owns the sessions; the C++ Engine did this from atexit. Returns null on success. */
char *msime_client_flush_all(void);
/* value must be NULL or a still-owned pointer returned by this library. */
void msime_client_string_free(char *value);

#ifdef __cplusplus
}
#endif
#endif
