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
/** Staged engine resources in; `{profile,sourceCommit}` from the packaged dictionary manifest out. */
export const dictionaryManifest: (resources: string) => string;
/** JSON resource request; returns a structured response containing contentType and byte values. */
export const skinResource: (request: string) => string;
/** JSON stylesheet request; returns a nullable stylesheet in the structured response. */
export const skinToolbarStylesheet: (request: string) => string;
/**
 * `{directory}` reads the named custom touch-keyboard designs; adding `{action}` applies one change
 * first. Both answer with the whole library. Takes the library's file lock, so call it off the UI
 * thread when the design carries a photo.
 */
export const customSkinLibrary: (request: string) => string;
/**
 * `{directory,id,name,design}` starts a skin trial and imports the design, answering `{skin,trial}`.
 * One call rather than two: the trial remembers the skin being replaced, so a failed import has to
 * end it. Writes preferences and two locked files, so call it off the UI thread.
 */
export const communitySkinInstall: (request: string) => string;
/** `{directory,action:{operation:"finish",id,keep}}` or `{operation:"restore_pending"}`. */
export const keyboardSkinTrial: (request: string) => string;
/**
 * `{file,action:{operation:"load"|"save_reply"|"remove",...}}` over the reply templates the user
 * kept. Every operation answers with the whole library; the keyboard process rereads the same file.
 */
export const communityResourceLibrary: (request: string) => string;
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
export const typingStatistics: (request: string) => string;
export const vocabularyReview: (request: string) => string;
/**
 * Locked mobile history operations. Harmony opts into migration of its original
 * `state/clipboard-history.json`; every mutation answers with the latest complete entry list.
 */
export const mobileClipboardHistory: (request: string) => string;
export const emojiCatalog: (query: string, resources: string) => string;
export const candidateGlosses: (request: string, resources: string) => string;
/** `{prefix,limit}` against the packaged English dictionary; no session, safe off the UI thread. */
export const englishCompletions: (request: string, resources: string) => string;
/** Clear the session's Engine candidate cache and refresh its view. */
export const resetCache: (handle: number) => string;
export const translationGlossSave: (request: string, userData: string) => string;
export const translationPlan: (request: string) => string;
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

export const character: (handle: number, ascii: number, shift: boolean) => string;
export const punctuationWithContext: (handle: number, ascii: number, preceding: number) => string;
export const balancePairedPunctuationAfterAutoClose: (handle: number, opening: number) => string;
export const command: (handle: number, command: number) => string;

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
export const voiceLocalModelRemove: (request: string) => string;

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
