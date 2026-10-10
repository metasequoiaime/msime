/**
 * Type surface of platforms/harmony/native/client_napi.cpp.
 *
 * Every method takes and returns UTF-8 JSON as an ArkTS string, mirroring the shared C ABI in
 * crates/host-api. Responses carry {"ok":true,"value":...} or {"ok":false,"error":...}; callers parse
 * them rather than relying on exceptions, exactly as the Android NativeClient does.
 */

/** Native ABI revision. The host refuses to run against a version it does not know. */
export const abiVersion: () => number;

/** Capabilities for a named platform, e.g. "harmony". Drives what the shared settings UI renders. */
export const hostCapabilities: (platform: string) => string;

/**
 * Simplified to Traditional through the shared phrase-level OpenCC s2t tables, the conversion the Windows, macOS, Linux and Android hosts use. Unlike the methods around it this answers with the converted text itself rather than a JSON response, and with null for text the C ABI refuses (an embedded NUL), in which case the caller keeps its own text.
 */
export const simplifiedToTraditional: (text: string) => string | null;

export const loadPreferences: (directory: string) => string;
export const skinCatalog: (directory: string) => string;
/** The global theme picker: `{themes:[{id,title,appearance,preview,candidate,keyboard}],default}` in picker order. */
export const themeCatalog: () => string;
/**
 * `{global_theme,custom_theme?,dark,layout,skins_directory}` in; `{id,source,appearance,candidate,keyboard,candidate_skin}` out. `skins_directory` reads the applied package from disk, so resolve on a theme, appearance or package change and never while drawing.
 */
export const resolveTheme: (request: string) => string;
/**
 * 应用主题选择器：按选择器顺序（`siji`、`chunya`、`xiayin`、`qiushan`、`dongxue`）返回 `{app_themes:[{id,title,season,seasonal,light,dark}],default:"siji"}`。`light` 和 `dark` 是 `{accent,accent_soft,on_accent,background,card,hair}`；`siji` 的 `season` 为 null、`seasonal` 为 true，并绘制为秋杉，因此目录不随时钟变化。
 */
export const appThemeCatalog: () => string;
/**
 * 输入 `{app_theme,month?,dark}`，其中 `month` 是宿主的本地日历月份（1-12，仅 `siji` 使用；省略时取 UTC 月份）；输出 `{id,season,accent,accent_soft,on_accent,background,card,hair}`，`season` 为实际绘制的季节。纯函数，可以在主线程上运行。
 */
export const resolveAppTheme: (request: string) => string;
/** Staged engine resources in; `{profile,sourceCommit}` from the packaged dictionary manifest out. */
export const dictionaryManifest: (resources: string) => string;
/** JSON resource request; returns a structured response containing contentType and byte values. */
export const skinResource: (request: string) => string;
/** JSON stylesheet request; returns a nullable stylesheet in the structured response. */
export const skinToolbarStylesheet: (request: string) => string;
/** `{source,directory}` validates and atomically imports a picked skin folder off the UI thread. */
export const skinImport: (request: string) => Promise<string>;
/**
 * `{directory}` reads the named custom touch-keyboard designs; adding `{action}` applies one change
 * first. Both answer with the whole library. Takes the library's file lock, so call it off the UI
 * thread when the design carries a photo.
 */
export const customSkinLibrary: (request: string) => Promise<string>;
/**
 * `{directory,id,name,design}` starts a skin trial and imports the design, answering `{skin,trial}`.
 * One call rather than two: the trial remembers the skin being replaced, so a failed import has to
 * end it. Writes preferences and two locked files, so call it off the UI thread.
 */
export const communitySkinInstall: (request: string) => Promise<string>;
/** `{directory,action:{operation:"finish",id,keep}}` or `{operation:"restore_pending"}`. */
export const keyboardSkinTrial: (request: string) => string;
/**
 * `{file,action:{operation:"load"|"save_reply"|"remove",...}}` over the reply templates the user
 * kept. Every operation answers with the whole library; the keyboard process rereads the same file.
 */
export const communityResourceLibrary: (request: string) => Promise<string>;
/**
 * The decisions in AI skin generation, for a host that performs the requests itself.
 *
 * `{operation:"compose",prompt,model}` answers `{path,body}` carrying the shared system prompt;
 * `{operation:"parse",text}` answers the three validated plans or refuses; `{operation:"artwork"}`
 * says whether a returned image is one this client will show. The instruction and the parser are
 * one contract — the prompt names the exact document the parser accepts.
 */
export const aiSkinPlan: (request: string) => string;
export const savePreferences: (
  directory: string,
  expectedRevision: number,
  snapshot: string,
) => string;
export const dictionary: (request: string) => string;
export const updatePreferences: (handle: number, snapshot: string) => string;
/** `{directory,action}`; a `record` action answers `{recorded,milestone}`, where milestone is the commit count just passed while achievements are switched on, else null. */
export const typingStatistics: (request: string) => string;
/** 与 `typingStatistics` 相同的请求和应答，在工作线程上执行；设置应用以这种方式发送 `summary` 和 `record_skin`。 */
export const typingStatisticsAsync: (request: string) => Promise<string>;
/**
 * `{state_root,sound_packs,pack}` in, all paths absolute; the validated files of that sound pack out: `{id,name,license,builtin,mode,sounds:{default,space,enter,backspace,commit,achievement},sequence:{sample,semitones,advance}|null,max_sample_millis,melody_idle_reset_millis}` with absolute paths. Reads the pack from disk: not for the key path.
 */
export const keySoundPack: (request: string) => string;
/**
 * `{state_root,sound_packs,pack}` in, as for `keySoundPack`; the validated tracks of that music pack out: `{id,name,license,tracks,max_track_seconds}`, the tracks as absolute paths in play order. Reads the pack from disk: not for the key path.
 */
export const musicPack: (request: string) => string;
/**
 * The 扩展 page's pack store: `{state_root,sound_packs,action}` in, with `action.operation` one of `catalog`, `import` `{source}` (an absolute path inside this sandbox), `remove` `{kind,id}`, `load_mentions` and `save_mentions` `{entries}`. Answers `{ok,value}` or `{ok:false,error,detail?}`, the error one of the desktop shell's codes and the detail the rule a refused pack or name broke.
 */
export const plugins: (request: string) => string;
/**
 * `plugins` on a worker thread, for an `import`, which extracts or copies up to a music pack's size and validates it before swapping it into place. Resolves with the same answer `plugins` returns; rejects only when the worker produced no answer.
 */
export const pluginsAsync: (request: string) => Promise<string>;
/**
 * 在工作线程上处理键盘的常用语（不带编码）：`{directory,action:{operation:"load"|"add"{text}|"remove"{id}|"replace"{id,text}|"move"{id,index}|"install_pack"{resource}|"remove_pack"{id}}}`，其中 `directory` 是偏好目录的绝对路径。每个操作都以 `{ok,value}` resolve，value 是整个文档 `{phrases:[{id,text,pack}],packs:[{id,name,revision}],skipped?}`；或以 `{ok:false,error}` resolve，error 是某个 `common_phrases_*` 错误码；只有工作线程没有给出应答时才 reject。设置进程和键盘进程在锁下共享该文件。
 */
export const commonPhrases: (request: string) => Promise<string>;
/**
 * Registers the device's anonymous MSIME account under `directory` (an absolute path; `anonymous-account.json` and `anonymous-session.json`) unless a session is already there, on a worker thread. Resolves with `{ok,value}` or `{ok:false,error}`; rejects only when the worker produced no answer.
 */
export const ensureAnonymousAccount: (directory: string) => Promise<string>;
/**
 * Starts the usage-reporting session at keyboard start: `{directory,platform,version,preferences_directory}` in, `{enabled,crash_record_path?,previous_session_crashed?,crashes?}` out. Closes the previous session (`session_crash` only when it left a crash record), queues crash records and today's `active`, writes the new session marker. With `usage_reporting` off it clears instead. Small files only, no network.
 */
export const telemetryBegin: (request: string) => string;
/** `{directory}`: the keyboard is shutting down normally; queues the `session` event. No network. */
export const telemetryEnd: (request: string) => string;
/** `{directory,message,stack}` from the crash observer: writes the running session's crash record, sent on the next start. No network. */
export const telemetryRecordCrash: (request: string) => string;
/** The same request as `telemetryBegin`; queues today's `active` and sends the queue on a worker thread. Resolves with `{ok,value:{enabled,sent,dropped,remaining,deferred}}`. */
export const telemetryFlush: (request: string) => Promise<string>;
/** `{directory}`: the user just turned usage reporting off; drops the queue, the session marker and crash records. */
export const telemetryClear: (request: string) => string;
/**
 * `{directory,platform,channel?}` in; `{items:[{id,title,body,html,targets,channels,published_at}]}` out on a worker thread, newest first, without dismissed ones. `html` is the Markdown body rendered with raw HTML escaped and only http, https and mailto links kept. Fetched at most once a minute; the cached copy answers otherwise.
 */
export const notices: (request: string) => Promise<string>;
/** `{directory,id}`: remembers that the user dismissed notice `id`. */
export const noticeDismiss: (request: string) => string;
/**
 * `{platform,current_version,edition?,arch?}` in; on a worker thread, `{ok,value:{status:"available"|"current",update:{version:{display,parts},release_url,installer_name,installer_sha256,signed}}}` or `{ok,value:{status:"none"}}` out, from the newest published release of `platform` on GitHub. Waits up to ten seconds for the network.
 */
export const updateCheck: (request: string) => Promise<string>;
/**
 * Decodes the WAV sample at `sample` once per semitone and writes each note to `<directory>/note-<index>.wav` at 48 kHz, pitched as a playback rate. Resolves with the files in semitone order; rejects a sample that is not WAV, lasts longer than `maxMillis`, or decodes past its declared length.
 */
export const keySoundRenderNotes: (
  sample: string,
  semitones: number[],
  directory: string,
  maxMillis: number,
) => Promise<string[]>;
/** The statistics master switch under an absolute state directory: 1 on, 0 off or never written, -1 for an invalid directory or unreadable document. */
export const typingStatisticsEnabled: (directory: string) => number;
export const vocabularyReview: (request: string) => Promise<string>;
/** Locked mobile history operations; every mutation answers with the latest complete entry list. */
export const mobileClipboardHistory: (request: string) => string;
export const emojiCatalog: (query: string, resources: string) => string;
export const candidateGlosses: (request: string, resources: string) => string;
/** `{prefix,limit}` against the packaged English dictionary; no session, safe off the UI thread. */
export const englishCompletions: (request: string, resources: string) => string;
/** Clear the session's Engine candidate cache and refresh its view. */
export const resetCache: (handle: number) => string;
export const translationGlossSave: (request: string, userData: string) => string;
export const translationPlan: (request: string) => string;
/** Google 登录的回环判定（`msime_client_google_loopback`）：operation 为 target、plan 或 reply，请求与返回值见头文件。设置应用自己监听 127.0.0.1、打开浏览器、经系统 HTTPS 栈申请 challenge 和提交授权码。 */
export const googleLoopback: (request: string) => string;
export const tencentTranslationHttpRequest: (request: string) => string;
export const niuTransTranslationHttpRequest: (request: string) => string;
export const customTranslationHttpRequest: (request: string) => string;
export const parseTencentTranslationResponse: (body: string, expected: number) => string;
export const parseNiuTransTranslationResponse: (body: string) => string;
export const parseCustomTranslationResponse: (body: string) => string;
export const translationQuery: (handle: number) => string;
export const onlineQuery: (handle: number) => string;
export const cloudRequestUrl: (query: string) => string;
export const aiRequestForQuery: (handle: number, query: string) => string;
export const aiHttpRequest: (request: string) => string;
export const parseAiResponse: (body: string, limit: number) => string;
export const applyCloudResponse: (handle: number, query: string, body: string) => string;
export const applyOnlineCandidates: (
  handle: number,
  query: string,
  candidates: string,
  source: number,
) => string;
export const personalDictionarySync: (options: string) => string;
/**
 * `{options,action}` against the queued personal dictionary rather than the Engine.
 *
 * The Engine route needs the maintenance lock and so needs the keyboard not to hold a session;
 * this one writes a queue the keyboard drains at its next session start. Used for importing a
 * file, where "maintenance busy" is not an answer to "add these words".
 */
export const personalDictionaryRequest: (request: string) => string;
export const prepareHost: (options: string) => string;

export const snapshotVersion: (options: string) => string;
export const snapshotInspect: (file: string) => string;
export const snapshotQueue: (request: string) => string;
/** Revalidates and streams a private snapshot from a native worker thread. */
export const snapshotRestore: (request: string, file: string) => Promise<string>;
export const snapshotPrepare: (request: string, file: string) => string;
export const snapshotDiscard: (handle: number) => string;
export const snapshotActivate: (handle: number, expected: string) => string;

export const create: (options: string) => string;
export const destroy: (handle: number) => string;
export const focus: (handle: number, focused: boolean) => string;
export const setNineKeyMode: (handle: number, enabled: boolean) => string;
export const setEnglishMode: (handle: number, enabled: boolean) => string;
/** Whether ASCII is committed as its fullwidth twin, which Ctrl+Shift+F toggles. */
export const setCharacterWidth: (handle: number, fullwidth: boolean) => string;
/** `msime_client_set_private_session`：把会话标记为私密（隐私模式），`host-api` 因此不再为它统计候选位置。学习本身仍跟随会话的 `learning` 偏好。应答 `{ok, value: enabled}`。 */
export const setPrivateSession: (handle: number, enabled: boolean) => string;

export const character: (handle: number, ascii: number, shift: boolean) => string;
/**
 * `msime_client_glide`: one glide stroke (滑行输入) across the letter keys, `request` being `{"keys":[[x,y] x 26],"key_width":w,"key_height":h,"points":[[x,y,ms], 2..1024]}` (at most 65536 bytes, unknown keys refused). Answers like `character`; `handled` false means the stroke was not typed (not quanpin, a local mode, dedicated English, nine-key digits composing, or nothing decoded) and its keys must not be typed either.
 */
export const glide: (handle: number, request: string) => string;
export const punctuationWithContext: (handle: number, ascii: number, preceding: number) => string;
export const balancePairedPunctuationAfterAutoClose: (handle: number, opening: number) => string;
export const command: (handle: number, command: number) => string;
/**
 * `msime_client_typing_effect`: count one key or commit into the session's combo and answer what to draw, packed into one integer. `event` bits 0-7 are 0 any other key, 1 space, 2 enter, 3 backspace, 4 commit, 5 a delete by another route; 0x100 marks an auto-repeat, 0x200 keeps the tier-up sound quiet. The answer's bits 0-15 are the combo count, bit 16 a tier-up, bits 17-19 the style (0 off, 1 flash, 2 sparks, 3 power mode) and bit 20 a tier-up sound this host plays itself; `TypingEffectPolicy.decode` unpacks it. 0 while the effect and the combo counter are both off. No disk, no allocation: safe on the key path.
 */
export const typingEffect: (handle: number, event: number) => number;
/**
 * `msime_client_typing_effect_settings`: the session's resolved typing effect, `{ok, value: {pack, issue, style, intensity, colors, duration_ms, particles, combo_counter}}`. With an effect pack selected its parameters replace the preference values; a pack that does not load answers style off with `issue` saying why. Read it after the preferences change or a field gains focus, not per key.
 */
export const typingEffectSettings: (handle: number) => string;

export const select: (handle: number, generation: number, index: number) => string;
export const selectEdge: (
  handle: number,
  generation: number,
  index: number,
  edge: number,
) => string;
export const selectAnyCandidate: (handle: number, generation: number, index: number) => string;
export const pinCandidate: (handle: number, generation: number, index: number) => string;
export const fixCandidatePosition: (
  handle: number,
  generation: number,
  index: number,
  position: number,
) => string;
export const clearCandidatePosition: (handle: number, generation: number, index: number) => string;
export const removeCandidate: (handle: number, generation: number, index: number) => string;
export const chooseNineKeySpelling: (handle: number, generation: number, index: number) => string;
/** 九宫格候选筛选：只留单字，并按首字笔顺前缀（`hspnz`，<=64 字节，空串不按笔画）筛选；答复同其他输入调用。 */
export const setNineKeyFilter: (
  handle: number,
  singleCharacter: boolean,
  strokes: string,
) => string;

export const view: (handle: number) => string;
export const allCandidates: (handle: number) => string;
export const applyTranslations: (
  handle: number,
  generation: number,
  translations: string,
) => string;
/** Start/cancel the shared voice generation used to reject stale asynchronous recognition. */
export const voiceStart: (handle: number) => string;
export const voiceCancel: (handle: number) => string;
export const voiceApply: (handle: number, generation: number, text: string) => string;
/** The user's own pinyin words as recognizer hotwords. Reads the dictionary store on a native worker thread. */
export const voiceHotwords: (request: string) => Promise<string>;
export const voiceHotwordCorrect: (request: string) => string;
export const voiceLocalModels: (request: string) => string;
/** Downloads and installs one catalog model on a native worker thread; `progress` receives each `{id,stage,downloaded,total}` document on the JS thread. */
export const voiceLocalModelInstall: (
  request: string,
  progress?: (document: string) => void,
) => Promise<string>;
export const voiceLocalModelCancel: (request: string) => string;
export const voiceLocalModelRemove: (request: string) => Promise<string>;

export interface DoubaoFrameResult {
  last: boolean;
  payload: string;
}

/** Native gzip framing keeps the ArkTS WebSocket adapter free of credential or transcript logging. */
export const doubaoEncodeFrame: (
  messageType: number,
  flags: number,
  sequence: number,
  payload: ArrayBuffer,
) => ArrayBuffer;
export const doubaoDecodeFrame: (frame: ArrayBuffer) => DoubaoFrameResult | null;
