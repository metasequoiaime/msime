import { type Dispatch, type RefObject, type SetStateAction } from "react";
import { deepEqual } from "../core/deep-equal";
import { errorMessage } from "../core/error-message";
import { useAsyncActionRunner } from "../core/use-async-action";
import type { Preferences, PreferencesRecovery, SettingsClient, Snapshot } from "../index";

export interface PreferenceRecoveryConfirmOptions {
  title: string;
  message: string;
  confirmLabel: string;
}

export interface UsePreferenceRecoveryOptions {
  client: Pick<SettingsClient, "loadDefaultPreferences" | "recoverPreferences">;
  busy: boolean;
  snapshotRef: RefObject<Snapshot | undefined>;
  draftRef: RefObject<Preferences | undefined>;
  setSnapshot: Dispatch<SetStateAction<Snapshot | undefined>>;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  setBusy: (busy: boolean) => void;
  setError: (error: string) => void;
  setNotice: (notice: string) => void;
  setRecoveredBackup: (path: string) => void;
  confirm: (options: PreferenceRecoveryConfirmOptions) => Promise<boolean>;
}

/** Owns the explicit restore-defaults and damaged-preferences recovery actions. */
export function usePreferenceRecovery({
  client,
  busy,
  snapshotRef,
  draftRef,
  setSnapshot,
  setDraft,
  setBusy,
  setError,
  setNotice,
  setRecoveredBackup,
  confirm,
}: UsePreferenceRecoveryOptions) {
  const publishError = (message: string) => {
    if (message) setError(message);
  };
  const publishNotice = (message: string) => {
    if (message) setNotice(message);
  };
  const { run } = useAsyncActionRunner(publishError, publishNotice);

  async function restoreDefaults() {
    if (!client.loadDefaultPreferences || busy) return;
    const loadDefaultPreferences = client.loadDefaultPreferences;
    await run(
      async (isCurrent) => {
        const confirmed = await confirm({
          title: "恢复默认设置",
          message: "语音和翻译服务的密钥、词库和学习数据都不会改变。恢复后立即生效。",
          confirmLabel: "恢复",
        });
        if (!isCurrent() || !confirmed || !client.loadDefaultPreferences) return;
        setError("");
        setNotice("");
        try {
          const restored = await loadDefaultPreferences();
          if (!isCurrent()) return;
          setDraft(restored);
          setNotice("所有设置已恢复默认。");
        } catch {
          if (isCurrent()) setError("无法读取默认设置，请重试。原有设置不会被自动重置。");
        }
      },
      { formatError: () => "无法读取默认设置，请重试。原有设置不会被自动重置。" },
    );
  }

  async function recoverPreferences() {
    if (!client.recoverPreferences || busy) return;
    const recoverPreferences = client.recoverPreferences;
    await run(
      async (isCurrent) => {
        const confirmed = await confirm({
          title: "修复配置文件",
          message:
            "损坏的配置文件会先备份到同一目录，然后尽量保留能识别的设置和服务密钥，其余恢复默认。",
          confirmLabel: "修复",
        });
        if (!isCurrent() || !confirmed || !client.recoverPreferences) return;
        setBusy(true);
        setError("");
        setNotice("");
        setRecoveredBackup("");
        try {
          const result: PreferencesRecovery = await recoverPreferences();
          if (!isCurrent()) return;
          const currentSnapshot = snapshotRef.current;
          const currentDraft = draftRef.current;
          const dirty =
            !!currentSnapshot &&
            !!currentDraft &&
            !deepEqual(currentDraft, currentSnapshot.preferences);
          setSnapshot(result.snapshot);
          if (!dirty) setDraft(result.snapshot.preferences);
          if (!result.backupPath) {
            setNotice("配置文件已可以正常读取，无需修复。");
            return;
          }
          const backupName = result.backupPath.split(/[\\/]/).pop() ?? result.backupPath;
          setRecoveredBackup(result.backupPath);
          setNotice(
            `配置文件已修复，原文件已备份为 ${backupName}。${
              result.salvaged ? "" : "原有设置无法识别，已恢复默认。"
            }${dirty ? "尚未保存的修改仍保留，并会自动保存。" : ""}`,
          );
        } catch (reason) {
          if (isCurrent()) setError(errorMessage(reason));
        } finally {
          setBusy(false);
        }
      },
      { formatError: errorMessage },
    );
  }

  return { restoreDefaults, recoverPreferences } as const;
}
