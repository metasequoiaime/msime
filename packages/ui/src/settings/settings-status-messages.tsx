import { unreadablePreferencesMessage } from "./preferences-recovery-message";
import { ActionButton } from "./action-button";
import { SettingsErrorMessage } from "./settings-error-message";

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
        <SettingsErrorMessage>
          {error}
          {error === unreadablePreferencesMessage && canRecover && (
            <>
              {" "}
              <ActionButton action={onRecover} disabled={busy} label="修复配置文件…" />
            </>
          )}
        </SettingsErrorMessage>
      )}
      {notice && (
        <p role="status" className="notice">
          {notice}
          {recoveredBackup && openPreferencesDirectory && (
            <>
              {" "}
              <ActionButton
                action={() =>
                  openPreferencesDirectory().catch(() => onError("无法打开配置文件所在的文件夹。"))
                }
                label={macos ? "在 Finder 中显示" : "打开所在文件夹"}
              />
            </>
          )}
        </p>
      )}
    </>
  );
}
