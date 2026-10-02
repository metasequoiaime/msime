// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
    voice_input: {
      enabled: true,
      language: "zh-CN",
      asr_provider: "local",
      asr_model_path: "/old/voice-models/paraformer",
    },
  },
};

async function openVoice(client: Record<string, unknown>) {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "macos" }),
        ...client,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  return screen.findByLabelText("本地模型目录");
}

test("choosing a model fills the path the recognizer loads", async () => {
  const pickVoiceModelPath = vi.fn(async () => "/Users/someone/voice-models/sense-voice");
  const field = (await openVoice({ pickVoiceModelPath })) as HTMLInputElement;
  expect(field.value).toBe("/old/voice-models/paraformer");

  fireEvent.click(screen.getByRole("button", { name: "选择…" }));

  await waitFor(() => expect(field.value).toBe("/Users/someone/voice-models/sense-voice"));
  expect(pickVoiceModelPath).toHaveBeenCalledTimes(1);
});

test("cancelling leaves the path that already worked", async () => {
  const pickVoiceModelPath = vi.fn(async () => null);
  const field = (await openVoice({ pickVoiceModelPath })) as HTMLInputElement;

  fireEvent.click(screen.getByRole("button", { name: "选择…" }));

  await waitFor(() => expect(pickVoiceModelPath).toHaveBeenCalledTimes(1));
  expect(field.value).toBe("/old/voice-models/paraformer");
});

test("a host that cannot pick files offers typing only", async () => {
  const field = (await openVoice({})) as HTMLInputElement;

  expect(screen.queryByRole("button", { name: "选择…" })).toBeNull();
  expect(field.value).toBe("/old/voice-models/paraformer");
});
