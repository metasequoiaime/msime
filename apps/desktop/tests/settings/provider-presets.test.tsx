// @vitest-environment jsdom
import { testHost } from "../support/host";
import { openVoiceServiceConfig, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

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
    voice_input: {
      enabled: true,
      language: "zh-CN",
      asr_provider: "groq",
      asr_model: "whisper-large-v3-turbo",
      polish_enabled: true,
      polish_text: true,
      polish_provider: "deepseek",
      polish_model: "deepseek-v4-flash",
    },
  },
};

async function openPage(
  page: string,
  openExternalUrl?: (url: string) => Promise<void>,
  platform = "android",
) {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform }),
        ...(openExternalUrl ? { openExternalUrl } : {}),
      }}
    />,
  );
  await settingsFormReady();
  // AI 辅助 从 标点与翻译 页面内部进入，触屏宿主把那个页面叫作 表达。
  const expression = ["android", "ios", "harmony"].includes(platform) ? "表达" : "标点与翻译";
  if (page === "AI 辅助") fireEvent.click(screen.getByRole("button", { name: expression }));
  fireEvent.click(screen.getByRole("button", { name: page }));
}

const optionValues = (select: HTMLSelectElement) =>
  [...select.options].map((option) => option.value);

test("the AI provider offers its known models in the one model list", async () => {
  await openPage("AI 辅助");

  const models = screen.getByLabelText("AI 模型") as HTMLSelectElement;
  expect(optionValues(models)).toEqual(["deepseek-v4-flash", "deepseek-v4-pro", ""]);
  expect(models.value).toBe("deepseek-v4-flash");
  expect(screen.queryByLabelText("AI 模型（自定义）")).toBeNull();

  fireEvent.change(models, { target: { value: "deepseek-v4-pro" } });
  expect(models.value).toBe("deepseek-v4-pro");
});

test("自定义… opens a field for a model the provider does not list", async () => {
  await openPage("AI 辅助");

  const models = screen.getByLabelText("AI 模型") as HTMLSelectElement;
  fireEvent.change(models, { target: { value: "" } });
  const custom = screen.getByLabelText("AI 模型（自定义）") as HTMLInputElement;
  expect(custom.value).toBe("deepseek-v4-flash");
  fireEvent.change(custom, { target: { value: "synthetic-private-model" } });
  expect(models.value).toBe("");
  expect((screen.getByLabelText("AI 模型（自定义）") as HTMLInputElement).value).toBe(
    "synthetic-private-model",
  );
});

test("the AI provider links to its own integration page through the host", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("AI 辅助", openExternalUrl);

  fireEvent.click(screen.getByRole("button", { name: "AI 接入说明与 API Key" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://api-docs.deepseek.com/");
});

test("an integration link failure is shown in the settings error area", async () => {
  const openExternalUrl = vi.fn().mockRejectedValue(new Error("synthetic host failure"));
  await openPage("AI 辅助", openExternalUrl);

  fireEvent.click(screen.getByRole("button", { name: "AI 接入说明与 API Key" }));

  expect(
    await screen.findByText("无法打开外部链接，请稍后重试。", {}, { timeout: 1000 }),
  ).toBeTruthy();
});

test("a host without an external link capability shows no integration link", async () => {
  await openPage("AI 辅助");

  expect(screen.queryByRole("button", { name: "AI 接入说明与 API Key" })).toBeNull();
  expect(screen.getByLabelText("AI 模型").tagName).toBe("SELECT");
});

test("自定义 has neither preset models nor an integration page", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("AI 辅助", openExternalUrl);

  fireEvent.click(screen.getByRole("combobox", { name: "AI 服务提供商" }));
  fireEvent.click(screen.getByRole("option", { name: "自定义" }));
  expect(screen.getByLabelText("AI 模型").tagName).toBe("INPUT");
  expect(screen.queryByRole("button", { name: "AI 接入说明与 API Key" })).toBeNull();
});

test("the recognition provider carries the same presets and link", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("语音输入", openExternalUrl, "windows");
  // 识别模型和接入说明都在「服务配置」子视图里，模型收在「更多选项」中。
  openVoiceServiceConfig();

  const models = screen.getByLabelText("识别模型") as HTMLSelectElement;
  expect(optionValues(models)).toEqual(["whisper-large-v3-turbo", "whisper-large-v3", ""]);
  expect(models.value).toBe("whisper-large-v3-turbo");

  fireEvent.change(models, { target: { value: "whisper-large-v3" } });
  expect(models.value).toBe("whisper-large-v3");

  fireEvent.click(screen.getByRole("button", { name: "识别服务接入说明与 API Key" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://console.groq.com/docs/speech-to-text");
});

// Doubao publishes an integration page but no OpenAI-style model catalogue. 它的识别请求也不读 `asr_model`，所以不画识别模型，只留接入说明。
test("a provider without preset models still links to its integration page", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("语音输入", openExternalUrl, "windows");

  fireEvent.change(screen.getByRole("combobox", { name: "识别服务" }), {
    target: { value: "doubao" },
  });
  openVoiceServiceConfig();
  expect(screen.queryByLabelText("识别模型")).toBeNull();
  expect(screen.getByRole("button", { name: "识别服务接入说明与 API Key" })).toBeTruthy();
});

// Android's voice settings hide the polish provider entirely, so the desktop
// host is where this section is reachable.
test("voice polish carries the same presets and link", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("语音输入", openExternalUrl, "windows");

  openVoiceServiceConfig();
  const models = screen.getByLabelText("文本润色模型") as HTMLSelectElement;
  expect(optionValues(models)).toEqual(["deepseek-v4-flash", "deepseek-v4-pro", ""]);

  fireEvent.change(models, { target: { value: "deepseek-v4-pro" } });
  expect(models.value).toBe("deepseek-v4-pro");

  fireEvent.click(screen.getByRole("button", { name: "文本润色接入说明与 API Key" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://api-docs.deepseek.com/");
});
