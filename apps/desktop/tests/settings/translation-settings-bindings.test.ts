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
      customTranslations: { load: async () => "", save: async () => undefined },
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
    mobile: false,
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
    customTranslationsText: "",
    customTranslationsPlaceholder: "synthetic",
    customTranslationsNotice: "",
    customTranslationsSummary: "",
    customTranslationsSaveState: "idle",
    customTranslationsSaveError: "",
    onCustomTranslationsChange: () => undefined,
    onFlushCustomTranslations: () => undefined,
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

test("binds the custom glosses editor apart from the services, only where the host can store the overlay", () => {
  const options = {
    client: {
      providerCredentials: undefined,
      customTranslations: { load: async () => "", save: async () => undefined },
    },
    candidateTranslations: true,
    translationTargetLanguage: "en",
    translationSecondaryLanguage: "",
    candidateGlossLanguagesEnabled: true,
    visibleTranslationLanguages: [["en", "英语"]],
    visibleSecondaryLanguages: [["", "不显示第二种语言"]],
    android: false,
    ios: false,
    macos: false,
    windows: false,
    harmony: true,
    linux: false,
    mobile: true,
    translationAccount: false,
    onPreferencesChange: () => undefined,
    onDeviceMissingLanguages: [],
    // 没有选择服务：释义编辑器不依赖任何服务。
    translationProvider: "none",
    setTranslationProvider: () => undefined,
    customTranslation: { enabled: false, endpoint: "", api_key: "" },
    tencentTranslation: { enabled: false, secret_id: "", secret_key: "", region: "ap-test" },
    niutrans: { enabled: false, app_id: "", apikey: "" },
    providerCredentials: undefined,
    tencentCredentialInput: { secretId: "", secretKey: "", region: undefined },
    updateTencentCredentialInput: () => undefined,
    providerCredentialBusy: undefined,
    providerCredentialMessages: {},
    runProviderCredential: async () => undefined,
    credentialTestControl: () => null,
    customTranslationsText: "你好\thello",
    customTranslationsPlaceholder: "synthetic",
    customTranslationsNotice: "",
    customTranslationsSummary: "1 条释义",
    customTranslationsSaveState: "saved",
    customTranslationsSaveError: "",
    onCustomTranslationsChange: () => undefined,
    onFlushCustomTranslations: () => undefined,
  } satisfies TranslationSettingsBindingsOptions;

  const bindings = createTranslationSettingsBindings(options);
  expect(bindings.providers?.provider).toBe("none");
  expect(bindings.customGlosses).toMatchObject({
    mobile: true,
    value: "你好\thello",
    summary: "1 条释义",
    saveState: "saved",
  });
  // 宿主没有访问释义覆盖文件的途径时，不提供编辑器。
  expect(
    createTranslationSettingsBindings({
      ...options,
      client: { providerCredentials: undefined },
    }).customGlosses,
  ).toBeUndefined();
  // 在确认 Android 宿主会读取释义覆盖文件之前，Android 仍然不提供它。
  expect(createTranslationSettingsBindings({ ...options, android: true }).customGlosses).toBe(
    undefined,
  );
});
