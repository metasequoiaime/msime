// @vitest-environment jsdom
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsPage,
  tencentCredentialIssue,
  tencentSecretConfigured,
  type Preferences,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 2,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    candidate_translations: true,
  },
};

test("a placeholder is not a configured secret", () => {
  // Mirrors usable_credential in client-core: the shipped config template
  // carries <YOUR_TENCENT_SECRET_ID>, which must not read as configured.
  expect(tencentSecretConfigured("")).toBe(false);
  expect(tencentSecretConfigured("   ")).toBe(false);
  expect(tencentSecretConfigured("<YOUR_TENCENT_SECRET_ID>")).toBe(false);
  expect(tencentSecretConfigured("FAKESECRET_abc")).toBe(false);
  expect(tencentSecretConfigured("AKIDreal")).toBe(true);
});

test("credential rules match the ones that would reject the save", () => {
  // Preferences::validate refuses these outright, so the page says so first
  // rather than letting the save fail with no explanation.
  expect(tencentCredentialIssue("AKID_ok-1", "key", "ap-guangzhou")).toBe("");
  expect(tencentCredentialIssue("has space", "key", "ap-guangzhou")).toContain("SecretId");
  expect(tencentCredentialIssue("AKID", "key", "ap guangzhou")).toContain("地域");
  expect(tencentCredentialIssue("AKID", "key", "a".repeat(65))).toContain("地域");
  expect(tencentCredentialIssue("a".repeat(4097), "key", "ap-guangzhou")).toContain("过长");
  expect(tencentCredentialIssue("AKID", "bad\u0001key", "ap-guangzhou")).toContain("控制字符");
});

test("the credentials can be entered and are saved", async () => {
  const save = vi.fn(async (_revision: number, _preferences: Preferences) => snapshot);
  render(<SettingsPage client={{ load: async () => snapshot, save }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "表达" }));

  // Before this change there was no way to enter these at all.
  const id = await screen.findByLabelText("腾讯云 SecretId");
  fireEvent.change(id, { target: { value: "AKIDexample" } });
  fireEvent.change(screen.getByLabelText("腾讯云 SecretKey"), { target: { value: "s3cret" } });
  fireEvent.change(screen.getByLabelText("腾讯云地域"), { target: { value: "ap-shanghai" } });

  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls[0][1];
  expect(saved.tencent_tmt).toEqual({
    enabled: true,
    secret_id: "AKIDexample",
    secret_key: "s3cret",
    region: "ap-shanghai",
  });
});

test("empty credentials are called out instead of silently returning nothing", async () => {
  render(<SettingsPage client={{ load: async () => snapshot, save: vi.fn() }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "表达" }));

  // This is the user-visible defect: translation on, no keys, no explanation.
  await screen.findByText(/未填写腾讯云凭据/);

  // Entering a usable pair clears it.
  fireEvent.change(screen.getByLabelText("腾讯云 SecretId"), { target: { value: "AKIDexample" } });
  fireEvent.change(screen.getByLabelText("腾讯云 SecretKey"), { target: { value: "s3cret" } });
  await waitFor(() => expect(screen.queryByText(/未填写腾讯云凭据/)).toBeNull());

  // A malformed SecretId reports the rule that would reject the save instead.
  fireEvent.change(screen.getByLabelText("腾讯云 SecretId"), { target: { value: "bad id" } });
  await screen.findByText(/SecretId 只能包含/);
});

test("the copy no longer claims a Linux provider on every platform", async () => {
  render(<SettingsPage client={{ load: async () => snapshot, save: vi.fn() }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "表达" }));
  await screen.findByLabelText("腾讯云 SecretId");
  // Windows performs the request natively in TranslationWorker, so telling
  // every user it goes through a Linux provider socket was simply wrong.
  // Scoped to the translation groups: the voice sections carry the same wrong
  // claim, but that is a separate gap and is not touched here.
  const online = screen.getByRole("group", { name: "在线翻译服务" });
  const custom = screen.getByRole("group", { name: "自定义翻译服务" });
  expect(online.textContent).not.toContain("Linux provider");
  expect(custom.textContent).not.toContain("Linux provider");
  expect(custom.textContent).toContain("DeepLX");
});

test("Linux delegates Tencent credentials to the user-managed provider", async () => {
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), host: { platform: "linux" } as never }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "表达" }));
  const online = screen.getByRole("group", { name: "在线翻译服务" });
  expect(online.textContent).toContain("tencent-provider.json");
  expect(screen.queryByLabelText("腾讯云 SecretId")).toBeNull();
  expect(screen.queryByLabelText("腾讯云 SecretKey")).toBeNull();
  expect(screen.queryByLabelText("腾讯云地域")).toBeNull();
});

test("Linux saves Tencent credentials to the provider file", async () => {
  const configured = {
    ai: [],
    aiInvalid: false,
    tencent: { region: "ap-guangzhou" },
    tencentInvalid: false,
    voiceAsr: [],
    voicePolish: [],
    voiceInvalid: false,
  };
  const credentials = {
    status: vi.fn(async () => ({ ...configured, tencent: null })),
    saveAi: vi.fn(),
    clearAi: vi.fn(),
    saveTencent: vi.fn(async () => configured),
    clearTencent: vi.fn(async () => ({ ...configured, tencent: null })),
    saveVoice: vi.fn(),
    clearVoice: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "linux" } as never,
        providerCredentials: credentials,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "表达" }));
  const online = screen.getByRole("group", { name: "在线翻译服务" });
  await waitFor(() => expect(credentials.status).toHaveBeenCalled());
  expect(online.textContent).toContain("tencent-provider.json");
  const save = screen.getByRole("button", { name: "保存凭据" }) as HTMLButtonElement;
  expect(save.disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("腾讯云 SecretId"), { target: { value: "AKIDexample" } });
  fireEvent.change(screen.getByLabelText("腾讯云 SecretKey"), { target: { value: "secret" } });
  fireEvent.change(screen.getByLabelText("腾讯云地域"), { target: { value: "ap-shanghai" } });
  fireEvent.click(save);
  await waitFor(() =>
    expect(credentials.saveTencent).toHaveBeenCalledWith({
      secretId: "AKIDexample",
      secretKey: "secret",
      region: "ap-shanghai",
    }),
  );
  await screen.findByRole("button", { name: "清除凭据" });
  expect((screen.getByLabelText("腾讯云 SecretKey") as HTMLInputElement).value).toBe("");
  fireEvent.click(screen.getByRole("button", { name: "清除凭据" }));
  await waitFor(() => expect(credentials.clearTencent).toHaveBeenCalled());
});

test("macOS exposes the native Tencent credential probe with current settings", async () => {
  const macosSnapshot: Snapshot = {
    ...snapshot,
    preferences: {
      ...snapshot.preferences,
      tencent_tmt: {
        enabled: true,
        secret_id: "AKIDmacos",
        secret_key: "macos-secret",
        region: "ap-tokyo",
      },
    },
  };
  const testApiCredential = vi
    .fn()
    .mockResolvedValue({ ok: true, message: "macOS fixture success" });
  render(
    <SettingsPage
      initialPage="expression"
      client={{
        load: async () => macosSnapshot,
        save: vi.fn(),
        testApiCredential,
        host: { platform: "macos" } as never,
      }}
    />,
  );

  const button = await screen.findByRole("button", { name: "测试腾讯云翻译配置" });
  fireEvent.click(button);
  await screen.findByText("macOS fixture success");
  expect(testApiCredential).toHaveBeenCalledWith("translation.tencent", {
    secret_id: "AKIDmacos",
    secret_key: "macos-secret",
    region: "ap-tokyo",
  });
});
