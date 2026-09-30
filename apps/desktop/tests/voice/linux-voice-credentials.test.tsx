// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsPage,
  VoiceCredentialControl,
  type HostCapabilities,
  type ProviderCredentialClient,
  type ProviderCredentialStatus,
  type Snapshot,
} from "@msime/ui";

afterEach(cleanup);

const snapshot: Snapshot = {
  format_version: 1,
  revision: 4,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    voice_input: {
      enabled: true,
      language: "zh-CN",
      asr_provider: "doubao",
      asr_model: "",
      asr_resource_id: "volc.bigasr.sauc.duration",
      doubao_auth_mode: "legacy",
      polish_provider: "deepseek",
      polish_model: "deepseek-v4-flash",
    },
  },
};

const empty: ProviderCredentialStatus = {
  ai: [],
  aiInvalid: false,
  tencent: null,
  tencentInvalid: false,
  voiceAsr: [],
  voicePolish: [],
  voiceInvalid: false,
};

function credentialClient(initial: ProviderCredentialStatus) {
  return {
    status: vi.fn(async () => initial),
    saveAi: vi.fn(async () => initial),
    clearAi: vi.fn(async () => initial),
    saveTencent: vi.fn(async () => initial),
    clearTencent: vi.fn(async () => initial),
    saveVoice: vi.fn(async () => ({
      status: {
        ...initial,
        voiceAsr: [
          {
            provider: "doubao",
            model: "",
            endpoint: "",
            resourceId: "volc.bigasr.sauc.duration",
            authMode: "legacy",
          },
        ],
      },
      serviceUpdated: false,
    })),
    clearVoice: vi.fn(async () => ({ status: initial, serviceUpdated: true })),
  } satisfies ProviderCredentialClient;
}

test("voice credential control projects the selected ASR provider", () => {
  render(
    <VoiceCredentialControl
      available
      kind="asr"
      voiceInput={{ enabled: true, language: "zh-CN", asr_provider: "doubao" }}
      doubaoAuthMode="legacy"
      providerCredentials={empty}
      voiceCredentialInput={{ asr: { token: "", appKey: "" }, polish: { token: "", appKey: "" } }}
      setVoiceCredentialInput={vi.fn()}
      providerCredentialBusy={undefined}
      providerCredentialMessages={{}}
      runVoiceCredential={vi.fn()}
    />,
  );
  expect(screen.getByRole("group", { name: "语音识别凭据" })).toBeTruthy();
  expect(screen.getByLabelText("Doubao App Key")).toBeTruthy();
});

async function openVoice(credentials: ProviderCredentialClient) {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "linux" } as HostCapabilities,
        providerCredentials: credentials,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
}

test("Linux saves the Doubao recognition credential bound to the current voice settings", async () => {
  const credentials = credentialClient(empty);
  await openVoice(credentials);
  const group = screen.getByRole("group", { name: "语音识别凭据" });
  expect(group.textContent).toContain("尚未保存");
  const save = screen.getByRole("button", { name: "保存识别凭据" }) as HTMLButtonElement;
  fireEvent.change(screen.getByLabelText("识别 API Token"), { target: { value: "access" } });
  // Legacy Doubao authentication needs the App Key as well.
  expect(save.disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("Doubao App Key"), { target: { value: "app-1" } });
  fireEvent.click(save);
  await waitFor(() => expect(credentials.saveVoice).toHaveBeenCalled());
  expect(credentials.saveVoice).toHaveBeenCalledWith({
    kind: "asr",
    provider: "doubao",
    endpoint: "",
    model: "",
    token: "access",
    appKey: "app-1",
    resourceId: "volc.bigasr.sauc.duration",
    authMode: "legacy",
  });
  // The file is saved even when the user service manager is not reachable, and the user is told how to start the service.
  expect((await screen.findByRole("alert")).textContent).toContain(
    "systemctl --user enable --now msime-linux-voice.socket",
  );
  expect(group.textContent).toContain("已保存");
  expect(screen.getByRole("button", { name: "清除识别凭据" })).toBeTruthy();
});

test("Linux polishing credential names the provider and model selected above", async () => {
  const credentials = credentialClient({
    ...empty,
    voicePolish: [
      {
        provider: "deepseek",
        model: "deepseek-chat",
        endpoint: "",
        resourceId: null,
        authMode: null,
      },
    ],
  });
  await openVoice(credentials);
  const group = await screen.findByRole("group", { name: "语音润色凭据" });
  await waitFor(() => expect(group.textContent).toContain("不一致"));
  // A stored token is kept: rebinding the model does not need it pasted again.
  fireEvent.click(screen.getByRole("button", { name: "保存润色凭据" }));
  await waitFor(() =>
    expect(credentials.saveVoice).toHaveBeenCalledWith({
      kind: "polish",
      provider: "deepseek",
      endpoint: "",
      model: "deepseek-v4-flash",
      resourceId: "",
      authMode: "",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "清除润色凭据" }));
  await waitFor(() => expect(credentials.clearVoice).toHaveBeenCalledWith("polish", "deepseek"));
});

test("Linux offers the Doubao streaming interfaces in the credential it saves", async () => {
  const credentials = credentialClient(empty);
  await openVoice(credentials);
  const stream = screen.getByLabelText("流式接口") as HTMLSelectElement;
  // No stored address means the provider default, the bidirectional endpoint.
  expect(stream.value).toBe("async");
  fireEvent.change(stream, { target: { value: "nostream" } });
  expect((screen.getByLabelText("识别接口地址") as HTMLInputElement).value).toBe(
    "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream",
  );
  fireEvent.change(screen.getByLabelText("识别 API Token"), { target: { value: "access" } });
  fireEvent.change(screen.getByLabelText("Doubao App Key"), { target: { value: "app-1" } });
  fireEvent.click(screen.getByRole("button", { name: "保存识别凭据" }));
  await waitFor(() =>
    expect(credentials.saveVoice).toHaveBeenCalledWith(
      expect.objectContaining({
        endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream",
      }),
    ),
  );
  // Only one picker: the Windows one writes the shared preference Linux does not read.
  expect(screen.getAllByLabelText("流式接口")).toHaveLength(1);
});
