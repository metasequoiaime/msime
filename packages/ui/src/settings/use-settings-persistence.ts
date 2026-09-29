import { useEffect, useRef, type Dispatch, type RefObject, type SetStateAction } from "react";
import { errorMessage } from "../core/error-message";
import { validCandidateFonts } from "../candidate/candidate-font-family";
import type { Preferences, SettingsClient, Snapshot } from "../index";

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
  saveMacosShuangpinKeymap?: (enabled: boolean) => Promise<void>;
  macosWubiAutoCommitUnique: boolean | undefined;
  saveMacosWubiAutoCommitUnique?: (enabled: boolean) => Promise<void>;
  setSavedMacosWubiAutoCommitUnique: (enabled: boolean) => void;
}

/** Owns loading, saving, and cross-window synchronization of shared preferences. */
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
  saveMacosShuangpinKeymap,
  macosWubiAutoCommitUnique,
  saveMacosWubiAutoCommitUnique,
  setSavedMacosWubiAutoCommitUnique,
}: UseSettingsPersistenceOptions) {
  const snapshotRef = useRef(snapshot);
  const draftRef = useRef(draft);

  useEffect(() => {
    snapshotRef.current = snapshot;
    draftRef.current = draft;
  }, [snapshot, draft]);

  // The refs are also written the moment a load or save resolves: the host's monitor echoes this
  // window's own save back as a change, and it can arrive before React commits the new snapshot.
  const adoptSnapshot = (value: Snapshot) => {
    if (!mounted.current) return;
    snapshotRef.current = value;
    draftRef.current = value.preferences;
    setSnapshot(value);
    setDraft(value.preferences);
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
    const currentDraft = draftRef.current;
    const dirty =
      !!currentSnapshot &&
      !!currentDraft &&
      JSON.stringify(currentDraft) !== JSON.stringify(currentSnapshot.preferences);
    if (dirty) {
      setNotice("设置已被其他窗口修改。请重新读取后再保存。");
      return;
    }
    adoptSnapshot(value);
    setError("");
    setNotice("设置已从其他窗口更新。");
  };
  // The subscription outlives renders; it reaches the handler through this so it never runs a stale
  // one (the handler itself only touches refs and state setters).
  const applyPreferencesChangeRef = useRef(applyPreferencesChange);
  applyPreferencesChangeRef.current = applyPreferencesChange;

  useEffect(() => {
    if (!client.onPreferencesChanged) return;
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void client
      .onPreferencesChanged((value) => {
        if (!active) return;
        if (savingRef.current) {
          if (!heldChange.current || value.revision > heldChange.current.revision)
            heldChange.current = value;
          return;
        }
        applyPreferencesChangeRef.current(value);
      })
      .then((value) => {
        if (active) unsubscribe = value;
        else value();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [client]);

  useEffect(() => {
    let active = true;
    client
      .load()
      .then((value) => {
        if (active) adoptSnapshot(value);
      })
      .catch((reason) => {
        if (active) setError(errorMessage(reason));
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
    setBusy(true);
    setError("");
    setNotice("");
    setRecoveredBackup("");
    try {
      const value = await client.load();
      if (!mounted.current) return;
      adoptSnapshot(value);
    } catch (reason) {
      if (mounted.current) setError(errorMessage(reason));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  useEffect(() => {
    if (!mobile || typeof document === "undefined") return;
    let hidden = document.hidden;
    const onVisibilityChange = () => {
      const nextHidden = document.hidden;
      const resumed = hidden && !nextHidden;
      hidden = nextHidden;
      if (!resumed) return;
      const currentSnapshot = snapshotRef.current;
      const currentDraft = draftRef.current;
      const dirty =
        !!currentSnapshot &&
        !!currentDraft &&
        JSON.stringify(currentDraft) !== JSON.stringify(currentSnapshot.preferences);
      if (dirty) {
        setNotice("设置已被其他窗口修改。请重新读取后再保存。");
        return;
      }
      void reload();
    };
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => document.removeEventListener("visibilitychange", onVisibilityChange);
  }, [client, mobile]);

  async function save() {
    if (!draft || !snapshot || !validCandidateFonts(draft)) return;
    setBusy(true);
    setError("");
    setNotice("");
    setRecoveredBackup("");
    savingRef.current = true;
    try {
      const value = await client.save(snapshot.revision, draft);
      if (macos && saveMacosShuangpinKeymap && macosShuangpinKeymap !== undefined) {
        await saveMacosShuangpinKeymap(macosShuangpinKeymap);
      }
      if (macos && saveMacosWubiAutoCommitUnique && macosWubiAutoCommitUnique !== undefined) {
        await saveMacosWubiAutoCommitUnique(macosWubiAutoCommitUnique);
        if (!mounted.current) return;
        setSavedMacosWubiAutoCommitUnique(macosWubiAutoCommitUnique);
      }
      if (!mounted.current) return;
      adoptSnapshot(value);
      setNotice("设置已保存。");
    } catch (reason) {
      if (mounted.current) setError(errorMessage(reason));
    } finally {
      if (mounted.current) setBusy(false);
      savingRef.current = false;
      const held = heldChange.current;
      heldChange.current = undefined;
      if (held) applyPreferencesChange(held);
    }
  }

  return { draftRef, snapshotRef, reload, save } as const;
}
