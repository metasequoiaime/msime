// @vitest-environment jsdom
import { expect, test } from "vitest";
import { act, renderHook } from "@testing-library/react";
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
  // 触屏键盘只显示一种候选翻译，Android 不给第二种语言。
  expect(settings.showSecondaryLanguage).toBe(false);
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
    renderHook(() =>
      useTranslationSettings({
        preferences,
        mobile: false,
        macos: false,
        linux: false,
        windows,
        candidateEnglishGlossAvailable: false,
        onDeviceDownloadable: [],
        onChange: () => undefined,
      }),
    ).result.current.translationProvider;
  // Tencent 的默认 enabled 没有可用密钥，不算用户的选择，挡不住账号；与 crates/host-api 的 selected_translation_services 一致。
  expect(derive(true)).toBe("account");
  expect(derive(false)).toBe("tencent");
});

test("macOS shows the unconfigured default Tencent as offline translation until the user picks Tencent", () => {
  const unconfigured = {
    candidate_translations: true,
    custom_translation: { enabled: false, endpoint: "", api_key: "" },
    tencent_tmt: { enabled: true, secret_id: "", secret_key: "", region: "ap-guangzhou" },
    niutrans: { enabled: false, app_id: "", apikey: "" },
  } as unknown as Preferences;
  const changes: Preferences[] = [];
  const hook = (platform: { macos: boolean; windows: boolean; linux: boolean }) =>
    renderHook(
      ({ preferences }) =>
        useTranslationSettings({
          preferences,
          mobile: false,
          ...platform,
          candidateEnglishGlossAvailable: false,
          onDeviceDownloadable: [],
          onChange: (next) => changes.push(next),
        }),
      { initialProps: { preferences: unconfigured } },
    );

  // 没有可用密钥的腾讯云在 macOS 上不会被请求，宿主用离线翻译，所以显示成 none（离线翻译）。
  const macos = hook({ macos: true, windows: false, linux: false });
  expect(macos.result.current.translationProvider).toBe("none");
  // 选腾讯云不改文档（默认已经 enabled），但本窗口要记住这次选择，凭据输入框才会出现。
  act(() => macos.result.current.setTranslationProvider("tencent"));
  expect(changes.at(-1)?.tencent_tmt?.enabled).toBe(true);
  macos.rerender({ preferences: changes.at(-1)! });
  expect(macos.result.current.translationProvider).toBe("tencent");
  act(() => macos.result.current.setTranslationProvider("none"));
  macos.rerender({ preferences: changes.at(-1)! });
  expect(macos.result.current.translationProvider).toBe("none");
  expect(changes.at(-1)?.tencent_tmt?.enabled).toBe(false);

  // 填好两个密钥后照实显示腾讯云。
  const configured = {
    ...unconfigured,
    tencent_tmt: {
      enabled: true,
      secret_id: "synthetic-id",
      secret_key: "synthetic-key",
      region: "",
    },
  } as unknown as Preferences;
  macos.rerender({ preferences: configured });
  expect(macos.result.current.translationProvider).toBe("tencent");

  // Windows 和 Linux 的显示不变：Linux 的密钥不在偏好里，偏好里的空密钥不代表没配置。
  expect(
    hook({ macos: false, windows: true, linux: false }).result.current.translationProvider,
  ).toBe("tencent");
  expect(
    hook({ macos: false, windows: false, linux: true }).result.current.translationProvider,
  ).toBe("tencent");
});

test("only macOS calls the no-service choice offline translation", () => {
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
  expect(createCandidateTranslationSettings({ ...options, macos: true }).offlineFallback).toBe(
    true,
  );
  expect(createCandidateTranslationSettings({ ...options, windows: true }).offlineFallback).toBe(
    false,
  );
  expect(createCandidateTranslationSettings({ ...options, linux: true }).offlineFallback).toBe(
    false,
  );
});
