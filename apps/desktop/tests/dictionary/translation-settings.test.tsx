// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, describe, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsPage,
  translationEndpointIssue,
  type Preferences,
  type SettingsClient,
  type Snapshot,
} from "@msime/ui";

afterEach(cleanup);

const base: Snapshot = {
  format_version: 1,
  revision: 11,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    candidate_translations: true,
    custom_translation: {
      enabled: true,
      endpoint: "https://example.com/translate",
      api_key: "secret-value",
    },
  },
};

async function mount(preferences: Record<string, unknown> = {}) {
  const snapshot: Snapshot = { ...base, preferences: { ...base.preferences, ...preferences } };
  const mounted = render(<SettingsPage client={{ load: async () => snapshot, save: vi.fn() }} />);
  await settingsFormReady();
  // The translation controls live on the 输入 page; other pages are hidden, and
  // hidden subtrees are absent from the accessibility tree.
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  return mounted;
}

const endpointField = () => screen.getByLabelText("自定义翻译 Endpoint") as HTMLInputElement;

describe("translationEndpointIssue mirrors the Rust rule", () => {
  // client-core::translation::is_supported_endpoint: non-empty, <= 2048 bytes,
  // no control characters, HTTPS for remote services, and loopback-only HTTP.
  test.each([
    ["", true],
    ["example.com/translate", true],
    ["ftp://example.com", true],
    ["https://example.com/translate", false],
    ["http://example.com/translate", true],
    ["http://localhost:1188/translate", false],
    ["http://127.0.0.1:1188/translate", false],
    ["http://[::1]:1188/translate", false],
    ["http://user:secret@localhost/translate", true],
    ["https://example.com/translate#fragment", true],
    ["https://example.com/\u0007", true],
    [`https://example.com/${"a".repeat(2048)}`, true],
    [`https://example.com/${"合".repeat(700)}`, true],
  ])("%s", (endpoint, expectIssue) => {
    expect(translationEndpointIssue(endpoint) !== "").toBe(expectIssue);
  });
});

test("a scheme-less endpoint warns instead of silently returning no glosses", async () => {
  await mount();
  expect(screen.queryByRole("status")).toBeNull();
  fireEvent.change(endpointField(), { target: { value: "example.com/translate" } });
  expect(screen.getByRole("status").textContent).toContain("http://");
  fireEvent.change(endpointField(), { target: { value: "https://example.com/translate" } });
  expect(screen.queryByRole("status")).toBeNull();
});

test("translation credentials are disabled while candidate translation is off", async () => {
  await mount({ candidate_translations: false });
  expect(endpointField().disabled).toBe(true);
  expect((screen.getByLabelText("自定义翻译 API Key") as HTMLInputElement).disabled).toBe(true);
  expect(
    (screen.getByRole("combobox", { name: "候选词翻译服务" }) as HTMLSelectElement).disabled,
  ).toBe(true);
  // A disabled field must not shout about its contents.
  expect(screen.queryByRole("status")).toBeNull();
});

test("only the chosen service's settings are shown, in the order the select lists them", async () => {
  await mount();
  const select = screen.getByRole("combobox", { name: "候选词翻译服务" }) as HTMLSelectElement;
  expect(Array.from(select.options).map((option) => option.value)).toEqual([
    "none",
    "tencent",
    "niutrans",
    "custom",
  ]);
  const shown = () => ({
    tencent: screen.queryByLabelText("腾讯云 SecretId") !== null,
    niutrans: screen.queryByLabelText("NiuTrans App ID") !== null,
    custom: screen.queryByLabelText("自定义翻译 Endpoint") !== null,
  });
  expect(shown()).toEqual({ tencent: false, niutrans: false, custom: true });
  fireEvent.change(select, { target: { value: "tencent" } });
  expect(shown()).toEqual({ tencent: true, niutrans: false, custom: false });
  expect(screen.getByRole("heading", { name: "腾讯云机器翻译" })).toBeTruthy();
  fireEvent.change(select, { target: { value: "niutrans" } });
  expect(shown()).toEqual({ tencent: false, niutrans: true, custom: false });
  fireEvent.change(select, { target: { value: "none" } });
  expect(shown()).toEqual({ tencent: false, niutrans: false, custom: false });
  // 下拉框是选择服务的唯一方式；没有哪个组自带开关。
  for (const name of ["腾讯云机器翻译", "小牛翻译（NiuTrans）", "自定义翻译服务"]) {
    expect(screen.queryByRole("switch", { name })).toBeNull();
  }
});

test("the API key can be revealed to check a pasted value", async () => {
  await mount();
  const key = screen.getByLabelText("自定义翻译 API Key") as HTMLInputElement;
  expect(key.type).toBe("password");
  const reveal = screen.getByRole("button", { name: "显示自定义翻译 API Key" });
  expect(reveal.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(reveal);
  expect((screen.getByLabelText("自定义翻译 API Key") as HTMLInputElement).type).toBe("text");
  expect(
    screen.getByRole("button", { name: "隐藏自定义翻译 API Key" }).getAttribute("aria-pressed"),
  ).toBe("true");
});

test("NiuTrans provider is mutually exclusive and exposes synthetic credential fields", async () => {
  await mount();
  fireEvent.change(screen.getByRole("combobox", { name: "候选词翻译服务" }), {
    target: { value: "niutrans" },
  });
  // 选择 NiuTrans 会关闭自定义服务，它的设置也随之离开页面。
  expect(screen.queryByLabelText("自定义翻译 Endpoint")).toBeNull();
  const appId = screen.getByLabelText("NiuTrans App ID") as HTMLInputElement;
  const apiKey = screen.getByLabelText("NiuTrans API Key") as HTMLInputElement;
  expect(appId.disabled).toBe(false);
  fireEvent.change(appId, { target: { value: "synthetic-app" } });
  fireEvent.change(apiKey, { target: { value: "synthetic-key" } });
  expect(appId.value).toBe("synthetic-app");
  expect(apiKey.value).toBe("synthetic-key");
});

describe("the MSIME account translation is an explicit choice", () => {
  async function mountOn(platform: string, preferences: Partial<Preferences> = {}) {
    const snapshot: Snapshot = { ...base, preferences: { ...base.preferences, ...preferences } };
    // Echo the saved document back, as the hosts do, so a second save starts from the first.
    const save = vi.fn(async (revision: number, saved: Preferences) => ({
      ...snapshot,
      revision: revision + 1,
      preferences: saved,
    }));
    render(
      <SettingsPage
        initialPage="expression"
        client={{
          load: async () => snapshot,
          save,
          host: testHost({ platform }),
        }}
      />,
    );
    await settingsFormReady();
    return save;
  }
  const serviceSelect = () =>
    screen.getByRole("combobox", { name: "候选词翻译服务" }) as HTMLSelectElement;
  const optionValues = () => Array.from(serviceSelect().options).map((option) => option.value);
  async function saveAndRead(save: ReturnType<typeof vi.fn>, call: number) {
    saveSettingsNow();
    await waitFor(() => expect(save).toHaveBeenCalledTimes(call + 1));
    return save.mock.calls[call][1] as Preferences;
  }
  const noOwnService = (saved: Preferences) => {
    expect(saved.custom_translation?.enabled).toBe(false);
    expect(saved.tencent_tmt?.enabled).toBe(false);
    expect(saved.niutrans?.enabled).toBe(false);
  };

  test.each(["macos", "linux"])(
    "%s offers the account and saves it with every other service off",
    async (platform) => {
      const save = await mountOn(platform);
      expect(optionValues()).toContain("account");
      expect(serviceSelect().value).toBe("custom");
      fireEvent.change(serviceSelect(), { target: { value: "account" } });
      expect(serviceSelect().value).toBe("account");
      const chosen = await saveAndRead(save, 0);
      expect(chosen.translation_account).toBe(true);
      noOwnService(chosen);

      fireEvent.change(serviceSelect(), { target: { value: "none" } });
      const off = await saveAndRead(save, 1);
      expect(off.translation_account).toBeUndefined();
      noOwnService(off);
    },
  );

  test.each(["windows", "linux", "macos"])(
    "%s: undoing a service change leaves nothing to save",
    async (platform) => {
      const save = await mountOn(platform, {
        tencent_tmt: { enabled: false, secret_id: "", secret_key: "", region: "ap-guangzhou" },
        niutrans: { enabled: false, app_id: "", apikey: "" },
      });
      fireEvent.change(serviceSelect(), { target: { value: "niutrans" } });
      fireEvent.change(serviceSelect(), { target: { value: "custom" } });
      // The saved document omits translation_account while it is false, so the draft must not grow the key either.
      saveSettingsNow();
      expect(save).not.toHaveBeenCalled();
    },
  );

  test("Windows has no account option", async () => {
    const platform = "windows";
    await mountOn(platform);
    expect(optionValues()).not.toContain("account");
  });

  test("a document without the key never shows the account as chosen on macOS", async () => {
    await mountOn("macos", {
      custom_translation: { enabled: false, endpoint: "", api_key: "" },
      tencent_tmt: { enabled: false, secret_id: "", secret_key: "", region: "ap-guangzhou" },
    });
    expect(serviceSelect().value).toBe("none");
  });

  test("choosing Tencent ends the account choice", async () => {
    const save = await mountOn("macos", {
      translation_account: true,
      custom_translation: { enabled: false, endpoint: "", api_key: "" },
      tencent_tmt: { enabled: false, secret_id: "", secret_key: "", region: "ap-guangzhou" },
    });
    expect(serviceSelect().value).toBe("account");
    // 账号不使用用户自己的凭据，所以选中它时下拉框下面什么都不显示。
    expect(screen.queryByLabelText("腾讯云 SecretId")).toBeNull();
    fireEvent.change(serviceSelect(), { target: { value: "tencent" } });
    expect(serviceSelect().value).toBe("tencent");
    // Unusable secrets must not leave the account quietly receiving candidates behind the Tencent selection.
    const saved = await saveAndRead(save, 0);
    expect(saved.tencent_tmt?.enabled).toBe(true);
    expect(saved.translation_account).toBeUndefined();
  });

  test("a chosen account shows despite Tencent's credential-less default", async () => {
    // 显式选了水杉账号的文档旁边还留着腾讯云默认的 `enabled: true`（没有凭据）；宿主走的是账号，页面必须显示账号。
    await mountOn("macos", {
      translation_account: true,
      custom_translation: { enabled: false, endpoint: "", api_key: "" },
      tencent_tmt: { enabled: true, secret_id: "", secret_key: "", region: "ap-guangzhou" },
    });
    expect(serviceSelect().value).toBe("account");
  });

  test("usable Tencent secrets take precedence over the account", async () => {
    await mountOn("linux", {
      translation_account: true,
      custom_translation: { enabled: false, endpoint: "", api_key: "" },
      tencent_tmt: { enabled: true, secret_id: "id", secret_key: "key", region: "ap-guangzhou" },
    });
    expect(serviceSelect().value).toBe("tencent");
  });

  test("choosing the custom service and then 关闭 does not bring the account back", async () => {
    const save = await mountOn("macos", {
      translation_account: true,
      custom_translation: { enabled: false, endpoint: "", api_key: "" },
      tencent_tmt: { enabled: false, secret_id: "", secret_key: "", region: "ap-guangzhou" },
    });
    fireEvent.change(serviceSelect(), { target: { value: "custom" } });
    expect(serviceSelect().value).toBe("custom");
    fireEvent.change(serviceSelect(), { target: { value: "none" } });
    expect(serviceSelect().value).toBe("none");
    const saved = await saveAndRead(save, 0);
    expect(saved.translation_account).toBeUndefined();
    expect(saved.custom_translation?.enabled).toBe(false);
  });

  test("Android shows the account as an unticked opt-in switch", async () => {
    const save = await mountOn("android");
    const account = screen.getByRole("switch", {
      name: "使用水杉账号翻译候选词",
    }) as HTMLInputElement;
    expect(account.checked).toBe(false);
    fireEvent.click(account);
    expect(account.checked).toBe(true);
    const chosen = await saveAndRead(save, 0);
    expect(chosen.translation_account).toBe(true);
    noOwnService(chosen);
    fireEvent.click(account);
    expect((await saveAndRead(save, 1)).translation_account).toBeUndefined();
  });

  test("the Android switch follows candidate translation", async () => {
    await mountOn("android", { candidate_translations: false });
    expect(
      (screen.getByRole("switch", { name: "使用水杉账号翻译候选词" }) as HTMLInputElement).disabled,
    ).toBe(true);
  });
});

describe("macOS points at undownloaded Apple translation languages", () => {
  const noService = {
    custom_translation: { enabled: false, endpoint: "", api_key: "" },
    // Tencent is on by default; without secrets it answers nothing and on-device translation fills in.
    tencent_tmt: { enabled: true, secret_id: "", secret_key: "", region: "ap-guangzhou" },
  };
  async function mountOnMacos(
    preferences: Partial<Preferences>,
    downloadable: string[],
    openSettings: boolean = true,
  ) {
    const snapshot: Snapshot = { ...base, preferences: { ...base.preferences, ...preferences } };
    const openLanguageSettings = vi.fn(async () => {});
    const downloadableLanguages = vi.fn(async () => downloadable);
    const onDeviceTranslation = openSettings
      ? { downloadableLanguages, openSettings: openLanguageSettings }
      : ({ downloadableLanguages } as unknown as NonNullable<
          SettingsClient["onDeviceTranslation"]
        >);
    render(
      <SettingsPage
        initialPage="expression"
        client={{
          load: async () => snapshot,
          save: vi.fn(),
          host: testHost({ platform: "macos" }),
          onDeviceTranslation,
        }}
      />,
    );
    await settingsFormReady();
    // Let the host's answer land, so a hidden hint means hidden and not merely not yet shown.
    await waitFor(() => expect(downloadableLanguages).toHaveBeenCalled());
    await act(async () => {});
    return openLanguageSettings;
  }
  const hint = () => screen.queryByRole("status", { name: "系统翻译语言未下载" });

  test("names only the chosen targets and opens System Settings", async () => {
    const openSettings = await mountOnMacos(
      { ...noService, translation_target_language: "en", translation_secondary_language: "ja" },
      ["en", "fr", "ja"],
    );
    expect(hint()).not.toBeNull();
    expect(hint()!.textContent).toContain("英语、日语");
    expect(hint()!.textContent).not.toContain("法语");
    fireEvent.click(screen.getByRole("button", { name: "打开语言与地区" }));
    expect(openSettings).toHaveBeenCalledTimes(1);
  });

  test("stays hidden while a service of the user's own answers", async () => {
    // base selects the custom DeepLX service, which translates every candidate itself.
    await mountOnMacos({}, ["en"]);
    expect(hint()).toBeNull();
  });

  test("stays hidden when candidate translation is off or nothing is missing", async () => {
    await mountOnMacos({ ...noService, candidate_translations: false }, ["en"]);
    expect(hint()).toBeNull();
    cleanup();
    await mountOnMacos(noService, []);
    expect(hint()).toBeNull();
  });

  test("omits the settings button when the host cannot open settings", async () => {
    await mountOnMacos(noService, ["en"], false);
    expect(hint()).not.toBeNull();
    expect(screen.queryByRole("button", { name: "打开语言与地区" })).toBeNull();
  });
});
