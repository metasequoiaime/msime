import { expect, test } from "vitest";
import type { Preferences } from "../../../../packages/ui/src/index";
import { createCandidateTranslationSettings } from "../../../../packages/ui/src/settings/translation-candidate-settings";
import { useTranslationSettings } from "../../../../packages/ui/src/settings/use-translation-settings";

test("builds candidate translation bindings for preference changes and provider selection", () => {
  const updates: Record<string, unknown>[] = [];
  const providers: string[] = [];
  const settings = createCandidateTranslationSettings({
    grouped: true,
    candidateTranslations: true,
    translationTargetLanguage: "en",
    translationSecondaryLanguage: null,
    candidateGlossLanguagesEnabled: true,
    visibleTranslationLanguages: [["en", "英语"]],
    visibleSecondaryLanguages: [["", "不显示第二种语言"]],
    android: true,
    ios: false,
    macos: false,
    harmony: false,
    linux: false,
    translationAccount: false,
    onPreferencesChange: (patch) => updates.push(patch),
    onDeviceMissingLanguages: [],
    translationProvider: "none",
    setTranslationProvider: (provider) => providers.push(provider),
  });

  settings.onEnabledChange(false);
  settings.onTargetLanguageChange("en");
  settings.onSecondaryLanguageChange("");
  settings.onAccountTranslationChange(true);

  expect(settings.grouped).toBe(true);
  expect(settings.showSecondaryLanguage).toBe(true);
  expect(settings.showAccountTranslation).toBe(true);
  expect(updates).toEqual([
    { candidate_translations: false },
    { translation_target_language: "en" },
    { translation_secondary_language: null },
  ]);
  expect(providers).toEqual(["account"]);
});

test("offers the secondary candidate language on Windows, whose card draws one gloss line per language", () => {
  const options = {
    candidateTranslations: true,
    translationTargetLanguage: "en",
    translationSecondaryLanguage: null,
    candidateGlossLanguagesEnabled: true,
    visibleTranslationLanguages: [["en", "英语"]],
    visibleSecondaryLanguages: [["", "不显示第二种语言"]],
    android: false,
    ios: false,
    macos: false,
    harmony: false,
    linux: false,
    translationAccount: false,
    onPreferencesChange: () => undefined,
    onDeviceMissingLanguages: [],
    translationProvider: "none",
    setTranslationProvider: () => undefined,
  } as const;
  expect(
    createCandidateTranslationSettings({ ...options, windows: true }).showSecondaryLanguage,
  ).toBe(true);
  expect(
    createCandidateTranslationSettings({ ...options, linux: true }).showSecondaryLanguage,
  ).toBe(false);
});

test("lists the MSIME account translation service on Windows, whose Server asks the account for candidate glosses", () => {
  const options = {
    candidateTranslations: true,
    translationTargetLanguage: "en",
    translationSecondaryLanguage: null,
    candidateGlossLanguagesEnabled: true,
    visibleTranslationLanguages: [["en", "英语"]],
    visibleSecondaryLanguages: [["", "不显示第二种语言"]],
    android: false,
    ios: false,
    macos: false,
    harmony: false,
    linux: false,
    translationAccount: false,
    onPreferencesChange: () => undefined,
    onDeviceMissingLanguages: [],
    translationProvider: "none",
    setTranslationProvider: () => undefined,
  } as const;
  expect(
    createCandidateTranslationSettings({ ...options, windows: true }).showAccountProvider,
  ).toBe(true);
  expect(createCandidateTranslationSettings({ ...options, macos: true }).showAccountProvider).toBe(
    true,
  );
  expect(createCandidateTranslationSettings(options).showAccountProvider).toBe(false);
});

test("reads a saved MSIME account choice back as the account provider on Windows", () => {
  const preferences = {
    translation_account: true,
    custom_translation: { enabled: false, endpoint: "", api_key: "" },
    tencent_tmt: {
      enabled: true,
      secret_id: "",
      secret_key: "",
      region: "ap-guangzhou",
      project_id: 0,
    },
    niutrans: { enabled: false, app_id: "", apikey: "" },
  } as unknown as Preferences;
  const derive = (windows: boolean) =>
    useTranslationSettings({
      preferences,
      mobile: false,
      macos: false,
      linux: false,
      windows,
      candidateEnglishGlossAvailable: false,
      onDeviceDownloadable: [],
      onChange: () => undefined,
    }).translationProvider;
  // Tencent 的默认 enabled 没有可用密钥，不算用户的选择，挡不住账号；与 crates/host-api 的 selected_translation_services 一致。
  expect(derive(true)).toBe("account");
  expect(derive(false)).toBe("tencent");
});
