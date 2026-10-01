// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
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
        host: { platform } as never,
        ...(openExternalUrl ? { openExternalUrl } : {}),
      }}
    />,
  );
  await settingsFormReady();
  // AI 辅助 is reached from inside the 标点与翻译 page.
  if (page === "AI 辅助") fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  fireEvent.click(screen.getByRole("button", { name: page }));
}

test("the AI provider offers its known models and fills the model box", async () => {
  await openPage("AI 辅助");

  const presets = screen.getByLabelText("AI 预置模型") as HTMLSelectElement;
  expect([...presets.options].map((option) => option.value)).toEqual([
    "",
    "deepseek-v4-flash",
    "deepseek-v4-pro",
  ]);
  expect(presets.value).toBe("deepseek-v4-flash");

  fireEvent.change(presets, { target: { value: "deepseek-v4-pro" } });
  expect((screen.getByLabelText("AI 模型") as HTMLInputElement).value).toBe("deepseek-v4-pro");
});

test("a model the provider does not list reads as 自定义模型 and is left alone", async () => {
  await openPage("AI 辅助");

  fireEvent.change(screen.getByLabelText("AI 模型"), {
    target: { value: "synthetic-private-model" },
  });
  const presets = screen.getByLabelText("AI 预置模型") as HTMLSelectElement;
  expect(presets.value).toBe("");

  // Selecting the placeholder is not a model; it must not overwrite what was typed.
  fireEvent.change(presets, { target: { value: "" } });
  expect((screen.getByLabelText("AI 模型") as HTMLInputElement).value).toBe(
    "synthetic-private-model",
  );
});

test("the AI provider links to its own integration page through the host", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("AI 辅助", openExternalUrl);

  fireEvent.click(screen.getByRole("button", { name: "AI 接入说明与 API Key" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://api-docs.deepseek.com/");
});

test("a host without an external link capability shows no integration link", async () => {
  await openPage("AI 辅助");

  expect(screen.queryByRole("button", { name: "AI 接入说明与 API Key" })).toBeNull();
  expect(screen.getByLabelText("AI 预置模型")).toBeTruthy();
});

test("自定义 has neither preset models nor an integration page", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("AI 辅助", openExternalUrl);

  fireEvent.change(screen.getByLabelText("AI 服务提供商"), { target: { value: "custom" } });
  expect(screen.queryByLabelText("AI 预置模型")).toBeNull();
  expect(screen.queryByRole("button", { name: "AI 接入说明与 API Key" })).toBeNull();
});

test("the recognition provider carries the same presets and link", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("语音输入", openExternalUrl, "windows");

  const presets = screen.getByLabelText("识别服务预置模型") as HTMLSelectElement;
  expect([...presets.options].map((option) => option.value)).toEqual([
    "",
    "whisper-large-v3-turbo",
    "whisper-large-v3",
  ]);
  expect(presets.value).toBe("whisper-large-v3-turbo");

  fireEvent.change(presets, { target: { value: "whisper-large-v3" } });
  expect((screen.getByLabelText("识别模型") as HTMLInputElement).value).toBe("whisper-large-v3");

  fireEvent.click(screen.getByRole("button", { name: "识别服务接入说明与 API Key" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://console.groq.com/docs/speech-to-text");
});

// Doubao publishes an integration page but no OpenAI-style model catalogue, so
// its row is the link alone rather than an empty model select.
test("a provider without preset models still links to its integration page", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("语音输入", openExternalUrl, "windows");

  fireEvent.change(screen.getByLabelText("识别服务"), { target: { value: "doubao" } });
  expect(screen.queryByLabelText("识别服务预置模型")).toBeNull();
  expect(screen.getByRole("button", { name: "识别服务接入说明与 API Key" })).toBeTruthy();
});

// Android's voice settings hide the polish provider entirely, so the desktop
// host is where this section is reachable.
test("voice polish carries the same presets and link", async () => {
  const openExternalUrl = vi.fn(async () => {});
  await openPage("语音输入", openExternalUrl, "windows");

  const presets = screen.getByLabelText("文本润色预置模型") as HTMLSelectElement;
  expect([...presets.options].map((option) => option.value)).toEqual([
    "",
    "deepseek-v4-flash",
    "deepseek-v4-pro",
  ]);

  fireEvent.change(presets, { target: { value: "deepseek-v4-pro" } });
  expect((screen.getByLabelText("文本润色模型") as HTMLInputElement).value).toBe("deepseek-v4-pro");

  fireEvent.click(screen.getByRole("button", { name: "文本润色接入说明与 API Key" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://api-docs.deepseek.com/");
});
