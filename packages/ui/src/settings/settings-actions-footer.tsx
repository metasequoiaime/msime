import * as settings from "./settings-style";
import type { SettingsSaveState } from "./use-settings-persistence";

export interface SettingsActionsFooterProps {
  busy: boolean;
  /** Where the automatic save stands; drawn as a quiet status on the right. */
  saveState: SettingsSaveState;
  /** Why the last save failed, shown while `saveState` is `failed`. */
  saveError: string;
  showRestoreDefaults: boolean;
  onRestoreDefaults: () => void;
  /** Saves the unsaved changes again after a failure. */
  onRetry: () => void;
  /** Discards the draft and reads the saved settings again; passed only when loading or saving failed and the page allows it. */
  onReload?: () => void;
}

/** Shared action row at the bottom of settings forms: 恢复默认设置 on the left, the save status and the recovery actions a failure needs on the right. */
export function SettingsActionsFooter({
  busy,
  saveState,
  saveError,
  showRestoreDefaults,
  onRestoreDefaults,
  onRetry,
  onReload,
}: SettingsActionsFooterProps) {
  const failed = saveState === "failed";
  return (
    <footer className={settings.settingsActions}>
      {showRestoreDefaults && (
        <button type="button" className="secondary" disabled={busy} onClick={onRestoreDefaults}>
          恢复默认设置
        </button>
      )}
      {failed ? (
        <span role="alert">{saveError}</span>
      ) : (
        <span aria-live="polite">
          {saveState === "saving" ? "正在保存…" : saveState === "saved" ? "已保存" : ""}
        </span>
      )}
      {failed && (
        <button type="button" className="secondary" disabled={busy} onClick={onRetry}>
          重试
        </button>
      )}
      {onReload && (
        <button type="button" className="secondary" disabled={busy} onClick={onReload}>
          重新读取
        </button>
      )}
    </footer>
  );
}
