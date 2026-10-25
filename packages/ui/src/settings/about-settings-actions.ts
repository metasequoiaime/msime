import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../core/preferences-model";
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
  // diagnostic_log 下的各项共用一个对象：先展开草稿里已有的整块，改一项时其他项（包括这个页面不显示的项）原样保留。
  const patchDiagnosticLog = (patch: NonNullable<Preferences["diagnostic_log"]>) =>
    setDraft((current) =>
      current
        ? {
            ...current,
            diagnostic_log: {
              ...current.diagnostic_log,
              ...diagnosticLogPreferences(current),
              ...patch,
            },
          }
        : current,
    );
  return {
    onCheckForUpdate: () => void checkForUpdate(),
    onChooseDataDirectory: () => void chooseDataDirectory(),
    onConfirmUninstall: () => void confirmUninstall(),
    onDiagnosticLogChange: (patch: Partial<DiagnosticLogPreferences>) => patchDiagnosticLog(patch),
    onCommandLineCandidatesChange: (command_line_candidates: boolean) =>
      patchDiagnosticLog({ command_line_candidates }),
    onTelemetryChange: (usage_reporting: boolean) => onPreferencesChange({ usage_reporting }),
    onHelp: () => selectPage("help"),
    onFeedback: () => selectPage("feedback"),
  } as const;
}
