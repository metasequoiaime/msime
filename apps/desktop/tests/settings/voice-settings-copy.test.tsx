// @vitest-environment jsdom
import {
  openVoiceServiceConfig,
  settingsFormReady,
  saveSettingsNow,
} from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within, waitFor } from "@testing-library/react";
import { SettingsPage, type HostCapabilities, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { desktopVoiceServiceStatus } from "../../../../packages/ui/src/settings/voice-settings-content";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const voiceContent = Object.values(
  import.meta.glob<string>("../../../../packages/ui/src/settings/voice-settings-content.tsx", {
    eager: true,
    query: "?raw",
    import: "default",
  }),
)[0];

test("voice settings content owns the shared polish settings section", () => {
  expect(voiceContent).toContain(
    'import { VoicePolishSettingsSection } from "./voice-polish-settings-section";',
  );
  expect(voiceContent).toContain("<VoicePolishSettingsSection");
  expect(voiceContent).not.toContain("customPrompts={{");
});

test("voice settings content owns the shared recognition result section", () => {
  expect(voiceContent).toContain(
    'import { VoiceRecognitionResultSection } from "./voice-recognition-result-section";',
  );
  expect(voiceContent).toContain("<VoiceRecognitionResultSection");
  // 桌面把边说边显示放在主页面的「识别」组里，用的是同一个共享开关，不另写一份。
  expect(voiceContent).toContain(
    'import { VoiceStreamPreeditSection } from "./voice-stream-preedit-section";',
  );
  expect(voiceContent).not.toContain('title="流式预编辑"');
});

test("voice settings content owns the shared ASR provider section", () => {
  expect(voiceContent).toContain(
    'import { VoiceAsrProviderSettingsSection } from "./voice-asr-provider-settings-section";',
  );
  expect(voiceContent).toContain("<VoiceAsrProviderSettingsSection");
  expect(voiceContent).not.toContain("<VoiceModelSection");
});

test("voice settings content owns the shared local model settings section", () => {
  expect(voiceContent).toContain(
    'import { VoiceLocalModelSettingsSection } from "./voice-local-model-settings-section";',
  );
  expect(voiceContent).toContain("<VoiceLocalModelSettingsSection");
  expect(voiceContent).not.toContain("<LocalModelManager");
});

test("voice settings content owns the shared input basics section", () => {
  expect(voiceContent).toContain(
    'import { VoiceInputBasicsSection } from "./voice-input-basics-section";',
  );
  expect(voiceContent).toContain("<VoiceInputBasicsSection");
  expect(voiceContent).not.toContain("<VoiceInputIntroSection");
  expect(voiceContent).not.toContain("<VoiceInputCoreSection");
});

test("voice settings content owns the shared recording behavior settings", () => {
  expect(voiceContent).toContain(
    'import { VoiceRecordingBehaviorSettingsSection } from "./voice-recording-behavior-settings-section";',
  );
  expect(voiceContent).toContain("<VoiceRecordingBehaviorSettingsSection");
  expect(voiceContent).not.toContain("<VoiceRecordingBehaviorSection");
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

function host(platform: HostCapabilities["platform"]) {
  return testHost({ platform, voice_capture_devices: true });
}

async function openVoice(platform: string) {
  render(
    <SettingsPage client={{ load: async () => snapshot, save: vi.fn(), host: host(platform) }} />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  // The token fields are hidden on Linux, where credentials belong to the
  // provider service, so wait on a control both platforms render.
  return await screen.findByRole("combobox", {
    name: platform === "android" ? "识别语言" : "识别服务",
  });
}

test("a pasted recognition token can be revealed to check it", async () => {
  await openVoice("windows");
  openVoiceServiceConfig();
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
  // 润色关闭时语音页把令牌连同整组收起，先打开润色。
  openVoiceServiceConfig();
  expect(screen.getByRole("button", { name: "显示Doubao App Key" })).toBeTruthy();
  fireEvent.click(screen.getByRole("switch", { name: "启用文本润色" }));
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
  expect(screen.queryByText(/语音需要单独运行的语音服务/)).toBeNull();
  expect(screen.queryByText(/录音和识别由已配置的语音服务完成/)).toBeNull();
  expect(screen.getByText("在本机录音，音频发送给下方选择的识别服务转写")).toBeTruthy();
  expect(screen.getByText(/随识别请求发送给豆包/)).toBeTruthy();
});

test("macOS keeps voice submission in the native input-method process", async () => {
  await openVoice("macos");
  expect(screen.getByText("macOS 输入法语音")).toBeTruthy();
  expect(screen.getByText(/识别结果直接输入到该应用/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "打开" })).toBeNull();
  expect(screen.queryByText("打开语音输入")).toBeNull();
});

test("Linux keeps the wording that is accurate there", async () => {
  await openVoice("linux");
  // 录音行为在各平台同名；Linux 的不同之处（选项随请求交给语音服务）写在组说明里，用户可见的文案不出现 provider。
  expect(screen.getByText("录音行为")).toBeTruthy();
  expect(screen.getByText(/随请求传给用户管理的语音服务/)).toBeTruthy();
  expect(screen.queryByText("Linux provider 行为")).toBeNull();
  // The shortcuts work the same under IBus and Fcitx5, so the section is not named after one host.
  expect(screen.getByText("语音快捷键")).toBeTruthy();
  expect(screen.queryByText(/IBus 快捷键/)).toBeNull();
  expect(screen.getByText(/语音需要单独运行的语音服务/)).toBeTruthy();
  expect(screen.queryByText(/IBus 属性/)).toBeNull();
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
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  const mode = (await screen.findByLabelText("豆包鉴权方式")) as HTMLSelectElement;
  expect(mode.value).toBe("api_key");
  expect(screen.queryByLabelText("Doubao API Key")).toBeNull();
  expect(screen.queryByLabelText("Doubao App Key")).toBeNull();
  fireEvent.change(mode, { target: { value: "legacy" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save.mock.calls[0][1].voice_input.doubao_auth_mode).toBe("legacy");
});

test("Android uses the system recognizer and hides desktop voice controls", async () => {
  await openVoice("android");
  expect(screen.getByText("Android 系统语音")).toBeTruthy();
  expect(
    screen.getByText(
      "从键盘的语音入口调用设备上的系统语音识别服务，在键盘里聆听，识别结果直接插入当前输入框。",
    ),
  ).toBeTruthy();
  expect(screen.getByLabelText("识别语言")).toBeTruthy();
  expect(screen.queryByLabelText("识别服务")).toBeNull();
  expect(screen.queryByLabelText("识别 API Token")).toBeNull();
  expect(screen.queryByLabelText("结果提交策略")).toBeNull();
  expect(screen.queryByText("录音行为")).toBeNull();
  expect(screen.queryByText("文本润色")).toBeNull();
  expect(screen.queryByText("语音快捷键")).toBeNull();
  expect(screen.queryByRole("button", { name: "打开" })).toBeNull();
});

test("iOS keeps voice in the app flow and hides the desktop voice panel", async () => {
  await openVoice("ios");
  expect(screen.getByText("iOS 应用语音")).toBeTruthy();
  expect(screen.getByText(/识别结果回到当前页面，确认后再使用/)).toBeTruthy();
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
  await settingsFormReady();
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
  await settingsFormReady();
  expect(screen.getByText("iOS 键盘手写")).toBeTruthy();
  expect(screen.getByText(/切换到“手写”输入方案/)).toBeTruthy();
  expect(screen.queryByText("打开手写识别板")).toBeNull();
  expect(screen.queryByRole("button", { name: "打开" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "打开系统键盘设置" }));
  expect(openSystemKeyboardSettings).toHaveBeenCalledOnce();
});

// 语音页主页面只留常用的几项：识别、快捷键、录音行为。识别服务的配置（检查按钮在配置组末尾）、识别结果、润色、提示音细分和录音设备收在「服务配置」子视图里，照鸿蒙手机的整理方案。
test("the voice page keeps the common rows and folds the service into its own view", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: host("windows"),
        testApiCredential: vi.fn(),
        listVoiceCaptureDevices: vi.fn().mockResolvedValue([]),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  const page = await screen.findByRole("group", { name: "语音输入" });
  const titles = (root: Element) =>
    [...root.querySelectorAll("[data-group-title]")].map((node) => node.textContent ?? "");
  const config = page.querySelector('[aria-label="服务配置"][role="region"]')!;
  expect(config.hasAttribute("hidden")).toBe(true);
  const main = [...page.querySelectorAll("[data-group-title]")]
    .filter((node) => !config.contains(node))
    .map((node) => node.textContent ?? "");
  expect(main).toEqual(["语音面板", "识别", "语音快捷键", "录音行为"]);
  expect(screen.queryByRole("textbox", { name: "Doubao 资源 ID" })).toBeNull();
  // 「服务配置」一行写着凭据填没填：旧版鉴权填了令牌、没填 App ID。
  const row = screen.getByRole("button", { name: "服务配置" });
  expect(row.textContent).toContain("未填写完整");

  // Windows 的宿主把空格锁定固定为开，不给开关。
  expect(screen.queryByRole("switch", { name: "长按录音时按空格锁定" })).toBeNull();

  const view = openVoiceServiceConfig();
  // Windows 只认提示音总开关，也只有一种录音方式：子视图里没有「提示音」组，录音设备只有一个「麦克风」下拉。
  expect(titles(view)).toEqual(["识别服务", "识别结果", "文本润色", "录音设备"]);
  expect(within(view).queryByLabelText("开始录音提示音")).toBeNull();
  expect(within(view).queryByLabelText("录音后端")).toBeNull();
  expect(within(view).getByRole("combobox", { name: "麦克风" })).toBeTruthy();
  expect(screen.queryByRole("switch", { name: "启用语音输入" })).toBeNull();
  // 豆包的识别选项和资源 ID 收在「更多选项」里，检查按钮是这一组的最后一项。
  const service = within(view).getByRole("region", { name: "识别服务" });
  expect(within(service).getByLabelText("数字格式化").closest("details")).toBeTruthy();
  expect(within(service).getByLabelText("Doubao 资源 ID").closest("details")).toBeTruthy();
  expect(within(service).getByLabelText("豆包鉴权方式").closest("details")).toBeNull();
  const test = within(service).getByRole("button", { name: "测试豆包识别配置" });
  const controls = [...service.querySelectorAll("input, select, button")];
  expect(controls.at(-1)).toBe(test);
  fireEvent.click(within(view).getByRole("button", { name: "返回" }));
  expect(screen.getByRole("switch", { name: "启用语音输入" })).toBeTruthy();
});

test("文本润色 shows only its switch until polishing is turned on", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: host("windows"),
        testApiCredential: vi.fn(),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  openVoiceServiceConfig();
  const polish = screen.getByRole("region", { name: "文本润色" });
  expect(within(polish).getByRole("switch", { name: "启用文本润色" })).toBeTruthy();
  expect(within(polish).queryByLabelText("文本润色服务提供商")).toBeNull();
  expect(within(polish).queryByLabelText("润色方案")).toBeNull();
  fireEvent.click(within(polish).getByRole("switch", { name: "启用文本润色" }));
  expect(within(polish).getByLabelText("文本润色服务提供商")).toBeTruthy();
  expect(within(polish).getByLabelText("润色方案")).toBeTruthy();
  // 测试按钮放在组末，测的是上面这些设置。
  const controls = [...polish.querySelectorAll("input, select, button, textarea")];
  expect(controls.at(-1)?.getAttribute("aria-label")).toBe("测试语音润色配置");
});

test("the Doubao resource ID is only offered for Doubao", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => ({
          ...snapshot,
          preferences: {
            ...snapshot.preferences,
            voice_input: { enabled: true, language: "zh-CN", asr_provider: "openai" },
          },
        }),
        save: vi.fn(),
        host: host("windows"),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  await screen.findByRole("button", { name: "服务配置" });
  openVoiceServiceConfig();
  expect(screen.getByRole("region", { name: "识别服务" })).toBeTruthy();
  expect(screen.queryByLabelText("Doubao 资源 ID")).toBeNull();
});

test("macOS 手写页能打开识别复制面板，且不重置无关偏好", async () => {
  const openHandwriting = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="handwriting"
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: host("macos"),
        openHandwriting,
        loadDefaultPreferences: vi.fn().mockResolvedValue(snapshot.preferences),
      }}
    />,
  );
  await settingsFormReady();
  // 旧的 handwriting 页 id 仍然能打开：macOS 上它指向合并后的「屏幕键盘与手写」页。
  expect(await screen.findByRole("group", { name: "屏幕键盘与手写" })).toBeTruthy();
  expect(screen.getByText("打开手写识别板")).toBeTruthy();
  expect(screen.getByLabelText("手写识别板预览")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "恢复默认设置" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "打开" }));
  await waitFor(() => expect(openHandwriting).toHaveBeenCalledOnce());
});

// macOS 宿主把空格锁定固定为开、只读提示音总开关、只经 CoreAudio 录音；豆包的识别请求不读识别语言和识别模型。这些行在 macOS 的语音页上都不画，值照旧保存。
test("macOS drops the rows its host fixes or does not read", async () => {
  const listVoiceCaptureDevices = vi
    .fn()
    .mockResolvedValue([{ backend: "macos", id: "uid-usb", label: "Synthetic USB microphone" }]);
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: host("macos"),
        listVoiceCaptureDevices,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  await screen.findByRole("combobox", { name: "识别服务" });
  // 豆包：不画识别语言，标点开关和边说边显示在「识别」组里。
  expect(screen.queryByLabelText("识别语言")).toBeNull();
  expect(screen.getByRole("switch", { name: "自动添加标点" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "边说边显示" })).toBeTruthy();
  // 三个按住录音的组合键是一个选择，空格锁定不再是开关，Ctrl+F9 收在更多选项里。
  expect(screen.getByRole("combobox", { name: "按住录音" })).toBeTruthy();
  expect(screen.queryByLabelText("空格锁定语音")).toBeNull();
  expect(screen.queryByRole("switch", { name: "按住右 Option 录音" })).toBeNull();
  // 设备列表只在子视图打开时读取。
  expect(listVoiceCaptureDevices).not.toHaveBeenCalled();

  const view = openVoiceServiceConfig();
  expect(within(view).queryByLabelText("开始录音提示音")).toBeNull();
  expect(within(view).queryByLabelText("识别模型")).toBeNull();
  expect(within(view).queryByLabelText("录音后端")).toBeNull();
  const microphone = within(view).getByRole("combobox", { name: "麦克风" });
  await waitFor(() =>
    expect(
      within(microphone).getByRole("option", { name: "Synthetic USB microphone" }),
    ).toBeTruthy(),
  );
  expect(within(microphone).getByRole("option", { name: "系统默认设备" })).toBeTruthy();
  expect(listVoiceCaptureDevices).toHaveBeenCalledOnce();
});

// 换成整句上传的服务时，识别语言回来，识别模型收在「更多选项」里。
test("an HTTP provider shows the language and folds the model away", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => ({
          ...snapshot,
          preferences: {
            ...snapshot.preferences,
            voice_input: { enabled: true, language: "zh-CN", asr_provider: "openai" },
          },
        }),
        save: vi.fn(),
        host: host("macos"),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  expect(await screen.findByLabelText("识别语言")).toBeTruthy();
  expect(screen.queryByRole("switch", { name: "自动添加标点" })).toBeNull();
  expect(screen.getByRole("button", { name: "服务配置" }).textContent).toContain("未填写");
  const view = openVoiceServiceConfig();
  expect(within(view).getByLabelText("识别模型").closest("details")).toBeTruthy();
  expect(within(view).getByLabelText("识别接口地址").closest("details")).toBeTruthy();
  expect(within(view).getByLabelText("识别 API Token").closest("details")).toBeNull();
});

test("the desktop service status says what the recorder will find", () => {
  const status = (
    voice: Record<string, unknown>,
    options: Partial<Parameters<typeof desktopVoiceServiceStatus>[1]> = {},
  ) =>
    desktopVoiceServiceStatus(voice as never, {
      serviceVoice: true,
      localVoice: false,
      linux: false,
      harmony: false,
      ...options,
    });
  // macOS 和 Windows 的豆包没填密钥时拒绝录音，不改用系统识别。
  expect(status({ asr_provider: "doubao" })).toBe("未填写");
  expect(status({ asr_provider: "doubao", asr_tokens: { doubao: "k" } })).toBe("已填写");
  expect(status({ asr_provider: "doubao", doubao_auth_mode: "legacy", asr_token: "k" })).toBe(
    "未填写完整",
  );
  expect(status({ asr_provider: "doubao", doubao_auth_mode: "legacy", asr_app_key: "a" })).toBe(
    "未填写完整",
  );
  expect(status({ asr_provider: "doubao" }, { harmony: true })).toBe("未填写，暂用系统识别");
  expect(status({ asr_provider: "groq", asr_token: " k " })).toBe("已填写");
  expect(status({ asr_provider: "groq" })).toBe("未填写");
  expect(status({ asr_provider: "local" }, { serviceVoice: false, localVoice: true })).toBe(
    "未选模型",
  );
  expect(status({ asr_provider: "system" }, { serviceVoice: false })).toBeUndefined();
  expect(status({ asr_provider: "doubao" }, { linux: true })).toBeUndefined();
});
