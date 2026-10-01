// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(cleanup);
test.each(["windows", "macos"])(
  "%s chat tests use the edited configuration and origin-bound token",
  async (platform) => {
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
          polish_enabled: true,
          polish_provider: "openai",
          polish_model: "fixture-model",
          polish_token: "synthetic-polish",
        },
        ai_assistant: {
          enabled: true,
          provider: "openai",
          endpoint: "https://fixture.invalid/chat",
          model: "fixture-model",
          candidate_limit: 3,
          tokens: { "https://fixture.invalid:443": "synthetic-ai" },
          prompt_custom_1: "",
          prompt_custom_2: "",
          prompt_custom_3: "",
        },
      },
    };
    const probe = vi.fn().mockResolvedValue({ ok: true, message: "fixture complete" });
    render(
      <SettingsPage
        client={{
          load: async () => snapshot,
          save: vi.fn(),
          testApiCredential: probe,
          host: { platform } as never,
        }}
      />,
    );
    await settingsFormReady();
    expect(probe).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
    fireEvent.click(screen.getByRole("button", { name: "AI 辅助" }));
    fireEvent.change(screen.getByLabelText("AI 模型"), { target: { value: "edited-model" } });
    fireEvent.click(screen.getByRole("button", { name: "测试 AI 辅助配置" }));
    await screen.findByText("fixture complete");
    expect(probe).toHaveBeenLastCalledWith("ai.assistant", {
      provider: "openai",
      endpoint: "https://fixture.invalid/chat",
      model: "edited-model",
      token: "synthetic-ai",
    });
    fireEvent.change(screen.getByLabelText("AI 接口地址"), {
      target: { value: "https://other.invalid/chat" },
    });
    expect(
      (screen.getByRole("button", { name: "测试 AI 辅助配置" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
    fireEvent.click(screen.getByRole("button", { name: "测试语音润色配置" }));
    await vi.waitFor(() => expect(probe).toHaveBeenCalledTimes(2));
    expect(probe).toHaveBeenLastCalledWith("voice.polish", {
      provider: "openai",
      endpoint: "https://api.openai.com/v1/chat/completions",
      model: "fixture-model",
      token: "synthetic-polish",
    });
  },
);
