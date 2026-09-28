import { unreadablePreferencesMessage } from "./preferences-recovery-message";

export interface SettingsStatusMessagesProps {
  error: string;
  notice: string;
  busy: boolean;
  recoveredBackup: string;
  canRecover: boolean;
  onRecover: () => void;
  openPreferencesDirectory?: () => Promise<void>;
  macos: boolean;
  onError: (message: string) => void;
}

/** Error and save-status messages shared by the settings content surface. */
export function SettingsStatusMessages({
  error,
  notice,
  busy,
  recoveredBackup,
  canRecover,
  onRecover,
  openPreferencesDirectory,
  macos,
  onError,
}: SettingsStatusMessagesProps) {
  return (
    <>
      {error && (
        <p role="alert" className="error">
          {error}
          {error === unreadablePreferencesMessage && canRecover && (
            <>
              {" "}
              <button type="button" className="secondary" disabled={busy} onClick={onRecover}>
                修复配置文件…
              </button>
            </>
          )}
        </p>
      )}
      {notice && (
        <p role="status" className="notice">
          {notice}
          {recoveredBackup && openPreferencesDirectory && (
            <>
              {" "}
              <button
                type="button"
                className="secondary"
                onClick={() =>
                  void openPreferencesDirectory().catch(() =>
                    onError("无法打开配置文件所在的文件夹。"),
                  )
                }
              >
                {macos ? "在 Finder 中显示" : "打开所在文件夹"}
              </button>
            </>
          )}
        </p>
      )}
    </>
  );
}
