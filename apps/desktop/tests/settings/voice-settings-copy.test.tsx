// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("voice settings pages reuse the shared polish settings section", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/voice-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/voice-settings-panel.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(page).toContain('import { VoicePolishSettingsSection } from "../voice-polish-settings-section";');
  expect(panel).toContain('import { VoicePolishSettingsSection } from "./voice-polish-settings-section";');
  expect(page).toContain("<VoicePolishSettingsSection");
  expect(panel).toContain("<VoicePolishSettingsSection");
  expect(page).not.toContain("customPrompts={{");
  expect(panel).not.toContain("customPrompts={{");
});

test("voice settings pages reuse the shared ASR provider section", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/voice-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/voice-settings-panel.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(page).toContain('import { VoiceAsrProviderSettingsSection } from "../voice-asr-provider-settings-section";');
  expect(panel).toContain('import { VoiceAsrProviderSettingsSection } from "./voice-asr-provider-settings-section";');
  expect(page).toContain("<VoiceAsrProviderSettingsSection");
  expect(panel).toContain("<VoiceAsrProviderSettingsSection");
  expect(page).not.toContain("<VoiceModelSection");
  expect(panel).not.toContain("<VoiceModelSection");
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
    voice_input: {
      enabled: true,
      language: "zh-CN",
      asr_provider: "doubao",
      doubao_auth_mode: "legacy",
      asr_token: "secret-token",
    },
  },
};

function host(platform: string) {
  return { platform, voice_capture_devices: true } as never;
}

async function openVoice(platform: string) {
  render(
    <SettingsPage client={{ load: async () => snapshot, save: vi.fn(), host: host(platform) }} />,
  );
  await screen.findByRole("button", { name: "保存设置" });
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  // The token fields are hidden on Linux, where credentials belong to the
  // provider service, so wait on a control both platforms render.
  return await screen.findByLabelText(platform === "android" ? "识别语言" : "识别服务");
}

test("a pasted recognition token can be revealed to check it", async () => {
  await openVoice("windows");
  const token = await screen.findByLabelText("识别 API Token");
  // Before this the field was a bare <input type=password>: a truncated or
  // mistyped paste could not be checked, even though SecretInput already
  // existed and was used for the translation key.
  expect((token as HTMLInputElement).type).toBe("password");
  fireEvent.click(screen.getByRole("button", { name: "显示识别 API Token" }));
  expect((screen.getByLabelText("识别 API Token") as HTMLInputElement).type).toBe("text");
  expect((screen.getByLabelText("识别 API Token") as HTMLInputElement).value).toBe("secret-token");
  fireEvent.click(screen.getByRole("button", { name: "隐藏识别 API Token" }));
  expect((screen.getByLabelText("识别 API Token") as HTMLInputElement).type).toBe("password");
});

test("the Doubao app key and polish token get the same toggle", async () => {
  await openVoice("windows");
  expect(screen.getByRole("button", { name: "显示Doubao App Key" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "显示润色 API Token" })).toBeTruthy();
});

// Asserted element by element rather than over document.body: every page is
// mounted at once, so a whole-body scan also picks up unrelated sections.
test("Windows is not told its voice input runs through a Linux provider", async () => {
  await openVoice("windows");
  // On Windows these options drive VoiceHotkeyController, CuePlayer and
  // SystemAudioMuter in-process; there is no provider socket involved.
  expect(screen.getByText("语音快捷键")).toBeTruthy();
  expect(screen.getByText("录音行为")).toBeTruthy();
  expect(screen.queryByText("Linux provider 行为")).toBeNull();
  expect(screen.queryByText(/语音需要 provider 服务/)).toBeNull();
  expect(screen.queryByText(/录音和识别由已配置的 provider 服务完成/)).toBeNull();
  expect(screen.getByText(/录音和识别在本机完成/)).toBeTruthy();
  expect(screen.getByText(/随识别请求发送给豆包/)).toBeTruthy();
});

test("macOS keeps voice submission in the native input-method process", async () => {
  await openVoice("macos");
  expect(screen.getByText("macOS 输入法语音")).toBeTruthy();
  expect(screen.getByText(/由当前输入法进程负责/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "打开" })).toBeNull();
  expect(screen.queryByText("打开语音输入")).toBeNull();
});

test("Linux keeps the wording that is accurate there", async () => {
  await openVoice("linux");
  expect(screen.getByText("Linux provider 行为")).toBeTruthy();
  // The shortcuts work the same under IBus and Fcitx5, so the section is not named after one host.
  expect(screen.getByText("语音快捷键")).toBeTruthy();
  expect(screen.queryByText(/IBus 快捷键/)).toBeNull();
  expect(screen.getByText(/语音需要 provider 服务/)).toBeTruthy();
  expect(screen.queryByText(/IBus 属性/)).toBeNull();
  expect(screen.queryByText("录音行为")).toBeNull();
});

test("Linux exposes Doubao auth mode without exposing provider credentials", async () => {
  const linuxSnapshot: Snapshot = {
    ...snapshot,
    preferences: {
      ...snapshot.preferences,
      voice_input: {
        enabled: true,
        language: "zh-CN",
        ...snapshot.preferences.voice_input,
        doubao_auth_mode: "api_key",
      },
    },
  };
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...linuxSnapshot,
    revision: 3,
    preferences,
  }));
  render(<SettingsPage client={{ load: async () => linuxSnapshot, save, host: host("linux") }} />);
  await screen.findByRole("button", { name: "保存设置" });
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  const mode = (await screen.findByLabelText("豆包鉴权方式")) as HTMLSelectElement;
  expect(mode.value).toBe("api_key");
  expect(screen.queryByLabelText("Doubao API Key")).toBeNull();
  expect(screen.queryByLabelText("Doubao App Key")).toBeNull();
  fireEvent.change(mode, { target: { value: "legacy" } });
  fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
  await screen.findByText("设置已保存。");
  expect(save.mock.calls[0][1].voice_input.doubao_auth_mode).toBe("legacy");
});

test("Android uses the system recognizer and hides desktop voice controls", async () => {
  await openVoice("android");
  expect(screen.getByText("Android 系统语音")).toBeTruthy();
  expect(
    screen.getByText(
      "从键盘工具栏的“语音”入口调用设备上的系统语音识别服务。识别结果会回到键盘，确认后才插入当前输入框。",
    ),
  ).toBeTruthy();
  expect(screen.getByLabelText("识别语言")).toBeTruthy();
  expect(screen.queryByLabelText("识别服务")).toBeNull();
  expect(screen.queryByLabelText("识别 API Token")).toBeNull();
  expect(screen.queryByLabelText("结果提交策略")).toBeNull();
  expect(screen.queryByText("录音行为")).toBeNull();
  expect(screen.queryByText("文本润色 provider")).toBeNull();
  expect(screen.queryByText("语音快捷键")).toBeNull();
  expect(screen.queryByRole("button", { name: "打开" })).toBeNull();
});

test("iOS keeps voice in the app flow and hides the desktop voice panel", async () => {
  await openVoice("ios");
  expect(screen.getByText("iOS 应用语音")).toBeTruthy();
  expect(screen.getByText(/录音、识别和文本提交在当前共享设置与应用语音服务中完成/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "打开" })).toBeNull();
  expect(screen.queryByText("打开语音输入")).toBeNull();
});

test("iOS exposes the shared in-app voice action", async () => {
  const openVoice = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: host("ios"),
        openVoice,
      }}
    />,
  );
  await screen.findByRole("button", { name: "保存设置" });
  fireEvent.click(screen.getByRole("button", { name: "开始 iOS 语音" }));
  expect(openVoice).toHaveBeenCalledOnce();
});

test("iOS handwriting points to the keyboard extension instead of a desktop panel", async () => {
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="handwriting"
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: host("ios"),
        openSystemKeyboardSettings,
      }}
    />,
  );
  await screen.findByRole("button", { name: "保存设置" });
  expect(screen.getByText("iOS 键盘手写")).toBeTruthy();
  expect(screen.getByText(/切换到“手写”输入方案/)).toBeTruthy();
  expect(screen.queryByText("打开手写识别板")).toBeNull();
  expect(screen.queryByRole("button", { name: "打开" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "打开系统键盘设置" }));
  expect(openSystemKeyboardSettings).toHaveBeenCalledOnce();
});
