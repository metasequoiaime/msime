// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ASR_PROVIDER_DEFAULTS, SettingsPage, type Snapshot } from "@msime/ui";

afterEach(cleanup);
test.each(
  ["openai", "siliconflow", "groq"].flatMap((provider) =>
    ["windows", "macos"].map((platform) => [platform, provider]),
  ),
)("%s %s ASR test uses synthetic-audio command configuration", async (platform, provider) => {
  const snapshot: Snapshot = {
    format_version: 1,
    revision: 1,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      voice_input: {
        enabled: true,
        language: "zh-cn",
        asr_provider: provider,
        asr_endpoint: "",
        asr_model: "",
        asr_token: "synthetic-key",
      },
    },
  };
  const probe = vi.fn().mockResolvedValue({ ok: true, message: "fixture complete" });
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        testApiCredential: probe,
        host: testHost({ platform }),
      }}
    />,
  );
  const button = await screen.findByRole("button", { name: "测试语音识别配置" });
  expect(probe).not.toHaveBeenCalled();
  expect(screen.getByText(/一秒合成静音/)).toBeTruthy();
  fireEvent.click(button);
  await screen.findByText("fixture complete");
  // Only the fields a request needs: the preset also carries the provider's
  // model catalogue and integration page, which are for the settings page.
  expect(probe).toHaveBeenCalledWith("voice.asr", {
    provider,
    endpoint: ASR_PROVIDER_DEFAULTS[provider].endpoint,
    model: ASR_PROVIDER_DEFAULTS[provider].model,
    token: "synthetic-key",
  });
  // Switching to a streaming provider must not send its credentials through
  // the multipart batch probe; Linux has its separate provider-based control.
  fireEvent.change(screen.getByLabelText("识别服务"), { target: { value: "doubao" } });
  expect(screen.queryByRole("button", { name: "测试语音识别配置" })).toBeNull();
});

test("macOS system recognition does not expose an API credential probe", async () => {
  const probe = vi.fn();
  const snapshot: Snapshot = {
    format_version: 1,
    revision: 1,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      voice_input: { enabled: true, language: "zh-CN", asr_provider: "system" },
    },
  };
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        testApiCredential: probe,
        host: testHost({ platform: "macos" }),
      }}
    />,
  );
  await screen.findByLabelText("识别服务");
  expect(screen.queryByRole("button", { name: "测试语音识别配置" })).toBeNull();
  expect(screen.queryByRole("button", { name: "测试豆包识别配置" })).toBeNull();
  expect(screen.queryByText(/一秒合成静音/)).toBeNull();
  expect(probe).not.toHaveBeenCalled();
});
