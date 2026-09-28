import * as settings from "./settings-style";

export interface SettingsActionsFooterProps {
  busy: boolean;
  dirty: boolean;
  canSave: boolean;
  showRestoreDefaults: boolean;
  onRestoreDefaults: () => void;
}

/** Shared save and restore controls at the bottom of settings forms. */
export function SettingsActionsFooter({
  busy,
  dirty,
  canSave,
  showRestoreDefaults,
  onRestoreDefaults,
}: SettingsActionsFooterProps) {
  return (
    <footer className={settings.settingsActions}>
      {showRestoreDefaults && (
        <button type="button" className="secondary" disabled={busy} onClick={onRestoreDefaults}>
          恢复默认设置
        </button>
      )}
      <span>{dirty ? "有未保存的修改" : ""}</span>
      <button type="submit" disabled={busy || !dirty || !canSave}>
        {busy ? "处理中…" : "保存设置"}
      </button>
    </footer>
  );
}
