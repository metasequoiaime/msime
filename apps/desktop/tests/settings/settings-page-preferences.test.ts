import { expect, test } from "vitest";
import {
  settingsPagePreferences,
  type Preferences,
  type ProviderCredentialStatus,
} from "@msime/ui";

const credentials: ProviderCredentialStatus = {
  ai: [{ provider: "openai", endpoint: "https://example.invalid", model: "synthetic-model" }],
  aiInvalid: false,
  tencent: null,
  tencentInvalid: false,
  voiceAsr: [],
  voicePolish: [],
  voiceInvalid: false,
};

test("combines default preference projections for the settings page", () => {
  const state = settingsPagePreferences({ ios: true });

  expect(state.clipboardHistory).toBe(true);
  expect(state.inputModeHUD).toBe(true);
  expect(state.numberRowSelection).toBe(true);
  expect(state.themeMode).toBe("system");
  expect(state.diagnosticLog).toEqual({ server: false, tsf: false });
  expect(state.storedAiCredential).toBeUndefined();
});

test("keeps provider credentials and dirty-state semantics in the combined projection", () => {
  const draft = {
    ai_assistant: { provider: "openai" },
    input_mode_hud: false,
    theme: "dark",
    diagnostic_log: { server: true },
    clipboard_history: false,
  } as Preferences;
  const state = settingsPagePreferences({
    draft,
    providerCredentials: credentials,
    snapshot: { preferences: {} as Preferences } as never,
    macosWubiAutoCommitUnique: true,
    savedMacosWubiAutoCommitUnique: false,
    ios: false,
  });

  expect(state.storedAiCredential).toEqual(credentials.ai[0]);
  expect(state.inputModeHUD).toBe(false);
  expect(state.themeMode).toBe("dark");
  expect(state.clipboardHistory).toBe(false);
  expect(state.diagnosticLog).toEqual({ server: true, tsf: false });
  expect(state.dirty).toBe(true);
});
