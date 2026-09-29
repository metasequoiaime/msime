import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";
import type { DiagnosticLogPreferences } from "./diagnostic-logs-section";

export interface CreateAboutSettingsActionsOptions {
  draft?: Preferences;
  diagnosticLog: DiagnosticLogPreferences;
  checkForUpdate: () => Promise<void>;
  chooseDataDirectory: () => Promise<void>;
  confirmUninstall: () => Promise<void>;
  selectPage: (page: "help" | "feedback") => void;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates maintenance, diagnostic, telemetry, and support-navigation callbacks. */
export function createAboutSettingsActions({
  draft,
  diagnosticLog,
  checkForUpdate,
  chooseDataDirectory,
  confirmUninstall,
  selectPage,
  setDraft,
}: CreateAboutSettingsActionsOptions) {
  return {
    onCheckForUpdate: () => void checkForUpdate(),
    onChooseDataDirectory: () => void chooseDataDirectory(),
    onConfirmUninstall: () => void confirmUninstall(),
    onDiagnosticLogChange: (patch: Partial<DiagnosticLogPreferences>) => {
      if (draft) setDraft({ ...draft, diagnostic_log: { ...diagnosticLog, ...patch } });
    },
    onTelemetryChange: (telemetry_enabled: boolean) => {
      if (draft) setDraft({ ...draft, telemetry_enabled });
    },
    onHelp: () => selectPage("help"),
    onFeedback: () => selectPage("feedback"),
  } as const;
}
