import type { Preferences } from "../core/preferences-model";
import {
  InputSourceStartupNotice,
  type InputSourceStartupNoticeProps,
} from "./input-source-startup-notice";
import {
  inputSourceNoticeVisible,
  type InputSourceStartupStatus,
} from "./input-source-startup-model";
import {
  SettingsStatusMessages,
  type SettingsStatusMessagesProps,
} from "./settings-status-messages";
import { StatusMessage } from "../core/status-message";

export interface SettingsPageStatusProps
  extends
    Omit<SettingsStatusMessagesProps, "onError">,
    Pick<InputSourceStartupNoticeProps, "onError"> {
  draft?: Preferences;
  inputSourceStartup?: InputSourceStartupStatus | null;
  onOpenSettings: InputSourceStartupNoticeProps["onOpenSettings"];
  onDismiss: InputSourceStartupNoticeProps["onDismiss"];
}

/** Composes status messages, startup notices, and the initial loading state for settings. */
export function SettingsPageStatus({
  draft,
  busy,
  inputSourceStartup,
  onOpenSettings,
  onDismiss,
  onError,
  ...statusProps
}: SettingsPageStatusProps) {
  return (
    <>
      <SettingsStatusMessages busy={busy} onError={onError} {...statusProps} />
      {inputSourceStartup && inputSourceNoticeVisible(inputSourceStartup) && (
        <InputSourceStartupNotice
          status={inputSourceStartup}
          onOpenSettings={onOpenSettings}
          onDismiss={onDismiss}
          onError={onError}
        />
      )}
      {busy && !draft && <StatusMessage role="status">正在读取设置…</StatusMessage>}
    </>
  );
}
