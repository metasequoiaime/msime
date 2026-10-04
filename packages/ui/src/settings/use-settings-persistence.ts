import {
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import { deepEqual } from "../core/deep-equal";
import { errorCode } from "../core/error-code";
import { errorMessage } from "../core/error-message";
import { validCandidateFonts } from "../candidate/candidate-font-family";
import type { Preferences, SettingsClient, Snapshot } from "../index";
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
  macos: boolean;
  mounted: RefObject<boolean>;
  snapshot: Snapshot | undefined;
  draft: Preferences | undefined;
  setSnapshot: Dispatch<SetStateAction<Snapshot | undefined>>;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  setBusy: (busy: boolean) => void;
  setError: (error: string) => void;
  setNotice: (notice: string) => void;
  setRecoveredBackup: (path: string) => void;
  macosShuangpinKeymap: boolean | undefined;
  savedMacosShuangpinKeymap: boolean | undefined;
  saveMacosShuangpinKeymap?: (enabled: boolean) => Promise<void>;
  setSavedMacosShuangpinKeymap: (enabled: boolean) => void;
  macosWubiAutoCommitUnique: boolean | undefined;
  savedMacosWubiAutoCommitUnique: boolean | undefined;
  saveMacosWubiAutoCommitUnique?: (enabled: boolean) => Promise<void>;
  setSavedMacosWubiAutoCommitUnique: (enabled: boolean) => void;
}

/** Owns loading, automatic saving, and cross-window synchronization of shared preferences. */
export function useSettingsPersistence({
  client,
  mobile,
  macos,
  mounted,
  snapshot,
  draft,
  setSnapshot,
  setDraft,
  setBusy,
  setError,
  setNotice,
  setRecoveredBackup,
  macosShuangpinKeymap,
  savedMacosShuangpinKeymap,
  saveMacosShuangpinKeymap,
  setSavedMacosShuangpinKeymap,
  macosWubiAutoCommitUnique,
  savedMacosWubiAutoCommitUnique,
  saveMacosWubiAutoCommitUnique,
  setSavedMacosWubiAutoCommitUnique,
}: UseSettingsPersistenceOptions) {
  const snapshotRef = useRef(snapshot);
  const draftRef = useRef(draft);
  const clientRef = useRef(client);
  clientRef.current = client;
  const [saveState, setSaveState] = useState<SettingsSaveState>("idle");
  const [saveError, setSaveError] = useState("");
  const [loadFailed, setLoadFailed] = useState(false);
  const reloadInFlightRef = useRef<Promise<void> | undefined>(undefined);
  const generation = useAsyncGeneration(client);

  useEffect(() => {
    snapshotRef.current = snapshot;
    draftRef.current = draft;
  }, [snapshot, draft]);

  // The macOS-only preferences live outside the shared document; the save loop reads them through this so a value changed while a save is in flight is still seen.
  const nativeRef = useRef({
    shuangpin: macosShuangpinKeymap,
    savedShuangpin: savedMacosShuangpinKeymap,
    wubi: macosWubiAutoCommitUnique,
    savedWubi: savedMacosWubiAutoCommitUnique,
  });
  nativeRef.current = {
    shuangpin: macosShuangpinKeymap,
    savedShuangpin: savedMacosShuangpinKeymap,
    wubi: macosWubiAutoCommitUnique,
    savedWubi: savedMacosWubiAutoCommitUnique,
  };
  const shuangpinPending = () => {
    const native = nativeRef.current;
    return (
      macos &&
      !!saveMacosShuangpinKeymap &&
      native.shuangpin !== undefined &&
      native.shuangpin !== native.savedShuangpin
    );
  };
  const wubiPending = () => {
    const native = nativeRef.current;
    return (
      macos &&
      !!saveMacosWubiAutoCommitUnique &&
      native.wubi !== undefined &&
      native.wubi !== native.savedWubi
    );
  };
  const draftPending = () => {
    const currentSnapshot = snapshotRef.current;
    const currentDraft = draftRef.current;
    return (
      !!currentSnapshot && !!currentDraft && !deepEqual(currentDraft, currentSnapshot.preferences)
    );
  };
  const savePending = () => draftPending() || shuangpinPending() || wubiPending();

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
  const applyPreferencesChangeRef = useRef(applyPreferencesChange);
  applyPreferencesChangeRef.current = applyPreferencesChange;

  useEffect(() => {
    if (!client.onPreferencesChanged) return;
    const current = generation.current;
    let unsubscribe: (() => void) | undefined;
    void client
      .onPreferencesChanged((value) => {
        if (generation.current !== current) return;
        if (savingRef.current) {
          if (!heldChange.current || value.revision > heldChange.current.revision)
            heldChange.current = value;
          return;
        }
        applyPreferencesChangeRef.current(value);
      })
      .then((value) => {
        if (generation.current === current) unsubscribe = value;
        else value();
      })
      .catch(() => undefined);
    return () => {
      unsubscribe?.();
    };
  }, [client, generation]);

  useEffect(() => {
    let active = true;
    client
      .load()
      .then((value) => {
        if (!active) return;
        adoptSnapshot(value);
        setLoadFailed(false);
      })
      .catch((reason) => {
        if (!active) return;
        setError(errorMessage(reason));
        setLoadFailed(true);
      })
      .finally(() => {
        if (active) setBusy(false);
      });
    return () => {
      active = false;
    };
  }, [client]);

  async function reload() {
    if (!mounted.current) return;
    const inFlight = reloadInFlightRef.current;
    if (inFlight) return inFlight;
    const operation = (async () => {
      clearAutosave();
      setBusy(true);
      setError("");
      setNotice("");
      setRecoveredBackup("");
      try {
        const value = await client.load();
        if (!mounted.current) return;
        adoptSnapshot(value);
        setLoadFailed(false);
        setSaveState("idle");
        setSaveError("");
      } catch (reason) {
        if (!mounted.current) return;
        setError(errorMessage(reason));
        setLoadFailed(true);
      } finally {
        if (mounted.current) setBusy(false);
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
  const savedStatusTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
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
   * Saves whatever differs from the saved state now, then keeps saving while edits made during the save are still unsaved. Only one save runs at a time; a call while one is in flight is absorbed by it.
   */
  async function flush() {
    clearAutosave();
    if (!mounted.current || savingRef.current || !savePending()) return;
    if (draftRef.current && !validCandidateFonts(draftRef.current)) return;
    savingRef.current = true;
    clearTimeout(savedStatusTimer.current);
    setSaveState("saving");
    setSaveError("");
    let failed = false;
    let conflicts = 0;
    try {
      // 即使设置页在请求期间卸载，也继续写完这段时间产生的编辑。卸载后 refs 和 client 仍可用，只有 React 状态更新停止。
      while (savePending()) {
        const base = snapshotRef.current;
        const sent = draftRef.current;
        if (!base || !sent || !validCandidateFonts(sent)) break;
        if (!deepEqual(sent, base.preferences)) {
          let value: Snapshot;
          try {
            value = await client.save(base.revision, sent);
          } catch (reason) {
            if (errorCode(reason) !== "conflict" || conflicts >= CONFLICT_RETRIES) throw reason;
            conflicts += 1;
            // Another window saved first: take its revision, put this window's edits on top, and save again.
            const latest = await client.load();
            if (rebase(latest, base) && mounted.current) setNotice(mergedNotice);
            continue;
          }
          adoptSaved(value, sent);
        }
        // The native macOS preferences follow the shared document, in the order the save button used to write them.
        if (shuangpinPending() && saveMacosShuangpinKeymap) {
          const enabled = nativeRef.current.shuangpin === true;
          await saveMacosShuangpinKeymap(enabled);
          nativeRef.current = { ...nativeRef.current, savedShuangpin: enabled };
          if (mounted.current) setSavedMacosShuangpinKeymap(enabled);
        }
        if (wubiPending() && saveMacosWubiAutoCommitUnique) {
          const enabled = nativeRef.current.wubi === true;
          await saveMacosWubiAutoCommitUnique(enabled);
          nativeRef.current = { ...nativeRef.current, savedWubi: enabled };
          if (mounted.current) setSavedMacosWubiAutoCommitUnique(enabled);
        }
      }
    } catch (reason) {
      failed = true;
      if (mounted.current) {
        setSaveState("failed");
        setSaveError(errorMessage(reason));
      }
    } finally {
      savingRef.current = false;
    }
    if (!mounted.current) return;
    if (!failed) {
      if (savePending()) {
        // Edited again after the loop last looked; save that too once the edits pause.
        setSaveState("idle");
        scheduleAutosave();
      } else {
        setSaveState("saved");
        savedStatusTimer.current = setTimeout(
          () => setSaveState((state) => (state === "saved" ? "idle" : state)),
          SETTINGS_SAVED_STATUS_MS,
        );
      }
    }
    const held = heldChange.current;
    heldChange.current = undefined;
    if (held) applyPreferencesChange(held);
  }
  const flushRef = useRef(flush);
  flushRef.current = flush;
  useFlushOnWindowLeave(() => void flushRef.current());

  // Every edit restarts the countdown; the loop in `flush` picks up edits made while a save is in flight, so nothing is scheduled then.
  useEffect(() => {
    if (!savePending()) {
      clearAutosave();
      return;
    }
    setSaveState((state) => (state === "saved" ? "idle" : state));
    if (!savingRef.current) scheduleAutosave();
  }, [
    draft,
    snapshot,
    macosShuangpinKeymap,
    savedMacosShuangpinKeymap,
    macosWubiAutoCommitUnique,
    savedMacosWubiAutoCommitUnique,
  ]);

  // Unmounting with an edit still counting down writes it without waiting for an answer: nothing is left on screen to show the result.
  useEffect(
    () => () => {
      clearTimeout(savedStatusTimer.current);
      if (autosaveTimer.current === undefined) return;
      clearAutosave();
      const currentSnapshot = snapshotRef.current;
      const currentDraft = draftRef.current;
      if (savingRef.current || !currentSnapshot || !currentDraft) return;
      if (deepEqual(currentDraft, currentSnapshot.preferences)) return;
      if (!validCandidateFonts(currentDraft)) return;
      const unmountedClient = clientRef.current;
      void Promise.resolve()
        .then(() => unmountedClient.save(currentSnapshot.revision, currentDraft))
        .catch(() => undefined);
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
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => document.removeEventListener("visibilitychange", onVisibilityChange);
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
