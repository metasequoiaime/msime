import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";
import type { DiagnosticLogPreferences } from "./diagnostic-logs-section";
import { diagnosticLogPreferences } from "./diagnostic-log-preferences";
import { createSettingsDraftActions } from "./settings-draft-actions";

export interface CreateAboutSettingsActionsOptions {
  checkForUpdate: () => Promise<void>;
  chooseDataDirectory: () => Promise<void>;
  confirmUninstall: () => Promise<void>;
  selectPage: (page: "help" | "feedback") => void;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates maintenance, diagnostic, telemetry, and support-navigation callbacks. */
export function createAboutSettingsActions({
  checkForUpdate,
  chooseDataDirectory,
  confirmUninstall,
  selectPage,
  setDraft,
}: CreateAboutSettingsActionsOptions) {
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  return {
    onCheckForUpdate: () => void checkForUpdate(),
    onChooseDataDirectory: () => void chooseDataDirectory(),
    onConfirmUninstall: () => void confirmUninstall(),
    onDiagnosticLogChange: (patch: Partial<DiagnosticLogPreferences>) =>
      setDraft((current) =>
        current
          ? { ...current, diagnostic_log: { ...diagnosticLogPreferences(current), ...patch } }
          : current,
      ),
    onTelemetryChange: (telemetry_enabled: boolean) => onPreferencesChange({ telemetry_enabled }),
    onHelp: () => selectPage("help"),
    onFeedback: () => selectPage("feedback"),
  } as const;
}
