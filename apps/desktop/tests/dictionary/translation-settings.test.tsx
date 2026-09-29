// @vitest-environment jsdom
import { afterEach, describe, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsPage,
  translationEndpointIssue,
  type HostCapabilities,
  type Preferences,
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
  await screen.findByRole("button", { name: "保存设置" });
  // The translation controls live on the 输入 page; other pages are hidden, and
  // hidden subtrees are absent from the accessibility tree.
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
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
  // The group and its switch share the label, so select the switch by role.
  expect(
    (screen.getByRole("checkbox", { name: "自定义翻译服务" }) as HTMLInputElement).disabled,
  ).toBe(true);
  // A disabled field must not shout about its contents.
  expect(screen.queryByRole("status")).toBeNull();
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
  fireEvent.click(screen.getByRole("checkbox", { name: "小牛翻译（NiuTrans）" }));
  expect(
    (screen.getByRole("checkbox", { name: "自定义翻译服务" }) as HTMLInputElement).checked,
  ).toBe(false);
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
        initialPage="input"
        client={{
          load: async () => snapshot,
          save,
          host: { platform } as HostCapabilities,
        }}
      />,
    );
    await screen.findByRole("button", { name: "保存设置" });
    return save;
  }
  const serviceSelect = () =>
    screen.getByRole("combobox", { name: "候选词翻译服务" }) as HTMLSelectElement;
  const optionValues = () => Array.from(serviceSelect().options).map((option) => option.value);
  async function saveAndRead(save: ReturnType<typeof vi.fn>, call: number) {
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
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
    "%s: undoing a service change leaves nothing unsaved",
    async (platform) => {
      await mountOn(platform, {
        tencent_tmt: { enabled: false, secret_id: "", secret_key: "", region: "ap-guangzhou" },
        niutrans: { enabled: false, app_id: "", apikey: "" },
      });
      fireEvent.change(serviceSelect(), { target: { value: "niutrans" } });
      expect(screen.getByText("有未保存的修改")).toBeTruthy();
      fireEvent.change(serviceSelect(), { target: { value: "custom" } });
      // The saved document omits translation_account while it is false, so the draft must not grow the key either.
      await waitFor(() => expect(screen.queryByText("有未保存的修改")).toBeNull());
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

  test("turning on Tencent from its own switch ends the account choice", async () => {
    const save = await mountOn("macos", {
      translation_account: true,
      custom_translation: { enabled: false, endpoint: "", api_key: "" },
      tencent_tmt: { enabled: false, secret_id: "", secret_key: "", region: "ap-guangzhou" },
    });
    expect(serviceSelect().value).toBe("account");
    fireEvent.click(screen.getByRole("checkbox", { name: "腾讯云机器翻译" }));
    expect(serviceSelect().value).toBe("tencent");
    // Unusable secrets must not leave the account quietly receiving candidates behind the Tencent selection.
    const saved = await saveAndRead(save, 0);
    expect(saved.tencent_tmt?.enabled).toBe(true);
    expect(saved.translation_account).toBeUndefined();
  });

  test("toggling the custom service on and off does not bring the account back", async () => {
    const save = await mountOn("macos", {
      translation_account: true,
      custom_translation: { enabled: false, endpoint: "", api_key: "" },
      tencent_tmt: { enabled: false, secret_id: "", secret_key: "", region: "ap-guangzhou" },
    });
    const custom = screen.getByRole("checkbox", { name: "自定义翻译服务" });
    fireEvent.click(custom);
    fireEvent.click(custom);
    expect(serviceSelect().value).toBe("none");
    const saved = await saveAndRead(save, 0);
    expect(saved.translation_account).toBeUndefined();
    expect(saved.custom_translation?.enabled).toBe(false);
  });

  test("Android shows the account as an unticked opt-in switch", async () => {
    const save = await mountOn("android");
    const account = screen.getByRole("checkbox", {
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
      (screen.getByRole("checkbox", { name: "使用水杉账号翻译候选词" }) as HTMLInputElement)
        .disabled,
    ).toBe(true);
  });
});

describe("macOS points at undownloaded Apple translation languages", () => {
  const noService = {
    custom_translation: { enabled: false, endpoint: "", api_key: "" },
    // Tencent is on by default; without secrets it answers nothing and on-device translation fills in.
    tencent_tmt: { enabled: true, secret_id: "", secret_key: "", region: "ap-guangzhou" },
  };
  async function mountOnMacos(preferences: Partial<Preferences>, downloadable: string[]) {
    const snapshot: Snapshot = { ...base, preferences: { ...base.preferences, ...preferences } };
    const openSettings = vi.fn(async () => {});
    const downloadableLanguages = vi.fn(async () => downloadable);
    render(
      <SettingsPage
        initialPage="input"
        client={{
          load: async () => snapshot,
          save: vi.fn(),
          host: { platform: "macos" } as HostCapabilities,
          onDeviceTranslation: { downloadableLanguages, openSettings },
        }}
      />,
    );
    await screen.findByRole("button", { name: "保存设置" });
    // Let the host's answer land, so a hidden hint means hidden and not merely not yet shown.
    await waitFor(() => expect(downloadableLanguages).toHaveBeenCalled());
    await act(async () => {});
    return openSettings;
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
});
