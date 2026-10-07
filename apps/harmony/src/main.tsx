import { StrictMode } from "react";
import { type ReactNode, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  SettingsPage,
  SettingsStartupPage,
  WelcomeFlowPage,
  type DictionaryClient,
  type DictionaryEntry,
  type DictionaryManifest,
  type DictionaryImportResult,
  type HostCapabilities,
  type LocalDictionaryFormat,
  type LocalDictionaryKind,
  type Preferences,
  type SavedTouchKeyboardSkin,
  type SkinCatalog,
  type SkinFont,
  type SkinImage,
  CloudClipboardPanel,
  CloudDictionaryPanel,
  CloudDictionaryCatalogPanel,
  CloudDictionaryFilesPanel,
  CloudDictionaryApplyPanel,
  CloudCandidatesPanel,
  completeOnboardingPreferences,
  type AccountClient,
  type AccountPreferences,
  type AiSkinClient,
  type AiSkinProposal,
  type CommunityResource,
  type CommunityResourceApplication,
  type CommunityResourceClient,
  type CommunityResourcePage,
  type CommunitySkin,
  type CommunitySkinClient,
  type CommunitySkinDownload,
  type CommunitySkinPage,
  type AccountPreferenceSchema,
  type SettingsSyncClient,
  type ChatClient,
  type ChatModels,
  type CloudClipboardPanelClient,
  type CloudDictionaryAction,
  type CloudDictionaryPanelClient,
  type SettingsClient,
  type Snapshot,
  type StatisticsRetention,
  type TypingStatisticsClient,
  type VocabularyReviewClient,
  type VocabularyReviewStatus,
  type TypingStatisticsStatus,
  type LocalVoiceModelClient,
  type LocalVoiceModelList,
  type LocalVoiceModelProgress,
  type MentionEntry,
  type PluginCatalogResult,
  type PluginClient,
  type PluginPackage,
  UNBATCHED_DICTIONARY_FILE_BYTES,
} from "@msime/ui";
import type {
  AiAssistantClient,
  ApiCredentialTestResult,
  ApiCredentialTestService,
  MobileKeyboardFeedback,
  VoiceCaptureDevice,
} from "@msime/ui";
import "@msime/ui/styles.css";

/**
 * The settings UI, hosted by the HarmonyOS application.
 *
 * Everything the page needs from the system arrives through one object the ArkTS side injects into
 * the Web component. It is deliberately thin: JSON in, JSON out, the same shape the shared C ABI
 * uses, so the bridge has no opinion about preferences and nothing to keep in sync with the schema.
 *
 * Only `load` and `save` are required of a SettingsClient. Everything else on the interface is a
 * capability some host happens to have, and the page renders what the host says it can do rather
 * than what the platform is called, so an unimplemented surface simply does not appear.
 */

interface NativeBridge {
  /** `{"ok":true,"value":{"revision":n,"preferences":{...}}}` or `{"ok":false,"error":"..."}`. */
  loadPreferences(): string;
  savePreferences(expectedRevision: number, document: string): string;
  /** The capability record for this host, as client-core writes it. */
  hostCapabilities(): string;
  scanSkinCatalog(): string;
  readSkinImage(id: string, relative: string): string;
  readSkinFont(id: string, relative: string): string;
  readSkinToolbarCss(id: string): string;
  /** `{profile,sourceCommit}` from the packaged dictionary manifest, or a refusal. */
  dictionaryManifest(): string;
  appVersion(): string;
  dictionary(action: string): string;
  cloudDictionaryDownload(entry: string): string;
  typingStatistics(action: string): string;
  /**
   * Starts one of the asynchronous requests and returns at once.
   *
   * A bridge method that returns a Promise never settles on this platform, so the account, the
   * account-backed chat, the cloud dictionary and its snapshots, the AI model list, the AI test and
   * the credential test are started by number and answered later through `msimeHarmonyBridgeReply`.
   *
   * The caller chooses the deadline, because the kinds do not share one: a completion is a model
   * writing text and is given the same 125 seconds the shared clients allow it, while everything
   * else answers from a database and should report a stalled network in seconds.
   */
  startRequest(kind: string, id: number, payload: string): string;
  openExternalUrl(url: string): void;
  copyText(text: string): void;
  openSystemKeyboardSettings(): void;
  /** `{"ok":true,"value":[{backend,id,label},...]}`; a refusal is reported, not an empty list. */
  listVoiceCaptureDevices(): string;
  /** `{"ok":true,"value":["Family",...]}`; a refusal is reported so the page can say so. */
  listFontFamilies(): string;
  /** `{operation:"load"|"save"|"preview",...}`; the reply carries the settings now in force. */
  keyboardFeedback(request: string): string;
  /**
   * Whether setup still has a step left: not enabled, or enabled but not the current keyboard.
   *
   * Synchronous, and the reply says whether the answer is ready yet — a bridge method that returns
   * a Promise never settles on this platform, so the host prepares the answer instead and the page
   * waits for `ready` rather than acting on a default.
   */
  onboardingStatus(): string;
  /** The system's keyboard picker, which is where the second setup step happens. */
  showInputMethodPicker(): string;
  /** Starts the picker and answers at once; the panel's own rescan is what shows the result. */
  importSkinFolder(): string;
  /**
   * The 插件 page's pack store and @ name list: `{operation:"catalog"|"remove"|"load_mentions"|"save_mentions",...}`, answered by `msime_client_plugins` as `{ok,value}` or `{ok:false,error,detail?}`. An import waits for the system picker, so it goes through `startRequest` as `plugin_import` instead.
   */
  plugins(action: string): string;
}

declare global {
  // eslint-disable-next-line no-var
  var msimeHarmonyPreferencesChanged: ((reply: string) => void) | undefined;
  // eslint-disable-next-line no-var
  var msimeHarmonyBridgeReply: ((id: number, reply: string) => void) | undefined;
  // eslint-disable-next-line no-var
  var msimeHarmonyAiSkinProgress: ((requestId: string, completed: number) => void) | undefined;
  // eslint-disable-next-line no-var
  var msimeHarmonyVoiceModelProgress: ((document: string) => void) | undefined;
}

/**
 * The asynchronous half of the bridge.
 *
 * The host cannot answer through a returned Promise on this platform, so a request is started by
 * number and the answer arrives later on a global the host calls. Each request keeps its own
 * resolver until then.
 *
 * Numbers are per page load and never reused: a reply for a number nobody is waiting on belongs to
 * a request that timed out or was made before a reload, and delivering it to whoever holds that
 * number now would answer the wrong question.
 *
 * Requests do time out. A host that never replies would otherwise leave a settings control
 * spinning with nothing to cancel it, and a reported failure is something the page can show.
 */
const pendingBridgeRequests = new Map<number, (reply: string) => void>();
let nextBridgeRequestId = 1;

globalThis.msimeHarmonyBridgeReply = (id: number, reply: string) => {
  const resolve = pendingBridgeRequests.get(id);
  if (!resolve) return;
  pendingBridgeRequests.delete(id);
  resolve(reply);
};

function bridgeRequest(
  native: NativeBridge,
  kind: string,
  payload: string,
  timeoutMs = 30000,
): Promise<string> {
  const id = nextBridgeRequestId++;
  return new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => {
      if (!pendingBridgeRequests.delete(id)) return;
      reject(new Error("请求超时，请重试。"));
    }, timeoutMs);
    pendingBridgeRequests.set(id, (reply) => {
      clearTimeout(timer);
      resolve(reply);
    });
    try {
      native.startRequest(kind, id, payload);
    } catch (error) {
      clearTimeout(timer);
      pendingBridgeRequests.delete(id);
      reject(error instanceof Error ? error : new Error(String(error)));
    }
  });
}

declare global {
  interface Window {
    msimeHarmony?: NativeBridge;
  }
}

interface Reply<T> {
  ok: boolean;
  value: T;
  error: string;
}

/**
 * Rejects the way the desktop host rejects: a plain record carrying a code.
 *
 * The shared settings UI decodes a failure by reading `error.code`, and only reads `message` off an
 * `Error` as a fallback. An `Error` carrying the code in its message therefore matched none of those
 * tables: every account failure arrived as the one generic sentence instead of the wording written
 * for it, a cancelled sign-in was not recognised as cancelled, a dictionary failure lost its
 * specific advice, and an AI failure put the machine code itself on screen.
 */
function unwrap<T>(raw: string): T {
  const reply = JSON.parse(raw) as Reply<T>;
  if (!reply.ok) throw { code: reply.error };
  return reply.value;
}

/** The startup failure has no settings page left to decode it, so it reads both shapes itself. */
function startupFailure(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null && "code" in error) {
    return String((error as { code: unknown }).code);
  }
  return String(error);
}

/**
 * The injected object is not guaranteed to exist the instant the document's script runs, and reading
 * it at module scope would take the whole page down with it when it does not. Wait for it instead,
 * and say so plainly if it never arrives rather than leaving a blank window to be puzzled over.
 */
function whenBridgeReady(): Promise<NativeBridge> {
  return new Promise((resolve, reject) => {
    const deadline = Date.now() + 5000;
    const poll = () => {
      const native = window.msimeHarmony;
      if (native) {
        resolve(native);
        return;
      }
      if (Date.now() > deadline) {
        reject(new Error("没有连接到水杉输入法。请从应用中打开设置。"));
        return;
      }
      setTimeout(poll, 50);
    };
    poll();
  });
}

/**
 * The host's answer to "is setup finished", once it has one.
 *
 * The reply carries `ready` because the host computes it while the window is being created and the
 * page may ask first. Waiting a moment is right where guessing is not: a default of "finished"
 * skips the welcome flow for someone who has not enabled the keyboard, and a default of
 * "unfinished" shows it to someone who has.
 *
 * It does give up. A host that never becomes ready is a host whose settings page should still open.
 */
async function whenOnboardingKnown(native: NativeBridge): Promise<boolean> {
  const deadline = Date.now() + 2000;
  for (;;) {
    try {
      const reply = JSON.parse(native.onboardingStatus()) as {
        ok: boolean;
        value: boolean;
        ready: boolean;
      };
      if (reply.ok && reply.ready) return reply.value;
    } catch {
      return false;
    }
    if (Date.now() > deadline) return false;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

function accountClient(native: NativeBridge): AccountClient {
  const request = <T,>(action: Record<string, unknown>): Promise<T> =>
    bridgeRequest(native, "account", JSON.stringify(action)).then(unwrap<T>);
  const user = (value: { id: string; display_name: string; created_at: string }) => ({
    id: value.id,
    displayName: value.display_name,
    createdAt: value.created_at,
  });
  const profile = (value: {
    user: { id: string; display_name: string; created_at: string };
    identities: { provider: string }[];
  }) => ({
    user: user(value.user),
    providers: value.identities.map((identity) => identity.provider),
  });
  return {
    status: async () => {
      const value = await request<{
        user: { id: string; display_name: string; created_at: string } | null;
      }>({ operation: "status" });
      return { user: value.user ? user(value.user) : null };
    },
    providers: async () => {
      const value = await request<{ providers: Record<string, boolean> }>({
        operation: "providers",
      });
      return {
        email: value.providers.email === true,
        phone: value.providers.phone === true,
        apple: value.providers.apple === true,
      };
    },
    requestCode: async (provider, target) => {
      const value = await request<{ challenge_id: string; expires_in: number }>({
        operation: "request_code",
        provider,
        target,
      });
      return { challengeId: value.challenge_id, expiresIn: value.expires_in };
    },
    login: async (challengeId, code) => {
      const value = await request<{
        user: { id: string; display_name: string; created_at: string };
      }>({ operation: "login", challenge_id: challengeId, credential: code });
      return { user: value.user ? user(value.user) : null };
    },
    profile: async () =>
      profile(
        await request<{
          user: { id: string; display_name: string; created_at: string };
          identities: { provider: string }[];
        }>({ operation: "profile" }),
      ),
    rename: async (displayName) =>
      profile(
        await request<{
          user: { id: string; display_name: string; created_at: string };
          identities: { provider: string }[];
        }>({ operation: "rename", display_name: displayName }),
      ),
    logout: async (all) => {
      await request({ operation: "logout", all });
    },
    deleteAccount: async () => {
      await request({ operation: "delete_account" });
    },
    clearExpired: async () => {
      await request({ operation: "clear_expired" });
    },
    settingsSync: settingsSyncClient(native),
  };
}

/**
 * Settings sync, which is the only account surface that writes to this device.
 *
 * The host does the mapping, not the page: which local settings are account settings is a decision
 * about this platform's preference document, and the page has no business holding a table of its
 * keys. All four calls are one bridge request each.
 *
 * `apply` names the account the values were read under. The host checks it against the live session
 * before and after the round trip, because a sign-out while the confirmation is on screen would
 * otherwise write a stranger's settings over the user's own — and nothing on that screen could put
 * them back.
 */
function settingsSyncClient(native: NativeBridge): SettingsSyncClient {
  const request = <T,>(action: Record<string, unknown>): Promise<T> =>
    bridgeRequest(native, "settings_sync", JSON.stringify(action)).then(unwrap<T>);
  return {
    schema: () => request<AccountPreferenceSchema>({ operation: "preferences_schema" }),
    load: () => request<AccountPreferences>({ operation: "preferences_load" }),
    upload: () => request<AccountPreferences>({ operation: "preferences_upload" }),
    apply: async (userId) => {
      await request({ operation: "preferences_apply", user_id: userId });
    },
  };
}

/**
 * The account-backed assistant, which signs in with the session the host already holds.
 *
 * This is not the user-configured AI service on the AI page: that one carries the user's own
 * endpoint and token and is reached through `aiAssistant`. This one has no credential to configure,
 * which is why the page offers it only once an account exists.
 *
 * The completion gets its own deadline. The host allows a model 125 seconds to answer, so a page
 * that gave up at 30 would report a timeout for a request that was about to succeed — and then the
 * reply would arrive for a number nobody is waiting on and be dropped.
 */
function chatClient(native: NativeBridge): ChatClient {
  return {
    models: async () =>
      unwrap<ChatModels>(
        await bridgeRequest(
          native,
          "chat",
          JSON.stringify({ operation: "chat", chat_operation: "models" }),
        ),
      ),
    complete: async (messages, model) =>
      unwrap<{ content: string }>(
        await bridgeRequest(
          native,
          "chat",
          JSON.stringify({ operation: "chat", chat_operation: "complete", messages, model }),
          130000,
        ),
      ).content,
  };
}

/**
 * The community skin gallery.
 *
 * Browsing is not gated on an account: the gallery is public, and a signed-out user who could not
 * look at it would have no way to decide whether an account is worth making. The host attaches the
 * session when there is one, which is what turns `owned` and `my_rating` into this user's answers.
 *
 * `download` is the only call that is more than a request. The host fetches the design, starts a
 * trial with it and imports it into the library in one step, and answers with both halves: the
 * saved skin for the library the page just grew, and the trial id the page answers 保留 or 还原
 * with afterwards.
 */
function communitySkinClient(native: NativeBridge): CommunitySkinClient {
  const request = <T,>(action: Record<string, unknown>): Promise<T> =>
    bridgeRequest(
      native,
      "community_skin",
      JSON.stringify({ operation: "community_skin", ...action }),
    ).then(unwrap<T>);
  return {
    // 我的作品 asks the host for scope "mine", which it sends with the session and fields=moderation so a removed skin carries its 已下架 badge.
    list: (offset, search, mine, category) =>
      request<CommunitySkinPage>({
        community_operation: "list",
        offset,
        search,
        ...(mine ? { scope: "mine" } : {}),
        category,
      }),
    detail: (id) => request<CommunitySkin>({ community_operation: "detail", id }),
    download: (id, name) =>
      request<CommunitySkinDownload>({ community_operation: "download", id, name }),
    rate: async (id, stars) => {
      await request({ community_operation: "rate", id, stars });
    },
    publish: async (id, name, description, design, category) => {
      await request({ community_operation: "publish", id, name, description, design, category });
    },
    setCategory: (id, category) =>
      request<CommunitySkin>({ community_operation: "set_category", id, category }),
    unpublish: async (id) => {
      await request({ community_operation: "unpublish", id });
    },
    finishTrial: async (id, keep) => {
      await request({ community_operation: "finish_trial", id, keep });
    },
    report: async (id, reason, detail) => {
      await request({ community_operation: "report", id, reason, detail });
    },
  };
}

/**
 * Community dictionaries and reply templates.
 *
 * The public list and one resource's detail read without an account, the way the skin gallery does.
 * 我的作品 and 收藏 do not: they are questions about an account, and answering them without one
 * would either be empty or be somebody else's.
 *
 * `storeReply` and `removeReply` never reach the network. They write the local file the keyboard
 * process rereads when a reply is asked for, which is the only thing the two sides of this app
 * share about the community — and it holds only what the user chose to keep.
 */
function communityResourceClient(native: NativeBridge): CommunityResourceClient {
  const request = <T,>(action: Record<string, unknown>): Promise<T> =>
    bridgeRequest(
      native,
      "community_resource",
      JSON.stringify({ operation: "community_resource", ...action }),
    ).then(unwrap<T>);
  return {
    list: (kind, scope, search, offset) =>
      request<CommunityResourcePage>({
        resource_operation: "list",
        kind,
        scope,
        search,
        offset,
      }),
    detail: (id) => request<CommunityResource>({ resource_operation: "detail", id }),
    publish: async (id, kind, name, description, content, revision) => {
      await request({
        resource_operation: "publish",
        id,
        kind,
        name,
        description,
        content,
        revision,
      });
    },
    apply: (id, resourceRevision) =>
      request<CommunityResourceApplication>({
        resource_operation: "apply",
        id,
        resource_revision: resourceRevision,
      }),
    save: async (id, saved) => {
      await request({ resource_operation: "save", id, saved });
    },
    rate: async (id, stars) => {
      await request({ resource_operation: "rate", id, stars });
    },
    unpublish: async (id) => {
      await request({ resource_operation: "unpublish", id });
    },
    storeReply: async (item) => {
      await request({ resource_operation: "store_reply", item });
    },
    removeReply: async (id) => {
      await request({ resource_operation: "remove_reply", id });
    },
    report: async (kind, id, reason, detail) => {
      await request({ resource_operation: "report", kind, id, reason, detail });
    },
  };
}

/**
 * AI skin generation, which is the one request that reports before it answers.
 *
 * Three pictures take minutes, so a run that said nothing until it finished would be
 * indistinguishable from one that had stopped. Progress arrives on its own global, the same
 * direction the preference-change notification uses, carrying the request id the page chose — a
 * stale run's counter must not drive a new one's display.
 *
 * The deadline is the sum of what the pieces are allowed: a chat completion may take 125 seconds
 * and each picture up to 200, and the three pictures run together. Giving this the ordinary 30
 * would report a timeout for a run that was working.
 */
function aiSkinClient(native: NativeBridge): AiSkinClient {
  return {
    generate: (requestId, prompt) =>
      bridgeRequest(
        native,
        "ai_skin",
        JSON.stringify({ operation: "generate", request_id: requestId, prompt }),
        360000,
      ).then(unwrap<AiSkinProposal[]>),
    cancel: async (requestId) => {
      await bridgeRequest(
        native,
        "ai_skin",
        JSON.stringify({ operation: "cancel", request_id: requestId }),
      ).then(unwrap<Record<string, never>>);
    },
    onProgress: async (listener) => {
      const previous = globalThis.msimeHarmonyAiSkinProgress;
      globalThis.msimeHarmonyAiSkinProgress = (requestId: string, completed: number) => {
        listener({ requestId, completed });
      };
      return () => {
        globalThis.msimeHarmonyAiSkinProgress = previous;
      };
    },
  };
}

/**
 * The on-device model store: the catalog models the `local` provider can run, downloaded into the app's own files directory.
 *
 * Every operation is an asynchronous bridge request, because an install blocks for the whole download on a native worker and a bridge method returning a Promise never settles here. Progress arrives on its own global as the `{id,stage,downloaded,total}` document client-core reports. The host drops the catalog's desktop-only models, and it has already turned the core's error text into the codes the page has sentences for.
 *
 * An install is given hours rather than the ordinary 30 seconds: it is a download of up to a few hundred megabytes on whatever network the phone has, and client-core reports a stalled connection itself. A deadline that fired first would report a failure for a download that then finishes.
 */
const voiceModelProgressListeners = new Set<(progress: LocalVoiceModelProgress) => void>();

globalThis.msimeHarmonyVoiceModelProgress = (document: string) => {
  let progress: LocalVoiceModelProgress;
  try {
    progress = JSON.parse(document) as LocalVoiceModelProgress;
  } catch {
    return;
  }
  for (const listener of voiceModelProgressListeners) listener(progress);
};

/**
 * Rejects the way the desktop shell's plugin commands reject: `{code, detail}`, where detail is the rule client-core reports for a refused pack or name, so `pluginErrorMessage` can say which file or entry to fix.
 */
function unwrapPlugin<T>(raw: string): T {
  const reply = JSON.parse(raw) as Reply<T> & { detail?: string };
  if (!reply.ok) throw { code: reply.error, detail: reply.detail ?? null };
  return reply.value;
}

/**
 * The 插件 page's host side. The host fills in the state root and the bundle's built-in sound packs, so the page, as on the desktop, never names a path. An import waits on the system picker for as long as the user leaves it open, so its deadline is the one the export save allows.
 */
function pluginClient(native: NativeBridge): PluginClient {
  const call = <T,>(action: Record<string, unknown>): T =>
    unwrapPlugin<T>(native.plugins(JSON.stringify(action)));
  return {
    catalog: async () => call<PluginCatalogResult>({ operation: "catalog" }),
    importPack: async (source) =>
      unwrapPlugin<PluginPackage | null>(
        await bridgeRequest(native, "plugin_import", JSON.stringify({ source }), 30 * 60 * 1000),
      ),
    remove: async (kind, id) => {
      call<null>({ operation: "remove", kind, id });
    },
    loadMentions: async () => call<MentionEntry[]>({ operation: "load_mentions" }),
    saveMentions: async (entries) => {
      call<null>({ operation: "save_mentions", entries });
    },
  };
}

function localVoiceModelClient(native: NativeBridge): LocalVoiceModelClient {
  const request = <T,>(action: Record<string, unknown>, timeoutMs?: number): Promise<T> =>
    bridgeRequest(native, "voice_local_model", JSON.stringify(action), timeoutMs).then(unwrap<T>);
  return {
    list: () => request<LocalVoiceModelList>({ operation: "list" }),
    install: (id) => request<string>({ operation: "install", id }, 6 * 60 * 60 * 1000),
    cancel: (id) => request<boolean>({ operation: "cancel", id }),
    remove: async (id) => {
      await request<null>({ operation: "remove", id });
    },
    onProgress: async (listener) => {
      voiceModelProgressListeners.add(listener);
      return () => {
        voiceModelProgressListeners.delete(listener);
      };
    },
  };
}

function cloudClipboardClient(native: NativeBridge, close: () => void): CloudClipboardPanelClient {
  return {
    close: async () => close(),
    copyText: async (text) => native.copyText(text),
    request: async (action) => {
      const { operation, ...payload } = action;
      const value = await bridgeRequest(
        native,
        "account",
        JSON.stringify({ operation: "clipboard", clipboard_operation: operation, ...payload }),
      );
      return unwrap<{ items?: { id: string; text: string }[]; enabled?: boolean }>(value);
    },
  };
}

type CloudDictionaryPage = "main" | "catalog" | "candidates" | "files" | "apply";

function cloudDictionaryClient(
  native: NativeBridge,
  close: () => void,
  setPage: (page: CloudDictionaryPage) => void,
): CloudDictionaryPanelClient {
  type Response = Awaited<ReturnType<CloudDictionaryPanelClient["request"]>>;
  return {
    close: async () => close(),
    back: async () => setPage("main"),
    openCatalog: async () => setPage("catalog"),
    openCandidates: async () => setPage("candidates"),
    openFiles: async () => setPage("files"),
    openApply: async () => setPage("apply"),
    snapshot: true,
    snapshotNative: true,
    exportNative: true,
    chooseSnapshotRestore: async () => {
      const snapshot = await bridgeRequest(
        native,
        "cloud_dictionary_snapshot",
        JSON.stringify({ operation: "snapshot_restore_preview", text: "" }),
        600000,
      );
      return unwrap<Response>(snapshot);
    },
    downloadToLocal: async (entry) => {
      unwrap<{ applied: boolean }>(native.cloudDictionaryDownload(JSON.stringify(entry)));
    },
    request: async (action: CloudDictionaryAction) => {
      const { operation, ...payload } = action;
      if (operation.startsWith("snapshot_")) {
        const snapshot = await bridgeRequest(
          native,
          "cloud_dictionary_snapshot",
          JSON.stringify(action),
          600000,
        );
        return unwrap<Response>(snapshot);
      }
      const value = await bridgeRequest(
        native,
        "cloud_dictionary",
        JSON.stringify({ operation: "dictionary", dictionary_operation: operation, ...payload }),
      );
      return unwrap<Response>(value);
    },
  };
}

function makeClient(
  native: NativeBridge,
  openCloudClipboard: () => void,
  openCloudDictionary: () => void,
): SettingsClient {
  const dictionaryReply = <T,>(action: Record<string, unknown>): T =>
    unwrap<T>(native.dictionary(JSON.stringify(action)));
  const dictionary: DictionaryClient = {
    list: async (offset: number, limit: number, kind?: LocalDictionaryKind, query?: string) =>
      dictionaryReply<{ entries: DictionaryEntry[]; has_more: boolean }>({
        operation: "list",
        offset,
        limit,
        ...(kind ? { kind } : {}),
        ...(query ? { query } : {}),
      }),
    edit: async (
      previous: DictionaryEntry | null,
      replacement: DictionaryEntry | null,
      request_id: string,
    ) => {
      dictionaryReply<{ applied: boolean }>({
        operation: "edit",
        previous,
        replacement,
        request_id,
      });
    },
    // The native bridge sends an import to the host in one request rather than in batches.
    maxImportFileBytes: UNBATCHED_DICTIONARY_FILE_BYTES,
    import: async (
      kind: LocalDictionaryKind,
      format: LocalDictionaryFormat,
      text: string,
      request_id: string,
    ): Promise<DictionaryImportResult> =>
      dictionaryReply<DictionaryImportResult>({
        operation: "import",
        kind,
        format,
        text,
        request_id,
      }),
    /**
     * The Apple-compatible personal dictionary file.
     *
     * Unlike the edits beside it, this one is queued rather than written: the keyboard may be open,
     * and the Engine's maintenance lock is not available while it is. The host writes the entries
     * to the queue the keyboard drains at its next session start, which is why the card says 已加入
     * 同步队列 rather than 已导入 — that is the truth on this host as it is on the Apple one.
     */
    importPersonal: async (text: string, request_id: string) =>
      dictionaryReply<{ queued: boolean; pending_count: number }>({
        operation: "import_personal",
        text,
        request_id,
      }),
    export: async (
      kind: LocalDictionaryKind,
      format: Exclude<LocalDictionaryFormat, "rime" | "hans">,
      offset: number,
      limit: number,
    ) =>
      dictionaryReply<{ text: string; has_more: boolean }>({
        operation: "export",
        kind,
        format,
        offset,
        limit,
      }),
    retry: async (request_id: string) => {
      dictionaryReply<{ applied: boolean }>({ operation: "retry", request_id });
    },
    dismissFailure: async (request_id: string) => {
      dictionaryReply<{ applied: boolean }>({ operation: "dismiss_failure", request_id });
    },
  };
  const typingStatistics: TypingStatisticsClient = {
    load: async () =>
      unwrap<TypingStatisticsStatus>(
        native.typingStatistics(JSON.stringify({ operation: "load" })),
      ),
    setEnabled: async (enabled: boolean) =>
      unwrap<TypingStatisticsStatus>(
        native.typingStatistics(JSON.stringify({ operation: "set_enabled", enabled })),
      ),
    setRetention: async (retention: StatisticsRetention) =>
      unwrap<TypingStatisticsStatus>(
        native.typingStatistics(JSON.stringify({ operation: "set_retention", retention })),
      ),
    reset: async () =>
      unwrap<TypingStatisticsStatus>(
        native.typingStatistics(JSON.stringify({ operation: "reset" })),
      ),
  };
  // Every call answers with the whole status; the native operation runs on a worker and the local
  // day is resolved on the ArkTS side, which is the process that knows the device's timezone.
  const vocabularyReview: VocabularyReviewClient = {
    load: async () =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(native, "vocabulary_review", JSON.stringify({ operation: "load" })),
      ),
    answer: async (word: string, known: boolean) =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(native, "vocabulary_review", JSON.stringify({ operation: "answer", word, known })),
      ),
    setSettings: async (settings) =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(
          native,
          "vocabulary_review",
          JSON.stringify({
            operation: "set_settings",
            wordbook: settings.wordbook,
            new_per_day: settings.newPerDay,
            session_limit: settings.sessionLimit,
          }),
        ),
      ),
    importWordbook: async (name: string, text: string) =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(native, "vocabulary_review", JSON.stringify({ operation: "import", name, text })),
      ),
    removeWordbook: async (wordbook: string) =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(native, "vocabulary_review", JSON.stringify({ operation: "remove", wordbook })),
      ),
    reset: async () =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(native, "vocabulary_review", JSON.stringify({ operation: "reset" })),
      ),
  };
  const aiAssistant: AiAssistantClient = {
    fetchModels: (configuration) =>
      bridgeRequest(native, "ai_models", JSON.stringify(configuration)).then(unwrap<string[]>),
    test: (configuration) =>
      bridgeRequest(native, "ai_test", JSON.stringify(configuration)).then(unwrap<string>),
  };
  const testApiCredential = async (
    service: ApiCredentialTestService,
    config: Record<string, unknown>,
  ): Promise<ApiCredentialTestResult> =>
    unwrap<ApiCredentialTestResult>(
      await bridgeRequest(native, "api_credential", JSON.stringify({ service, config })),
    );
  return {
    // Wrapped like every other reply from the shared ABI. Reading it as the record itself leaves every
    // capability undefined, which the page reads as "this host cannot", and the whole surface silently
    // shrinks to the few controls that have no capability behind them.
    host: unwrap<HostCapabilities>(native.hostCapabilities()),
    load: async () => unwrap<Snapshot>(native.loadPreferences()),
    // The keyboard writes preferences from its toolbar and the two are separate processes, so the
    // host calls this when the settings window comes back to the front and the document has moved.
    // The reply is the one loadPreferences would have returned, so both paths parse identically.
    onPreferencesChanged: async (listener) => {
      globalThis.msimeHarmonyPreferencesChanged = (reply: string) => {
        try {
          listener(unwrap<Snapshot>(reply));
        } catch {
          // A document this cannot read is not worth interrupting the page for; the next save still
          // has the store's revision check behind it.
        }
      };
      return () => {
        globalThis.msimeHarmonyPreferencesChanged = undefined;
      };
    },
    save: async (revision: number, preferences: Preferences) => {
      // The revision sent is the one the page read; the document carries the next. The store compares
      // the former against what is on disk and refuses the save if the keyboard moved in between.
      //
      // `format_version` is not optional: the C ABI parses this document as `PreferencesSnapshot`,
      // whose field is required and which denies unknown fields, so a document without it fails to
      // parse and comes back as `invalid preferences snapshot` — which this host maps to `format`,
      // the "配置文件无法读取或版本较新" banner. Leaving it out meant no save on HarmonyOS had ever
      // reached the disk: the page reported nothing, the banner sat above the fold, and
      // preferences.json was never created. Android and Apple both send it.
      const document = JSON.stringify({ format_version: 1, revision: revision + 1, preferences });
      return unwrap<Snapshot>(native.savePreferences(revision, document));
    },
    readAppVersion: async () => native.appVersion(),
    scanSkinCatalog: async () => unwrap<SkinCatalog>(native.scanSkinCatalog()),
    readSkinImage: async (id: string, relative: string) =>
      unwrap<SkinImage>(native.readSkinImage(id, relative)),
    readSkinFont: async (id: string, relative: string) =>
      unwrap<SkinFont>(native.readSkinFont(id, relative)),
    readSkinToolbarCss: async (id: string, relative?: string) =>
      relative ? null : unwrap<string | null>(native.readSkinToolbarCss(id)),
    openExternalUrl: async (url: string) => native.openExternalUrl(url),
    copyText: async (text: string) => native.copyText(text),
    openSystemKeyboardSettings: async () => native.openSystemKeyboardSettings(),
    // The source opens its skin folder so a skin can be dropped in. That folder is inside the
    // sandbox here, so the direction is reversed: the user points at a skin and it is copied in.
    // The page renders this behind the same control and says 导入皮肤 instead, because the host
    // capability tells it which of the two this is.
    openSkinDirectory: async () => {
      native.importSkinFolder();
    },
    // ArkWeb drops the page's download link, so the host saves the export through the system save picker instead. The deadline covers a user who leaves the picker open: timing out under them would report a failure for a file that is then written anyway.
    saveExport: async (name: string, contents: string) => {
      const reply = await bridgeRequest(
        native,
        "save_export",
        JSON.stringify({ name, contents }),
        30 * 60 * 1000,
      );
      try {
        return unwrap<string | null>(reply);
      } catch {
        throw new Error("无法保存导出文件，词库未导出。");
      }
    },
    listVoiceCaptureDevices: async () =>
      unwrap<VoiceCaptureDevice[]>(native.listVoiceCaptureDevices()),
    listFontFamilies: async () => unwrap<string[]>(native.listFontFamilies()),
    mobileKeyboardFeedback: {
      load: async () =>
        unwrap<MobileKeyboardFeedback>(
          native.keyboardFeedback(JSON.stringify({ operation: "load" })),
        ),
      save: async (settings) =>
        unwrap<MobileKeyboardFeedback>(
          native.keyboardFeedback(JSON.stringify({ operation: "save", settings })),
        ),
      preview: async (strength) => {
        unwrap<MobileKeyboardFeedback>(
          native.keyboardFeedback(JSON.stringify({ operation: "preview", strength })),
        );
      },
    },
    dictionary,
    typingStatistics,
    vocabularyReview,
    aiAssistant,
    testApiCredential,
    // Four surfaces the keyboard already honours. Each writes shared preferences and nothing else,
    // so opting in is all that was ever needed; without it the page saved nothing and the keyboard
    // went on reading defaults the user had no way to change.
    fuzzyPinyin: true,
    touchKeyboardSchemes: true,
    customTouchKeyboardSkins: true,
    // The manifest is packaged and never changes while the application runs, so this is read on
    // demand rather than kept: the dictionary page is not a screen anyone leaves open.
    dictionaryManifest: async () => unwrap<DictionaryManifest>(native.dictionaryManifest()),
    // A named design can carry a bounded photo. Both reads and writes use the numbered request
    // channel while the native worker parses and writes the library under its file lock.
    customSkinLibrary: {
      load: async () => unwrap<SavedTouchKeyboardSkin[]>(await bridgeRequest(native, "custom_skin_library", "")),
      mutate: async (action) =>
        unwrap<SavedTouchKeyboardSkin[]>(await bridgeRequest(native, "custom_skin_library", JSON.stringify(action))),
    },
    candidateEnglishGloss: true,
    account: accountClient(native),
    chat: chatClient(native),
    // The Apple home surface, adapted rather than copied. Two of its five actions exist here and
    // three do not, and the page draws only what the host says it has.
    //
    // The two setup actions are this host's: 设置 opens the system input-method list, and the
    // picker is where the second setup step happens.
    //
    // `openKeyboard` is deliberately absent. Android opens a separate panel window for it; this
    // host's keyboard is an InputMethodExtensionAbility that appears when an editor asks for it,
    // and there is no window for the settings app to open. Without the action the card falls back
    // to the shared screen-keyboard page, which is the honest version of "show me the keyboard"
    // here. The emoji and clipboard actions are absent for the same reason: on this host those are
    // surfaces on the keyboard's own key faces, not windows.
    home: {
      openSystemKeyboardSettings: async () => native.openSystemKeyboardSettings(),
      // The reply is unwrapped rather than ignored so a host that could not open the picker says
      // so, which the card reports; the welcome flow's own copy of this call is the exception,
      // because that screen has its own failure to show and nothing to add to it.
      showInputMethodPicker: async () => {
        unwrap<boolean>(native.showInputMethodPicker());
      },
    },
    communitySkins: communitySkinClient(native),
    communityResources: communityResourceClient(native),
    aiSkins: aiSkinClient(native),
    localVoiceModels: localVoiceModelClient(native),
    // The page is offered only on a 2in1, the form factor that plays packs and routes the / and @ modes; a phone hides it whatever the host supplies.
    plugins: pluginClient(native),
    openCloudClipboard: async () => openCloudClipboard(),
    cloudClipboardRequest: cloudClipboardClient(native, () => undefined).request,
    openCloudDictionary: async () => openCloudDictionary(),
  };
}

function HarmonySettings({
  native,
  onboarding,
}: {
  native: NativeBridge;
  onboarding: boolean;
}): ReactNode {
  // Setup is the one thing that has to happen before anything in the settings page can matter, so
  // the flow replaces the page rather than sitting somewhere inside it. Skipping is allowed: a
  // keyboard the user has decided to set up later is not a reason to withhold its settings.
  const [bootstrapRequired, setBootstrapRequired] = useState(onboarding);
  // The splash belongs to a first launch; a flow replayed from settings opens on its first step.
  const [replayed, setReplayed] = useState(false);
  // 登录 at the end of the flow opens the settings on 我的, where signing in lives.
  const [initialPage, setInitialPage] = useState<string>();
  const [cloudClipboardOpen, setCloudClipboardOpen] = useState(false);
  const [cloudDictionaryOpen, setCloudDictionaryOpen] = useState(false);
  const [cloudDictionaryPage, setCloudDictionaryPage] = useState<CloudDictionaryPage>("main");
  // Each client is built once per bridge. SettingsPage and the cloud panels key their load and subscription effects on the client, so a fresh one on every render (opening a cloud panel re-renders this component) reloaded the page and threw away the unsaved draft. The callbacks only close over state setters, which React keeps stable.
  const client = useMemo(
    () =>
      makeClient(
        native,
        () => setCloudClipboardOpen(true),
        () => {
          setCloudDictionaryPage("main");
          setCloudDictionaryOpen(true);
        },
      ),
    [native],
  );
  const cloudClipboard = useMemo(
    () => cloudClipboardClient(native, () => setCloudClipboardOpen(false)),
    [native],
  );
  const dictionaryClient = useMemo(
    () =>
      cloudDictionaryClient(native, () => setCloudDictionaryOpen(false), setCloudDictionaryPage),
    [native],
  );
  const filesClient = useMemo<CloudDictionaryPanelClient>(
    () => ({
      ...dictionaryClient,
      snapshot: true,
      snapshotNative: true,
    }),
    [dictionaryClient],
  );
  if (bootstrapRequired) {
    // A 2-in-1 reports mobile_settings false: the flow draws itself as a desktop sheet there and opens without the phone's splash.
    const mobileSettings = client.host?.mobile_settings;
    return (
      <WelcomeFlowPage
        actions={{
          platform: "harmony",
          mobileSettings,
          // Resources are staged by the keyboard when it starts, and there is no separate step to
          // run here; the flow expects the promise, not work.
          prepareResources: async () => {},
          openSystemKeyboardSettings: async () => native.openSystemKeyboardSettings(),
          showInputMethodPicker: async () => {
            native.showInputMethodPicker();
          },
        }}
        onComplete={async (scheme, choices) => {
          // The scheme picked in the flow is the whole point of that step; dropping it would leave
          // the user with a keyboard laid out the way they had just declined. Written the same way
          // the mobile hosts write it, so a profile carried between them means the same thing.
          const snapshot = await client.load();
          await client.save(
            snapshot.revision,
            completeOnboardingPreferences(snapshot, scheme, choices),
          );
          setInitialPage(choices.openAccount ? "account" : undefined);
          setBootstrapRequired(false);
        }}
        onSkip={async () => setBootstrapRequired(false)}
        splash={mobileSettings !== false && !replayed}
      />
    );
  }
  return (
    <>
      <SettingsPage
        client={client}
        initialPage={initialPage}
        onReplayOnboarding={() => {
          setReplayed(true);
          setBootstrapRequired(true);
        }}
      />
      {cloudClipboardOpen && <CloudClipboardPanel client={cloudClipboard} />}
      {cloudDictionaryOpen && cloudDictionaryPage === "main" && (
        <CloudDictionaryPanel client={dictionaryClient} />
      )}
      {cloudDictionaryOpen && cloudDictionaryPage === "catalog" && (
        <CloudDictionaryCatalogPanel client={dictionaryClient} />
      )}
      {cloudDictionaryOpen && cloudDictionaryPage === "candidates" && (
        <CloudCandidatesPanel client={dictionaryClient} />
      )}
      {cloudDictionaryOpen && cloudDictionaryPage === "files" && (
        <CloudDictionaryFilesPanel client={filesClient} />
      )}
      {cloudDictionaryOpen && cloudDictionaryPage === "apply" && (
        <CloudDictionaryApplyPanel client={dictionaryClient} />
      )}
    </>
  );
}

const root = document.getElementById("root");
if (root) {
  const app = createRoot(root);
  // Waiting for the bridge took up to five seconds against a blank white window. Every other host
  // shows the shared startup page while it opens; there was never a reason for this one not to.
  app.render(
    <StrictMode>
      <SettingsStartupPage />
    </StrictMode>,
  );
  whenBridgeReady()
    .then(async (native) => {
      // A refused query answers "no onboarding": someone who has been using the keyboard for weeks
      // should not be sent back to a welcome screen because one system call did not answer.
      const onboarding = await whenOnboardingKnown(native);
      app.render(
        <StrictMode>
          <HarmonySettings native={native} onboarding={onboarding} />
        </StrictMode>,
      );
    })
    .catch((error: unknown) => {
      // A blank window explains nothing. This is the one failure the page has to render itself,
      // because it is the failure that means none of the rest of it can be rendered at all. Both
      // rejection shapes reach here: the bridge itself refuses with an Error carrying a sentence,
      // while a refused host call arrives as a record carrying a code.
      root.textContent = startupFailure(error);
      root.setAttribute("style", "padding:24px;font:16px system-ui;color:#c0392b");
    });
}
