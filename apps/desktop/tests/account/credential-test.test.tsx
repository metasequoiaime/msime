// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type SettingsClient, type Snapshot } from "@msime/ui";

afterEach(cleanup);

const snapshot: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    candidate_translations: true,
    niutrans: { enabled: true, app_id: "synthetic-app", apikey: "synthetic-key" },
    custom_translation: {
      enabled: false,
      endpoint: "https://fixture.invalid/translate",
      api_key: "synthetic-custom",
    },
    tencent_tmt: { enabled: false, secret_id: "", secret_key: "", region: "ap-guangzhou" },
    voice_input: {
      enabled: true,
      language: "zh-cn",
      asr_provider: "openai",
      asr_model: "whisper-1",
      asr_token: "must-not-cross-linux-provider-boundary",
      polish_enabled: true,
      polish_text: true,
      polish_provider: "deepseek",
      polish_model: "deepseek-v4-flash",
      polish_token: "must-not-cross-linux-provider-boundary",
    },
    ai_assistant: {
      enabled: true,
      provider: "deepseek",
      endpoint: "https://api.deepseek.com/chat/completions",
      model: "deepseek-v4-flash",
      candidate_limit: 3,
      token: "must-not-cross-linux-provider-boundary",
      tokens: { "https://api.deepseek.com:443": "must-not-cross-linux-provider-boundary" },
      prompt_custom_1: "",
      prompt_custom_2: "",
      prompt_custom_3: "",
    },
  },
};

function mount(testApiCredential: NonNullable<SettingsClient["testApiCredential"]>) {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        testApiCredential,
        host: { platform: "linux" } as never,
      }}
    />,
  );
}

test("Linux settings tests translation configuration through its provider", async () => {
  const testApiCredential = vi
    .fn()
    .mockResolvedValue({ ok: true, message: "连接成功，当前配置有效。" });
  mount(testApiCredential);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  fireEvent.click(screen.getByRole("button", { name: "测试 NiuTrans 配置" }));
  await screen.findByText("连接成功，当前配置有效。");
  expect(testApiCredential).toHaveBeenCalledWith("translation.niutrans", {
    app_id: "synthetic-app",
    apikey: "synthetic-key",
  });
});

test("Linux voice and AI tests never send private tokens from preferences", async () => {
  const testApiCredential = vi.fn().mockResolvedValue({ ok: true, message: "fixture ok" });
  mount(testApiCredential);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  fireEvent.click(screen.getByRole("button", { name: "测试语音识别配置" }));
  await screen.findByText("fixture ok");
  fireEvent.click(screen.getByRole("button", { name: "测试语音润色配置" }));
  await vi.waitFor(() => expect(testApiCredential).toHaveBeenCalledTimes(2));
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  fireEvent.click(screen.getByRole("button", { name: "AI 辅助" }));
  fireEvent.click(screen.getByRole("button", { name: "测试 AI 辅助配置" }));
  await vi.waitFor(() => expect(testApiCredential).toHaveBeenCalledTimes(3));

  expect(testApiCredential.mock.calls).toEqual([
    ["voice.asr", expect.objectContaining({ asr_provider: "openai", asr_model: "whisper-1" })],
    ["voice.polish", { polish_provider: "deepseek", polish_model: "deepseek-v4-flash" }],
    [
      "ai.assistant",
      {
        provider: "deepseek",
        endpoint: "https://api.deepseek.com/chat/completions",
        model: "deepseek-v4-flash",
      },
    ],
  ]);
  expect(JSON.stringify(testApiCredential.mock.calls)).not.toContain(
    "must-not-cross-linux-provider-boundary",
  );
});

test("provider transport failures remain actionable and stale results disappear after edits", async () => {
  const testApiCredential = vi.fn().mockRejectedValue({ code: "unavailable" });
  mount(testApiCredential);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  fireEvent.click(screen.getByRole("button", { name: "AI 辅助" }));
  fireEvent.click(screen.getByRole("button", { name: "测试 AI 辅助配置" }));
  await screen.findByText("无法连接 provider，请确认服务已启动。");
  fireEvent.change(screen.getByLabelText("AI 模型"), { target: { value: "new-model" } });
  expect(screen.queryByText("无法连接 provider，请确认服务已启动。")).toBeNull();
});
