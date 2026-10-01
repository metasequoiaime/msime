import { expect, test } from "vitest";
import { createCandidateTranslationSettings } from "../../../../packages/ui/src/settings/translation-candidate-settings";

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
