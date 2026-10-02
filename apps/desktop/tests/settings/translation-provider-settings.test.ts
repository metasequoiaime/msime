import { expect, test } from "vitest";
import {
  createTranslationProviderSettings,
  type TranslationProviderSettingsOptions,
} from "../../../../packages/ui/src/settings/translation-provider-settings";

test("builds provider controls that update the matching preference fields", () => {
  const updates: Record<string, unknown>[] = [];
  const providers = createTranslationProviderSettings({
    client: {
      providerCredentials: undefined,
    },
    candidateTranslations: true,
    linux: false,
    windows: true,
    macos: false,
    customTranslation: { enabled: false, endpoint: "", api_key: "" },
    tencentTranslation: {
      enabled: true,
      secret_id: "",
      secret_key: "",
      region: "ap-test",
    },
    niutrans: { enabled: false, app_id: "", apikey: "" },
    translationProvider: "tencent",
    providerCredentials: undefined,
    tencentCredentialInput: { secretId: "", secretKey: "", region: undefined },
    updateTencentCredentialInput: () => undefined,
    providerCredentialBusy: undefined,
    providerCredentialMessages: {},
    runProviderCredential: async () => undefined,
    credentialTestControl: () => null,
    onPreferencesChange: (patch) => updates.push(patch),
  } satisfies TranslationProviderSettingsOptions);

  expect(providers.provider).toBe("tencent");
  providers.tencent.onSecretIdChange("synthetic-id");
  providers.tencent.onSecretKeyChange("synthetic-key");
  providers.tencent.onRegionChange("ap-synthetic");
  providers.custom.onPreferencesChange({
    custom_translation: { enabled: true, endpoint: "", api_key: "" },
  });

  expect(updates).toEqual([
    {
      tencent_tmt: { enabled: true, secret_id: "synthetic-id", secret_key: "", region: "ap-test" },
    },
    {
      tencent_tmt: { enabled: true, secret_id: "", secret_key: "synthetic-key", region: "ap-test" },
    },
    { tencent_tmt: { enabled: true, secret_id: "", secret_key: "", region: "ap-synthetic" } },
    { custom_translation: { enabled: true, endpoint: "", api_key: "" } },
  ]);
});
