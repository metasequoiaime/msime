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

/* ABI 2. All functions return owned, NUL-terminated UTF-8 JSON. Free exactly once
 * using msime_client_string_free, including error responses. Never use free().
 * Responses: {"ok":true,"value":...} or {"ok":false,"error":"..."}.
 * Creation returns a View; its session field is the handle. Handles are confined
 * to their creating thread. Dispatch, focus, view and destroy on that thread.
 * Text and candidate values are copied; no Engine pointers escape.
 */
uint32_t msime_client_abi_version(void);
/* Worker-thread bootstrap: {resources: absolute path, state_root: absolute path}.
 * Verifies pinned resources, delegates working data preparation to Engine and
 * returns HostOptions. Maximum 16384 bytes; no session may use state_root during
 * preparation. Caller publishes the returned config atomically after success.
 */
char *msime_client_prepare_host(const uint8_t *options, size_t length);
/* path is an absolute UTF-8 runtime options file path of length bytes; maximum 4096. When its dictionaries directory is not the installed resource generation (after a package upgrade), prepares that generation, replays the user dictionary into it and atomically rewrites resources/dictionaries, keeping every other key. Value is true when the file was rewritten. Call before creating any session from the file. When the recorded resource directory does not match the compiled dictionary lock (downloaded dictionaries an upgrade did not replace) the error text begins with "dictionary_outdated:" and the file is left unchanged; the rest of that text may name private paths. */
char *msime_client_refresh_host(const uint8_t *path, size_t length);
/* options is a readable UTF-8 buffer of length bytes; maximum 1 MiB.
 * Object: api_version=1, resources/user_data/cache/dictionaries (absolute paths),
 * preferences={scheme, candidate_page_size, learning, chinese_punctuation,
 *              shuangpin_profile?}. Missing profile defaults to xiaohe; allowed
 * profiles: xiaohe, ziranma, shoudao, microsoft. Unknown values are rejected.
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
 * Do not delete .msime-dictionary-access.lock files. Legacy/external writers do
 * not participate; preparation/upgrades still require stopped sessions.
 */
char *msime_client_create(const uint8_t *options, size_t length);
/* Management JSON (<=2248576 bytes), trusted native caller only:
 * {options: <same HostOptions as create>, action: {operation:"list",offset:0,limit:100}}
 * List takes optional kind and query (a code prefix). Without them it lists the user's own words; with a kind and a nonblank query (quick_phrase needs none) it also finds the bundled words of that dictionary, user words first. user_only:true keeps any list to the user's own words, filtered by the kind and code prefix across the whole store.
 * or action:{operation:"edit",previous:null|Entry,replacement:null|Entry,request_id:"..."}.
 * Batch import: action:{operation:"import",kind:"pinyin"|"wubi"|"quick_phrase"|"english",
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
 * Entry:{kind:"pinyin"|"wubi"|"quick_phrase"|"english",key,value,weight,source?:"user"|"bundled"}.
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
/* 内置候选皮肤，JSON 形如
 * {"skins":[{"id":"fluent","title":"Fluent"},…],"default":"willow_green"}。
 * 数组顺序就是宿主的展示与循环顺序。宿主不要另存一份 id 或标题：两个 Linux 宿主曾
 * 各存一份，于是同一个 graphite 在一边叫 Graphite、在另一边叫石墨。
 */
/* The transcription provider and optional rewrite this device is configured for, read from an
 * absolute preferences directory. Response value: {provider:{...}|null, polish:{...}|null}; both
 * absent means nothing is configured and the host uses whatever it falls back to. Contains
 * credentials: never log the response; release with msime_client_string_free. */
char *msime_client_mobile_voice_configuration(const uint8_t *directory, size_t length);
char *msime_client_builtin_skins(void);
/* Per-key double-pinyin hint text for one profile name, as a JSON object mapping
 * an uppercase key to "initials / finals" - or to whichever side that key carries.
 * Read out of the Engine's own profile tables so a keyboard face never carries a
 * second copy of the keymap. An unknown profile name yields an empty object
 * rather than the default profile's hints. */
char *msime_client_shuangpin_key_hints(const uint8_t *profile, size_t length);
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
 * `day` is the caller's local day and is required by every action: the counts and
 * the queue are per-day and this layer cannot resolve the host's timezone.
 * Requests may be up to 8 MiB rather than the usual 64 KiB, because an imported
 * word list is a few hundred kilobytes of text. Takes a file lock and reads the
 * library: call on a worker, never on the input path. */
char *msime_client_vocabulary_review(const uint8_t *request, size_t length);
/* Scan an absolute UTF-8 skin root and return the catalog the settings page
 * sees: {packages:[...],issues:[...]}. Reads the directory: use a worker.
 * An unreadable root is an empty catalog; an invalid package becomes an issue
 * and is never returned as renderable. Presenters must still check that a
 * package supports the layout and theme before adopting its colors.
 * Keys are camelCase, the same document the settings page consumes. */
char *msime_client_skin_catalog(const uint8_t *directory, size_t length);
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
 * action:{operation:"set_pinned",text,pinned}. The fixed Apple legacy file is
 * validated and migrated to directory/MSIME/clipboard_history.json before use. */
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
/* Call on the session thread with a PreferencesSnapshot JSON buffer (<=1048576):
 * {format_version:1, revision, preferences:{...}}. Revision order is per session;
 * identical retries are allowed, older/conflicting snapshots are rejected.
 * Returns {revision, deferred, view}. Active composition defers application until
 * a successful dispatch/focus leaves it idle. Newer snapshots replace pending ones.
 * Build failure retains the old session and pending snapshot for retry; dispatch
 * reports retry failure in diagnostic without losing completed input.
 * Does not read/write preferences files; the host supplies an already loaded snapshot.
 */
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
 * voice_hotword_correct: {text, hotwords:[{text,pinyin}]} -> {text}. Pinyin-similarity replacement for models whose msime-model.json has "hotwords":"pinyin". Pure. <=1048576 bytes.
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
enum MsimeCommand {
    MSIME_BACKSPACE = 0, MSIME_COMMIT_CANDIDATE = 1, MSIME_COMMIT_RAW = 2,
    MSIME_CANCEL = 3, MSIME_MOVE_LEFT = 4, MSIME_MOVE_RIGHT = 5,
    MSIME_MOVE_HOME = 6, MSIME_MOVE_END = 7, MSIME_DELETE_FORWARD = 8,
    MSIME_FINISH_COMPOSITION = 9,
    MSIME_CYCLE_KANA_VARIANT = 10, MSIME_COMMIT_READING = 11,
    MSIME_BACKSPACE_SEGMENT = 12, MSIME_MOVE_LEFT_SEGMENT = 13, MSIME_MOVE_RIGHT_SEGMENT = 14,
    MSIME_COMMIT_RAW_WITHOUT_LEARNING = 15,
    MSIME_NEXT_PAGE = 100, MSIME_PREVIOUS_PAGE = 101,
    MSIME_NEXT_CANDIDATE = 102, MSIME_PREVIOUS_CANDIDATE = 103,
    MSIME_FIRST_CANDIDATE = 104, MSIME_LAST_CANDIDATE = 105
};
char *msime_client_command(uint64_t session, uint32_t command);
/* Re-rank the visible candidates with the settled model, once the host's typing pause elapses.
 * The host owns the clock: it is the only side that knows whether a keystroke arrived while the
 * pass was being decided. Answers {"moved": bool, "view": ...}; "moved" is false when the order
 * did not change, which is the signal to leave the candidate window alone rather than repaint it
 * identically. Inert, and immediately false, when no settled model is installed. */
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
/* View.local_mode is the Engine-owned mode, not a preedit-prefix heuristic:
 * View.microsoft_shuangpin reports the applied Engine configuration, never a
 * newer deferred preference. Hosts use it with mode, editing text and caret.
 * none, unicode, date_time, quick_phrase, emoji, kaomoji, super_jianpin,
 * temporary_english, temporary_japanese. Treat unknown as unusable state.
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
 * Advance offset by limit, not returned item count: each page deduplicates text. */
char *msime_client_emoji_catalog_request(const uint8_t *query,
                                         size_t query_length,
                                         const uint8_t *resources,
                                         size_t resources_length);
#endif
/* Shared Doubao authentication policy. Input (max 32768 bytes):
 * {auth_mode,app_id,token,resource_id}; absent mode supports legacy documents.
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
/* msime-mcp beside the calling executable, for a settings host other than the desktop shell. JSON request (<=65536 bytes) {options:absolute runtime options path|null}. Returns {command,installed,options,config,clients:[{id,path,configured}]}. Reads the assistants' configuration files: use a worker thread. */
char *msime_client_mcp_status(const uint8_t *request, size_t length);
/* Write the msime entry into one assistant's configuration, keeping every other key. JSON request {options,client:"claude_desktop"|"cursor",replace:bool}. Returns "added"|"replaced"|"unchanged"; a different msime entry fails with mcp_entry_exists unless replace is set. Writes a file: use a worker thread. */
char *msime_client_mcp_install(const uint8_t *request, size_t length);
char *msime_client_destroy(uint64_t session);
/* value must be NULL or a still-owned pointer returned by this library. */
void msime_client_string_free(char *value);

#ifdef __cplusplus
}
#endif
#endif
