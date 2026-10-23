import {
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import { useLatestRef } from "../core/use-latest-ref";
import { useDebouncedCallback } from "../core/use-debounced-callback";
import { deepEqual } from "../core/deep-equal";
import { errorCode } from "../core/error-code";
import { errorMessage } from "../core/error-message";
import { trackAsyncSubscription } from "../core/async-subscription";
import { subscribeToDocumentVisibilityChange } from "../core/window-focus";
import { validCandidateFonts } from "../candidate/candidate-font-family";
import type { SettingsClient } from "../core/settings-client";
import type { Preferences, Snapshot } from "../core/preferences-model";
import {
  applyPreferenceChanges,
  preferenceChanges,
  preferenceChangesCollide,
} from "./preference-changes";
import { useAsyncGeneration } from "./use-async-generation";
import { useFlushOnWindowLeave } from "./use-flush-on-window-leave";

/** Where the automatic save of the settings form stands, for the quiet status in its action row. */
export type SettingsSaveState = "idle" | "saving" | "saved" | "failed";

/** How long the draft must stay unchanged before it is saved: a toggle applies almost at once, typing saves once it pauses. */
export const SETTINGS_AUTOSAVE_DELAY_MS = 400;
/** How long 已保存 stays in the action row after a save. */
export const SETTINGS_SAVED_STATUS_MS = 2000;
/** How many times one save rebases onto another window's newer revision before it reports the conflict. */
const CONFLICT_RETRIES = 3;

const mergedNotice = "设置同时在其他窗口修改，已合并。";
const externalNotice = "设置已从其他窗口更新。";

export interface UseSettingsPersistenceOptions {
  client: SettingsClient;
  mobile: boolean;
  mounted: RefObject<boolean>;
  snapshot: Snapshot | undefined;
  draft: Preferences | undefined;
  setSnapshot: Dispatch<SetStateAction<Snapshot | undefined>>;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  setBusy: (busy: boolean) => void;
  setError: (error: string) => void;
  setNotice: (notice: string) => void;
  setRecoveredBackup: (path: string) => void;
}

/** Owns loading, automatic saving, and cross-window synchronization of shared preferences. */
export function useSettingsPersistence({
  client,
  mobile,
  mounted,
  snapshot,
  draft,
  setSnapshot,
  setDraft,
  setBusy,
  setError,
  setNotice,
  setRecoveredBackup,
}: UseSettingsPersistenceOptions) {
  const snapshotRef = useRef(snapshot);
  const draftRef = useRef(draft);
  const clientRef = useLatestRef(client);
  const loadingClientRef = useRef<SettingsClient | undefined>(undefined);
  const [saveState, setSaveState] = useState<SettingsSaveState>("idle");
  const { schedule: scheduleSavedStatusReset, cancel: cancelSavedStatusReset } =
    useDebouncedCallback(
      () => setSaveState((state) => (state === "saved" ? "idle" : state)),
      SETTINGS_SAVED_STATUS_MS,
    );
  const [saveError, setSaveError] = useState("");
  const [loadFailed, setLoadFailed] = useState(false);
  const reloadInFlightRef = useRef<Promise<void> | undefined>(undefined);
  const loadTokenRef = useRef(0);
  const generation = useAsyncGeneration(client);

  useEffect(() => {
    if (loadingClientRef.current === client) return;
    snapshotRef.current = snapshot;
    draftRef.current = draft;
  }, [client, snapshot, draft]);

  const draftPending = () => {
    const currentSnapshot = snapshotRef.current;
    const currentDraft = draftRef.current;
    return (
      !!currentSnapshot && !!currentDraft && !deepEqual(currentDraft, currentSnapshot.preferences)
    );
  };
  const savePending = () => draftPending();

  // The refs are also written the moment a load or save resolves: the host's monitor echoes this
  // window's own save back as a change, and it can arrive before React commits the new snapshot.
  const adoptSnapshot = (value: Snapshot) => {
    if (!mounted.current) return;
    snapshotRef.current = value;
    draftRef.current = value.preferences;
    setSnapshot(value);
    setDraft(value.preferences);
  };

  // A save resolved: its result becomes the saved base, and whatever was edited after `sent` was captured stays on top of it.
  const adoptSaved = (value: Snapshot, sent: Preferences) => {
    const keepNewer = (current: Preferences | undefined) =>
      current
        ? applyPreferenceChanges(value.preferences, preferenceChanges(sent, current))
        : value.preferences;
    const nextDraft = keepNewer(draftRef.current);
    snapshotRef.current = value;
    draftRef.current = nextDraft;
    if (!mounted.current) return;
    setSnapshot(value);
    setDraft(nextDraft);
  };

  // Moves the local edits made since `base` onto `latest`, another writer's newer revision. A field both changed keeps the local value; the result says whether that happened.
  const rebase = (latest: Snapshot, base: Snapshot) => {
    const onto = (current: Preferences | undefined) =>
      current
        ? applyPreferenceChanges(latest.preferences, preferenceChanges(base.preferences, current))
        : latest.preferences;
    const local = preferenceChanges(base.preferences, draftRef.current);
    const remote = preferenceChanges(base.preferences, latest.preferences);
    const nextDraft = onto(draftRef.current);
    snapshotRef.current = latest;
    draftRef.current = nextDraft;
    if (!mounted.current) return preferenceChangesCollide(local, remote, latest.preferences);
    setSnapshot(latest);
    setDraft(nextDraft);
    return preferenceChangesCollide(local, remote, latest.preferences);
  };

  // A change that arrives while this window's save is in flight waits for it rather than being
  // judged against the revision the save is about to replace; the revision check then drops the
  // echo and still applies another window's later write.
  const savingRef = useRef(false);
  const saveTokenRef = useAsyncGeneration();
  // 保存归属只随宿主重置失效，卸载后仍需写完请求期间产生的编辑。
  const saveOwnerRef = useRef(0);
  const heldChange = useRef<Snapshot>(undefined);
  const applyPreferencesChange = (value: Snapshot) => {
    const currentSnapshot = snapshotRef.current;
    // Our own save echoed back, or an event older than what a reload already read.
    if (currentSnapshot && value.revision <= currentSnapshot.revision) return;
    if (currentSnapshot && draftPending()) {
      // Edits still waiting for their save move onto the newer revision, which the next save then replaces.
      setNotice(rebase(value, currentSnapshot) ? mergedNotice : externalNotice);
      return;
    }
    adoptSnapshot(value);
    setError("");
    setNotice(externalNotice);
  };
  // The subscription outlives renders; it reaches the handler through this so it never runs a stale
  // one (the handler itself only touches refs and state setters).
  const applyPreferencesChangeRef = useLatestRef(applyPreferencesChange);

  useEffect(() => {
    if (!client.onPreferencesChanged) return;
    const current = generation.current;
    const pending = client.onPreferencesChanged((value) => {
      if (generation.current !== current) return;
      if (savingRef.current) {
        if (!heldChange.current || value.revision > heldChange.current.revision)
          heldChange.current = value;
        return;
      }
      applyPreferencesChangeRef.current(value);
    });
    return trackAsyncSubscription(pending, () => generation.current === current);
  }, [client, generation]);

  useEffect(() => {
    clearAutosave();
    cancelSavedStatusReset();
    saveTokenRef.current += 1;
    saveOwnerRef.current += 1;
    savingRef.current = false;
    heldChange.current = undefined;
    reloadInFlightRef.current = undefined;
    loadTokenRef.current += 1;
    loadingClientRef.current = client;
    snapshotRef.current = undefined;
    draftRef.current = undefined;
    setBusy(true);
    setLoadFailed(false);
    setSaveState("idle");
    setSaveError("");
  }, [client, generation]);

  useEffect(() => {
    const current = generation.current;
    const loadToken = ++loadTokenRef.current;
    client
      .load()
      .then((value) => {
        if (generation.current !== current || loadTokenRef.current !== loadToken) return;
        loadingClientRef.current = undefined;
        // 加载期间可能已收到更新的宿主通知，迟到的读取不能把版本退回去。
        if (!snapshotRef.current || value.revision >= snapshotRef.current.revision)
          adoptSnapshot(value);
        setLoadFailed(false);
      })
      .catch((reason) => {
        if (generation.current !== current || loadTokenRef.current !== loadToken) return;
        setError(errorMessage(reason));
        setLoadFailed(true);
      })
      .finally(() => {
        if (generation.current === current && loadTokenRef.current === loadToken) setBusy(false);
      });
  }, [client, generation]);

  async function reload() {
    if (!mounted.current) return;
    const inFlight = reloadInFlightRef.current;
    if (inFlight) return inFlight;
    const current = generation.current;
    const loadToken = ++loadTokenRef.current;
    const operation = (async () => {
      clearAutosave();
      setBusy(true);
      setError("");
      setNotice("");
      setRecoveredBackup("");
      try {
        const value = await client.load();
        if (
          !mounted.current ||
          generation.current !== current ||
          loadTokenRef.current !== loadToken
        )
          return;
        loadingClientRef.current = undefined;
        // 重载也遵守通知的版本顺序，保留请求期间已经采纳的新快照。
        if (!snapshotRef.current || value.revision >= snapshotRef.current.revision)
          adoptSnapshot(value);
        setLoadFailed(false);
        setSaveState("idle");
        setSaveError("");
      } catch (reason) {
        if (
          !mounted.current ||
          generation.current !== current ||
          loadTokenRef.current !== loadToken
        )
          return;
        setError(errorMessage(reason));
        setLoadFailed(true);
      } finally {
        if (mounted.current && generation.current === current && loadTokenRef.current === loadToken)
          setBusy(false);
      }
    })();
    reloadInFlightRef.current = operation;
    try {
      await operation;
    } finally {
      if (reloadInFlightRef.current === operation) reloadInFlightRef.current = undefined;
    }
  }

  const autosaveTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  function clearAutosave() {
    if (autosaveTimer.current === undefined) return;
    clearTimeout(autosaveTimer.current);
    autosaveTimer.current = undefined;
  }
  function scheduleAutosave() {
    clearAutosave();
    autosaveTimer.current = setTimeout(() => {
      autosaveTimer.current = undefined;
      void flushRef.current();
    }, SETTINGS_AUTOSAVE_DELAY_MS);
  }

  /**
   * Saves whatever differs from the saved state now, then keeps saving while edits made during the save are still unsaved. Only one save runs at a time: a call while one is in flight waits for it and then saves whatever it left unsaved, so once the returned promise resolves the draft as it stood at the call has been written (or the save failed).
   */
  const saveInFlightRef = useRef<Promise<void>>(undefined);
  async function flush(): Promise<void> {
    const inFlight = saveInFlightRef.current;
    if (savingRef.current && inFlight) {
      const owner = saveOwnerRef.current;
      const saveClient = client;
      clearAutosave();
      await inFlight;
      if (saveOwnerRef.current !== owner || clientRef.current !== saveClient) return;
      return flushRef.current();
    }
    const operation = saveNow();
    saveInFlightRef.current = operation;
    try {
      await operation;
    } finally {
      if (saveInFlightRef.current === operation) saveInFlightRef.current = undefined;
    }
  }

  async function saveNow() {
    clearAutosave();
    if (!mounted.current || savingRef.current || !savePending()) return;
    if (draftRef.current && !validCandidateFonts(draftRef.current)) return;
    const saveClient = client;
    const owner = saveOwnerRef.current;
    const ownsSave = () => saveOwnerRef.current === owner && clientRef.current === saveClient;
    const saveToken = ++saveTokenRef.current;
    savingRef.current = true;
    cancelSavedStatusReset();
    setSaveState("saving");
    setSaveError("");
    let failed = false;
    let conflicts = 0;
    try {
      // 即使设置页在请求期间卸载，也继续写完这段时间产生的编辑。卸载后 refs 和 client 仍可用，只有 React 状态更新停止。
      while (savePending()) {
        if (!ownsSave()) break;
        const base = snapshotRef.current;
        const sent = draftRef.current;
        if (!base || !sent || !validCandidateFonts(sent)) break;
        if (!deepEqual(sent, base.preferences)) {
          let value: Snapshot;
          try {
            value = await client.save(base.revision, sent);
            if (!ownsSave()) break;
          } catch (reason) {
            if (!ownsSave()) break;
            if (errorCode(reason) !== "conflict" || conflicts >= CONFLICT_RETRIES) throw reason;
            conflicts += 1;
            // 其他窗口先保存时，把本地编辑移到其新版本上再重试。
            const latest = await client.load();
            if (!ownsSave()) break;
            if (rebase(latest, base) && mounted.current) setNotice(mergedNotice);
            continue;
          }
          adoptSaved(value, sent);
        }
        if (!ownsSave()) break;
      }
    } catch (reason) {
      failed = true;
      if (mounted.current && ownsSave()) {
        setSaveState("failed");
        setSaveError(errorMessage(reason));
      }
    } finally {
      if (saveTokenRef.current === saveToken) savingRef.current = false;
    }
    if (!mounted.current) return;
    if (!ownsSave()) return;
    if (!failed) {
      if (savePending()) {
        // Edited again after the loop last looked; save that too once the edits pause.
        setSaveState("idle");
        scheduleAutosave();
      } else {
        setSaveState("saved");
        scheduleSavedStatusReset();
      }
    }
    const held = heldChange.current;
    heldChange.current = undefined;
    if (held) applyPreferencesChange(held);
  }
  const flushRef = useLatestRef(flush);
  useFlushOnWindowLeave(() => void flushRef.current());

  // 组件可能在自动保存收到首次响应前卸载。这里沿用挂载路径的有界冲突重试，但只修改本地副本；
  // 卸载后不再有可更新的 React 状态。
  async function saveDetached(
    saveClient: SettingsClient,
    initialBase: Snapshot,
    initialDraft: Preferences,
  ): Promise<void> {
    let base = initialBase;
    let sent = initialDraft;
    let conflicts = 0;
    while (!deepEqual(sent, base.preferences)) {
      if (!validCandidateFonts(sent)) return;
      try {
        await saveClient.save(base.revision, sent);
        return;
      } catch (reason) {
        if (errorCode(reason) !== "conflict" || conflicts >= CONFLICT_RETRIES) return;
        conflicts += 1;
        const latest = await saveClient.load();
        sent = applyPreferenceChanges(
          latest.preferences,
          preferenceChanges(base.preferences, sent),
        );
        base = latest;
      }
    }
  }

  // Every edit restarts the countdown; the loop in `flush` picks up edits made while a save is in flight, so nothing is scheduled then.
  useEffect(() => {
    if (!savePending()) {
      clearAutosave();
      return;
    }
    setSaveState((state) => (state === "saved" ? "idle" : state));
    if (!savingRef.current) scheduleAutosave();
  }, [draft, snapshot]);

  // Unmounting with an edit still counting down writes it without waiting for an answer: nothing is left on screen to show the result.
  useEffect(
    () => () => {
      if (autosaveTimer.current === undefined) return;
      clearAutosave();
      const currentSnapshot = snapshotRef.current;
      const currentDraft = draftRef.current;
      if (savingRef.current || !currentSnapshot || !currentDraft) return;
      if (deepEqual(currentDraft, currentSnapshot.preferences)) return;
      if (!validCandidateFonts(currentDraft)) return;
      const unmountedClient = clientRef.current;
      void saveDetached(unmountedClient, currentSnapshot, currentDraft).catch(() => undefined);
    },
    [],
  );

  useEffect(() => {
    if (!mobile || typeof document === "undefined") return;
    let hidden = document.hidden;
    const onVisibilityChange = () => {
      const nextHidden = document.hidden;
      const resumed = hidden && !nextHidden;
      hidden = nextHidden;
      if (!resumed) return;
      // Edits not yet written are saved, and rebased if the keyboard changed the file meanwhile, rather than replaced by a reload.
      if (savingRef.current || savePending()) {
        void flushRef.current();
        return;
      }
      void reload();
    };
    return subscribeToDocumentVisibilityChange(onVisibilityChange);
  }, [client, mobile]);

  return {
    draftRef,
    snapshotRef,
    reload,
    flush,
    saveState,
    saveError,
    loadFailed,
  } as const;
}
