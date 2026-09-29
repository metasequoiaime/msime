// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsPage,
  type ProviderCredentialClient,
  type ProviderCredentialStatus,
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
    ai_assistant: {
      enabled: true,
      provider: "deepseek",
      model: "deepseek-v4-flash",
      endpoint: "https://api.deepseek.com/chat/completions",
      candidate_limit: 3,
      token: "synthetic-token",
      tokens: {},
      prompt_custom_1: "",
      prompt_custom_2: "",
      prompt_custom_3: "",
    },
  },
};

async function openAi(platform: string) {
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), host: { platform } as never }}
    />,
  );
  await screen.findByRole("button", { name: "保存设置" });
  fireEvent.click(screen.getByRole("button", { name: "表达" }));
  fireEvent.click(screen.getByRole("button", { name: "AI 辅助" }));
}

test("Linux points AI credentials at the private provider config", async () => {
  await openAi("linux");

  expect(screen.queryByLabelText("AI API Token")).toBeNull();
  expect(screen.getByText("ai-provider.json")).toBeTruthy();
  expect(screen.getByLabelText("AI 接口地址")).toBeTruthy();
});

function credentialClient(initial: ProviderCredentialStatus) {
  const saved: ProviderCredentialStatus = {
    ...initial,
    ai: [
      {
        provider: "deepseek",
        endpoint: "https://api.deepseek.com/chat/completions",
        model: "deepseek-v4-flash",
      },
    ],
  };
  return {
    status: vi.fn(async () => initial),
    saveAi: vi.fn(async () => saved),
    clearAi: vi.fn(async () => ({ ...initial, ai: [] })),
    saveTencent: vi.fn(async () => initial),
    clearTencent: vi.fn(async () => initial),
    saveVoice: vi.fn(async () => ({ status: initial, serviceUpdated: true })),
    clearVoice: vi.fn(async () => ({ status: initial, serviceUpdated: true })),
  } satisfies ProviderCredentialClient;
}

test("Linux writes the AI token to the provider file, bound to the current settings", async () => {
  const credentials = credentialClient({
    ai: [],
    aiInvalid: false,
    tencent: null,
    tencentInvalid: false,
    voiceAsr: [],
    voicePolish: [],
    voiceInvalid: false,
  });
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
  await screen.findByRole("button", { name: "保存设置" });
  fireEvent.click(screen.getByRole("button", { name: "表达" }));
  fireEvent.click(screen.getByRole("button", { name: "AI 辅助" }));
  const group = screen.getByRole("group", { name: "AI 凭据" });
  expect(group.textContent).toContain("尚未保存");
  const save = screen.getByRole("button", { name: "保存凭据" }) as HTMLButtonElement;
  expect(save.disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("AI API Token"), { target: { value: "sk-live" } });
  fireEvent.click(save);
  await waitFor(() => expect(credentials.saveAi).toHaveBeenCalled());
  expect(credentials.saveAi).toHaveBeenCalledWith({
    provider: "deepseek",
    endpoint: "https://api.deepseek.com/chat/completions",
    model: "deepseek-v4-flash",
    token: "sk-live",
  });
  await screen.findByText("凭据已保存，provider 服务下次请求时生效。");
  expect((screen.getByLabelText("AI API Token") as HTMLInputElement).value).toBe("");
  expect(group.textContent).toContain("留空则保留原凭据");

  // Rebinding to another model keeps the stored token: no token is sent.
  fireEvent.change(screen.getByLabelText("AI 模型"), { target: { value: "deepseek-v4-pro" } });
  expect(group.textContent).toContain("与上方设置不一致");
  fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));
  await waitFor(() => expect(credentials.saveAi).toHaveBeenCalledTimes(2));
  expect(credentials.saveAi).toHaveBeenLastCalledWith({
    provider: "deepseek",
    endpoint: "https://api.deepseek.com/chat/completions",
    model: "deepseek-v4-pro",
  });
});

test("Linux reports a provider file it cannot use", async () => {
  const credentials = credentialClient({
    ai: [],
    aiInvalid: true,
    tencent: null,
    tencentInvalid: false,
    voiceAsr: [],
    voicePolish: [],
    voiceInvalid: false,
  });
  credentials.saveAi.mockRejectedValueOnce({ code: "provider_credentials_existing_invalid" });
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
  await screen.findByRole("button", { name: "保存设置" });
  fireEvent.click(screen.getByRole("button", { name: "表达" }));
  fireEvent.click(screen.getByRole("button", { name: "AI 辅助" }));
  await screen.findByText(/现有 ai-provider.json 无效/);
  fireEvent.change(screen.getByLabelText("AI API Token"), { target: { value: "sk-live" } });
  fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));
  expect((await screen.findByRole("alert")).textContent).toContain("请修复或删除后重试");
});

test("Windows keeps the shared AI token field", async () => {
  await openAi("windows");

  expect(screen.getByLabelText("AI API Token")).toBeTruthy();
  expect(screen.queryByText("ai-provider.json")).toBeNull();
});

function expectRevealToggle() {
  const token = screen.getByLabelText("AI API Token") as HTMLInputElement;
  const toggle = screen.getByRole("button", { name: "显示AI API Token" });
  expect(token.type).toBe("password");
  expect(toggle.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(toggle);
  expect(token.type).toBe("text");
  expect(
    screen.getByRole("button", { name: "隐藏AI API Token" }).getAttribute("aria-pressed"),
  ).toBe("true");
}

test("Linux lets the user reveal the AI token before saving it", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "linux" } as never,
        providerCredentials: credentialClient({
          ai: [],
          aiInvalid: false,
          tencent: null,
          tencentInvalid: false,
          voiceAsr: [],
          voicePolish: [],
          voiceInvalid: false,
        }),
      }}
    />,
  );
  await screen.findByRole("button", { name: "保存设置" });
  fireEvent.click(screen.getByRole("button", { name: "表达" }));
  fireEvent.click(screen.getByRole("button", { name: "AI 辅助" }));
  await screen.findByRole("group", { name: "AI 凭据" });

  expectRevealToggle();
});

test("Windows lets the user reveal the shared AI token", async () => {
  await openAi("windows");

  expectRevealToggle();
});

test("the AI token and its toggle stay disabled without an HTTPS endpoint", async () => {
  await openAi("windows");
  fireEvent.change(screen.getByLabelText("AI 接口地址"), { target: { value: "not a url" } });

  expect((screen.getByLabelText("AI API Token") as HTMLInputElement).disabled).toBe(true);
  expect(
    (screen.getByRole("button", { name: "显示AI API Token" }) as HTMLButtonElement).disabled,
  ).toBe(true);
});
