import { expect, test } from "vitest";
import {
  createTranslationSettingsBindings,
  type TranslationSettingsBindingsOptions,
} from "../../../../packages/ui/src/settings/translation-settings-bindings";

test("composes candidate and provider bindings while omitting providers on Android", () => {
  const options = {
    grouped: true,
    client: {
      providerCredentials: undefined,
    },
    candidateTranslations: true,
    translationTargetLanguage: "en",
    translationSecondaryLanguage: "",
    candidateGlossLanguagesEnabled: true,
    visibleTranslationLanguages: [["en", "英语"]],
    visibleSecondaryLanguages: [["", "不显示第二种语言"]],
    android: false,
    ios: false,
    macos: true,
    windows: false,
    harmony: false,
    linux: false,
    translationAccount: false,
    onPreferencesChange: () => undefined,
    onDeviceMissingLanguages: [],
    translationProvider: "tencent",
    setTranslationProvider: () => undefined,
    customTranslation: { enabled: false, endpoint: "", api_key: "" },
    tencentTranslation: {
      enabled: true,
      secret_id: "",
      secret_key: "",
      region: "ap-test",
    },
    niutrans: { enabled: false, app_id: "", apikey: "" },
    providerCredentials: undefined,
    tencentCredentialInput: { secretId: "", secretKey: "", region: undefined },
    updateTencentCredentialInput: () => undefined,
    providerCredentialBusy: undefined,
    providerCredentialMessages: {},
    runProviderCredential: async () => undefined,
    credentialTestControl: () => null,
  } satisfies TranslationSettingsBindingsOptions;
  const bindings = createTranslationSettingsBindings(options);

  expect(bindings.candidate.grouped).toBe(true);
  expect(bindings.providers?.grouped).toBe(true);

  const androidBindings = createTranslationSettingsBindings({
    ...options,
    android: true,
  });
  expect(androidBindings.providers).toBeUndefined();
});
