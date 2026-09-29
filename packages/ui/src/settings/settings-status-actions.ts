import type { Dispatch, SetStateAction } from "react";
import type { InputSourceStartupStatus } from "./input-source-startup-notice";

export interface CreateSettingsStatusActionsOptions {
  recoverPreferences: () => Promise<void>;
  inputSourceStartup?: { openSettings: () => Promise<void> };
  setInputSourceStartup: Dispatch<SetStateAction<InputSourceStartupStatus | null>>;
}

/** Creates recovery and startup-notice actions for the settings status surface. */
export function createSettingsStatusActions({
  recoverPreferences,
  inputSourceStartup,
  setInputSourceStartup,
}: CreateSettingsStatusActionsOptions) {
  return {
    onRecover: () => void recoverPreferences(),
    onOpenSettings: () => inputSourceStartup?.openSettings(),
    onDismiss: () => setInputSourceStartup(null),
  } as const;
}
