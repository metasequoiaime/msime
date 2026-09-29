import type { Preferences, ProviderCredentialStatus, Snapshot } from "../index";
import { aiSettingsPreferences, type AiSettingsPreferences } from "./ai-settings-preferences";
import { clipboardHistoryEnabled } from "./clipboard-history-preferences";
import { diagnosticLogPreferences, type DiagnosticLogPreferences } from "./diagnostic-logs-section";
import { settingsDirty } from "./settings-dirty";
import {
  settingsInputPreferences,
  type SettingsInputPreferences,
} from "./settings-input-preferences";
import {
  settingsVisualPreferences,
  type SettingsVisualPreferences,
} from "./settings-visual-preferences";

export interface SettingsPagePreferencesOptions {
  draft?: Preferences;
  providerCredentials?: ProviderCredentialStatus;
  snapshot?: Snapshot;
  macosWubiAutoCommitUnique?: boolean;
  savedMacosWubiAutoCommitUnique?: boolean;
  ios: boolean;
}

export interface SettingsPagePreferences
  extends AiSettingsPreferences, SettingsInputPreferences, SettingsVisualPreferences {
  clipboardHistory: boolean;
  diagnosticLog: DiagnosticLogPreferences;
  dirty: boolean;
}

/** Combines the preference projections consumed by the settings page panels. */
export function settingsPagePreferences({
  draft,
  providerCredentials,
  snapshot,
  macosWubiAutoCommitUnique,
  savedMacosWubiAutoCommitUnique,
  ios,
}: SettingsPagePreferencesOptions): SettingsPagePreferences {
  return {
    ...aiSettingsPreferences(draft?.ai_assistant, providerCredentials),
    ...settingsInputPreferences(draft),
    ...settingsVisualPreferences(draft),
    clipboardHistory: clipboardHistoryEnabled(ios, draft),
    diagnosticLog: diagnosticLogPreferences(draft?.diagnostic_log),
    dirty: settingsDirty({
      draft,
      snapshot,
      macosWubiAutoCommitUnique,
      savedMacosWubiAutoCommitUnique,
    }),
  };
}
