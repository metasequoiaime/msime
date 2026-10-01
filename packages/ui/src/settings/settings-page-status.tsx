import type { Preferences } from "../index";
import {
  InputSourceStartupNotice,
  type InputSourceStartupNoticeProps,
  type InputSourceStartupStatus,
} from "./input-source-startup-notice";
import {
  SettingsStatusMessages,
  type SettingsStatusMessagesProps,
} from "./settings-status-messages";

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
      {inputSourceStartup &&
        (inputSourceStartup.action !== "up_to_date" ||
          inputSourceStartup.enabled === false ||
          Boolean(inputSourceStartup.system_bundles?.length)) && (
          <InputSourceStartupNotice
            status={inputSourceStartup}
            onOpenSettings={onOpenSettings}
            onDismiss={onDismiss}
            onError={onError}
          />
        )}
      {busy && !draft && <p role="status">正在读取设置…</p>}
    </>
  );
}
