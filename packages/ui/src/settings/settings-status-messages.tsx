import { unreadablePreferencesMessage } from "../core/error-message";
import { ActionButton } from "./action-button";
import { ErrorAlert } from "../core/error-alert";
import { SettingsNotice } from "./settings-notice";

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
  /** 自动保存失败的原因；空串表示没有失败。保存状态本来只写在表单最后一行，页面一长就得滚到底才看得见，改了设置却存不下来时用户在原处什么也看不到，所以失败也在页面顶部说一次。 */
  saveError?: string;
  onRetrySave?: () => void;
  /** 关掉页首的提示（其他窗口改了设置、配置已修复等）。 */
  onDismissNotice?: () => void;
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
  saveError = "",
  onRetrySave,
  onDismissNotice,
}: SettingsStatusMessagesProps) {
  return (
    <>
      {saveError && (
        <ErrorAlert>
          设置没有保存：{saveError}
          {onRetrySave && (
            <>
              {" "}
              <ActionButton action={onRetrySave} disabled={busy} label="重试" />
            </>
          )}
        </ErrorAlert>
      )}
      {error && (
        <ErrorAlert>
          {error}
          {error === unreadablePreferencesMessage && canRecover && (
            <>
              {" "}
              <ActionButton action={onRecover} disabled={busy} label="修复配置文件…" />
            </>
          )}
        </ErrorAlert>
      )}
      {notice && (
        <SettingsNotice role="status" onDismiss={onDismissNotice}>
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
        </SettingsNotice>
      )}
    </>
  );
}
